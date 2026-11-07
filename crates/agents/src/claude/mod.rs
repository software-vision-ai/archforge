//! The Claude Code adapter: headless sessions over stream-json (AR-201, after the AR-196 spike).
//!
//! [`ClaudeSession::start`] writes the conversation's tool server configuration, starts
//! `claude -p --bare` with the stream-json input and output, Archforge's bridge as the only tool
//! server (or none, without a bridge), every built-in tool removed and anything that would ask
//! denied; the model is named on
//! every start because a resume does not keep it. Claude's session id is the conversation's UUID,
//! and its transcripts stay in the conversation's private settings folder (`CLAUDE_CONFIG_DIR`).
//!
//! The key reaches `claude` only as `ANTHROPIC_API_KEY` in its environment. Claude hands its whole
//! environment to the tool servers it starts, so the generated configuration blanks every secret
//! variable for the bridge.
//!
//! The bridge reads the conversation's own token from the file the application granted it
//! (`--token-file`). A conversation is read-only unless "Allow changes" is on: its token's role is
//! read-only, which the control channel enforces, and the bridge runs with `--read-only` as a
//! second layer. Switching it means shutting the runtime down and starting it again with
//! [`Start::Resume`] and the other setting, which rewrites the configuration. That a resume picks up
//! a changed configuration is not yet verified with a real round trip (spike item 5).

mod parse;

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use archforge_secrets::Secret;
use serde_json::{Map, Value, json};

pub use parse::{ClaudeParser, MAX_INPUT_BYTES, MAX_NAME_BYTES, MAX_SUMMARY_BYTES, kind_of};

use crate::session::{AgentSession, SessionEvent, TurnOutcome};
use crate::{AgentError, ConversationFolder, ErrorKind, Exit, Limits, OutputEvent, Runtime, RuntimeKind, RuntimeProcess, SpawnRequest, spawn};

/// The file, in the conversation's folder, that holds the tool server configuration.
pub const MCP_CONFIG_FILE: &str = "claude-mcp.json";
/// The name of Archforge's tool server in the configuration, which prefixes its tools.
pub const SERVER_NAME: &str = "archforge";
/// The environment variable that carries the key to `claude`.
pub const KEY_VAR: &str = "ANTHROPIC_API_KEY";
/// Every secret variable the adapter gives `claude`; each is blanked for the tool server.
pub const SECRET_VARS: [&str; 1] = [KEY_VAR];
/// The longest model name accepted, in bytes.
pub const MAX_MODEL_BYTES: usize = 128;
/// The most output lines one `next_event` call reads while none means an event.
const LINES_PER_CALL: u32 = 256;
/// How long the session waits for the runtime's exit status once its output closed.
const EXIT_WAIT: Duration = Duration::from_millis(500);

