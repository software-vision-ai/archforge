//! The neutral face of a conversation with an agent runtime: one trait every adapter implements
//! (Claude here, Codex in AR-202) and the events the Chat panel reads, the same for every runtime.

use std::time::Duration;

use crate::{AgentError, Exit};

/// A conversation with one running agent runtime, driven turn by turn.
///
/// A turn starts with [`send`](AgentSession::send) and ends with exactly one
/// [`SessionEvent::TurnFinished`]; between the two come text, tool calls and retries. Only one turn
/// runs at a time. Every method returns at once except [`next_event`](AgentSession::next_event),
/// which waits at most its timeout, and [`shutdown`](AgentSession::shutdown), which waits up to
/// twice its grace period, so both belong on a worker thread, never the interface thread.
pub trait AgentSession {
    /// Starts a turn with the person's message. Refused while a turn runs
    /// ([`AgentError::TurnRunning`]) and once the runtime has ended.
    fn send(&mut self, text: &str) -> Result<(), AgentError>;

    /// Asks the runtime to stop the running turn; it stays up for the next one. The turn still ends
    /// with [`SessionEvent::TurnFinished`], normally [`TurnOutcome::Stopped`]. Nothing happens when
    /// no turn runs.
    fn stop(&mut self) -> Result<(), AgentError>;

    /// The next event, waiting at most `timeout` (when the runtime's output closes, a short wait
    /// for its exit status may come on top); `None` when nothing came in time, and for every call
    /// after [`SessionEvent::Ended`].
    fn next_event(&mut self, timeout: Duration) -> Option<SessionEvent>;

    /// Ends the runtime: its input is closed so it can finish, then it is terminated, then killed,
    /// each step given at most `grace`. Answers how it ended.
    fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError>;
}

/// What happened in a conversation, in the order it happened. Every text an event carries came from
/// the runtime: ids, names, tool input and summaries are capped, the answer's text only by the line
/// cap. A key the runtime writes whole in a line is replaced before the line is read; a key split
/// across two text deltas cannot be caught in either, only in the message they make up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// The runtime is ready for the turn: the model it resolved, the tools the model may call, and
    /// the state of each tool server. It may come again on a later turn.
    Started { session_id: String, model: String, tools: Vec<String>, servers: Vec<ToolServer> },
    /// A piece of the answer being written, to show as it arrives.
    TextDelta { text: String },
    /// One message of the answer is complete; `text` is the whole message and replaces the deltas
    /// shown since the last message.
    MessageDone { text: String },
    /// The model called a tool. `server` is the tool server's name (`archforge`) when the runtime
    /// says it; `input` is the call's arguments as JSON text, capped.
    ToolStarted { id: String, server: Option<String>, tool: String, input: String },
    /// A tool call ended. `ok` is false when the tool, or the permission check before it, refused
    /// it; `summary` is the start of what it answered.
    ToolFinished { id: String, tool: Option<String>, ok: bool, summary: String },
    /// The provider could not be reached or refused for now, and the runtime tries again: progress,
    /// not a failure. `kind` is why, in the neutral categories.
    Retrying { attempt: u32, max_attempts: Option<u32>, delay_ms: Option<u64>, kind: crate::ErrorKind },
    /// The turn ended. `usage` counts tokens; it never holds a cost.
    TurnFinished { outcome: TurnOutcome, usage: Usage },
    /// A line of output that was not understood and was skipped; it is also counted.
    Ignored(Skipped),
    /// The runtime ended. `error` says why when the end was not asked for with `shutdown`.
    Ended { exit: Option<Exit>, error: Option<AgentError> },
}

/// How a turn ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnOutcome {
    /// The answer is complete.
    Completed,
    /// The turn was stopped on request.
    Stopped,
    /// The turn failed; the error's [`kind`](AgentError::kind) is the neutral category to show.
    Failed(AgentError),
}

/// The tokens a turn used, as the runtime counted them. Missing counts are zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

/// A tool server the runtime started and its state as the runtime reports it (`connected`,
/// `pending`, `failed`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolServer {
    pub name: String,
    pub status: String,
}

/// Why a line of output was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skipped {
    /// An empty line.
    Empty,
    /// Not JSON, or nested past the parser's depth limit; only its length is kept.
    NotJson { bytes: usize },
    /// JSON, but not an object.
    NotAnObject,
    /// An object without a `type`.
    Untyped,
    /// A type this adapter does not know; `kind` is its name, cut short and kept to letters, digits,
    /// `_`, `/` and `-`.
    Unknown { kind: String },
    /// A known type whose fields could not be read.
    Malformed { kind: &'static str, reason: &'static str },
    /// Longer than the line cap; it was never held, only counted.
    TooLong { bytes: u64 },
    /// Not UTF-8 text.
    NotText { bytes: u64 },
}
