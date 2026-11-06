//! Archforge's own copy of a Codex conversation, kept in the conversation's private folder.
//!
//! Codex keeps a thread's history in its settings folder, where the sign-in lives too, and that
//! folder is shared by every conversation (the keyring entry is named after its path). So threads
//! are ephemeral: Codex keeps nothing of them once the process ends, and this copy, the person's
//! messages and the answers in order, goes back into a new thread with `thread/inject_items` when
//! the conversation is resumed. The copy is deleted with the conversation's folder.

use std::io::Read;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::{AgentError, ConversationFolder};

/// The file, in the conversation's folder, that holds the copy.
pub const HISTORY_FILE: &str = "codex-history.json";
/// The most bytes of text the copy keeps; past it the oldest messages are dropped.
pub const MAX_HISTORY_BYTES: usize = 4 * 1024 * 1024;
/// The most bytes one message keeps; a longer one is cut (and the cut logged), so one message can
/// never push the others out.
pub const MAX_ENTRY_BYTES: usize = 1024 * 1024;
/// The most messages the copy keeps; past it the oldest are dropped.
pub const MAX_HISTORY_ENTRIES: usize = 4096;
/// The most bytes the file may have on disk (its text may grow by escaping).
const MAX_FILE_BYTES: u64 = 6 * 4 * 1024 * 1024;
/// The version of the file's layout.
const VERSION: u64 = 1;

/// Who wrote a message of the copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The person.
    User,
    /// The agent's answer.
    Assistant,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// One message of the copy.
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    pub role: Role,
    pub text: String,
}

impl std::fmt::Debug for Entry {
    /// Writes the role and the length; never the text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry").field("role", &self.role).field("bytes", &self.text.len()).finish()
    }
}

/// The messages of a conversation, oldest first, within [`MAX_HISTORY_ENTRIES`] and
/// [`MAX_HISTORY_BYTES`].
#[derive(Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<Entry>,
}

impl std::fmt::Debug for History {
    /// Writes the number of messages; never their text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("History").field("entries", &self.entries.len()).finish()
    }
}

impl History {
    /// The messages, oldest first.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Adds a message, cut to [`MAX_ENTRY_BYTES`] at a character boundary; the oldest messages are
    /// dropped while the copy is past its caps, never the one just added.
    pub fn push(&mut self, role: Role, text: String) {
        let text = if text.len() > MAX_ENTRY_BYTES {
            log::warn!("a message of {} bytes is kept in the Codex conversation copy cut to {MAX_ENTRY_BYTES}", text.len());
            super::rpc::capped(&text, MAX_ENTRY_BYTES)
        } else {
            text
        };
        self.entries.push(Entry { role, text });
        trim(&mut self.entries, MAX_HISTORY_BYTES);
    }

    /// Takes back the newest message (a message that could not be sent).
    pub(crate) fn pop(&mut self) {
        self.entries.pop();
    }

    /// The items `thread/inject_items` takes (Responses API messages) for every message but the
    /// newest, which is the one waiting to be sent; the oldest are dropped until they fit
    /// `max_bytes` of JSON text.
    pub fn items_before_last(&self, max_bytes: usize) -> Vec<Value> {
        let earlier = self.entries.len().saturating_sub(1);
        let mut entries: Vec<Entry> = self.entries.iter().take(earlier).cloned().collect();
        // JSON escaping may grow text up to six times (a control character becomes \u00XX).
        trim(&mut entries, max_bytes / 6);
        entries
            .iter()
            .map(|e| {
                let part = match e.role {
                    Role::User => "input_text",
                    Role::Assistant => "output_text",
                };
                json!({ "type": "message", "role": e.role.as_str(), "content": [{ "type": part, "text": e.text }] })
            })
            .collect()
    }

    /// The file's text.
    fn to_json(&self) -> String {
        let entries: Vec<Value> = self.entries.iter().map(|e| json!({ "role": e.role.as_str(), "text": e.text })).collect();
        json!({ "version": VERSION, "entries": entries }).to_string()
    }

    /// The copy in `folder`: empty when there is none yet. A copy that cannot be read (past its
    /// size, not a regular file, not this layout) is set aside as `codex-history.json.damaged-<n>`
    /// so a later write cannot replace it, logged, and refused as a typed error naming the file.
    pub fn read(folder: &ConversationFolder) -> Result<History, AgentError> {
        let path = path(folder);
        match read_file(&path) {
            Ok(history) => Ok(history),
            Err(Unreadable::Io(error)) => Err(error),
            Err(Unreadable::Damaged(problem)) => {
                let aside = set_aside(&path);
                Err(AgentError::Folder { path, problem: format!("{problem}; it was set aside as {aside}") })
            }
        }
    }

    /// Writes the copy into `folder`, atomically and readable by this user alone (0600).
    pub fn write(&self, folder: &ConversationFolder) -> Result<PathBuf, AgentError> {
        let path = path(folder);
        crate::folder::write_private_file(&path, self.to_json().as_bytes(), "the Codex conversation copy")?;
        Ok(path)
    }
}

