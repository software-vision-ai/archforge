//! The engine's errors. Every variant names the value it is about and, where it helps, the values
//! that would have been accepted.

use std::fmt;

/// Why a call did not run, or failed; in every case nothing changed.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    UnknownCommand {
        id: String,
        close: Vec<String>,
        count: usize,
    },
    NotAnObject,
    TooLarge {
        bytes: usize,
        max: usize,
    },
    TooDeep {
        max: usize,
    },
    UnknownKey {
        key: String,
        accepted: Vec<String>,
    },
    MissingKey {
        key: String,
    },
    WrongType {
        key: String,
        expected: &'static str,
    },
    OutOfRange {
        key: String,
        range: String,
    },
    NotAChoice {
        key: String,
        choices: Vec<String>,
    },
    TooLong {
        key: String,
        max: usize,
    },
    Disabled {
        id: String,
        reason: &'static str,
    },
    Failed {
        id: String,
        error: CommandError,
    },
    /// A handler panicked; its message is kept for the log and the report.
    Internal {
        id: String,
        message: String,
    },
    /// Rolling a failed call back failed too, so the session's state can no longer be trusted:
    /// every edit is refused until the project is opened again.
    Poisoned {
        message: String,
    },
    /// The session is read-only for now ([`crate::Session::set_read_only`]); nothing was changed.
    ReadOnly,
    InteractionRunning,
    NoInteraction,
    /// The command cannot run inside a batch (undo and redo).
    NotInBatch {
        id: String,
    },
}

/// The most of a caller's own text an error repeats, in characters.
const ECHO: usize = 200;

/// A caller's text as an error repeats it: whole when short, else cut at a character boundary and
/// marked as cut.
pub(crate) fn echo(s: &str) -> String {
    if s.chars().count() <= ECHO {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(ECHO).collect();
        out.push_str("... (truncated)");
        out
    }
}

/// What a handler reports when it refuses: a message, the parameter it is about, and, for a reason
/// an interface says in its own words, that reason typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    /// The refusal as an English sentence, which the command line, the control channel and the log
    /// show.
    pub message: String,
    pub key: Option<String>,
    /// The refusal as a reason with its values, for an interface to translate; none for a reason
    /// not typed yet, which an interface shows as `message`.
    pub reason: Option<Reason>,
}

/// Why a handler refused, typed, so an interface can say it in the user's language while the
/// engine keeps no interface text. A kind is the element's kind as the notation names it in
/// English, a lower-case noun such as `"class"` or `"schema"`; an interface that doesn't know a kind
/// shows the message instead.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// Another element of the same kind where this one would be has the name already, whatever
    /// the case; `name` is that other element's name.
    NameTaken { kind: &'static str, name: String },
    /// An element of `kind` would depend on itself.
    DependsOnItself { kind: &'static str },
    /// A package would go into itself or a package it holds.
    PackageIntoItself,
    /// A name is empty.
    NameEmpty,
    /// A text that has to say something, as a note's, is empty.
    TextEmpty,
    /// A name or a text is longer than `max` characters.
    TooLong { max: usize },
    /// A name or a text holds a control character, or a name a hidden one.
    ControlCharacter,
    /// A name or a text starts or ends with a space.
    EdgeSpace,
}

impl CommandError {
    /// A refusal about no parameter in particular, its reason not typed.
    pub fn new(message: impl Into<String>) -> CommandError {
        CommandError { message: message.into(), key: None, reason: None }
    }

    /// A refusal about the parameter `key`, its reason not typed.
    pub fn about(key: &str, message: impl Into<String>) -> CommandError {
        CommandError { message: message.into(), key: Some(key.to_string()), reason: None }
    }

    /// The same refusal with its reason typed as `reason`; the message stays as it was.
    #[must_use]
    pub fn because(self, reason: Reason) -> CommandError {
        CommandError { reason: Some(reason), ..self }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.key {
            Some(k) => write!(f, "{k}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::UnknownCommand { id, close, count } if close.is_empty() => write!(f, "no command {id:?}; {count} commands are known"),
            EngineError::UnknownCommand { id, close, count } => write!(f, "no command {id:?}; close ones: {} (of {count})", close.join(", ")),
            EngineError::NotAnObject => f.write_str("the parameters must be a JSON object"),
            EngineError::TooLarge { bytes, max } => write!(f, "the parameters weigh over {bytes} bytes; the limit is {max}"),
            EngineError::TooDeep { max } => write!(f, "the parameters nest deeper than {max} levels"),
            EngineError::UnknownKey { key, accepted } if accepted.is_empty() => write!(f, "unknown parameter {key:?}; this command takes none"),
            EngineError::UnknownKey { key, accepted } => write!(f, "unknown parameter {key:?}; accepted: {}", accepted.join(", ")),
            EngineError::MissingKey { key } => write!(f, "the parameter {key:?} is required"),
            EngineError::WrongType { key, expected } => write!(f, "the parameter {key:?} must be of type {expected}"),
            EngineError::OutOfRange { key, range } => write!(f, "the parameter {key:?} must be within {range}"),
            EngineError::NotAChoice { key, choices } => write!(f, "the parameter {key:?} must be one of {}", choices.join(", ")),
            EngineError::TooLong { key, max } => write!(f, "the parameter {key:?} is longer than {max} bytes"),
            EngineError::Disabled { id, reason } => write!(f, "{id} is not available: {reason}"),
            EngineError::Failed { id, error } => write!(f, "{id} failed: {error}"),
            EngineError::Internal { id, message } => write!(f, "{id} stopped on an internal error ({message}); nothing was changed"),
            EngineError::Poisoned { message } => {
                write!(f, "the session's state could not be restored after a failure ({message}); reopen the project")
            }
            EngineError::ReadOnly => f.write_str("the model cannot be changed at the moment; nothing was changed"),
            EngineError::InteractionRunning => f.write_str("a gesture is in progress; retry after the gesture"),
            EngineError::NoInteraction => f.write_str("no gesture is in progress"),
            EngineError::NotInBatch { id } => write!(f, "{id} cannot run inside a batch"),
        }
    }
}

impl std::error::Error for EngineError {}
