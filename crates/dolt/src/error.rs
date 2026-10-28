//! The crate's one error type. Every variant names the value it is about; none ever carries the
//! server's password.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub enum Error {
    /// A path that is not UTF-8 or holds a control character.
    BadPath {
        what: &'static str,
        path: String,
    },
    /// Archforge's dolt home is not a private folder of this user.
    HomeUnsafe {
        path: PathBuf,
        why: String,
    },
    /// No dolt binary where it was looked for.
    BinaryMissing {
        looked: String,
    },
    /// `ARCHFORGE_DOLT`, or a path given, is not absolute.
    BinaryNotAbsolute {
        path: PathBuf,
    },
    /// `dolt version` printed something else than `dolt version X.Y.Z`.
    VersionUnreadable {
        output: String,
    },
    /// A dolt version outside the accepted range.
    VersionRefused {
        found: String,
        accepted: &'static str,
    },
    /// The project folder is missing or not a folder.
    NotAFolder {
        path: PathBuf,
    },
    /// Another server holds the project: another Archforge window, or another program.
    OpenElsewhere {
        pid: u32,
    },
    /// The project's lock file cannot be read as dolt writes it.
    DamagedLock {
        path: PathBuf,
    },
    /// The server's socket path is longer than the platform allows.
    SocketPathTooLong {
        path: PathBuf,
        len: usize,
        max: usize,
    },
    /// The server records folder is not a private folder of this user.
    RecordsUnsafe {
        path: PathBuf,
        why: String,
    },
    /// The folder for run folders is not usable.
    RunRootUnsafe {
        path: PathBuf,
        why: String,
    },
    /// The server exited before it was ready.
    StartFailed {
        status: String,
        log_tail: String,
    },
    /// The server did not become ready within its budget.
    NotReady {
        waited: Duration,
        log_tail: String,
    },
    /// The server runs but did not set up its socket.
    SocketMissing {
        path: PathBuf,
    },
    /// A helper program (dolt, ps, kill, taskkill, powershell) failed.
    HelperFailed {
        helper: String,
        status: String,
        output: String,
    },
    /// A helper program did not finish within its budget.
    HelperTimedOut {
        helper: String,
        waited: Duration,
    },
    /// A server that had to be stopped is still running.
    StillRunning {
        pid: u32,
        waited: Duration,
    },
    Io {
        what: String,
        path: PathBuf,
        source: std::io::Error,
    },
    Sql {
        what: &'static str,
        source: mysql::Error,
    },
}

impl Error {
    pub(crate) fn io(what: impl Into<String>, path: impl Into<PathBuf>, source: std::io::Error) -> Error {
        Error::Io { what: what.into(), path: path.into(), source }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadPath { what, path } => write!(f, "the {what} {path:?} is not plain text: it must be UTF-8 without control characters"),
            Error::HomeUnsafe { path, why } => write!(f, "the dolt home {} is not private: {why}", path.display()),
            Error::BinaryMissing { looked } => {
                write!(f, "no dolt found ({looked}); install dolt 2.6 or later within major version 2, or set ARCHFORGE_DOLT to its absolute path")
            }
            Error::BinaryNotAbsolute { path } => write!(f, "the dolt path {} is not absolute", path.display()),
            Error::VersionUnreadable { output } => write!(f, "dolt did not say its version as `dolt version X.Y.Z`: {output:?}"),
            Error::VersionRefused { found, accepted } => write!(f, "dolt {found} is not supported; accepted: {accepted}"),
            Error::NotAFolder { path } => write!(f, "{} is not a folder", path.display()),
            Error::OpenElsewhere { pid } => write!(f, "the project is open in another window or program (process {pid})"),
            Error::DamagedLock { path } => {
                write!(f, "the project's lock file {} is damaged; remove it if no other program has the project open", path.display())
            }
            Error::SocketPathTooLong { path, len, max } => {
                write!(f, "the socket path {} is {len} bytes long; the limit here is {max}", path.display())
            }
            Error::RecordsUnsafe { path, why } => write!(f, "the server records folder {} is not private: {why}", path.display()),
            Error::RunRootUnsafe { path, why } => write!(f, "the run folder location {} cannot be used: {why}", path.display()),
            Error::StartFailed { status, log_tail } => write!(f, "the database server stopped while starting ({status}); its log ends: {log_tail}"),
            Error::NotReady { waited, log_tail } => write!(f, "the database server was not ready after {waited:?}; its log ends: {log_tail}"),
            Error::SocketMissing { path } => write!(f, "the database server runs but did not set up its socket {}", path.display()),
            Error::HelperFailed { helper, status, output } => write!(f, "{helper} failed ({status}): {output}"),
            Error::HelperTimedOut { helper, waited } => write!(f, "{helper} did not finish within {waited:?}"),
            Error::StillRunning { pid, waited } => write!(f, "the database server (process {pid}) still runs after {waited:?}"),
            Error::Io { what, path, source } => write!(f, "{what} {}: {source}", path.display()),
            Error::Sql { what, source } => write!(f, "{what}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::Sql { source, .. } => Some(source),
            _ => None,
        }
    }
}
