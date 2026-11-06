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
/// The most bytes the file may have on disk: every copy this module writes fits. Its text is at
/// most [`MAX_HISTORY_BYTES`], which JSON escaping may grow six times (a control character becomes
/// `\u00XX`), and each message and the file itself add at most 64 bytes of JSON around it.
const MAX_FILE_BYTES: u64 = 6 * MAX_HISTORY_BYTES as u64 + 64 * (MAX_HISTORY_ENTRIES as u64 + 1);
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

    /// The items `thread/inject_items` takes (Responses API messages) for every message but the
    /// newest, which is the one waiting to be sent: the newest of them whose JSON array, as it is
    /// written (escaping, the objects around each text, the commas and brackets), takes at most
    /// `max_bytes`. The oldest are dropped first, so what is kept is the most recent stretch of the
    /// conversation, without a gap; when even the newest earlier message alone does not fit, none
    /// is. Each drop is logged by count.
    pub fn items_before_last(&self, max_bytes: usize) -> Vec<Value> {
        let earlier = self.entries.len().saturating_sub(1);
        let items: Vec<(Value, usize)> = self
            .entries
            .iter()
            .take(earlier)
            .map(|e| {
                let part = match e.role {
                    Role::User => "input_text",
                    Role::Assistant => "output_text",
                };
                let item = json!({ "type": "message", "role": e.role.as_str(), "content": [{ "type": part, "text": e.text }] });
                let bytes = item.to_string().len();
                (item, bytes)
            })
            .collect();
        // `[` and `]`, then each item and a comma before every item but the first.
        let mut total = 2usize;
        let mut first_kept = items.len();
        for (at, (_, bytes)) in items.iter().enumerate().rev() {
            let comma = usize::from(first_kept < items.len());
            let grown = total.saturating_add(*bytes).saturating_add(comma);
            if grown > max_bytes {
                break;
            }
            total = grown;
            first_kept = at;
        }
        if first_kept > 0 {
            log::info!("{first_kept} of the earlier messages do not fit the line limit and are not given to the new thread");
        }
        items.into_iter().skip(first_kept).map(|(item, _)| item).collect()
    }

    /// The file's text.
    fn to_json(&self) -> String {
        let entries: Vec<Value> = self.entries.iter().map(|e| json!({ "role": e.role.as_str(), "text": e.text })).collect();
        json!({ "version": VERSION, "entries": entries }).to_string()
    }

    /// The copy in `folder`: empty when there is none yet. A copy that cannot be read (past its
    /// size, not a regular file, not this layout) is renamed to `codex-history.json.damaged-<n>`,
    /// logged, and refused as a typed error naming the file and where it went. When the rename
    /// fails, or 64 damaged copies are already set aside, or the system refuses the read itself,
    /// the file stays where it is and the error says so; a session start that would write the copy
    /// (`Start::New`, `Start::ResumeFrom`) is then refused too, so the file is never replaced.
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
    // One message past the cap is what a copy holds while its newest is added, and it is trimmed
    // as before; more would have its newest messages left out, so it is set aside whole instead.
    if list.len() > MAX_HISTORY_ENTRIES.saturating_add(1) {
        return Err(Unreadable::Damaged(format!("holds {} messages, more than the {MAX_HISTORY_ENTRIES} a conversation copy keeps", list.len())));
    }
    let mut history = History::default();
    for item in list {
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
/// copy that cannot be moved stays, which the answer says. A name taken meanwhile, by another
/// process setting a copy aside too, is skipped, never replaced ([`move_aside`]).
fn set_aside(path: &std::path::Path) -> String {
    for n in 1..=64u32 {
        let aside = path.with_file_name(format!("{HISTORY_FILE}.damaged-{n}"));
        match move_aside(path, &aside) {
            Ok(()) => {
                log::warn!("the damaged Codex conversation copy was set aside as {}", aside.display());
                return aside.display().to_string();
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                log::warn!("the damaged Codex conversation copy could not be set aside: {e}");
                return "nothing: it could not be moved".to_owned();
            }
        }
    }
    "nothing: 64 damaged copies are already set aside".to_owned()
}

/// Moves `path` to `aside` without ever replacing what is there. A rename alone would replace a
/// file that appeared at `aside` after it was found free, so the name is first claimed with a new
/// empty file (a new empty folder when `path` is a folder), which fails with `AlreadyExists` when
/// anything is there; the rename then replaces only that placeholder. A failed rename removes the
/// placeholder again.
fn move_aside(path: &std::path::Path, aside: &std::path::Path) -> std::io::Result<()> {
    let folder = std::fs::symlink_metadata(path)?.is_dir();
    if folder {
        std::fs::create_dir(aside)?;
    } else {
        std::fs::OpenOptions::new().write(true).create_new(true).open(aside)?;
    }
    std::fs::rename(path, aside).inspect_err(|_| {
        let removed = if folder { std::fs::remove_dir(aside) } else { std::fs::remove_file(aside) };
        if let Err(e) = removed {
            log::warn!("the placeholder {} stays: {e}", aside.display());
        }
    })
}

/// Whether the conversation's folder holds a copy (readable or not), for a caller choosing between
/// [`Start::Resume`](super::Start::Resume), which reads it, and
/// [`Start::ResumeFrom`](super::Start::ResumeFrom), which replaces it. Only a missing file is
/// `false`: when the system cannot tell (the folder cannot be searched, say), that is an error,
/// never taken for "no copy".
pub fn exists(folder: &ConversationFolder) -> Result<bool, AgentError> {
    let path = path(folder);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(AgentError::Folder { path, problem: format!("cannot be checked for a conversation copy: {e}") }),
    }
}