/// Claude's session id: the conversation's UUID, in its canonical lowercase form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// A session id, which must be a lowercase UUID (`8-4-4-4-12` hexadecimal digits). A refused id
    /// is not repeated in the error.
    pub fn new(id: &str) -> Result<SessionId, AgentError> {
        if id.len() != 36 {
            return Err(setting("session id", id, "it is not a lowercase UUID such as 7f3c2a10-0000-4000-8000-0000000000d1"));
        }
        let groups: Vec<&str> = id.split('-').collect();
        let lengths = [8, 4, 4, 4, 12];
        let shaped = groups.len() == lengths.len()
            && groups.iter().zip(lengths).all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        if shaped {
            Ok(SessionId(id.to_owned()))
        } else {
            Err(setting("session id", id, "it is not a lowercase UUID such as 7f3c2a10-0000-4000-8000-0000000000d1"))
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    /// Writes the id; an id is never secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The model a session asks for: an alias (`sonnet`) or a full name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Model(String);

impl Model {
    /// A model name: 1 to [`MAX_MODEL_BYTES`] ASCII letters, digits and `-._:[]/@`, not starting
    /// with `-`, so it can never be read as a flag. A refused name is not repeated in the error, as
    /// it may be a key pasted into the wrong field.
    pub fn new(name: &str) -> Result<Model, AgentError> {
        if name.is_empty() || name.len() > MAX_MODEL_BYTES {
            return Err(setting("model", name, &format!("a model name is 1 to {MAX_MODEL_BYTES} bytes")));
        }
        if name.starts_with('-') || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._:[]/@".contains(&b)) {
            return Err(setting("model", name, "a model name is ASCII letters, digits and -._:[]/@, not starting with -"));
        }
        Ok(Model(name.to_owned()))
    }

    /// The name as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether a start opens a new conversation or continues a stored one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// A new conversation under the session id (`--session-id`).
    New,
    /// The stored conversation of the session id (`--resume`).
    Resume,
}

/// Archforge's tool server as a runtime is to start it:
/// `archforge-cli mcp --bridge <port> --token-file <file> [--read-only] --origin <label>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bridge {
    /// The absolute path of `archforge-cli` (`archforge_control::cli_path`).
    pub cli: PathBuf,
    /// The port the application's control channel listens on.
    pub port: u16,
    /// The absolute path of the file that holds this conversation's own token
    /// (`archforge_control::Grant::path`); the bridge reads it for every call.
    pub token_file: PathBuf,
    /// The label on every change this conversation makes (`--origin`).
    pub origin: String,
    /// "Allow changes": without it the bridge runs with `--read-only`, a second layer over the
    /// read-only role of the conversation's token.
    pub allow_changes: bool,
}

/// The bridge's arguments after `archforge-cli`, every path and label checked: absolute UTF-8
/// paths, a port of 1 to 65535, an origin label `archforge-cli` takes that does not start with `-`.
/// Gives the command and its arguments.
pub(crate) fn bridge_command(bridge: &Bridge) -> Result<(String, Vec<String>), AgentError> {
    let cli = absolute_text("archforge-cli path", &bridge.cli)?;
    let token_file = absolute_text("token file", &bridge.token_file)?;
    if bridge.port == 0 {
        return Err(AgentError::InvalidSetting { setting: "bridge port", bytes: 1, reason: "the port is 1 to 65535".to_owned() });
    }
    if bridge.origin.starts_with('-') {
        return Err(setting("origin label", &bridge.origin, "a label may not start with -"));
    }
    archforge_control::check_origin(&bridge.origin).map_err(|e| setting("origin label", &bridge.origin, &e.to_string()))?;
    let mut args = vec!["mcp".to_owned(), "--bridge".to_owned(), bridge.port.to_string(), "--token-file".to_owned(), token_file];
    if !bridge.allow_changes {
        args.push("--read-only".to_owned());
    }
    args.push("--origin".to_owned());
    args.push(bridge.origin.clone());
    Ok((cli, args))
}

/// `path` as text: refused when it is relative or not UTF-8.
fn absolute_text(what: &'static str, path: &Path) -> Result<String, AgentError> {
    if !path.is_absolute() {
        return Err(AgentError::InvalidSetting { setting: what, bytes: path.as_os_str().len(), reason: "it is not an absolute path".to_owned() });
    }
    path.to_str().map(str::to_owned).ok_or_else(|| AgentError::InvalidSetting {
        setting: what,
        bytes: path.as_os_str().len(),
        reason: "it is not UTF-8 text".to_owned(),
    })
}

/// Everything a Claude session is started with, but the key. Without a bridge the session has no
/// tool server at all (the configuration names none), as a test message or a conversation without
/// agent access runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeConfig {
    pub session: SessionId,
    pub start: Start,
    pub model: Model,
    pub bridge: Option<Bridge>,
    pub limits: Limits,
}

