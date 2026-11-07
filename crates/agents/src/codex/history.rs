//! The conversation's copy: the text messages of a Codex conversation, kept by Archforge in the
//! conversation's private folder (`codex-history.json`, mode 0600), because every Codex thread is
//! ephemeral (its history stays out of the shared `CODEX_HOME`, where the sign-in lives). A resumed
//! conversation starts a new thread and hands it the copy with `thread/inject_items`. The copy
//! holds what was said, not tool calls or their results.
//!
//! Budget: at most [`MAX_MESSAGES`] messages and [`MAX_TEXT_BYTES`] of message text, each message
//! counted as its text plus [`MESSAGE_OVERHEAD`] bytes and cut at [`MAX_MESSAGE_BYTES`]; the oldest
//! go first. The open turn's question comes on top until the turn's first answer, so a copy holds
//! at most [`MAX_COPY_ENTRIES`] messages and is at most [`LARGEST_COPY_BYTES`] long.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::AgentError;
use crate::folder::write_private_file;

/// The copy's file in the conversation's folder.
pub const COPY_FILE: &str = "codex-history.json";
/// The most messages the history keeps.
pub const MAX_MESSAGES: usize = 4096;
/// The most message text the history keeps, in bytes, each message counted with
/// [`MESSAGE_OVERHEAD`].
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
/// What each message counts on top of its text against [`MAX_TEXT_BYTES`].
pub const MESSAGE_OVERHEAD: usize = 64;
/// The longest message kept, in bytes; a longer answer is cut at a character boundary, and a longer
/// question is refused before it is sent.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
/// The most messages a copy on disk may hold: the budget and the open turn's question. A copy with
/// more is damaged; one with exactly this many is trimmed to [`MAX_MESSAGES`] when read.
pub const MAX_COPY_ENTRIES: usize = MAX_MESSAGES + 1;
/// The copy's format version.
const VERSION: u64 = 1;

/// The JSON around the messages and around each message, as `serde_json` writes the copy (its keys
/// sorted): `{"messages":[` … `],"version":1}` and `{"role":"…","text":"…"}`.
const HEAD: &str = "{\"messages\":[";
const TAIL: &str = "],\"version\":1}";
const ENTRY_WITHOUT_ROLE: usize = "{\"role\":\"\",\"text\":\"\"}".len();
/// The most bytes JSON writes for one byte of text: a control character as `\u00XX`.
const ESCAPE_FACTOR: usize = 6;

/// The longest copy the writer can make, in bytes: four assistant messages of
/// `MAX_MESSAGE_BYTES - MESSAGE_OVERHEAD` bytes fill the text budget exactly (no other split puts
/// more text in it, as each further message costs [`MESSAGE_OVERHEAD`] of text for fewer bytes of
/// JSON), the open turn's question adds [`MAX_MESSAGE_BYTES`], and every byte of text is a control
/// character that JSON writes in six: 31,455,920 bytes.
pub const LARGEST_COPY_BYTES: usize = HEAD.len()
    + TAIL.len()
    + FULL_MESSAGES * (ENTRY_WITHOUT_ROLE + "assistant".len() + ESCAPE_FACTOR * (MAX_MESSAGE_BYTES - MESSAGE_OVERHEAD))
    + (ENTRY_WITHOUT_ROLE + "user".len() + ESCAPE_FACTOR * MAX_MESSAGE_BYTES)
    + FULL_MESSAGES;
/// How many messages of the longest size fill the text budget.
const FULL_MESSAGES: usize = MAX_TEXT_BYTES / MAX_MESSAGE_BYTES;
/// The longest copy file the reader takes, in bytes: above [`LARGEST_COPY_BYTES`], so every copy the
/// writer makes reads back. A longer file is damaged.
pub const MAX_COPY_FILE_BYTES: usize = 32 * 1024 * 1024;

/// Who said a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The person.
    User,
    /// Codex.
    Assistant,
}