/// Where the copy of the conversation in `folder` is kept.
pub fn path(folder: &ConversationFolder) -> PathBuf {
    folder.path().join(HISTORY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A copy one message past the cap is trimmed as it is read, as while its newest message is
    /// added; one with more is set aside whole, so none of its newest messages is left out.
    #[test]
    fn an_oversized_copy_is_set_aside_whole() {
        let base = std::env::temp_dir().join(format!("archforge-codex-oversized-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let ws = crate::Workspace::open(&base.join("ws")).unwrap();
        let folder = ws.conversation(&crate::ConversationId::new("long").unwrap()).unwrap();
        let file = |count: usize| {
            let entries: Vec<Value> = (0..count).map(|n| json!({ "role": "user", "text": format!("m{n}") })).collect();
            json!({ "version": VERSION, "entries": entries }).to_string()
        };
        std::fs::write(path(&folder), file(MAX_HISTORY_ENTRIES + 1)).unwrap();
        let history = History::read(&folder).unwrap();
        assert_eq!(history.entries().len(), MAX_HISTORY_ENTRIES);
        assert_eq!(history.entries()[0].text, "m1", "the oldest is trimmed");
        let oversized = file(4100);
        std::fs::write(path(&folder), &oversized).unwrap();
        let error = History::read(&folder).unwrap_err();
        assert!(matches!(&error, AgentError::Folder { problem, .. } if problem.contains("holds 4100 messages")), "{error:?}");
        assert!(!path(&folder).exists(), "set aside");
        let aside = folder.path().join(format!("{HISTORY_FILE}.damaged-1"));
        assert_eq!(std::fs::read_to_string(aside).unwrap(), oversized, "whole");
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A damaged copy is never moved over a name that is taken, even one taken after it was found
    /// free: the move claims the name first and refuses a taken one, and setting aside skips to the
    /// next free name, for a file and for a folder alike.
    #[test]
    fn a_damaged_copy_never_replaces_another() {
        let dir = std::env::temp_dir().join(format!("archforge-codex-aside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join(HISTORY_FILE);
        let first = dir.join(format!("{HISTORY_FILE}.damaged-1"));
        std::fs::write(&copy, "newer").unwrap();
        std::fs::write(&first, "older").unwrap();
        assert_eq!(move_aside(&copy, &first).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "older", "the taken name is not replaced");
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "newer", "and the copy stays");
        assert!(set_aside(&copy).ends_with(".damaged-2"));
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "older");
        assert_eq!(std::fs::read_to_string(dir.join(format!("{HISTORY_FILE}.damaged-2"))).unwrap(), "newer");
        assert!(!copy.exists());
        std::fs::create_dir(&copy).unwrap();
        std::fs::write(copy.join("inside"), "kept").unwrap();
        std::fs::create_dir(dir.join(format!("{HISTORY_FILE}.damaged-3"))).unwrap();
        assert!(set_aside(&copy).ends_with(".damaged-4"));
        assert_eq!(std::fs::read_to_string(dir.join(format!("{HISTORY_FILE}.damaged-4/inside"))).unwrap(), "kept");
        assert_eq!(std::fs::read_dir(dir.join(format!("{HISTORY_FILE}.damaged-3"))).unwrap().count(), 0, "the taken folder is untouched");
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
        let items = history.items_before_last(1000);
        assert!(items.len() < 20 && !items.is_empty());
        assert!(Value::Array(items.clone()).to_string().len() <= 1000);
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

    /// The budget is measured on the JSON as it is written: text that grows when escaped (quotes,
    /// backslashes, control characters) counts at its escaped size, right to the byte.
    #[test]
    fn the_injected_items_fit_their_budget_as_json() {
        let mut history = History::default();
        let grows = "\"\\\u{1}\n".repeat(50);
        for role in [Role::User, Role::Assistant, Role::User] {
            history.push(role, grows.clone());
        }
        history.push(Role::Assistant, "older".into());
        history.push(Role::User, "waiting".into());
        let all = history.items_before_last(usize::MAX);
        assert_eq!(all.len(), 4, "the waiting message is never among them");
        let size = |items: &[Value]| Value::Array(items.to_vec()).to_string().len();
        let last_two = size(&all[2..]);
        assert!(last_two > 2 * (grows.len() + 64), "the escaped size is what is measured: {last_two}");
        assert_eq!(history.items_before_last(last_two), all[2..].to_vec(), "exactly at the budget, both fit");
        assert_eq!(history.items_before_last(last_two - 1), all[3..].to_vec(), "one byte less, the older one goes");
        assert!(history.items_before_last(size(&all[3..]) - 1).is_empty(), "the newest alone too large: none, no gap");
        for budget in [2, 100, 1000, last_two, size(&all)] {
            assert!(size(&history.items_before_last(budget)) <= budget.max(2), "{budget}");
        }
    }

    /// A copy as large as the caps allow, all of it text that grows the most when escaped, is
    /// written and read back: every copy this module writes fits the file cap.
    #[test]
    fn the_largest_copy_reads_back() {
        let base = std::env::temp_dir().join(format!("archforge-codex-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let ws = crate::Workspace::open(&base.join("ws")).unwrap();
        let folder = ws.conversation(&crate::ConversationId::new("big").unwrap()).unwrap();
        let mut history = History::default();
        for _ in 0..(MAX_HISTORY_BYTES / MAX_ENTRY_BYTES + 2) {
            history.push(Role::User, "\u{1}".repeat(MAX_ENTRY_BYTES));
        }
        // Fill what the text cap leaves with one more message.
        let cost: usize = history.entries().iter().map(|e| e.text.len() + 64).sum();
        history.push(Role::Assistant, "\u{1}".repeat(MAX_HISTORY_BYTES - cost - 64));
        let cost: usize = history.entries().iter().map(|e| e.text.len() + 64).sum();
        assert_eq!(cost, MAX_HISTORY_BYTES, "the copy is at its text cap, and escaping grows every byte six times");
        history.write(&folder).unwrap();
        let written = std::fs::metadata(path(&folder)).unwrap().len();
        assert!(written <= MAX_FILE_BYTES, "{written} > {MAX_FILE_BYTES}");
        assert_eq!(History::read(&folder).unwrap(), history);
        // The largest file any copy makes (the figure the README gives): four messages of control
        // characters filling the text cap, since a fifth message costs 64 bytes of text, which is
        // 384 escaped bytes, for about 30 bytes of JSON.
        let mut worst = History::default();
        for _ in 0..4 {
            worst.push(Role::Assistant, "\u{1}".repeat((MAX_HISTORY_BYTES - 4 * 64) / 4));
        }
        worst.write(&folder).unwrap();
        assert_eq!(std::fs::metadata(path(&folder)).unwrap().len(), 25_164_437);
        assert_eq!(History::read(&folder).unwrap(), worst);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
