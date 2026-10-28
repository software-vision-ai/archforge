//! Run folders: one private folder per server start, holding its configuration, its log, its
//! privileges and its socket. Only a folder this crate made is ever removed, and only its known
//! entries.

use std::path::{Path, PathBuf};

use crate::{Error, paths};

const PREFIX: &str = "af-";
const NAME_LEN: usize = 8;
/// The files a run folder holds, and the folders.
const FILES: &[&str] = &["server.yaml", "server.log", "db.sock"];
const FOLDERS: &[&str] = &["doltcfg", "nofiles"];

/// Where run folders go by default: per-user temporary folders, short enough for a socket path.
pub(crate) fn default_root(records: &Path) -> PathBuf {
    if cfg!(windows) {
        return records.join("run");
    }
    let var = if cfg!(target_os = "macos") { "TMPDIR" } else { "XDG_RUNTIME_DIR" };
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute() && p.is_dir()).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// The run root, canonicalised and checked to be a usable folder, with this user's id on unix. A
/// root others can write to must be sticky (as `/tmp` is), so nobody else can rename or replace
/// a run folder in it.
pub(crate) fn checked_root(root: &Path) -> Result<(PathBuf, Option<u32>), Error> {
    let unsafe_root = |why: String| Error::RunRootUnsafe { path: root.to_path_buf(), why };
    if cfg!(windows) {
        std::fs::create_dir_all(root).map_err(|e| unsafe_root(format!("it cannot be created: {e}")))?;
    }
    let canon = root.canonicalize().map_err(|e| unsafe_root(format!("it cannot be read: {e}")))?;
    paths::text("run folder location", &canon)?;
    if !canon.is_dir() {
        return Err(unsafe_root("it is not a folder".into()));
    }
    let uid = root_owner_check(&canon).map_err(unsafe_root)?;
    Ok((canon, uid))
}

#[cfg(unix)]
fn root_owner_check(root: &Path) -> Result<Option<u32>, String> {
    use std::os::unix::fs::MetadataExt;
    let uid = crate::home::current_uid(root).map_err(|e| format!("no folder can be made in it: {e}"))?;
    let meta = std::fs::metadata(root).map_err(|e| format!("it cannot be read: {e}"))?;
    let shared = meta.mode() & 0o022 != 0;
    let sticky = meta.mode() & 0o1000 != 0;
    if shared && !sticky {
        return Err(format!("others can write to it and it is not sticky (mode {:o})", meta.mode() & 0o7777));
    }
    if !shared && meta.uid() != uid && meta.uid() != 0 {
        return Err("another user owns it".into());
    }
    Ok(Some(uid))
}

#[cfg(not(unix))]
fn root_owner_check(_root: &Path) -> Result<Option<u32>, String> {
    Ok(None)
}

