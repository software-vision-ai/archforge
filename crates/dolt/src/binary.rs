//! The dolt binary: found by absolute path only, and of a version Archforge is tested with.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::{Error, Home, paths, runner};

/// The variable that names the dolt binary, overriding the PATH.
pub const DOLT_VAR: &str = "ARCHFORGE_DOLT";
/// The versions accepted: 2.6.0 and later within major version 2 (tested with 2.6.1).
const ACCEPTED: &str = "2.6.0 or later within major version 2";
/// Where dolt's installers put it, looked at after the PATH. macOS starts an application from
/// Finder, the Dock or Launchpad with a PATH of the system folders alone, without the folders
/// Homebrew (`/opt/homebrew/bin`) and dolt's own installer (`/usr/local/bin`) install into.
const INSTALLED: &[&str] = if cfg!(target_os = "macos") { &["/opt/homebrew/bin", "/usr/local/bin"] } else { &[] };

/// A dolt binary of an accepted version, with the home it runs in.
#[derive(Debug, Clone)]
pub struct Dolt {
    path: PathBuf,
    version: (u32, u32, u32),
    home: PathBuf,
}

impl Dolt {
    /// The binary `ARCHFORGE_DOLT` names, else the first `dolt` in an absolute PATH entry, else
    /// the first in the folders dolt's installers use on this system.
    pub fn find(home: &Home) -> Result<Dolt, Error> {
        if let Some(p) = std::env::var_os(DOLT_VAR).filter(|v| !v.is_empty()) {
            return Dolt::at(Path::new(&p), home);
        }
        let name = if cfg!(windows) { "dolt.exe" } else { "dolt" };
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        let found = locate(&path_var, name, INSTALLED).map_err(|looked| Error::BinaryMissing { looked })?;
        Dolt::at(&found, home)
    }

    /// The binary at `path`, which must be absolute.
    pub fn at(path: &Path, home: &Home) -> Result<Dolt, Error> {
        if !path.is_absolute() {
            return Err(Error::BinaryNotAbsolute { path: path.to_path_buf() });
        }
        paths::text("dolt binary", path)?;
        if !path.is_file() {
            return Err(Error::BinaryMissing { looked: path.display().to_string() });
        }
        let out = runner::run(path, &[OsStr::new("version")], &env(home.path()), runner::HELPER_BUDGET)?;
        if !out.success {
            return Err(runner::failed(path, &out));
        }
        let version = parse_version(&out.stdout)?;
        accept(version)?;
        Ok(Dolt { path: path.to_path_buf(), version, home: home.path().to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The version, as `X.Y.Z`.
    pub fn version(&self) -> String {
        let (a, b, c) = self.version;
        format!("{a}.{b}.{c}")
    }

    /// The environment every dolt call runs in: the cleared base and its own home.
    pub(crate) fn env(&self) -> Vec<(OsString, OsString)> {
        env(&self.home)
    }
}

fn env(home: &Path) -> Vec<(OsString, OsString)> {
    let mut e = runner::base_env();
    e.push(("DOLT_ROOT_PATH".into(), home.as_os_str().to_owned()));
    // No detached event flush: it outlives the call that started it and writes into the home.
    e.push(("DOLT_DISABLE_EVENT_FLUSH".into(), "true".into()));
    e
}

/// The version from `dolt version`'s first line, `dolt version X.Y.Z`.
pub(crate) fn parse_version(out: &str) -> Result<(u32, u32, u32), Error> {
    let unreadable = || Error::VersionUnreadable { output: out.chars().take(200).collect() };
    let rest = out.lines().next().and_then(|l| l.trim().strip_prefix("dolt version ")).ok_or_else(unreadable)?;
    let mut parts = rest.trim().split('.').map(str::parse::<u32>);
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Ok(a)), Some(Ok(b)), Some(Ok(c)), None) => Ok((a, b, c)),
        _ => Err(unreadable()),
    }
}