/// The command line after `claude`, for a tool server configuration at `mcp_config`.
pub fn command_line(config: &ClaudeConfig, mcp_config: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-p",
        "--bare",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--mcp-config",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(mcp_config.as_os_str().to_owned());
    for arg in ["--strict-mcp-config", "--tools", "", "--allowedTools", "mcp__archforge__*", "--permission-mode", "dontAsk", "--model"] {
        args.push(arg.into());
    }
    args.push(config.model.as_str().into());
    args.push(match config.start {
        Start::New => "--session-id".into(),
        Start::Resume => "--resume".into(),
    });
    args.push(config.session.as_str().into());
    args
}

/// The tool server configuration `claude` reads (`--mcp-config`), as JSON text: Archforge's bridge
/// alone, read-only unless changes are allowed, with every secret variable blanked, as Claude hands
/// its whole environment to the servers it starts; `{"mcpServers":{}}` without a bridge. Refused: a
/// relative or non-UTF-8 path, and an origin label `archforge-cli` would refuse.
pub fn mcp_config(bridge: Option<&Bridge>) -> Result<String, AgentError> {
    let Some(bridge) = bridge else { return Ok(json!({ "mcpServers": {} }).to_string()) };
    let (cli, args) = bridge_command(bridge)?;
    let mut env = Map::new();
    for name in SECRET_VARS {
        env.insert(name.to_owned(), Value::String(String::new()));
    }
    let config = json!({ "mcpServers": { SERVER_NAME: { "command": cli, "args": args, "env": env } } });
    Ok(config.to_string())
}