impl Role {
    /// The role's name in the copy.
    fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// One message of the conversation.
#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

impl std::fmt::Debug for Message {
    /// Writes the role and the text's length, never the text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message").field("role", &self.role).field("text_bytes", &self.text.len()).finish()
    }
}

impl Message {
    /// What the message counts against [`MAX_TEXT_BYTES`].
    fn cost(&self) -> usize {
        self.text.len().saturating_add(MESSAGE_OVERHEAD)
    }
}

/// The conversation's messages within the budget, and the snapshot taken when a turn began.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct History {
    messages: Vec<Message>,
    /// The messages before the open turn's question, while the turn is not yet accepted.
    snapshot: Option<Vec<Message>>,
}

impl std::fmt::Debug for History {
    /// Writes counts only.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("History").field("messages", &self.messages.len()).field("snapshot", &self.snapshot.as_ref().map(Vec::len)).finish()
    }
}

/// Why a copy on disk could not be taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Damage(pub(crate) String);

impl History {
    /// The messages, oldest first, the open turn's question last while it is open.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// The messages before the open turn's question (all of them when no turn is open): what a new
    /// thread is given before the question is sent with `turn/start`.
    pub fn before_turn(&self) -> &[Message] {
        self.snapshot.as_deref().unwrap_or(&self.messages)
    }

    /// Begins a turn: takes a snapshot of the history, then adds the question, which is not trimmed
    /// until the turn's first answer. Refused while a turn is open, and for a question past
    /// [`MAX_MESSAGE_BYTES`].
    pub fn begin_turn(&mut self, text: &str) -> Result<(), AgentError> {
        if self.snapshot.is_some() {
            return Err(AgentError::TurnRunning { runtime: crate::RuntimeKind::Codex });
        }
        if text.len() > MAX_MESSAGE_BYTES {
            return Err(too_long(text.len()));
        }
        self.snapshot = Some(self.messages.clone());
        self.messages.push(Message { role: Role::User, text: text.to_owned() });
        Ok(())
    }

    /// Puts the snapshot back: the turn was never sent, or Codex refused it.
    pub fn restore(&mut self) {
        if let Some(snapshot) = self.snapshot.take() {
            self.messages = snapshot;
        }
    }

    /// Keeps the question for good: Codex accepted the turn, or may have received it.
    pub fn keep(&mut self) {
        self.snapshot = None;
    }

    /// Adds one of Codex's answers, cut at [`MAX_MESSAGE_BYTES`], then trims to the budget.
    pub fn push_answer(&mut self, text: &str) {
        if text.len() > MAX_MESSAGE_BYTES {
            log::debug!("an answer of {} bytes is cut to {MAX_MESSAGE_BYTES} in the conversation's copy", text.len());
        }
        let text = crate::error::capped_to(text.to_owned(), MAX_MESSAGE_BYTES.saturating_sub('…'.len_utf8()));
        self.messages.push(Message { role: Role::Assistant, text });
        self.trim();
    }

    /// Drops the oldest messages until at most [`MAX_MESSAGES`] are left and they cost at most
    /// [`MAX_TEXT_BYTES`].
    pub fn trim(&mut self) {
        let mut cost: usize = self.messages.iter().map(Message::cost).fold(0, usize::saturating_add);
        let mut cut = self.messages.len().saturating_sub(MAX_MESSAGES);
        for message in self.messages.iter().take(cut) {
            cost = cost.saturating_sub(message.cost());
        }
        for message in self.messages.iter().skip(cut) {
            if cost <= MAX_TEXT_BYTES {
                break;
            }
            cost = cost.saturating_sub(message.cost());
            cut = cut.saturating_add(1);
        }
        if cut > 0 {
            log::debug!("the conversation's copy drops its {cut} oldest messages to stay within its budget");
            self.messages.drain(..cut.min(self.messages.len()));
        }
    }