/// The first `name` in an absolute entry of `path_var`, else the first in `installed`; when there
/// is none, every place looked at, for the message.
fn locate(path_var: &OsStr, name: &str, installed: &[&str]) -> Result<PathBuf, String> {
    if let Some(found) = runner::search(path_var, name) {
        return Ok(found);
    }
    if let Some(found) = installed.iter().map(|d| Path::new(d).join(name)).find(|p| p.is_file()) {
        return Ok(found);
    }
    let mut looked = format!("{DOLT_VAR} is not set and no {name} is on the PATH");
    if !installed.is_empty() {
        looked.push_str(&format!(" or in {}", installed.join(", ")));
    }
    Err(looked)
}

/// Refuses a version outside major 2 or below 2.6.0.
pub(crate) fn accept(v: (u32, u32, u32)) -> Result<(), Error> {
    if v.0 == 2 && v.1 >= 6 { Ok(()) } else { Err(Error::VersionRefused { found: format!("{}.{}.{}", v.0, v.1, v.2), accepted: ACCEPTED }) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_strictly_and_only_major_two_from_two_six_is_accepted() {
        assert_eq!(parse_version("dolt version 2.6.1\n").unwrap(), (2, 6, 1));
        assert_eq!(parse_version("dolt version 2.10.0\nWarning: a newer version exists\n").unwrap(), (2, 10, 0));
        for bad in ["", "dolt 2.6.1", "dolt version 2.6", "dolt version 2.6.1.4", "dolt version two"] {
            assert!(matches!(parse_version(bad), Err(Error::VersionUnreadable { .. })), "{bad}");
        }
        assert!(accept((2, 6, 0)).is_ok() && accept((2, 11, 3)).is_ok());
        for refused in [(2, 5, 9), (1, 99, 0), (3, 0, 0)] {
            assert!(matches!(accept(refused), Err(Error::VersionRefused { .. })), "{refused:?}");
        }
    }

    #[test]
    fn every_dolt_call_runs_in_its_own_home_with_no_detached_event_flush() {
        let e = env(Path::new("/h"));
        let has = |k: &str, v: &str| e.iter().any(|(a, b)| a == k && b == v);
        assert!(has("DOLT_ROOT_PATH", "/h") && has("DOLT_DISABLE_EVENT_FLUSH", "true"), "{e:?}");
    }

    #[test]
    fn dolt_is_looked_for_on_the_path_first_then_where_its_installers_put_it() {
        let base = crate::home::tests::scratch("locate");
        let (on_path, installed, other) = (base.join("on-path"), base.join("installed"), base.join("other"));
        for d in [&on_path, &installed, &other] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(installed.join("dolt"), "").unwrap();
        let installed_dirs = [installed.to_str().unwrap()];
        let path_var = std::env::join_paths([&other, &on_path]).unwrap();
        // Not on the PATH: the installer's folder.
        assert_eq!(locate(&path_var, "dolt", &installed_dirs), Ok(installed.join("dolt")));
        // On the PATH: that one, before the installer's.
        std::fs::write(on_path.join("dolt"), "").unwrap();
        assert_eq!(locate(&path_var, "dolt", &installed_dirs), Ok(on_path.join("dolt")));
        // Nowhere: the message names every place looked at.
        let looked = locate(&path_var, "dolt-none", &installed_dirs).unwrap_err();
        assert!(looked.contains("ARCHFORGE_DOLT is not set") && looked.contains("on the PATH") && looked.contains(installed_dirs[0]), "{looked}");
        assert!(!locate(&path_var, "dolt-none", &[]).unwrap_err().contains(" or in "));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_relative_dolt_path_is_refused() {
        let base = crate::home::tests::scratch("binary");
        let home = Home::prepare(&base.join("dolt")).unwrap();
        assert!(matches!(Dolt::at(Path::new("bin/dolt"), &home), Err(Error::BinaryNotAbsolute { .. })));
        assert!(matches!(Dolt::at(&base.join("missing"), &home), Err(Error::BinaryMissing { .. })));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