pub(crate) fn is_run_dir_name(name: &str) -> bool {
    name.strip_prefix(PREFIX).is_some_and(|rest| rest.len() == NAME_LEN && rest.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

/// A new run folder under `root`, private to this user, made exclusively, and checked again once
/// made.
pub(crate) fn create(root: &Path, uid: Option<u32>) -> Result<PathBuf, Error> {
    let mut last = None;
    for _ in 0..5 {
        let dir = root.join(format!("{PREFIX}{}", crate::secret::random_name(NAME_LEN)?));
        match make_private(&dir) {
            Ok(()) => {
                let meta = std::fs::symlink_metadata(&dir).map_err(|e| Error::io("cannot read the run folder", &dir, e))?;
                if !meta.is_dir() || meta.file_type().is_symlink() || !owned_by(&meta, uid) || !private_mode(&meta) {
                    return Err(Error::RunRootUnsafe {
                        path: root.to_path_buf(),
                        why: format!("the new run folder {} is not private", dir.display()),
                    });
                }
                return Ok(dir);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(Error::io("cannot create the run folder", &dir, e)),
        }
    }
    Err(Error::io("cannot create a run folder under", root, last.unwrap_or_else(|| std::io::Error::other("every name was taken"))))
}

#[cfg(unix)]
fn make_private(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn make_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir(dir)
}

/// Removes a run folder if it passes every check: named as this crate names them, directly under
/// `root`, a real folder (not a link), owned by `uid` on unix. Only its known entries go; anything
/// else keeps it in place and is logged. Returns whether it is gone.
pub(crate) fn remove(root: &Path, dir: &Path, uid: Option<u32>) -> bool {
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else { return false };
    if !is_run_dir_name(name) || dir.parent() != Some(root) {
        log::warn!("not removing {}: it is not a run folder under {}", dir.display(), root.display());
        return false;
    }
    let meta = match std::fs::symlink_metadata(dir) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
        Err(e) => {
            log::warn!("not removing {}: {e}", dir.display());
            return false;
        }
    };
    if !meta.is_dir() || meta.file_type().is_symlink() || !owned_by(&meta, uid) {
        log::warn!("not removing {}: it is not a folder of this user", dir.display());
        return false;
    }
    for f in FILES {
        let p = dir.join(f);
        if let Ok(m) = std::fs::symlink_metadata(&p)
            && !m.is_dir()
        {
            let _ = std::fs::remove_file(&p);
        }
    }
    for f in FOLDERS {
        let p = dir.join(f);
        if let Ok(m) = std::fs::symlink_metadata(&p)
            && m.is_dir()
            && !m.file_type().is_symlink()
        {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
    match std::fs::remove_dir(dir) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("the run folder {} stays: {e}", dir.display());
            false
        }
    }
}

#[cfg(unix)]
fn private_mode(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    meta.mode() & 0o777 == 0o700
}

#[cfg(not(unix))]
fn private_mode(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn owned_by(meta: &std::fs::Metadata, uid: Option<u32>) -> bool {
    use std::os::unix::fs::MetadataExt;
    uid.is_some_and(|u| meta.uid() == u)
}

/// Windows: std cannot read a folder's owner; run folders sit in the user's own profile.
#[cfg(not(unix))]
fn owned_by(_meta: &std::fs::Metadata, _uid: Option<u32>) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_run_folder_of_ours_is_removed_and_only_its_known_entries() {
        assert!(is_run_dir_name("af-abc12345"));
        for bad in ["af-abc1234", "af-ABC12345", "xx-abc12345", "af-abc12345x", "af-abc_2345"] {
            assert!(!is_run_dir_name(bad), "{bad}");
        }
        let (root, uid) = checked_root(&crate::home::tests::scratch("runroot")).unwrap();
        let dir = create(&root, uid).unwrap();
        std::fs::write(dir.join("server.yaml"), "x").unwrap();
        std::fs::create_dir(dir.join("doltcfg")).unwrap();
        std::fs::write(dir.join("doltcfg/privileges.db"), "x").unwrap();
        assert!(remove(&root, &dir, uid));
        assert!(!dir.exists());
        // An unknown entry keeps the folder.
        let dir = create(&root, uid).unwrap();
        std::fs::write(dir.join("keep.txt"), "x").unwrap();
        assert!(!remove(&root, &dir, uid));
        assert!(dir.join("keep.txt").exists());
        // A wrongly named folder, or one outside the root, is never touched.
        let other = root.join("af-notours");
        std::fs::create_dir(&other).unwrap();
        assert!(!remove(&root, &other, uid));
        let nested = dir.join("af-abc12345");
        std::fs::create_dir(&nested).unwrap();
        assert!(!remove(&root, &nested, uid));
        #[cfg(unix)]
        {
            let target = crate::home::tests::scratch("linktarget");
            let link = root.join("af-link0000");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(!remove(&root, &link, uid));
            assert!(target.exists());
            std::fs::remove_dir_all(&target).unwrap();
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_root_others_can_write_to_must_be_sticky() {
        use std::os::unix::fs::PermissionsExt;
        let root = crate::home::tests::scratch("sharedroot");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(matches!(checked_root(&root), Err(Error::RunRootUnsafe { why, .. }) if why.contains("not sticky")));
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o1777)).unwrap();
        assert!(checked_root(&root).is_ok());
        assert!(checked_root(Path::new("/tmp")).is_ok(), "the system's /tmp is sticky");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }
}
