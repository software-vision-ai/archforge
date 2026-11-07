//! The private folders runtimes run in: an app-owned workspace holding one folder per conversation,
//! each readable by this user alone, and short-lived folders for the version probe. Only a folder
//! this crate names and checks is ever removed.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{AgentError, sys};

/// The longest conversation id, in bytes.
pub const MAX_CONVERSATION_ID_BYTES: usize = 64;

/// The prefix of a conversation's folder name.
const CONVERSATION_PREFIX: &str = "conversation-";
/// The prefix of a version probe's folder name.
const PROBE_PREFIX: &str = "probe-";
/// The folders inside a conversation's folder.
const HOME: &str = "home";
const CONFIG: &str = "config";
const WORK: &str = "work";
const TMP: &str = "tmp";

/// The id of a conversation, which names its folder: 1 to [`MAX_CONVERSATION_ID_BYTES`] lowercase
/// ASCII letters, digits and hyphens, starting with a letter or digit (a lowercase UUID fits).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConversationId(String);

impl ConversationId {
    /// An id, checked against the rules above. A refused id is not repeated in the error.
    pub fn new(id: &str) -> Result<ConversationId, AgentError> {
        let refuse = |reason: &str| AgentError::InvalidConversationId { bytes: id.len(), reason: reason.to_owned() };
        if id.is_empty() {
            return Err(refuse("it is empty"));
        }
        if id.len() > MAX_CONVERSATION_ID_BYTES {
            return Err(refuse("it is too long"));
        }
        if !id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit()) {
            return Err(refuse("it does not start with a lowercase letter or a digit"));
        }
        if !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return Err(refuse("it holds a character other than a lowercase letter, a digit or a hyphen"));
        }
        Ok(ConversationId(id.to_owned()))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConversationId {
    /// Writes the id; an id is never secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The app-owned folder that holds the runtimes' private folders: a real folder, owned by this user,
/// that no one else may read or enter (mode 0700).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
    uid: u32,
}

impl Workspace {
    /// The workspace at `root`, which must be absolute. A missing folder is made with mode 0700
    /// (with any missing folder above it); an existing one must already be private to this user,
    /// and is refused, never changed, when it is not.
    pub fn open(root: &Path) -> Result<Workspace, AgentError> {
        if !sys::AVAILABLE {
            return Err(AgentError::NotAvailable);
        }
        let problem = |problem: String| AgentError::Folder { path: root.to_path_buf(), problem };
        if !root.is_absolute() {
            return Err(problem("is not an absolute path".to_owned()));
        }
        let uid = sys::current_uid();
        match std::fs::symlink_metadata(root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                sys::make_private_folder(root, true).map_err(|e| problem(format!("cannot be made: {e}")))?;
            }
            Err(e) => return Err(problem(format!("cannot be read: {e}"))),
            Ok(_) => {}
        }
        check_private(root, uid)?;
        let root = root.canonicalize().map_err(|e| problem(format!("cannot be resolved: {e}")))?;
        Ok(Workspace { root, uid })
    }

    /// The workspace's folder, resolved.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The private folder of conversation `id`, made when it does not exist yet, checked when it
    /// does: the folder and each folder inside it (home, configuration, work, temporary) are real
    /// folders of this user with mode 0700. Its contents are kept, so a conversation can be resumed.
    pub fn conversation(&self, id: &ConversationId) -> Result<ConversationFolder, AgentError> {
        let path = self.root.join(format!("{CONVERSATION_PREFIX}{id}"));
        ensure_private(&path, self.uid)?;
        for inner in [HOME, CONFIG, WORK, TMP] {
            ensure_private(&path.join(inner), self.uid)?;
        }
        Ok(ConversationFolder { id: id.clone(), home: path.join(HOME), config: path.join(CONFIG), work: path.join(WORK), tmp: path.join(TMP), path })
    }

    /// Removes the private folder of conversation `id` and everything in it, and says whether there
    /// was one. A folder that is not a real folder of this user is refused and left in place. Stop
    /// the conversation's runtime first: a runtime still running may write into it again.
    pub fn remove_conversation(&self, id: &ConversationId) -> Result<bool, AgentError> {
        let path = self.root.join(format!("{CONVERSATION_PREFIX}{id}"));
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(AgentError::Folder { path, problem: format!("cannot be read: {e}") }),
            Ok(meta) => {
                if let Some(problem) = sys::private_folder_problem(&meta, self.uid) {
                    return Err(AgentError::Folder { path, problem: format!("{problem}, so it is not removed") });
                }
            }
        }
        // `remove_dir_all` never follows a link inside the folder; it removes the link itself.
        std::fs::remove_dir_all(&path).map_err(|e| AgentError::Folder { path, problem: format!("cannot be removed: {e}") })?;
        Ok(true)
    }

    /// A new, empty private folder for one version probe, removed when the returned guard drops.
    pub(crate) fn probe_folder(&self) -> Result<ProbeFolder, AgentError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let mut last = None;
        for _ in 0..16 {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = self.root.join(format!("{PROBE_PREFIX}{}-{n}", std::process::id()));
            match sys::make_private_folder(&path, false) {
                Ok(()) => {
                    check_private(&path, self.uid)?;
                    return Ok(ProbeFolder { path });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
                Err(e) => return Err(AgentError::Folder { path, problem: format!("cannot be made: {e}") }),
            }
        }
        let detail = last.map(|e| e.to_string()).unwrap_or_default();
        Err(AgentError::Folder { path: self.root.clone(), problem: format!("has no free name for a probe folder: {detail}") })
    }
}