    /// The copy as JSON: `{"messages":[{"role","text"},…],"version":1}`.
    pub(crate) fn to_json(&self) -> String {
        let messages: Vec<Value> = self.messages.iter().map(|m| json!({ "role": m.role.as_str(), "text": m.text })).collect();
        json!({ "messages": messages, "version": VERSION }).to_string()
    }

    /// A copy read from `bytes`: version 1, at most [`MAX_COPY_ENTRIES`] messages, each a known role
    /// and a text of at most [`MAX_MESSAGE_BYTES`]; then trimmed to the budget (a copy of exactly
    /// [`MAX_COPY_ENTRIES`] loses its oldest message). Anything else is damaged.
    pub(crate) fn from_json(bytes: &[u8]) -> Result<History, Damage> {
        if !structure_fits(bytes) {
            return Err(Damage(format!("it holds more values than a copy of {MAX_COPY_ENTRIES} messages can")));
        }
        let value: Value = serde_json::from_slice(bytes).map_err(|e| Damage(format!("it is not JSON ({e})")))?;
        let object = value.as_object().ok_or_else(|| Damage("it is not a JSON object".to_owned()))?;
        if object.get("version").and_then(Value::as_u64) != Some(VERSION) {
            return Err(Damage(format!("its version is not {VERSION}")));
        }
        let entries = object.get("messages").and_then(Value::as_array).ok_or_else(|| Damage("it has no list of messages".to_owned()))?;
        if entries.len() > MAX_COPY_ENTRIES {
            return Err(Damage(format!("it holds {} messages, more than the {MAX_COPY_ENTRIES} a copy can hold", entries.len())));
        }
        let mut messages = Vec::new();
        for (at, entry) in entries.iter().enumerate() {
            messages.push(message(entry.as_object()).map_err(|why| Damage(format!("message {at} {why}")))?);
        }
        let mut history = History { messages, snapshot: None };
        history.trim();
        Ok(history)
    }

    /// Reads the copy at `path`: a missing copy is an empty history; a damaged one (not a regular
    /// file, longer than [`MAX_COPY_FILE_BYTES`], or unreadable as [`History::from_json`] says) is
    /// set aside whole under a free `.damaged-<n>` name, whose path is answered beside an empty
    /// history. Fails only when the copy can be neither read nor set aside.
    pub fn load(path: &Path) -> Result<(History, Option<PathBuf>), AgentError> {
        let meta = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((History::default(), None)),
            Err(e) => {
                return Err(AgentError::Folder { path: path.to_path_buf(), problem: format!("cannot be read as the conversation's copy: {e}") });
            }
        };
        let damage = if !meta.file_type().is_file() {
            Some(Damage("it is not a regular file".to_owned()))
        } else {
            match read_capped(path) {
                Ok(bytes) => match History::from_json(&bytes) {
                    Ok(history) => return Ok((history, None)),
                    Err(damage) => Some(damage),
                },
                Err(why) => Some(why),
            }
        };
        let why = damage.map(|d| d.0).unwrap_or_default();
        let aside = set_aside(path, meta.is_dir())?;
        log::warn!(
            "the conversation's copy {} is damaged ({why}); it was set aside as {} and the conversation goes on without it",
            path.display(),
            aside.display()
        );
        Ok((History::default(), Some(aside)))
    }

    /// Writes the copy to `path` (atomically, mode 0600).
    pub fn save(&self, path: &Path) -> Result<(), AgentError> {
        write_private_file(path, self.to_json().as_bytes(), "the conversation's copy")
    }
}

/// One message of a copy, or what is wrong with it.
fn message(entry: Option<&Map<String, Value>>) -> Result<Message, &'static str> {
    let entry = entry.ok_or("is not an object")?;
    let role = match entry.get("role").and_then(Value::as_str) {
        Some("user") => Role::User,
        Some("assistant") => Role::Assistant,
        _ => return Err("has no known role"),
    };
    let text = entry.get("text").and_then(Value::as_str).ok_or("has no text")?;
    if text.len() > MAX_MESSAGE_BYTES {
        return Err("is longer than a message can be");
    }
    Ok(Message { role, text: text.to_owned() })
}