/// Drops the oldest of `entries` until at most [`MAX_HISTORY_ENTRIES`] remain and their text takes
/// at most `max_bytes`, never the newest; the drop is logged by count.
fn trim(entries: &mut Vec<Entry>, max_bytes: usize) {
    let cost = |e: &Entry| e.text.len().saturating_add(64);
    let mut total = entries.iter().fold(0usize, |sum, e| sum.saturating_add(cost(e)));
    let mut drop = 0usize;
    let last = entries.len().saturating_sub(1);
    for entry in entries.iter().take(last) {
        if entries.len().saturating_sub(drop) <= MAX_HISTORY_ENTRIES && total <= max_bytes {
            break;
        }
        total = total.saturating_sub(cost(entry));
        drop = drop.saturating_add(1);
    }
    if drop > 0 {
        entries.drain(..drop);
        log::info!("the Codex conversation copy dropped its {drop} oldest messages to stay within its caps");
    }
}

/// Why a copy could not be read.
enum Unreadable {
    /// The system refused: the file stays where it is.
    Io(AgentError),
    /// The file is not a copy Archforge can read.
    Damaged(String),
}

/// Reads the copy at `path`; empty when there is none.
fn read_file(path: &std::path::Path) -> Result<History, Unreadable> {
    let io = |problem: String| Unreadable::Io(AgentError::Folder { path: path.to_path_buf(), problem });
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_file() => return Err(Unreadable::Damaged("is not a regular file".to_owned())),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(History::default()),
        Err(e) => return Err(io(format!("cannot be read: {e}"))),
    }
    let file = std::fs::File::open(path).map_err(|e| io(format!("cannot be opened: {e}")))?;
    let meta = file.metadata().map_err(|e| io(format!("cannot be read: {e}")))?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(Unreadable::Damaged(format!("holds {} bytes, more than the {MAX_FILE_BYTES} a conversation copy may", meta.len())));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES.saturating_add(1)).read_to_end(&mut bytes).map_err(|e| io(format!("cannot be read: {e}")))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(Unreadable::Damaged("grew past its cap while it was read".to_owned()));
    }
    let damaged = || Unreadable::Damaged("is not a Codex conversation copy Archforge can read".to_owned());
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| damaged())?;
    if value.get("version").and_then(Value::as_u64) != Some(VERSION) {
        return Err(damaged());
    }
    let list = value.get("entries").and_then(Value::as_array).ok_or_else(damaged)?;
    let mut history = History::default();
    for item in list.iter().take(MAX_HISTORY_ENTRIES.saturating_add(1)) {
        let role = match item.get("role").and_then(Value::as_str) {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => return Err(damaged()),
        };
        let text = item.get("text").and_then(Value::as_str).ok_or_else(damaged)?;
        history.push(role, text.to_owned());
    }
    Ok(history)
}

/// Renames a damaged copy aside under the first free `.damaged-<n>` name and answers that name; a
/// copy that cannot be moved stays, which the answer says.
fn set_aside(path: &std::path::Path) -> String {
    for n in 1..=64u32 {
        let aside = path.with_file_name(format!("{HISTORY_FILE}.damaged-{n}"));
        if std::fs::symlink_metadata(&aside).is_ok() {
            continue;
        }
        return match std::fs::rename(path, &aside) {
            Ok(()) => {
                log::warn!("the damaged Codex conversation copy was set aside as {}", aside.display());
                aside.display().to_string()
            }
            Err(e) => {
                log::warn!("the damaged Codex conversation copy could not be set aside: {e}");
                "nothing: it could not be moved".to_owned()
            }
        };
    }
    "nothing: 64 damaged copies are already set aside".to_owned()
}

/// Where the copy of the conversation in `folder` is kept.
pub fn path(folder: &ConversationFolder) -> PathBuf {
    folder.path().join(HISTORY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oldest messages go first when the copy passes its caps, never the newest; the injected
    /// items leave out the waiting message and fit their budget.
    #[test]
    fn the_copy_keeps_the_newest_messages_within_its_caps() {
        let mut history = History::default();
        for n in 0..(MAX_HISTORY_ENTRIES + 10) {
            history.push(if n % 2 == 0 { Role::User } else { Role::Assistant }, format!("m{n}"));
        }
        assert_eq!(history.entries().len(), MAX_HISTORY_ENTRIES);
        assert_eq!(history.entries()[0].text, "m10");
        let items = history.items_before_last(6 * 1000);
        assert!(items.len() < 20 && !items.is_empty());
        assert_eq!(items.last().unwrap()["content"][0]["text"], format!("m{}", MAX_HISTORY_ENTRIES + 8), "the waiting message is not injected");
        assert_eq!(items.last().unwrap()["content"][0]["type"], "input_text");

        let mut big = History::default();
        big.push(Role::User, "first".into());
        big.push(Role::Assistant, "second".into());
        big.push(Role::User, "x".repeat(MAX_HISTORY_BYTES + 1));
        assert_eq!(big.entries().len(), 3, "one long message never pushes the others out");
        assert_eq!(big.entries()[2].text.len(), MAX_ENTRY_BYTES + '…'.len_utf8());
        let items = big.items_before_last(8 * 1024 * 1024);
        assert_eq!(items.len(), 2, "a long waiting message does not crowd out the earlier ones");
        assert!(!format!("{big:?}").contains("first"), "Debug shows no text");
    }
}
