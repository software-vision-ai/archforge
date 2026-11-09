//! The crate's one error type. Every variant names the value it is about.

use std::fmt;
use std::path::PathBuf;

use archforge_names::Problem;

#[derive(Debug)]
pub enum Error {
    /// A rename of the branch in use may or may not have happened, so nothing is read from or
    /// written to either name until the rename is settled or the project closes.
    BranchUnclear {
        path: PathBuf,
    },
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
    /// The project's model tables were made for other metamodels than this Archforge's.
    OtherModel {
        path: PathBuf,
        found: String,
        expected: String,
    },
    Damaged {
        path: PathBuf,
        why: String,
    },
    /// What was asked for as a version is no commit on the project's line of work: not the shape
    /// of a commit's hash, one Dolt does not know, or a commit of another branch.
    NotAVersion {
        commit: String,
    },
    /// A version is a commit of the project, but its model tables are not ones this Archforge can
    /// read: another model's or a later Archforge's, or none it can recognise; `why` says which.
    VersionShape {
        commit: String,
        why: String,
    },
    /// The project's database holds triggers, stored procedures or events.
    Scripted {
        path: PathBuf,
        what: &'static str,
    },
    /// A conversation's value over its limit, refused rather than cut: a question longer than may
    /// be sent, a title longer than may be given.
    TooLarge {
        what: &'static str,
        size: usize,
        max: usize,
        unit: &'static str,
    },
    /// The project holds as many conversations, or the conversation as many messages, as it may.
    Full {
        what: &'static str,
        max: usize,
    },
    /// A question holds one of the API keys Archforge keeps, named here: it is neither sent nor
    /// stored.
    HeldKey {
        name: String,
    },
    /// The API keys could not be read to check a question against them, so it is not stored.
    KeyCheck(archforge_secrets::SecretError),
    /// A conversation's value that cannot be stored as given.
    ChatValue {
        what: &'static str,
        why: String,
    },
    /// No conversation, turn or change link with that id is in the project.
    ChatMissing {
        what: &'static str,
        id: String,
    },
    /// The project's conversations cannot be read or written: a chat table of another shape, or a
    /// row that does not read. The model is not affected.
    ChatsUnreadable {
        path: PathBuf,
        why: String,
    },
    /// One conversation cannot be read: a row that does not read, an orphan, a broken order.
    ChatUnreadable {
        id: String,
        why: String,
    },
    /// A conversation is not deleted: what waited could not be committed first, or the deletion's
    /// own commit failed; nothing of it was deleted.
    ChatNotDeleted {
        id: String,
        why: String,
    },
    /// A deleted conversation is not brought back: it is listed already, the commit given is not one
    /// of the project's that holds it, its label is taken, or the rows could not be written back;
    /// nothing was written.
    ChatNotRestored {
        id: String,
        why: String,
    },
    /// A deleted conversation is not brought back because it is in the list already.
    ChatListed {
        id: String,
    },
    /// Don't save is refused: the conversations could not be committed first, and the reset would
    /// lose them.
    ChatsNotKept {
        path: PathBuf,
        why: String,
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
            Error::OtherModel { path, found, expected } => write!(
                f,
                "the project {} holds model tables {found} where this Archforge has {expected}: it was made by another version",
                path.display()
            ),
            Error::Damaged { path, why } => write!(f, "the project {} is damaged: {why}", path.display()),
            Error::NotAVersion { commit } => write!(f, "{commit:?} is not a version of this project"),
            Error::VersionShape { commit, why } => write!(f, "the version {commit} cannot be read by this Archforge: {why}"),
            Error::BranchUnclear { path } => write!(f, "whether a branch of {} was renamed cannot be told", path.display()),
            Error::Scripted { path, what } => write!(f, "the project {} holds {what}, which Archforge does not run", path.display()),
            Error::TooLarge { what, size, max, unit } => write!(f, "{what} of {size} {unit} is longer than the {max} {unit} allowed"),
            Error::Full { what, max } => write!(f, "{what} already holds {max}, the most it may"),
            Error::HeldKey { name } => write!(f, "the message holds the API key kept as {name}; it is neither sent nor stored"),
            Error::KeyCheck(e) => write!(f, "the message cannot be checked against the API keys Archforge keeps: {e}"),
            Error::ChatValue { what, why } => write!(f, "{what} cannot be stored: {why}"),
            Error::ChatMissing { what, id } => write!(f, "no {what} {id} is in the project"),
            Error::ChatsUnreadable { path, why } => write!(f, "the conversations in the project {} cannot be read: {why}", path.display()),
            Error::ChatUnreadable { id, why } => write!(f, "the conversation {id} cannot be read: {why}"),
            Error::ChatNotDeleted { id, why } => write!(f, "the conversation {id} is not deleted: {why}"),
            Error::ChatListed { id } => write!(f, "the conversation {id} cannot be brought back: it is in the conversation list already"),
            Error::ChatNotRestored { id, why } => write!(f, "the conversation {id} cannot be brought back: {why}"),
            Error::ChatsNotKept { path, why } => {
                write!(f, "the conversations in the project {} could not be kept, so it is not put back to its last save: {why}", path.display())
            }

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
            Error::KeyCheck(e) => Some(e),
            Error::Sql { source, .. } => Some(source),
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
