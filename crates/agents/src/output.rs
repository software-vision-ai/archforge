//! A runtime's output: its standard output read as lines with a cap on each, and the end of its
//! error stream kept for a diagnosis. Neither ever grows without bound.

use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, PoisonError};

use archforge_secrets::Secret;
use zeroize::Zeroize;

use crate::RuntimeKind;

/// What the reader of a runtime's standard output saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputEvent {
    /// One line, without its line break.
    Line(String),
    /// A line that was not kept; it was read to its end and dropped.
    Dropped(Dropped),
    /// Reading failed; no more events follow but [`OutputEvent::End`].
    ReadFailed(String),
    /// The output is closed: the runtime closed it or ended. Every later read answers this too.
    End,
}

/// Why a line was dropped, with its length in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// Longer than the cap: never held whole, only counted.
    TooLong { bytes: u64 },
    /// Not UTF-8 text.
    NotText { bytes: u64 },
}

/// The bytes read from the pipe at a time.
const READ_CHUNK: usize = 64 * 1024;

/// Reads `stream` as lines of at most `max` bytes each and sends them to `events`, then
/// [`OutputEvent::End`]. A longer line is counted, never buffered, and reported as dropped. Stops
/// when the receiver is gone.
pub(crate) fn read_lines(stream: impl Read, max: usize, runtime: RuntimeKind, events: SyncSender<OutputEvent>) {
    let mut reader = BufReader::with_capacity(READ_CHUNK, stream);
    let mut line: Vec<u8> = Vec::new();
    // The length so far of a line past the cap, while the rest of it is skipped.
    let mut skipping: Option<u64> = None;
    loop {
        let buf = match reader.fill_buf() {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ = events.send(OutputEvent::ReadFailed(e.to_string()));
                let _ = events.send(OutputEvent::End);
                return;
            }
        };
        if buf.is_empty() {
            // The end: a last line without a line break still counts.
            let last = match skipping {
                Some(bytes) => Some(dropped(runtime, Dropped::TooLong { bytes }, max)),
                None if !line.is_empty() => Some(finished(runtime, std::mem::take(&mut line))),
                None => None,
            };
            if let Some(event) = last {
                let _ = events.send(event);
            }
            let _ = events.send(OutputEvent::End);
            return;
        }
        let newline = buf.iter().position(|b| *b == b'\n');
        let chunk = match newline.and_then(|at| buf.get(..at)) {
            Some(head) => head,
            None => buf,
        };
        let consumed = chunk.len().saturating_add(usize::from(newline.is_some()));
        match skipping.as_mut() {
            Some(bytes) => *bytes = bytes.saturating_add(len(chunk.len())),
            None if line.len().saturating_add(chunk.len()) > max => {
                skipping = Some(len(line.len()).saturating_add(len(chunk.len())));
                line = Vec::new();
            }
            None => line.extend_from_slice(chunk),
        }
        reader.consume(consumed);
        if newline.is_some() {
            let event = match skipping.take() {
                Some(bytes) => dropped(runtime, Dropped::TooLong { bytes }, max),
                None => finished(runtime, std::mem::take(&mut line)),
            };
            if events.send(event).is_err() {
                return;
            }
        }
    }
}

