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
/// The prefix of a runtime's account folder name.
const ACCOUNT_PREFIX: &str = "account-";
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

    /// The private folder of `kind`'s account, shaped like a conversation's folder and made or
    /// checked the same way: `account-<program>` in the workspace, never a conversation's (those
    /// are `conversation-<id>`), so removing a conversation never touches it. Its configuration
    /// folder is the settings folder every conversation of that runtime shares: Codex keeps its
    /// sign-in reference there (`CODEX_HOME`; the ChatGPT tokens themselves stay in the system's
    /// keyring), and its home, work and temporary folders serve the processes that only read or
    /// change the account (`codex::CodexAccounts`). Its `id()` is `account-<program>`.
    pub fn account(&self, kind: crate::RuntimeKind) -> Result<ConversationFolder, AgentError> {
        let name = format!("{ACCOUNT_PREFIX}{}", kind.program());
        let id = ConversationId::new(&name)?;
        let path = self.root.join(&name);
        ensure_private(&path, self.uid)?;
        for inner in [HOME, CONFIG, WORK, TMP] {
            ensure_private(&path.join(inner), self.uid)?;
        }
        Ok(ConversationFolder { id, home: path.join(HOME), config: path.join(CONFIG), work: path.join(WORK), tmp: path.join(TMP), path })
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

/// How many temporary names [`write_private_file`] tries before it gives up.
const TEMP_TRIES: usize = 16;

/// Writes `bytes` to `path` atomically (a temporary file beside it, synchronised, renamed over it),
/// readable by this user alone (0600); a failed write leaves an earlier file whole. Nothing on the
/// way follows a link: the temporary file is created new (it fails when anything, a link included,
/// already has its name) with mode 0600, and the rename replaces whatever is at `path`, a link
/// itself rather than what it names, so a link put in its place, even between a check and the
/// write, can never send the bytes out of the private folder. A folder at `path` is refused. `what`
/// names the file in errors.
pub(crate) fn write_private_file(path: &Path, bytes: &[u8], what: &str) -> Result<(), AgentError> {
    use craft_atomic::AtomicFs;
    use std::io::Write;
    let failed = |problem: String| AgentError::Folder { path: path.to_path_buf(), problem };
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(failed(format!("names no file in a folder for {what}")));
    };
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(failed(format!("holds a folder where {what} belongs")));
    }
    let fs = craft_atomic::RealFs;
    let mut created = None;
    for _ in 0..TEMP_TRIES {
        let temp = dir.join(craft_atomic::temp_name(&name.to_string_lossy()));
        match fs.create_new(&temp, craft_atomic::NewFilePermissions::Private) {
            Ok(file) => {
                created = Some((temp, file));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(failed(format!("could not hold {what}: {e}"))),
        }
    }
    let Some((temp, mut file)) = created else {
        return Err(failed(format!("found no free temporary name beside {what} after {TEMP_TRIES} tries")));
    };
    let written = file.write_all(bytes).and_then(|()| fs.sync(&file));
    drop(file);
    if let Err(e) = written.and_then(|()| fs.rename(&temp, path)) {
        let _ = fs.remove(&temp);
        return Err(failed(format!("could not hold {what}: {e}")));
    }
    // The bytes are in place; a failed folder sync cannot be undone, so it is best effort.
    let _ = fs.sync_dir(dir);
    if !std::fs::symlink_metadata(path).is_ok_and(|m| private_file(&m)) {
        let _ = std::fs::remove_file(path);
        return Err(failed(format!("did not keep {what} private to this user")));
    }
    Ok(())
}

/// Whether `meta` is a regular file (not a link) of this user that no one else may read.
fn private_file(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.file_type().is_file() && meta.uid() == sys::current_uid() && meta.mode() & 0o077 == 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// Makes `path` as a private folder, or checks the one there.
pub(crate) fn ensure_private(path: &Path, uid: u32) -> Result<(), AgentError> {
    match sys::make_private_folder(path, false) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(AgentError::Folder { path: path.to_path_buf(), problem: format!("cannot be made: {e}") }),
    }
    check_private(path, uid)
}

/// Refuses `path` unless it is a real folder of `uid` with mode 0700.
fn check_private(path: &Path, uid: u32) -> Result<(), AgentError> {
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

    /// A private write replaces a link at its place, never the file the link names: the file
    /// outside stays as it was, and the place holds a private regular file with the bytes.
    #[test]
    #[cfg(unix)]
    fn a_private_write_never_follows_a_link() {
        let base = std::env::temp_dir().join(format!("archforge-private-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        sys::make_private_folder(&base.join("private"), true).unwrap();
        let outside = base.join("outside.txt");
        std::fs::write(&outside, "keep").unwrap();
        let target = base.join("private/copy.json");
        std::os::unix::fs::symlink(&outside, &target).unwrap();
        write_private_file(&target, b"new", "the copy").unwrap();
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep", "the file the link named is untouched");
        let meta = std::fs::symlink_metadata(&target).unwrap();
        assert!(private_file(&meta), "a private regular file took the link's place");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        let names: Vec<_> = std::fs::read_dir(base.join("private")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("copy.json")], "no temporary file is left");
        // A folder in its place is refused, and stays.
        std::fs::remove_file(&target).unwrap();
        std::fs::create_dir(&target).unwrap();
        assert!(write_private_file(&target, b"new", "the copy").is_err());
        assert!(target.is_dir());
        std::fs::remove_dir_all(&base).unwrap();
    }

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
