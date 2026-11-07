//! Every failure of this crate, and the neutral categories the interface shows for them.

use std::fmt;
use std::path::PathBuf;

use crate::{RuntimeKind, Version};

/// The longest detail an error keeps from elsewhere (a system message, a runtime's error stream),
/// in bytes.
pub const MAX_DETAIL_BYTES: usize = 4 * 1024;

/// The neutral category of a failure, the same for every runtime, which the interface turns into
/// its message and its next step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The runtime is not installed, too old, of an unsupported version, or cannot be started.
    MissingRuntime,
    /// The key was refused, or no key reached the runtime.
    InvalidCredential,
    /// A sign-in that was valid has expired.
    SignInExpired,
    /// The provider's rate limit or usage limit was reached.
    RateLimited,
    /// The provider could not be reached.
    Network,
    /// The model asked for does not exist, or is not available now.
    ModelUnavailable,
    /// Anything else.
    Other,
}

/// Why something failed. No variant carries a secret: keys are never placed in an error, and a
/// runtime's error stream is scrubbed of them before an error keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    /// Agent runtimes are not available on this system yet (macOS and Linux come first).
    NotAvailable,
    /// The runtime was not found; `looked` says where it was looked for.
    Missing { runtime: RuntimeKind, looked: String },
    /// The runtime found is older than the oldest version accepted.
    TooOld { runtime: RuntimeKind, path: PathBuf, found: Version, floor: Version },
    /// The runtime found cannot be used: `reason` says why (a version past the range, an unreadable
    /// version, a probe that failed).
    Unsupported { runtime: RuntimeKind, path: PathBuf, reason: String },
    /// An environment variable name was refused. The name itself is kept only when it is a
    /// well-formed name (a reserved one); otherwise only its length, as it may be a key pasted into
    /// the wrong place.
    InvalidVariable { name: Option<String>, bytes: usize, reason: String },
    /// A secret's value appears in the command line or in a plain variable, where anyone could see
    /// it. `variable` names the secret variable, never its value.
    SecretExposed { variable: String, place: &'static str },
    /// A limit or duration given by the caller is outside what is accepted.
    InvalidLimit { limit: &'static str, value: u128, accepted: String },
    /// A conversation id was refused; only its length is kept.
    InvalidConversationId { bytes: usize, reason: String },
    /// A private folder could not be made, checked, or removed.
    Folder { path: PathBuf, problem: String },
    /// The runtime could not be started.
    Spawn { runtime: RuntimeKind, path: PathBuf, detail: String, not_found: bool },
    /// The runtime ended with an exit code (`None` when the system gave neither a code nor a signal).
    Exited { runtime: RuntimeKind, code: Option<i32>, stderr_tail: String },
    /// The runtime was ended by a signal.
    Crashed { runtime: RuntimeKind, signal: i32, stderr_tail: String },
    /// The runtime has already ended, so it cannot be signalled.
    NotRunning { runtime: RuntimeKind },
    /// The runtime's input is closed.
    InputClosed { runtime: RuntimeKind },
    /// A line for the runtime's input was refused: `reason` says why.
    InvalidLine { runtime: RuntimeKind, bytes: usize, reason: &'static str },
    /// The runtime's input queue is full: the runtime is not reading its input.
    InputBacklog { runtime: RuntimeKind },
    /// Talking to the runtime's process failed: `action` says what was being done.
    Io { runtime: RuntimeKind, action: &'static str, detail: String },
    /// A setting an adapter was given was refused: `setting` names it, `bytes` is its length; the
    /// value itself is never repeated, as it may be a key pasted into the wrong field.
    InvalidSetting { setting: &'static str, bytes: usize, reason: String },
    /// A message was sent while a turn is still running; stop it or wait for its end.
    TurnRunning { runtime: RuntimeKind },
    /// A failure an adapter read from the runtime's own output, already in its category.
    Reported { kind: ErrorKind, detail: String },
}

impl AgentError {
    /// The neutral category of this failure.
    pub fn kind(&self) -> ErrorKind {
        match self {
            AgentError::NotAvailable | AgentError::Missing { .. } | AgentError::TooOld { .. } | AgentError::Unsupported { .. } => {
                ErrorKind::MissingRuntime
            }
            AgentError::Spawn { not_found: true, .. } => ErrorKind::MissingRuntime,
            AgentError::Reported { kind, .. } => *kind,
            AgentError::InvalidVariable { .. }
            | AgentError::SecretExposed { .. }
            | AgentError::InvalidLimit { .. }
            | AgentError::InvalidConversationId { .. }
            | AgentError::Folder { .. }
            | AgentError::Spawn { not_found: false, .. }
            | AgentError::Exited { .. }
            | AgentError::Crashed { .. }
            | AgentError::NotRunning { .. }
            | AgentError::InputClosed { .. }
            | AgentError::InvalidLine { .. }
            | AgentError::InputBacklog { .. }
            | AgentError::InvalidSetting { .. }
            | AgentError::TurnRunning { .. }
            | AgentError::Io { .. } => ErrorKind::Other,
        }
    }

    /// A failure an adapter read from the runtime's output, with `detail` cut to
    /// [`MAX_DETAIL_BYTES`]. The adapter gives a detail that holds no secret.
    pub fn reported(kind: ErrorKind, detail: &str) -> AgentError {
        AgentError::Reported { kind, detail: capped(detail.to_owned()) }
    }
}

impl fmt::Display for AgentError {
    /// Writes one sentence naming what failed and why, built only from fields that hold no secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::NotAvailable => f.write_str("agent runtimes are not available on this system yet"),
            AgentError::Missing { runtime, looked } => write!(f, "{runtime} was not found: {looked}"),
            AgentError::TooOld { runtime, path, found, floor } => {
                write!(f, "{runtime} at {} is version {found}, older than {floor}, the oldest Archforge works with; update it", path.display())
            }
            AgentError::Unsupported { runtime, path, reason } => write!(f, "{runtime} at {} cannot be used: {reason}", path.display()),
            AgentError::InvalidVariable { name: Some(name), reason, .. } => write!(f, "the environment variable {name} was refused: {reason}"),
            AgentError::InvalidVariable { name: None, bytes, reason } => {
                write!(f, "an environment variable name of {bytes} bytes was refused: {reason}")
            }
            AgentError::SecretExposed { variable, place } => {
                write!(
                    f,
                    "the value of the secret variable {variable} appears in the {place}; a secret reaches a runtime only through its own variable"
                )
            }
            AgentError::InvalidLimit { limit, value, accepted } => write!(f, "{limit} of {value} is refused; accepted: {accepted}"),
            AgentError::InvalidConversationId { bytes, reason } => {
                write!(
                    f,
                    "a conversation id of {bytes} bytes was refused because {reason}; an id is 1 to {} lowercase letters, digits and hyphens, starting with a letter or digit",
                    crate::MAX_CONVERSATION_ID_BYTES
                )
            }
            AgentError::Folder { path, problem } => write!(f, "the private folder {} {problem}", path.display()),
            AgentError::Spawn { runtime, path, detail, .. } => write!(f, "{runtime} at {} could not be started: {detail}", path.display()),
            AgentError::Exited { runtime, code: Some(code), stderr_tail } => write!(f, "{runtime} ended with exit code {code}{}", tail(stderr_tail)),
            AgentError::Exited { runtime, code: None, stderr_tail } => write!(f, "{runtime} ended with an unknown status{}", tail(stderr_tail)),
            AgentError::Crashed { runtime, signal, stderr_tail } => write!(f, "{runtime} was ended by signal {signal}{}", tail(stderr_tail)),
            AgentError::NotRunning { runtime } => write!(f, "{runtime} has already ended"),
            AgentError::InputClosed { runtime } => write!(f, "the input of {runtime} is closed"),
            AgentError::InvalidLine { runtime, bytes, reason } => write!(f, "a line of {bytes} bytes for {runtime} was refused: {reason}"),
            AgentError::InputBacklog { runtime } => write!(f, "{runtime} is not reading its input; the line was not queued"),
            AgentError::Io { runtime, action, detail } => write!(f, "could not {action} {runtime}: {detail}"),
            AgentError::InvalidSetting { setting, bytes, reason } => write!(f, "the {setting} of {bytes} bytes was refused: {reason}"),
            AgentError::TurnRunning { runtime } => write!(f, "{runtime} is still answering; stop the turn or wait for its end before sending"),
            AgentError::Reported { detail, .. } => f.write_str(detail),
        }
    }
}

impl std::error::Error for AgentError {}

/// The error stream's tail as the end of a message: nothing when it is empty.
fn tail(text: &str) -> String {
    if text.trim().is_empty() { String::new() } else { format!("; its last output: {}", text.trim()) }
}

/// `text` cut to [`MAX_DETAIL_BYTES`] at a character boundary, with an ellipsis when cut.
pub(crate) fn capped(text: String) -> String {
    capped_to(text, MAX_DETAIL_BYTES)
}

/// `text` cut to at most `max` bytes at a character boundary, with an ellipsis after the cut.
pub(crate) fn capped_to(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        text.truncate(end);
        text.push('…');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The process-level failures fall into the categories the interface shows.
    #[test]
    fn process_failures_map_to_neutral_categories() {
        let path = PathBuf::from("/x/claude");
        let runtime = RuntimeKind::Claude;
        assert_eq!(AgentError::NotAvailable.kind(), ErrorKind::MissingRuntime);
        assert_eq!(AgentError::Missing { runtime, looked: "nowhere".into() }.kind(), ErrorKind::MissingRuntime);
        let v = Version::new(1, 0, 0);
        assert_eq!(AgentError::TooOld { runtime, path: path.clone(), found: v, floor: v }.kind(), ErrorKind::MissingRuntime);
        assert_eq!(AgentError::Unsupported { runtime, path: path.clone(), reason: "x".into() }.kind(), ErrorKind::MissingRuntime);
        assert_eq!(AgentError::Spawn { runtime, path: path.clone(), detail: "gone".into(), not_found: true }.kind(), ErrorKind::MissingRuntime);
        assert_eq!(AgentError::Spawn { runtime, path, detail: "busy".into(), not_found: false }.kind(), ErrorKind::Other);
        assert_eq!(AgentError::Exited { runtime, code: Some(1), stderr_tail: String::new() }.kind(), ErrorKind::Other);
        assert_eq!(AgentError::Crashed { runtime, signal: 9, stderr_tail: String::new() }.kind(), ErrorKind::Other);
        assert_eq!(AgentError::reported(ErrorKind::RateLimited, "slow down").kind(), ErrorKind::RateLimited);
    }

    /// A long detail is cut at a character boundary.
    #[test]
    fn a_reported_detail_is_capped() {
        let long = "é".repeat(MAX_DETAIL_BYTES);
        let AgentError::Reported { detail, .. } = AgentError::reported(ErrorKind::Other, &long) else { panic!("not reported") };
        assert!(detail.len() <= MAX_DETAIL_BYTES + '…'.len_utf8() && detail.ends_with('…'));
    }
}
