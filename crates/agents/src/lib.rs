//! The agent runtimes a person installs beside Archforge, `claude` and `codex`: found on this
//! computer, checked against the versions Archforge accepts, and run as supervised child processes.
//! Archforge never bundles them.
//!
//! - [`find`] looks for a runtime: the override variable first (`ARCHFORGE_CLAUDE`,
//!   `ARCHFORGE_CODEX`), then the absolute entries of the PATH, then the folders installers use.
//!   It resolves links, requires a regular executable file, runs `<runtime> --version` in a private
//!   folder with a short budget, and answers a [`Discovery`].
//! - [`Workspace`] is the app-owned folder that holds one private folder per conversation
//!   ([`ConversationFolder`]), each with its own home, configuration home, temporary folder and empty
//!   working folder, all readable by this user alone.
//! - [`spawn`] starts a runtime in a cleared environment and its own process group;
//!   [`RuntimeProcess`] reads its output as capped lines, keeps the end of its error stream for a
//!   diagnosis, stops a turn with an interrupt, shuts it down in steps, and kills its whole process
//!   group when dropped.
//! - [`AgentSession`] is a conversation with a running runtime, turn by turn, and [`SessionEvent`]
//!   what it says, the same for every runtime; [`claude::ClaudeSession`] drives Claude Code over
//!   stream-json, and [`codex::CodexSession`] Codex's app-server over JSON-RPC, with
//!   [`codex::CodexAccount`] for its sign-in.
//! - [`AgentError`] is every failure, and [`ErrorKind`] the neutral category the interface shows.
//!
//! No secret appears in an error, a log line or a `Debug` output: keys arrive as
//! [`archforge_secrets::Secret`], reach the child only through its environment, and are scrubbed
//! from the error stream's tail. The runtimes are available on macOS and Linux; elsewhere every
//! call answers [`AgentError::NotAvailable`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

use std::fmt;

pub mod claude;
pub mod codex;
mod discover;
mod error;
mod folder;
mod output;
mod process;
mod session;
mod sys;
mod version;

pub use discover::{Discovery, HostEnv, Runtime, Unsupported, find};
pub use error::{AgentError, ErrorKind, MAX_DETAIL_BYTES};
pub use folder::{ConversationFolder, ConversationId, MAX_CONVERSATION_ID_BYTES, Workspace};
pub use output::{Dropped, OutputEvent};
pub use process::{Exit, Limits, MAX_GRACE, RuntimeProcess, SpawnRequest, spawn};
pub use session::{AgentSession, SessionEvent, Skipped, ToolServer, TurnOutcome, Usage};
pub use version::{Fit, Version, VersionRange};

/// How long `<runtime> --version` may take before the probe is stopped and the runtime refused.
pub const PROBE_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// The most of each output stream the version probe keeps, in bytes; the rest is read and dropped.
pub const PROBE_OUTPUT_BYTES: usize = 64 * 1024;

/// An agent runtime Archforge can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeKind {
    /// Claude Code, the program `claude`.
    Claude,
    /// OpenAI Codex, the program `codex`.
    Codex,
}

impl RuntimeKind {
    /// Every runtime, in the order the interface lists them.
    pub const ALL: [RuntimeKind; 2] = [RuntimeKind::Claude, RuntimeKind::Codex];

    /// The program's file name, as installers put it on the PATH.
    pub fn program(self) -> &'static str {
        match self {
            RuntimeKind::Claude => "claude",
            RuntimeKind::Codex => "codex",
        }
    }

    /// The environment variable that names the program's absolute path, overriding the search.
    pub fn override_var(self) -> &'static str {
        match self {
            RuntimeKind::Claude => "ARCHFORGE_CLAUDE",
            RuntimeKind::Codex => "ARCHFORGE_CODEX",
        }
    }

    /// The environment variable that moves the runtime's settings tree, which a spawned runtime
    /// gets pointed at its conversation's private configuration folder.
    pub fn config_var(self) -> &'static str {
        match self {
            RuntimeKind::Claude => "CLAUDE_CONFIG_DIR",
            RuntimeKind::Codex => "CODEX_HOME",
        }
    }

    /// The versions Archforge accepts (owner decision): Claude Code 2.1.286 up to, not including,
    /// 3.0.0, tested with 2.1.317; Codex 0.166.0 up to, not including, 1.0.0, tested with 0.166.0.
    pub fn range(self) -> VersionRange {
        match self {
            RuntimeKind::Claude => VersionRange { floor: Version::new(2, 1, 286), below: Version::new(3, 0, 0), tested: Version::new(2, 1, 317) },
            RuntimeKind::Codex => VersionRange { floor: Version::new(0, 166, 0), below: Version::new(1, 0, 0), tested: Version::new(0, 166, 0) },
        }
    }
}

impl fmt::Display for RuntimeKind {
    /// Writes the program's name, as messages name the runtime.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.program())
    }
}
