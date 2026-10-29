//! The crate's one error type. Every variant names the value it is about.

use std::fmt;
use std::path::PathBuf;

use archforge_names::Problem;

#[derive(Debug)]
pub enum Error {
    /// A name that cannot be a file name.
    BadName {
        name: String,
        problem: Problem,
    },
    /// Something of that name is already in the folder.
    NameTaken {
        path: PathBuf,
    },
    /// A project whose creation did not finish: it holds the marker, or nothing.
    Unfinished {
        path: PathBuf,
    },
    /// Creating was cancelled; nothing is left.
    Cancelled,
    /// The undo found the new folder changed and left it in place.
    Kept {
        path: PathBuf,
        cause: String,
    },
    /// The folder for new projects cannot be made or used.
    FolderUnusable {
        path: PathBuf,
        why: String,
    },
    NotAProject {
        path: PathBuf,
        why: String,
    },
    /// Others may write to the project folder, so they could plant server settings in it.
    NotPrivate {
        path: PathBuf,
        mode: u32,
    },
    /// Another user owns the project folder.
    NotYours {
        path: PathBuf,
    },
    /// The project folder's own `.dolt` holds more than Archforge's server leaves there.
    Foreign {
        path: PathBuf,
        entry: String,
    },
    NewerFormat {
        found: u32,
        supported: u32,
    },
    Damaged {
        path: PathBuf,
        why: String,
    },
    /// The project's database holds triggers, stored procedures or events.
    Scripted {
        path: PathBuf,
        what: &'static str,
    },

    Server(archforge_dolt::Error),
    Sql {
        what: &'static str,
        source: mysql::Error,
    },
    Io {
        what: String,
        path: PathBuf,
        source: std::io::Error,
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
            Error::BadName { name, problem } => write!(f, "{name:?} cannot name a project file: {problem:?}"),
            Error::NameTaken { path } => write!(f, "{} is already there", path.display()),
            Error::Unfinished { path } => write!(f, "the project {} was not finished; the folder can be deleted", path.display()),
            Error::Cancelled => write!(f, "creating the project was cancelled"),
            Error::Kept { path, cause } => write!(f, "{cause}; {} changed meanwhile and was left in place", path.display()),
            Error::FolderUnusable { path, why } => write!(f, "the folder {} cannot hold projects: {why}", path.display()),
            Error::NotAProject { path, why } => write!(f, "{} is not an Archforge project: {why}", path.display()),
            Error::NotPrivate { path, mode } => write!(
                f,
                "others may change the project folder {} (mode {mode:o}); this happens on drives that keep no permissions, such as FAT, exFAT or network drives",
                path.display()
            ),
            Error::NotYours { path } => write!(f, "the project folder {} belongs to another user", path.display()),
            Error::Foreign { path, entry } => write!(f, "the project {} carries server settings ({entry}) Archforge does not use", path.display()),
            Error::NewerFormat { found, supported } => write!(f, "the project is in format {found}; this Archforge reads up to format {supported}"),
            Error::Damaged { path, why } => write!(f, "the project {} is damaged: {why}", path.display()),
            Error::Scripted { path, what } => write!(f, "the project {} holds {what}, which Archforge does not run", path.display()),

            Error::Server(e) => write!(f, "{e}"),
            Error::Sql { what, source } => write!(f, "{what}: {source}"),
            Error::Io { what, path, source } => write!(f, "{what} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Server(e) => Some(e),
            Error::Sql { source, .. } => Some(source),
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