/// A conversation's private folder and the folders a runtime is given inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationFolder {
    id: ConversationId,
    path: PathBuf,
    home: PathBuf,
    config: PathBuf,
    work: PathBuf,
    tmp: PathBuf,
}

impl ConversationFolder {
    /// The conversation this folder belongs to.
    pub fn id(&self) -> &ConversationId {
        &self.id
    }

    /// The conversation's folder.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The runtime's `HOME`.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The runtime's settings tree (`CLAUDE_CONFIG_DIR` or `CODEX_HOME`).
    pub fn config(&self) -> &Path {
        &self.config
    }

    /// The runtime's working folder, empty when the conversation starts.
    pub fn work(&self) -> &Path {
        &self.work
    }

    /// The runtime's `TMPDIR`.
    pub fn tmp(&self) -> &Path {
        &self.tmp
    }
}

/// A version probe's folder, removed with everything in it when dropped.
#[derive(Debug)]
pub(crate) struct ProbeFolder {
    path: PathBuf,
}

impl ProbeFolder {
    /// The folder.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProbeFolder {
    /// Removes the folder; a failure is logged, as nothing can be returned from here.
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.path) {
            log::warn!("the probe folder {} stays: {e}", self.path.display());
        }
    }
}

/// Makes `path` as a private folder, or checks the one there.
fn ensure_private(path: &Path, uid: u32) -> Result<(), AgentError> {
    match sys::make_private_folder(path, false) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(AgentError::Folder { path: path.to_path_buf(), problem: format!("cannot be made: {e}") }),
    }
    check_private(path, uid)
}

/// Refuses `path` unless it is a real folder of `uid` with mode 0700.
pub(crate) fn check_private(path: &Path, uid: u32) -> Result<(), AgentError> {
    let meta =
        std::fs::symlink_metadata(path).map_err(|e| AgentError::Folder { path: path.to_path_buf(), problem: format!("cannot be read: {e}") })?;
    match sys::private_folder_problem(&meta, uid) {
        Some(problem) => Err(AgentError::Folder { path: path.to_path_buf(), problem }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids are lowercase letters, digits and hyphens, and a refused id is never repeated.
    #[test]
    fn conversation_ids_are_checked_and_never_repeated_when_refused() {
        assert!(ConversationId::new("6f1c0a52-9d3e-4b7a-8f00-1234567890ab").is_ok());
        assert!(ConversationId::new("a").is_ok());
        for bad in ["", "-a", "A", "a_b", "a/b", "..", "a b", &"a".repeat(65)] {
            let error = ConversationId::new(bad).unwrap_err();
            assert!(matches!(error, AgentError::InvalidConversationId { .. }), "{bad}");
            if bad.len() > 1 {
                assert!(!error.to_string().contains(bad), "{error}");
            }
        }
    }
}