/// Writes the tool server configuration into the conversation's folder, atomically and readable by
/// this user alone (0600); answers its path. Refused before anything is written: a configuration
/// that holds the key (pasted into the origin label, say), which would put it on the bridge's
/// command line. Whatever stands at the file's place from an earlier start, a link among them, is
/// removed first, so the write never follows a link out of the private folder.
pub fn write_mcp_config(folder: &ConversationFolder, bridge: Option<&Bridge>, key: &Secret) -> Result<PathBuf, AgentError> {
    let text = mcp_config(bridge)?;
    if !key.expose().is_empty() && text.contains(key.expose()) {
        return Err(AgentError::SecretExposed { variable: KEY_VAR.to_owned(), place: "tool server configuration" });
    }
    let path = folder.path().join(MCP_CONFIG_FILE);
    let failed = |problem: String| AgentError::Folder { path: path.clone(), problem };
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_dir() => return Err(failed("holds a folder where the tool server configuration belongs".to_owned())),
        Ok(_) => std::fs::remove_file(&path).map_err(|e| failed(format!("could not remove the earlier tool server configuration: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(failed(format!("could not be checked for the tool server configuration: {e}"))),
    }
    craft_atomic::WriteOptions::default()
        .with_new_file_permissions(craft_atomic::NewFilePermissions::Private)
        .atomic_write(&path, text.as_bytes())
        .map_err(|e| failed(format!("could not hold the tool server configuration: {e}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file() && m.permissions().mode() & 0o077 == 0);
        if !private {
            let _ = std::fs::remove_file(&path);
            return Err(failed("did not keep the tool server configuration private to this user".to_owned()));
        }
    }
    Ok(path)
}

/// A running Claude Code session: the runtime process and what its output has said so far.
#[derive(Debug)]
pub struct ClaudeSession {
    session: SessionId,
    process: RuntimeProcess,
    parser: ClaudeParser,
    pending: VecDeque<SessionEvent>,
    turn_open: bool,
    stop_sent: bool,
    stop_requests: u64,
    lines_seen: bool,
    shutting_down: bool,
    ended: bool,
}

impl ClaudeSession {
    /// Starts `claude` for the conversation in `folder` with `config`, the key going to its
    /// environment alone. `runtime` must be a Claude runtime from [`crate::find`].
    pub fn start(runtime: &Runtime, folder: &ConversationFolder, config: ClaudeConfig, key: Secret) -> Result<ClaudeSession, AgentError> {
        if runtime.kind() != RuntimeKind::Claude {
            return Err(AgentError::InvalidSetting {
                setting: "runtime",
                bytes: 0,
                reason: format!("the Claude adapter cannot drive {}", runtime.kind()),
            });
        }
        if !crate::sys::AVAILABLE {
            return Err(AgentError::NotAvailable);
        }
        config.limits.check()?;
        let mcp = write_mcp_config(folder, config.bridge.as_ref(), &key)?;
        let request = SpawnRequest {
            args: command_line(&config, &mcp),
            vars: vec![("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_owned(), "1".into())],
            secret_vars: vec![(KEY_VAR.to_owned(), key)],
            input_secrets: Vec::new(),
            config_dir: None,
            limits: config.limits,
        };
        let process = spawn(runtime, folder, request)?;
        Ok(ClaudeSession {
            session: config.session,
            process,
            parser: ClaudeParser::new(config.limits.max_line_bytes),
            pending: VecDeque::new(),
            turn_open: false,
            stop_sent: false,
            stop_requests: 0,
            lines_seen: false,
            shutting_down: false,
            ended: false,
        })
    }

    /// The session id, the conversation's UUID.
    pub fn session_id(&self) -> &SessionId {
        &self.session
    }

    /// How many output lines were skipped so far.
    pub fn skipped(&self) -> u64 {
        self.parser.skipped()
    }

    /// The runtime process, for its id and the end of its error stream.
    pub fn process(&self) -> &RuntimeProcess {
        &self.process
    }

    /// Whether a turn runs.
    pub fn turn_running(&self) -> bool {
        self.turn_open
    }

    /// Queues the events of one output event. A line is scrubbed of the key whole, before it is
    /// parsed, so no field, cut, log line or kept state can hold any part of a key written in it (a
    /// key's characters are never escaped in JSON).
    fn take(&mut self, event: OutputEvent) {
        match event {
            OutputEvent::Line(line) => {
                self.lines_seen = true;
                let line = self.process.redact(line);
                for event in self.parser.line(&line) {
                    if matches!(event, SessionEvent::TurnFinished { .. }) {
                        self.turn_open = false;
                        self.stop_sent = false;
                    }
                    self.pending.push_back(event);
                }
                if self.parser.take_stop_refused() && self.turn_open {
                    log::warn!("claude refused the stop request; it is sent SIGINT instead");
                    if let Err(e) = self.process.interrupt() {
                        log::warn!("claude could not be interrupted: {e}");
                    }
                }
            }
            OutputEvent::Dropped(why) => {
                let event = self.parser.dropped(why);
                self.pending.push_back(event);
            }
            OutputEvent::ReadFailed(detail) => log::warn!("claude's output could not be read: {detail}"),
            OutputEvent::End => self.finish(),
        }
    }

    /// The runtime's output closed: an open turn fails, then the session ends.
    fn finish(&mut self) {
        self.ended = true;
        let exit = self.exit_status();
        let error = if self.shutting_down {
            None
        } else {
            let tail = self.process.stderr_tail();
            Some(if !self.lines_seen && tail.contains("unknown option") {
                // A runtime too old for a flag stops before its first event (spike, Versions).
                AgentError::reported(ErrorKind::MissingRuntime, &format!("claude does not know a flag Archforge needs; update it. {}", tail.trim()))
            } else {
                match exit {
                    Some(exit) => self.process.exit_error(exit),
                    None => AgentError::reported(ErrorKind::Other, "claude closed its output and its exit was not seen"),
                }
            })
        };
        if self.turn_open {
            self.turn_open = false;
            let failed = error.clone().unwrap_or_else(|| AgentError::reported(ErrorKind::Other, "claude ended before the turn did"));
            self.pending.push_back(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(failed), usage: Default::default() });
        }
        self.pending.push_back(SessionEvent::Ended { exit, error });
    }

    /// The runtime's exit, waiting a little for it once its output closed.
    fn exit_status(&mut self) -> Option<Exit> {
        let started = Instant::now();
        loop {
            match self.process.status() {
                Ok(Some(exit)) => return Some(exit),
                Ok(None) if started.elapsed() < EXIT_WAIT => std::thread::sleep(Duration::from_millis(10)),
                Ok(None) => return None,
                Err(e) => {
                    log::warn!("claude's exit could not be checked: {e}");
                    return None;
                }
            }
        }
    }
}

impl AgentSession for ClaudeSession {
    /// Writes the message as one stream-json user line.
    fn send(&mut self, text: &str) -> Result<(), AgentError> {
        if self.ended {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Claude });
        }
        if self.turn_open {
            return Err(AgentError::TurnRunning { runtime: RuntimeKind::Claude });
        }
        let line = json!({ "type": "user", "message": { "role": "user", "content": text }, "parent_tool_use_id": Value::Null });
        self.process.send_line(&line.to_string())?;
        self.turn_open = true;
        self.stop_sent = false;
        Ok(())
    }

    /// Sends the `interrupt` control request; when it cannot be sent, or on a second stop of the
    /// same turn, SIGINT to the runtime alone. Both end the turn and leave the runtime up.
    fn stop(&mut self) -> Result<(), AgentError> {
        if !self.turn_open || self.ended {
            return Ok(());
        }
        self.parser.stop_requested();
        if self.stop_sent {
            return self.process.interrupt();
        }
        self.stop_sent = true;
        self.stop_requests = self.stop_requests.saturating_add(1);
        let id = format!("{}{}", parse::STOP_REQUEST_PREFIX, self.stop_requests);
        let line = json!({ "type": "control_request", "request_id": id, "request": { "subtype": "interrupt" } });
        match self.process.send_line(&line.to_string()) {
            Ok(()) => Ok(()),
            Err(e) => {
                log::warn!("claude's stop request could not be sent ({e}); it is sent SIGINT instead");
                self.process.interrupt()
            }
        }
    }

    /// Reads output lines until one means an event, the timeout passes, or a call's budget of lines
    /// is spent, so a runtime writing only lines that mean nothing cannot hold the caller. When the
    /// output closes, the wait for the exit status may add up to half a second.
    fn next_event(&mut self, timeout: Duration) -> Option<SessionEvent> {
        let deadline = Instant::now().checked_add(timeout);
        let mut budget = LINES_PER_CALL;
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            if self.ended || budget == 0 || deadline.is_some_and(|d| Instant::now() >= d) {
                return None;
            }
            budget -= 1;
            let left = deadline.map_or(timeout, |d| d.saturating_duration_since(Instant::now()));
            match self.process.next_event(left) {
                Some(event) => self.take(event),
                None => return None,
            }
        }
    }

    fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        self.shutting_down = true;
        self.process.shutdown(grace)
    }
}

/// A refused setting, named by its length only: the value is never repeated.
fn setting(what: &'static str, value: &str, reason: &str) -> AgentError {
    AgentError::InvalidSetting { setting: what, bytes: value.len(), reason: reason.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Session ids are lowercase UUIDs; model names cannot pass for flags.
    #[test]
    fn ids_and_models_are_checked() {
        assert!(SessionId::new("7f3c2a10-0000-4000-8000-0000000000d1").is_ok());
        for bad in ["", "7F3C2A10-0000-4000-8000-0000000000D1", "7f3c2a10-0000-4000-8000-0000000000d", "7f3c2a10000040008000-0000000000d1", "g f"] {
            assert!(matches!(SessionId::new(bad), Err(AgentError::InvalidSetting { .. })), "{bad}");
        }
        for good in ["sonnet", "claude-sonnet-5-5", "claude-opus-5-5[1m]", "us.anthropic.claude-x:0"] {
            assert!(Model::new(good).is_ok(), "{good}");
        }
        for bad in ["", "-p", "--resume", "son net", "a\nb", &"m".repeat(MAX_MODEL_BYTES + 1)] {
            assert!(matches!(Model::new(bad), Err(AgentError::InvalidSetting { .. })), "{bad}");
        }
    }
}