/// Whether `bytes` hold no more JSON values than a copy can, counted before anything is parsed, so
/// a file of tiny values cannot make the parser build millions of them: every comma and every
/// opening bracket or brace outside strings counts, and a copy has three a message (its brace, its
/// inner comma and the comma after it) and a few around them.
fn structure_fits(bytes: &[u8]) -> bool {
    let limit = MAX_COPY_ENTRIES.saturating_mul(3).saturating_add(16);
    let (mut in_string, mut escaped, mut count) = (false, false, 0usize);
    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b',' | b'[' | b'{' => {
                count = count.saturating_add(1);
                if count > limit {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// The file at `path`, refused when longer than [`MAX_COPY_FILE_BYTES`]; never more is read. It is
/// opened without following a link or blocking, and must be a regular file on the open handle.
fn read_capped(path: &Path) -> Result<Vec<u8>, Damage> {
    let file = crate::sys::open_regular(path).map_err(|e| Damage(format!("it cannot be opened as a regular file ({e})")))?;
    let mut bytes = Vec::new();
    let limit = u64::try_from(MAX_COPY_FILE_BYTES).unwrap_or(u64::MAX).saturating_add(1);
    file.take(limit).read_to_end(&mut bytes).map_err(|e| Damage(format!("it cannot be read ({e})")))?;
    if bytes.len() > MAX_COPY_FILE_BYTES {
        return Err(Damage(format!("it is longer than {MAX_COPY_FILE_BYTES} bytes")));
    }
    Ok(bytes)
}

/// The most `.damaged-<n>` names tried.
const DAMAGED_TRIES: u32 = 64;

/// Moves the damaged copy at `path` (a folder when `is_dir`) to the first free
/// `<name>.damaged-<n>`: the name is claimed first by making an empty file (or folder) there
/// exclusively, so an earlier damaged copy is never replaced, then the copy is renamed over the
/// claim. Answers the new path.
fn set_aside(path: &Path, is_dir: bool) -> Result<PathBuf, AgentError> {
    let failed = |problem: String| AgentError::Folder { path: path.to_path_buf(), problem };
    let mut name = path.file_name().ok_or_else(|| failed("names no file".to_owned()))?.to_os_string();
    name.push(".damaged-");
    for n in 1..=DAMAGED_TRIES {
        let mut candidate = name.clone();
        candidate.push(n.to_string());
        let aside = path.with_file_name(candidate);
        let claimed = if is_dir { crate::sys::make_private_folder(&aside, false) } else { crate::sys::create_private_file(&aside).map(drop) };
        match claimed {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(failed(format!("is damaged and its new name could not be claimed: {e}"))),
        }
        return match std::fs::rename(path, &aside) {
            Ok(()) => Ok(aside),
            Err(e) => {
                let removed = if is_dir { std::fs::remove_dir(&aside) } else { std::fs::remove_file(&aside) };
                if let Err(r) = removed {
                    log::warn!("the claimed name {} stays: {r}", aside.display());
                }
                Err(failed(format!("is damaged and could not be set aside: {e}")))
            }
        };
    }
    Err(failed(format!("is damaged and every name up to .damaged-{DAMAGED_TRIES} is taken")))
}

/// A question past [`MAX_MESSAGE_BYTES`], refused.
pub(crate) fn too_long(bytes: usize) -> AgentError {
    AgentError::InvalidSetting { setting: "message", bytes, reason: format!("a message is at most {MAX_MESSAGE_BYTES} bytes") }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder in the temporary directory for one test, named with the process id.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("archforge-history-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A history of `n` short answers.
    fn answers(n: usize) -> History {
        let mut history = History::default();
        for i in 0..n {
            history.messages.push(Message { role: Role::Assistant, text: format!("a{i}") });
        }
        history
    }

    /// The snapshot comes back when the turn is refused; the question stays once kept.
    #[test]
    fn a_turn_restores_its_snapshot_or_keeps_its_question() {
        let mut history = answers(2);
        let before = history.clone();
        history.begin_turn("question").unwrap();
        assert_eq!(history.before_turn(), before.messages());
        assert_eq!(history.messages().len(), 3);
        assert!(matches!(history.begin_turn("again"), Err(AgentError::TurnRunning { .. })));
        history.restore();
        assert_eq!(history, before, "a refused turn leaves the history as it was");
        history.begin_turn("question").unwrap();
        history.keep();
        assert_eq!(history.before_turn(), history.messages());
        assert_eq!(history.messages().last(), Some(&Message { role: Role::User, text: "question".into() }));
        assert!(matches!(history.begin_turn(&"x".repeat(MAX_MESSAGE_BYTES + 1)), Err(AgentError::InvalidSetting { .. })));
        history.push_answer(&"y".repeat(MAX_MESSAGE_BYTES * 2));
        assert!(history.messages().last().unwrap().text.len() <= MAX_MESSAGE_BYTES);
    }

    /// The budget holds the count and the text; the open turn's question comes on top until the
    /// first answer.
    #[test]
    fn the_budget_holds_and_the_question_comes_on_top() {
        let mut history = answers(MAX_MESSAGES);
        history.begin_turn("q").unwrap();
        history.keep();
        assert_eq!(history.messages().len(), MAX_COPY_ENTRIES, "the question is not trimmed");
        history.push_answer("answer");
        assert_eq!(history.messages().len(), MAX_MESSAGES);
        assert_eq!(history.messages().first().map(|m| m.text.as_str()), Some("a2"), "the oldest went first");
        let mut big = History::default();
        for _ in 0..5 {
            big.push_answer(&"z".repeat(MAX_MESSAGE_BYTES - MESSAGE_OVERHEAD));
        }
        assert_eq!(big.messages().len(), 4, "five full messages do not fit the text budget");
        let cost: usize = big.messages().iter().map(Message::cost).sum();
        assert_eq!(cost, MAX_TEXT_BYTES);
    }

    /// The longest copy the writer can make is exactly `LARGEST_COPY_BYTES`, the reader's cap is
    /// above it, and it reads back, trimmed to the budget as at the end of its turn.
    #[test]
    fn the_largest_copy_reads_back() {
        let mut history = History::default();
        let control = "\u{1}";
        for _ in 0..FULL_MESSAGES {
            history.push_answer(&control.repeat(MAX_MESSAGE_BYTES - MESSAGE_OVERHEAD));
        }
        assert_eq!(history.messages().len(), FULL_MESSAGES);
        history.begin_turn(&control.repeat(MAX_MESSAGE_BYTES)).unwrap();
        history.keep();
        let json = history.to_json();
        assert_eq!(json.len(), LARGEST_COPY_BYTES);
        assert_eq!(LARGEST_COPY_BYTES, 31_455_920);
        const { assert!(MAX_COPY_FILE_BYTES > LARGEST_COPY_BYTES) };
        let dir = scratch_dir("largest");
        let path = dir.join(COPY_FILE);
        history.save(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), u64::try_from(LARGEST_COPY_BYTES).unwrap());
        let (read, aside) = History::load(&path).unwrap();
        assert_eq!(aside, None, "the largest copy is not damaged");
        // Read as at the end of its turn: trimmed to the budget, the question kept.
        let mut expected = history.clone();
        expected.trim();
        assert_eq!(read.messages(), expected.messages());
        assert_eq!(read.messages().last().map(|m| (m.role, m.text.len())), Some((Role::User, MAX_MESSAGE_BYTES)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A copy of tiny values is refused before it is parsed; the largest real copies pass the count.
    #[test]
    fn a_copy_of_tiny_values_is_refused_before_parsing() {
        let tiny = format!("{{\"messages\":[{}0],\"version\":1}}", "0,".repeat(1_000_000));
        let error = History::from_json(tiny.as_bytes()).unwrap_err();
        assert!(error.0.contains("more values"), "{error:?}");
        assert!(structure_fits(answers(MAX_COPY_ENTRIES).to_json().as_bytes()));
        let commas_in_text = History { messages: vec![Message { role: Role::User, text: ",[{".repeat(300_000) }], snapshot: None };
        assert!(History::from_json(commas_in_text.to_json().as_bytes()).is_ok(), "brackets inside a text do not count");
    }

    /// A copy of exactly 4097 messages loses its oldest; one more is damaged.
    #[test]
    fn a_copy_of_4097_is_trimmed_and_4098_is_damaged() {
        let mut history = answers(MAX_COPY_ENTRIES);
        let json = history.to_json();
        let read = History::from_json(json.as_bytes()).unwrap();
        assert_eq!(read.messages().len(), MAX_MESSAGES);
        assert_eq!(read.messages().first().map(|m| m.text.as_str()), Some("a1"));
        history.messages.push(Message { role: Role::User, text: "one more".into() });
        assert!(History::from_json(history.to_json().as_bytes()).is_err());
        for bad in [
            &b"not json"[..],
            br#"[]"#,
            br#"{"messages":[],"version":2}"#,
            br#"{"version":1}"#,
            br#"{"messages":[{"role":"system","text":"x"}],"version":1}"#,
            br#"{"messages":[{"role":"user"}],"version":1}"#,
            br#"{"messages":[7],"version":1}"#,
        ] {
            assert!(History::from_json(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
    }

    /// A damaged copy is set aside whole under a free name; an earlier damaged copy is never
    /// replaced, and a folder in the copy's place is set aside too.
    #[test]
    fn a_damaged_copy_never_replaces_an_earlier_one() {
        let dir = scratch_dir("damaged");
        let path = dir.join(COPY_FILE);
        std::fs::write(&path, b"broken one").unwrap();
        std::fs::write(dir.join(format!("{COPY_FILE}.damaged-1")), b"earlier").unwrap();
        let (history, aside) = History::load(&path).unwrap();
        assert_eq!(history, History::default());
        assert_eq!(aside.as_deref(), Some(dir.join(format!("{COPY_FILE}.damaged-2")).as_path()));
        assert_eq!(std::fs::read(dir.join(format!("{COPY_FILE}.damaged-1"))).unwrap(), b"earlier", "the earlier one is kept");
        assert_eq!(std::fs::read(dir.join(format!("{COPY_FILE}.damaged-2"))).unwrap(), b"broken one", "set aside whole");
        assert!(!path.exists());
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("inside"), b"x").unwrap();
        let (_, aside) = History::load(&path).unwrap();
        let aside = aside.unwrap();
        assert!(aside.ends_with(format!("{COPY_FILE}.damaged-3")) && aside.join("inside").is_file());
        // Too long: never read past the cap.
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(u64::try_from(MAX_COPY_FILE_BYTES).unwrap() + 1).unwrap();
        let (_, aside) = History::load(&path).unwrap();
        assert!(aside.unwrap().ends_with(format!("{COPY_FILE}.damaged-4")));
        assert_eq!(History::load(&dir.join("missing.json")).unwrap(), (History::default(), None));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `Debug` shows lengths, never a message's text.
    #[test]
    fn debug_never_shows_the_text() {
        let mut history = History::default();
        history.begin_turn("secret words").unwrap();
        for shown in [format!("{history:?}"), format!("{history:#?}"), format!("{:?}", history.messages()[0])] {
            assert!(!shown.contains("secret words"), "{shown}");
        }
    }
}
