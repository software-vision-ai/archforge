//! Archforge's own dolt home: the folder dolt keeps its global configuration and usage records in
//! (`DOLT_ROOT_PATH`). It is private to this user, and usage metrics and the online version check
//! are off before dolt ever runs. The user's own `~/.dolt` is never read or written.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{Error, paths};

/// The global settings Archforge requires of dolt.
const REQUIRED: &[(&str, &str)] = &[("metrics.disabled", "true"), ("versioncheck.disabled", "true")];
/// The most of dolt's global configuration file read.
const CONFIG_CAP: u64 = 64 * 1024;

/// A dolt home that passed its checks.
#[derive(Debug, Clone)]
pub struct Home {
    path: PathBuf,
}

impl Home {
    /// Creates the home if it is missing, checks that it is a private folder of this user, and
    /// writes the required global settings.
    pub fn prepare(path: &Path) -> Result<Home, Error> {
        paths::text("dolt home", path)?;
        private_folder(path).map_err(|why| Error::HomeUnsafe { path: path.to_path_buf(), why })?;
        let dot = path.join(".dolt");
        private_folder(&dot).map_err(|why| Error::HomeUnsafe { path: dot.clone(), why })?;
        let file = dot.join("config_global.json");
        if let Ok(m) = std::fs::symlink_metadata(&file)
            && !m.is_file()
        {
            return Err(Error::HomeUnsafe { path: file, why: "its configuration is not a plain file".into() });
        }
        let mut config = match std::fs::File::open(&file) {
            Ok(f) => {
                let mut bytes = Vec::new();
                f.take(CONFIG_CAP + 1).read_to_end(&mut bytes).map_err(|e| Error::io("cannot read", &file, e))?;
                if bytes.len() as u64 > CONFIG_CAP {
                    return Err(Error::HomeUnsafe { path: file, why: format!("its configuration is over {CONFIG_CAP} bytes") });
                }
                match serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes) {
                    Ok(map) if map.values().all(serde_json::Value::is_string) => map,
                    _ => return Err(Error::HomeUnsafe { path: file, why: "its configuration is not a JSON object of strings".into() }),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::Map::new(),
            Err(e) => return Err(Error::io("cannot read", &file, e)),
        };
        let wanted = REQUIRED.iter().all(|(k, v)| config.get(*k).and_then(|x| x.as_str()) == Some(*v));
        if !wanted {
            for (k, v) in REQUIRED {
                config.insert((*k).to_string(), serde_json::Value::String((*v).to_string()));
            }
            let bytes = serde_json::to_vec(&config).map_err(|e| Error::io("cannot write", &file, std::io::Error::other(e)))?;
            craft_atomic::atomic_write(&file, &bytes).map_err(|e| Error::io("cannot write", &file, e))?;
        }
        restrict_file(&file)?;
        Ok(Home { path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Makes `path` a folder only this user can use, or says why it is not one.
pub(crate) fn private_folder(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => create_private(path).map_err(|e| format!("it cannot be created: {e}"))?,
        Err(e) => return Err(format!("it cannot be read: {e}")),
        Ok(_) => {}
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("it cannot be read: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("it is a link".into());
    }
    if !meta.is_dir() {
        return Err("it is not a folder".into());
    }
    check_owner_and_mode(path, &meta)
}

#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(unix)]
fn check_owner_and_mode(path: &Path, meta: &std::fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    if meta.mode() & 0o077 != 0 {
        return Err(format!("group or others can use it (mode {:o})", meta.mode() & 0o777));
    }
    let me = current_uid(path).map_err(|e| format!("its owner cannot be checked: {e}"))?;
    if meta.uid() != me {
        return Err("another user owns it".into());
    }
    Ok(())
}

/// Windows: std cannot read a folder's owner; the home sits in the user's own profile.
#[cfg(not(unix))]
fn check_owner_and_mode(_path: &Path, _meta: &std::fs::Metadata) -> Result<(), String> {
    Ok(())
}

/// This user's id, read from a folder made for the purpose inside `dir` and removed again.
#[cfg(unix)]
pub(crate) fn current_uid(dir: &Path) -> std::io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    let probe = dir.join(format!(".af-probe-{}", crate::secret::random_name(8).map_err(|e| std::io::Error::other(e.to_string()))?));
    std::fs::create_dir(&probe)?;
    let uid = std::fs::metadata(&probe).map(|m| m.uid());
    let _ = std::fs::remove_dir(&probe);
    uid
}

#[cfg(unix)]
fn restrict_file(file: &Path) -> Result<(), Error> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600)).map_err(|e| Error::io("cannot restrict", file, e))
}

#[cfg(not(unix))]
fn restrict_file(_file: &Path) -> Result<(), Error> {
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh folder for one test, removed by the caller.
    pub fn scratch(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("af-test-{name}-{}-{}", std::process::id(), crate::secret::random_name(6).unwrap()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_home_is_made_private_and_carries_the_required_settings() {
        let base = scratch("home");
        let home = Home::prepare(&base.join("dolt")).unwrap();
        let text = std::fs::read_to_string(home.path().join(".dolt/config_global.json")).unwrap();
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(map["metrics.disabled"], "true");
        assert_eq!(map["versioncheck.disabled"], "true");
        // Settings of its own survive; a damaged file is refused, not overwritten.
        std::fs::write(home.path().join(".dolt/config_global.json"), r#"{"user.name":"A"}"#).unwrap();
        Home::prepare(home.path()).unwrap();
        let text = std::fs::read_to_string(home.path().join(".dolt/config_global.json")).unwrap();
        assert!(text.contains("user.name") && text.contains("metrics.disabled"), "{text}");
        std::fs::write(home.path().join(".dolt/config_global.json"), "{ damaged").unwrap();
        assert!(matches!(Home::prepare(home.path()), Err(Error::HomeUnsafe { .. })));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_home_others_can_use_or_a_link_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let base = scratch("home-unsafe");
        let path = base.join("dolt");
        Home::prepare(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o770)).unwrap();
        assert!(matches!(Home::prepare(&path), Err(Error::HomeUnsafe { why, .. }) if why.contains("group or others")));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&path, base.join("link")).unwrap();
        assert!(matches!(Home::prepare(&base.join("link")), Err(Error::HomeUnsafe { why, .. }) if why.contains("link")));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
