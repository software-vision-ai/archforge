//! The private folders runtimes run in: an app-owned workspace holding one folder per conversation,
//! each readable by this user alone, and short-lived folders for the version probe. Only a folder
//! this crate names and checks is ever removed.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{AgentError, sys};

/// The longest conversation id, in bytes.
pub const MAX_CONVERSATION_ID_BYTES: usize = 64;

/// The prefix of a conversation's folder name.
const CONVERSATION_PREFIX: &str = "conversation-";
/// The prefix of a version probe's folder name.
const PROBE_PREFIX: &str = "probe-";
/// How many names a private file's temporary file tries before giving up.
const TEMP_TRIES: u32 = 16;
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

/// Writes `bytes` to the file `path` so that it holds either its old content or all of the new,
/// readable by this user alone. `purpose` names the content in errors and the log, which never hold
/// the content itself.
///
/// The steps: a new temporary file is made beside the target (exclusively, mode 0600, never
/// following a link), written and synchronised; before the rename it is checked to be the very file
/// written (the same device and inode as the open handle), a regular file of this user with mode
/// 0600; then it is renamed over the target, which replaces whatever stood there, a link among
/// them, without touching what a link pointed at. A failure before the rename removes the temporary
/// file alone; after the rename the target is never deleted. The folder is synchronised last, and a
/// failure there is logged as a warning, not an error: the file is already in place.
pub(crate) fn write_private_file(path: &Path, bytes: &[u8], purpose: &str) -> Result<(), AgentError> {
    let failed = |problem: String| AgentError::Folder { path: path.to_path_buf(), problem };
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(failed(format!("cannot hold {purpose}: it names no file in a folder")));
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut created = None;
    for _ in 0..TEMP_TRIES {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(".tmp-{}-{n}", std::process::id()));
        let temp = parent.join(temp_name);
        match sys::create_private_file(&temp) {
            Ok(file) => {
                created = Some((temp, file));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(failed(format!("could not hold {purpose}: its temporary file could not be made: {e}"))),
        }
    }
    let Some((temp, mut file)) = created else {
        return Err(failed(format!("could not hold {purpose}: no free name for its temporary file after {TEMP_TRIES} tries")));
    };
    let uid = sys::current_uid();
    let prepared = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("could not be written: {e}"))
        .and_then(|()| file.metadata().map_err(|e| format!("could not be checked: {e}")))
        .and_then(|handle| match sys::same_private_file(&handle, &temp, uid) {
            Some(problem) => Err(format!("its temporary file {problem}")),
            None => Ok(()),
        })
        .and_then(|()| std::fs::rename(&temp, path).map_err(|e| format!("could not be put in place: {e}")));
    drop(file);
    if let Err(problem) = prepared {
        // Only the temporary file goes; whatever stands at its name now is removed, never followed.
        if let Err(e) = std::fs::remove_file(&temp) {
            log::warn!("the temporary file for {purpose} in {} stays: {e}", parent.display());
        }
        return Err(failed(format!("could not hold {purpose}: {problem}")));
    }
    if let Err(e) = std::fs::File::open(parent).and_then(|dir| dir.sync_all()) {
        log::warn!("the folder {} could not be synchronised after writing {purpose}: {e}", parent.display());
    }
    Ok(())
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

    /// A fresh folder in the temporary directory for one test, named with the process id.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("archforge-folder-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    /// The names of the entries in `dir`, sorted.
    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    /// A private file is written whole with mode 0600, replaces an earlier one, and leaves no
    /// temporary file behind.
    #[cfg(unix)]
    #[test]
    fn a_private_file_is_written_whole_and_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_dir("private-file");
        let target = dir.join("copy.json");
        write_private_file(&target, b"first", "a test copy").unwrap();
        write_private_file(&target, b"second", "a test copy").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
        assert_eq!(std::fs::symlink_metadata(&target).unwrap().permissions().mode() & 0o7777, 0o600);
        assert_eq!(names(&dir), vec!["copy.json".to_owned()], "no temporary file stays");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A link at the target's place is replaced by the file; the file it pointed at, outside the
    /// folder, keeps its content.
    #[cfg(unix)]
    #[test]
    fn a_link_at_the_target_never_lets_the_write_reach_its_target() {
        let dir = scratch_dir("private-link");
        let outside = dir.join("outside.txt");
        std::fs::write(&outside, b"keep me").unwrap();
        let inner = dir.join("inner");
        std::fs::create_dir(&inner).unwrap();
        let target = inner.join("copy.json");
        std::os::unix::fs::symlink(&outside, &target).unwrap();
        write_private_file(&target, b"new content", "a test copy").unwrap();
        assert_eq!(std::fs::read(&outside).unwrap(), b"keep me", "the linked file is untouched");
        let meta = std::fs::symlink_metadata(&target).unwrap();
        assert!(meta.file_type().is_file(), "the link was replaced by a regular file");
        assert_eq!(std::fs::read(&target).unwrap(), b"new content");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A write that fails before the rename removes its temporary file alone and keeps the target.
    #[cfg(unix)]
    #[test]
    fn a_failed_write_removes_only_its_temporary_file() {
        let dir = scratch_dir("private-fail");
        // A folder at the target's place: the rename of a file over it fails.
        let target = dir.join("copy.json");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("inside"), b"x").unwrap();
        let error = write_private_file(&target, b"data", "a test copy").unwrap_err();
        assert!(matches!(&error, AgentError::Folder { problem, .. } if problem.contains("a test copy")), "{error:?}");
        assert!(!error.to_string().contains("data"));
        assert_eq!(names(&dir), vec!["copy.json".to_owned()], "only the temporary file went");
        assert!(target.join("inside").is_file(), "the target is kept");
        std::fs::remove_dir_all(&dir).unwrap();
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