/// A byte count as `u64`.
fn len(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// A complete line as an event: its text, or a drop when it is not UTF-8.
fn finished(runtime: RuntimeKind, bytes: Vec<u8>) -> OutputEvent {
    match String::from_utf8(bytes) {
        Ok(text) => OutputEvent::Line(text),
        Err(e) => dropped(runtime, Dropped::NotText { bytes: len(e.as_bytes().len()) }, 0),
    }
}

/// A dropped line as an event, said in the log by its length alone, never its content.
fn dropped(runtime: RuntimeKind, why: Dropped, max: usize) -> OutputEvent {
    match why {
        Dropped::TooLong { bytes } => log::warn!("{runtime} wrote a line of {bytes} bytes, past the {max} kept; it was dropped"),
        Dropped::NotText { bytes } => log::warn!("{runtime} wrote a line of {bytes} bytes that is not UTF-8; it was dropped"),
    }
    OutputEvent::Dropped(why)
}

/// The last bytes of a runtime's error stream, overwritten when dropped.
#[derive(Debug)]
pub(crate) struct Tail {
    /// Allocated once at the cap and never grown, so no copy is left behind in freed memory.
    bytes: Vec<u8>,
    cap: usize,
    dropped: u64,
    /// The stream was read to its end.
    closed: bool,
}

impl Tail {
    /// An empty tail that keeps at most `cap` bytes.
    pub(crate) fn new(cap: usize) -> Tail {
        Tail { bytes: Vec::with_capacity(cap), cap, dropped: 0, closed: false }
    }

    /// Whether the stream was read to its end (or its reading failed), so nothing more comes.
    pub(crate) fn closed(&self) -> bool {
        self.closed
    }

    /// Adds `chunk`, dropping the oldest bytes past the cap.
    fn push(&mut self, chunk: &[u8]) {
        let keep = chunk.len().min(self.cap);
        let start = chunk.len().saturating_sub(keep);
        self.dropped = self.dropped.saturating_add(len(start));
        let excess = self.bytes.len().saturating_add(keep).saturating_sub(self.cap);
        if excess > 0 {
            self.bytes.drain(..excess.min(self.bytes.len()));
            self.dropped = self.dropped.saturating_add(len(excess));
        }
        self.bytes.extend_from_slice(chunk.get(start..).unwrap_or_default());
    }

    /// At most the last `max` bytes of the tail as text, with every occurrence of each of `secrets`
    /// replaced. When the front was cut (earlier bytes dropped, or past `max`), it starts after a
    /// line break, so no part of a line cut at the front (a part of a key among it) is shown.
    pub(crate) fn render(&self, secrets: &[Secret], max: usize) -> String {
        let mut bytes = self.bytes.clone();
        let cut_at = bytes.len().saturating_sub(max);
        let from = if cut_at == 0 && self.dropped == 0 {
            0
        } else if cut_at > 0 && bytes.get(cut_at.saturating_sub(1)) == Some(&b'\n') {
            cut_at
        } else {
            let rest = bytes.get(cut_at..).unwrap_or_default();
            rest.iter().position(|b| *b == b'\n').map_or(bytes.len(), |at| cut_at.saturating_add(at).saturating_add(1))
        };
        let text = String::from_utf8_lossy(bytes.get(from..).unwrap_or_default()).into_owned();
        bytes.zeroize();
        scrub(text, secrets)
    }
}

impl Drop for Tail {
    /// Overwrites the kept bytes, and the spare room where dropped ones were, before they are freed.
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// Reads `stream` to its end into `tail`, overwriting its read buffer at the end, then marks the
/// tail closed.
pub(crate) fn read_tail(mut stream: impl Read, tail: Arc<Mutex<Tail>>) {
    let mut buf = vec![0u8; 8 * 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => tail.lock().unwrap_or_else(PoisonError::into_inner).push(buf.get(..n).unwrap_or_default()),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    buf.zeroize();
    tail.lock().unwrap_or_else(PoisonError::into_inner).closed = true;
}

/// What replaces a secret in shown text.
const REDACTED: &str = "<redacted>";

/// `text` with every occurrence of each non-empty secret replaced by `<redacted>`. The text that held
/// a secret is overwritten before it is freed.
pub(crate) fn scrub(mut text: String, secrets: &[Secret]) -> String {
    for secret in secrets {
        let value = secret.expose();
        if !value.is_empty() && text.contains(value) {
            let clean = text.replace(value, REDACTED);
            text.zeroize();
            text = clean;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::sync_channel;

    use super::*;

    /// Every event the reader sends for `input` with a cap of `max`.
    fn events(input: &[u8], max: usize) -> Vec<OutputEvent> {
        let (tx, rx) = sync_channel(64);
        read_lines(input, max, RuntimeKind::Claude, tx);
        rx.into_iter().collect()
    }

    /// Lines come whole; a line past the cap is counted and dropped, and the next line is kept.
    #[test]
    fn lines_past_the_cap_are_dropped_and_counted() {
        let mut input = b"one\n".to_vec();
        input.extend(std::iter::repeat_n(b'x', 300_000));
        input.extend(b"\ntwo\nlast");
        assert_eq!(
            events(&input, 1024),
            vec![
                OutputEvent::Line("one".into()),
                OutputEvent::Dropped(Dropped::TooLong { bytes: 300_000 }),
                OutputEvent::Line("two".into()),
                OutputEvent::Line("last".into()),
                OutputEvent::End,
            ]
        );
        // Exactly at the cap is kept; one more is not.
        assert_eq!(
            events(b"abcd\nabcde\n", 4),
            vec![OutputEvent::Line("abcd".into()), OutputEvent::Dropped(Dropped::TooLong { bytes: 5 }), OutputEvent::End]
        );
        assert_eq!(
            events(b"\xff\xfe\nok\n", 16),
            vec![OutputEvent::Dropped(Dropped::NotText { bytes: 2 }), OutputEvent::Line("ok".into()), OutputEvent::End]
        );
        assert_eq!(events(b"", 16), vec![OutputEvent::End]);
    }

    /// The tail keeps the last bytes, starts at a line once it was cut, and hides secrets.
    #[test]
    fn the_tail_is_capped_starts_at_a_line_and_hides_secrets() {
        let mut tail = Tail::new(16);
        tail.push(b"short\n");
        assert_eq!(tail.render(&[], usize::MAX), "short\n");
        tail.push(b"0123456789\nend sk-1\n");
        let text = tail.render(&[], usize::MAX);
        assert_eq!(text, "end sk-1\n");
        let secret = Secret::new("sk-1".to_owned()).unwrap();
        assert_eq!(tail.render(&[secret], usize::MAX), "end <redacted>\n");
        let mut big = Tail::new(4);
        big.push(&[b'a'; 100]);
        assert_eq!(big.bytes.len(), 4);
        assert_eq!(big.render(&[], usize::MAX), "", "a cut tail without a line break shows nothing of the cut line");
        // A smaller cap on rendering starts at a line too.
        let mut lines = Tail::new(1024);
        lines.push(b"first line\nsecond\nthird\n");
        assert_eq!(lines.render(&[], 13), "second\nthird\n");
        assert_eq!(lines.render(&[], 12), "third\n");
        assert_eq!(lines.render(&[], 6), "third\n", "a cut right after a line break keeps that line");
        assert_eq!(lines.render(&[], 4), "");
    }
}
