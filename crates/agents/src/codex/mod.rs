//! The Codex adapter: `codex app-server` sessions over JSON-RPC, and the account operations the
//! Settings page needs (AR-202, after the AR-196 spike).
//!
//! **Folders.** Codex keeps its sign-in reference in its settings folder (`CODEX_HOME`): the
//! ChatGPT tokens stay in the system's keyring under an entry named after that folder's path, so
//! every conversation must use the same folder or each would need its own sign-in. That folder is
//! the workspace's account folder ([`crate::Workspace::account`]); everything a conversation
//! leaves behind stays in the conversation's own folder instead: Codex's state and log databases
//! (`sqlite_home` and `log_dir` point into the conversation's configuration folder), and the
//! conversation itself, as Codex threads are ephemeral and Archforge keeps its own copy
//! ([`history`]), which goes back into a new thread on a resume. Deleting the conversation's
//! folder deletes all of it; the account folder never holds a conversation.
//!
//! **Keys.** An OpenAI API key reaches Codex once, over its standard input, with
//! `account/login/start {"type":"apiKey"}`, and Codex runs with
//! `cli_auth_credentials_store="ephemeral"`, so the key is never written to a file or the keyring
//! (owner decision, 2026-11-06). It is in no argument, variable or file of Archforge's; every line of
//! Codex's output is scrubbed of it before it is read. A ChatGPT sign-in is kept by Codex itself in
//! the keyring (`cli_auth_credentials_store="keyring"`); Archforge never sees its tokens.
//!
//! **Tools.** The bridge is the only tool server, read-only unless "Allow changes" is on; Codex's
//! own tools are switched off where a switch exists, and the read-only sandbox with every approval
//! declined holds back `apply_patch`, which has none.

mod accounts;
pub mod history;
mod parse;
pub mod rpc;

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use archforge_secrets::Secret;
use serde_json::{Value, json};
use zeroize::Zeroizing;

pub use accounts::{
    AccountEvent, AccountOptions, AccountState, AccountStatus, CancelStatus, CodexAccounts, LoginStarted, ModelInfo, RateLimitWindow, RateLimits,
    SIGN_IN_HOSTS, checked_url,
};
pub use history::History;
pub use parse::{CodexParser, HANDLED_NOTIFICATIONS, MAX_INPUT_BYTES, MAX_NAME_BYTES, MAX_SUMMARY_BYTES, QUIET_NOTIFICATIONS, kind_of};
use rpc::{Incoming, Requests, RpcError};

pub use crate::claude::{Bridge, BridgeHome, Model};
use crate::claude::{bridge_command, path_text, setting};
use crate::session::{AgentSession, SessionEvent, Skipped, TurnOutcome};
use crate::{
    AgentError, ConversationFolder, ErrorKind, Exit, Fit, Limits, OutputEvent, Runtime, RuntimeKind, RuntimeProcess, SpawnRequest, Version, spawn,
};

/// The name of Archforge's tool server in Codex's configuration.
pub const SERVER_NAME: &str = "archforge";
/// The client name Archforge gives in `initialize`; Codex answers with `<name>/<its version>`.
pub const CLIENT_NAME: &str = "archforge";
/// How long a request waits for its answer by default.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest a request may be given to answer.
pub const MAX_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
/// The longest developer instructions accepted, in bytes.
pub const MAX_INSTRUCTIONS_BYTES: usize = 64 * 1024;
/// The folders, in a conversation's configuration folder, that hold Codex's state and log databases.
pub const STATE_FOLDER: &str = "codex-state";
pub const LOG_FOLDER: &str = "codex-log";
/// Codex's features Archforge switches off (`--disable`): every tool but the bridge's, and
/// everything that would keep data or reach the network on its own. Each was accepted by codex-cli
/// 0.166.0 under `--strict-config`.
pub const DISABLED_FEATURES: [&str; 16] = [
    "shell_tool",
    "unified_exec",
    "apps",
    "plugins",
    "multi_agent",
    "view_image",
    "image_generation",
    "browser_use",
    "computer_use",
    "goals",
    "hooks",
    "skill_search",
    "tool_suggest",
    "sleep_tool",
    "memories",
    "shell_snapshot",
];
/// Every request method this adapter sends; each is in the 0.166.0 schema (the contract test).
pub const CLIENT_METHODS: [&str; 11] = [
    "initialize",
    "account/read",
    "account/login/start",
    "account/login/cancel",
    "account/logout",
    "account/rateLimits/read",
    "model/list",
    "thread/start",
    "thread/inject_items",
    "turn/start",
    "turn/interrupt",
];
/// The requests from Codex this adapter answers with a decline; any other gets an error answer.
pub const DECLINED_REQUESTS: [&str; 7] = [
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
    "mcpServer/elicitation/request",
    "item/tool/requestUserInput",
    "applyPatchApproval",
    "execCommandApproval",
];
/// The most output lines one `next_event` call reads while none means an event.
const LINES_PER_CALL: u32 = 256;
/// How long the session waits for the runtime's exit status once its output closed.
const EXIT_WAIT: Duration = Duration::from_millis(500);

/// How Codex is signed in, without the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignInKind {
    /// An OpenAI API key, given over standard input to each process and kept in its memory alone.
    ApiKey,
    /// A ChatGPT sign-in, which Codex keeps in the system's keyring.
    ChatGpt,
}

/// How a session signs in.
#[derive(Debug)]
pub enum SignIn {
    /// With this OpenAI API key, sent once over standard input.
    ApiKey(Secret),
    /// With the ChatGPT sign-in Codex keeps in the keyring, reached through `keyring`.
    ChatGpt(KeyringAccess),
}

impl SignIn {
    /// The kind of sign-in.
    pub fn kind(&self) -> SignInKind {
        match self {
            SignIn::ApiKey(_) => SignInKind::ApiKey,
            SignIn::ChatGpt(_) => SignInKind::ChatGpt,
        }
    }
}

/// What a Codex process needs from the host's environment to reach the system's keyring: nothing on
/// macOS, the session bus on Linux (`DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`). Never a secret.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyringAccess {
    pub vars: Vec<(String, OsString)>,
}

impl KeyringAccess {
    /// The variables this process has for the keyring, each kept only when set and not empty.
    pub fn from_process() -> KeyringAccess {
        let names: &[&str] = if cfg!(target_os = "linux") { &["DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR"] } else { &[] };
        let vars = names.iter().filter_map(|&n| std::env::var_os(n).filter(|v| !v.is_empty()).map(|v| (n.to_owned(), v))).collect();
        KeyringAccess { vars }
    }
}

/// Whether a start opens a new conversation or continues a stored one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// A new conversation: the first message starts a new thread.
    New,
    /// A conversation with earlier messages: the first message starts a new thread that is given
    /// the copy in the conversation's folder first (`thread/inject_items`).
    Resume,
    /// A conversation whose folder holds no copy (opened on another computer, or the folder was
    /// lost or its copy damaged): the new thread is primed with these messages instead, built from
    /// the project's own record of the conversation (AR-206). The folder's copy starts from them
    /// and replaces any copy there at the end of the first turn.
    ResumeFrom(History),
}

/// Everything a Codex session is started with, but the sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConfig {
    pub start: Start,
    /// The model; Codex's default when `None`.
    pub model: Option<Model>,
    /// Developer instructions for the thread, at most [`MAX_INSTRUCTIONS_BYTES`].
    pub instructions: Option<String>,
    pub bridge: Bridge,
    pub limits: Limits,
    /// How long each request waits for its answer.
    pub request_timeout: Duration,
}

/// The folders a Codex process keeps its state and logs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFolders {
    pub sqlite_home: PathBuf,
    pub log_dir: PathBuf,
}

impl StateFolders {
    /// The folders inside the conversation's configuration folder.
    pub fn of(folder: &ConversationFolder) -> StateFolders {
        StateFolders { sqlite_home: folder.config().join(STATE_FOLDER), log_dir: folder.config().join(LOG_FOLDER) }
    }
}

/// The command line after `codex`: `app-server --strict-config`, the configuration as `-c`
/// overrides (Codex refuses an unknown key under `--strict-config`), and the switched-off
/// features. The credential store follows `sign_in`; `state` moves the state and log databases;
/// `bridge` adds Archforge's tool server, read-only unless changes are allowed. Refused: a path
/// that is relative or not UTF-8, and a bridge `archforge-cli` would refuse.
pub fn command_line(sign_in: SignInKind, state: Option<&StateFolders>, bridge: Option<&Bridge>) -> Result<Vec<OsString>, AgentError> {
    let (store, method) = match sign_in {
        SignInKind::ApiKey => ("ephemeral", "api"),
        SignInKind::ChatGpt => ("keyring", "chatgpt"),
    };
    let mut config: Vec<String> = vec![
        format!("cli_auth_credentials_store={}", toml_string(store)),
        format!("forced_login_method={}", toml_string(method)),
        "history.persistence=\"none\"".to_owned(),
        "analytics.enabled=false".to_owned(),
        "check_for_update_on_startup=false".to_owned(),
        "web_search=\"disabled\"".to_owned(),
        "approval_policy=\"on-request\"".to_owned(),
        "sandbox_mode=\"read-only\"".to_owned(),
    ];
    if let Some(state) = state {
        config.push(format!("sqlite_home={}", toml_string(&path_text("Codex state folder", &state.sqlite_home)?)));
        config.push(format!("log_dir={}", toml_string(&path_text("Codex log folder", &state.log_dir)?)));
    }
    if let Some(bridge) = bridge {
        let command = bridge_command(bridge)?;
        let server = format!("mcp_servers.{SERVER_NAME}");
        let args: Vec<String> = command.args.iter().map(|a| toml_string(a)).collect();
        let env: Vec<String> = command.env.iter().map(|(k, v)| format!("{k}={}", toml_string(v))).collect();
        config.push(format!("{server}.command={}", toml_string(&command.cli)));
        config.push(format!("{server}.args=[{}]", args.join(",")));
        config.push(format!("{server}.env={{{}}}", env.join(",")));
        config.push(format!("{server}.startup_timeout_sec=10"));
        config.push(format!("{server}.tool_timeout_sec=60"));
        // The bridge's own read-only mode guards the model; Codex asking about each call would
        // only meet Archforge's decline.
        config.push(format!("{server}.default_tools_approval_mode=\"approve\""));
    }
    let mut args: Vec<OsString> = vec!["app-server".into(), "--strict-config".into()];
    for c in config {
        args.push("-c".into());
        args.push(c.into());
    }
    for feature in DISABLED_FEATURES {
        args.push("--disable".into());
        args.push(feature.into());
    }
    Ok(args)
}

/// `text` as a TOML basic string: quotes and backslashes escaped, and every control character
/// (DEL among them) written as `\uXXXX`, so no value can end the string or the line.
pub fn toml_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_add(2));
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The `initialize` request's parameters.
fn initialize_params() -> Value {
    json!({
        "clientInfo": { "name": CLIENT_NAME, "title": "Archforge", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "experimentalApi": false, "requestAttestation": false }
    })
}

/// The version Codex names in its `initialize` answer's `userAgent` (`archforge/0.166.0 (…)`).
pub fn agent_version(user_agent: &str) -> Option<Version> {
    let rest = user_agent.strip_prefix(CLIENT_NAME)?.strip_prefix('/')?;
    Version::parse(rest.split(' ').next()?)
}

/// The version check of an `initialize` answer: an error when Codex answers as a version Archforge
/// does not accept; a newer one in the range is logged.
fn check_agent(runtime: &Runtime, result: &Value) -> Result<(), AgentError> {
    let agent = result.get("userAgent").and_then(Value::as_str).unwrap_or_default();
    let Some(version) = agent_version(agent) else {
        log::warn!("codex did not name its version in its handshake; the version from --version stands");
        return Ok(());
    };
    let range = RuntimeKind::Codex.range();
    match range.fit(version) {
        Fit::Tested => Ok(()),
        Fit::NewerThanTested => {
            log::warn!("codex {version} is newer than {}, the version Archforge was tested with", range.tested);
            Ok(())
        }
        Fit::TooOld => {
            Err(AgentError::TooOld { runtime: RuntimeKind::Codex, path: runtime.path().to_path_buf(), found: version, floor: range.floor })
        }
        Fit::TooNew => Err(AgentError::Unsupported {
            runtime: RuntimeKind::Codex,
            path: runtime.path().to_path_buf(),
            reason: format!("it answers as version {version}, and Archforge supports only versions below {}", range.below),
        }),
    }
}

/// The answer to a request from Codex: a decline for the approvals and questions Archforge knows,
/// an error answer for anything else, so no request ever blocks a turn.
pub(crate) fn decline_line(id: &Value, method: &str) -> String {
    let result = match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => json!({ "decision": "decline" }),
        "mcpServer/elicitation/request" => json!({ "action": "decline", "content": null, "_meta": null }),
        "item/permissions/requestApproval" => json!({ "permissions": {}, "scope": "turn" }),
        "item/tool/requestUserInput" => json!({ "answers": {} }),
        "applyPatchApproval" | "execCommandApproval" => json!({ "decision": { "denied": { "rejection": "Archforge does not allow this" } } }),
        other => {
            let name = parse::kind_name(other);
            return rpc::error_line(id, -32601, &format!("Archforge does not handle {name}"));
        }
    };
    rpc::result_line(id, result)
}

/// The error for a request Codex refused: a missing runtime when Codex does not know the method.
fn refused(method: &str, error: &RpcError, kind: ErrorKind) -> AgentError {
    if error.unknown_method() {
        AgentError::reported(ErrorKind::MissingRuntime, &format!("codex does not know {method}, which Archforge needs; update it"))
    } else {
        AgentError::reported(kind, &format!("codex refused {method}: {}", error.message))
    }
}

/// Refuses a request timeout of zero or past [`MAX_REQUEST_TIMEOUT`].
fn check_timeout(timeout: Duration) -> Result<(), AgentError> {
    if timeout.is_zero() || timeout > MAX_REQUEST_TIMEOUT {
        return Err(AgentError::InvalidLimit {
            limit: "request timeout (ms)",
            value: timeout.as_millis(),
            accepted: format!("1 to {}", MAX_REQUEST_TIMEOUT.as_millis()),
        });
    }
    Ok(())
}

/// Makes the state and log folders private to this user, or checks the ones there.
fn ensure_state(state: &StateFolders) -> Result<(), AgentError> {
    let uid = crate::sys::current_uid();
    crate::folder::ensure_private(&state.sqlite_home, uid)?;
    crate::folder::ensure_private(&state.log_dir, uid)
}

/// The `account/login/start` line for an API key, in a buffer that is overwritten when dropped and
/// is sized up front, so no copy of the key is left behind by a reallocation.
fn api_key_login_line(id: u64, key: &Secret) -> Result<Zeroizing<String>, AgentError> {
    let value = key.expose();
    let mut escaped = Zeroizing::new(Vec::with_capacity(value.len().saturating_mul(6).saturating_add(2)));
    serde_json::to_writer(&mut *escaped, value).map_err(|e| AgentError::reported(ErrorKind::Other, &format!("the key could not be written: {e}")))?;
    let head = format!("{{\"id\":{id},\"method\":\"account/login/start\",\"params\":{{\"apiKey\":");
    let tail = ",\"type\":\"apiKey\"}}";
    let mut line = Zeroizing::new(String::with_capacity(head.len().saturating_add(escaped.len()).saturating_add(tail.len())));
    line.push_str(&head);
    line.push_str(std::str::from_utf8(&escaped).map_err(|_| AgentError::reported(ErrorKind::Other, "the key is not text"))?);
    line.push_str(tail);
    Ok(line)
}

/// What a request of the session was for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Initialize,
    Login,
    AccountRead,
    ThreadStart,
    Inject,
    TurnStart,
    Interrupt,
}

/// Where the session's thread stands.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Thread {
    /// None started yet: the first message starts it.
    None,
    /// `thread/start` sent.
    Starting,
    /// Started, with this id.
    Ready(String),
}

/// A running Codex session: the runtime process, the requests waiting for answers, and what its
/// output has said so far.
pub struct CodexSession {
    runtime: Runtime,
    folder: ConversationFolder,
    process: RuntimeProcess,
    parser: CodexParser,
    requests: Requests<Purpose>,
    pending: VecDeque<SessionEvent>,
    history: History,
    inject: bool,
    model: Option<Model>,
    instructions: Option<String>,
    timeout: Duration,
    max_line_bytes: usize,
    sign_in: SignInKind,
    ready: bool,
    sign_in_problem: Option<AgentError>,
    fatal: Option<AgentError>,
    thread: Thread,
    queued: Option<String>,
    turn_open: bool,
    turn_requested: bool,
    awaiting: bool,
    stop_requested: bool,
    interrupt_sent: bool,
    lines_seen: bool,
    shutting_down: bool,
    ended: bool,
}

impl std::fmt::Debug for CodexSession {
    /// Writes the session's state; never a message, an answer or a secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexSession")
            .field("process", &self.process)
            .field("parser", &self.parser)
            .field("sign_in", &self.sign_in)
            .field("ready", &self.ready)
            .field("thread", &self.thread)
            .field("requests", &self.requests.len())
            .field("turn_open", &self.turn_open)
            .field("history", &self.history.entries().len())
            .field("ended", &self.ended)
            .finish()
    }
}

impl CodexSession {
    /// Starts `codex app-server` for the conversation in `folder`, its settings folder the shared
    /// `account` folder ([`crate::Workspace::account`]), and begins the handshake: `initialize`,
    /// `initialized`, the API key's `account/login/start` when signing in with one, and
    /// `account/read`. Returns at once; the answers are read by [`next_event`](Self::next_event).
    /// No thread is started until the first message, as starting one already reaches the network.
    pub fn start(
        runtime: &Runtime,
        account: &ConversationFolder,
        folder: &ConversationFolder,
        config: CodexConfig,
        sign_in: SignIn,
    ) -> Result<CodexSession, AgentError> {
        if runtime.kind() != RuntimeKind::Codex {
            return Err(AgentError::InvalidSetting {
                setting: "runtime",
                bytes: 0,
                reason: format!("the Codex adapter cannot drive {}", runtime.kind()),
            });
        }
        if !crate::sys::AVAILABLE {
            return Err(AgentError::NotAvailable);
        }
        config.limits.check()?;
        check_timeout(config.request_timeout)?;
        if let Some(text) = &config.instructions
            && text.len() > MAX_INSTRUCTIONS_BYTES
        {
            return Err(setting("developer instructions", text, &format!("instructions are at most {MAX_INSTRUCTIONS_BYTES} bytes")));
        }
        let history = match &config.start {
            Start::New => History::default(),
            Start::Resume => History::read(folder)?,
            Start::ResumeFrom(history) => history.clone(),
        };
        let state = StateFolders::of(folder);
        let kind = sign_in.kind();
        let args = command_line(kind, Some(&state), Some(&config.bridge))?;
        ensure_state(&state)?;

        let now = Instant::now();
        let mut requests = Requests::new();
        let initialize = requests.open("initialize", Purpose::Initialize, config.request_timeout, now)?;
        let (vars, scrub, login) = match sign_in {
            SignIn::ApiKey(key) => {
                let id = requests.open("account/login/start", Purpose::Login, config.request_timeout, now)?;
                let line = api_key_login_line(id, &key)?;
                (Vec::new(), vec![key], Some(line))
            }
            SignIn::ChatGpt(keyring) => (keyring.vars, Vec::new(), None),
        };
        let read = requests.open("account/read", Purpose::AccountRead, config.request_timeout, now)?;
        let request =
            SpawnRequest { args, vars, scrub, config_home: Some(account.config().to_path_buf()), limits: config.limits, ..SpawnRequest::default() };
        let mut process = spawn(runtime, folder, request)?;
        process.send_line(&rpc::request_line(initialize, "initialize", Some(initialize_params())))?;
        process.send_line(&rpc::notification_line("initialized"))?;
        if let Some(line) = login {
            process.send_line(&line)?;
        }
        process.send_line(&rpc::request_line(read, "account/read", Some(json!({}))))?;
        Ok(CodexSession {
            runtime: runtime.clone(),
            folder: folder.clone(),
            process,
            parser: CodexParser::new(config.limits.max_line_bytes, kind),
            requests,
            pending: VecDeque::new(),
            inject: config.start != Start::New && !history.entries().is_empty(),
            history,
            model: config.model,
            instructions: config.instructions,
            timeout: config.request_timeout,
            max_line_bytes: config.limits.max_line_bytes,
            sign_in: kind,
            ready: false,
            sign_in_problem: None,
            fatal: None,
            thread: Thread::None,
            queued: None,
            turn_open: false,
            turn_requested: false,
            awaiting: false,
            stop_requested: false,
            interrupt_sent: false,
            lines_seen: false,
            shutting_down: false,
            ended: false,
        })
    }

    /// The thread's id, once the first message started it.
    pub fn thread_id(&self) -> Option<&str> {
        match &self.thread {
            Thread::Ready(id) => Some(id),
            _ => None,
        }
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

    /// Archforge's copy of the conversation so far.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Sends a request and registers it for its answer.
    fn request(&mut self, method: &'static str, purpose: Purpose, params: Value) -> Result<u64, AgentError> {
        let id = self.requests.open(method, purpose, self.timeout, Instant::now())?;
        if let Err(e) = self.process.send_line(&rpc::request_line(id, method, Some(params))) {
            self.requests.close(id);
            return Err(e);
        }
        Ok(id)
    }

    /// Queues the events of one output event. A line is scrubbed of the key whole before it is
    /// read, so no field, cut, log line or kept state can hold any part of a key written in it.
    fn take(&mut self, event: OutputEvent) {
        match event {
            OutputEvent::Line(line) => {
                self.lines_seen = true;
                let line = self.process.redact(line);
                match rpc::read_line(&line, self.max_line_bytes) {
                    Incoming::Response { id, result } => match self.requests.close(id) {
                        Some((method, purpose)) => self.answered(purpose, method, result),
                        None => {
                            log::debug!("codex answered a request that is not waiting (id {id})");
                            let event = self.parser.skip(Skipped::Malformed { kind: "response", reason: "it answers no request waiting" });
                            self.pending.push_back(event);
                        }
                    },
                    Incoming::Notification { method, params } => self.notified(&method, &params),
                    Incoming::Request { id, method, .. } => self.decline(&id, &method),
                    Incoming::Skipped(why) => {
                        let event = self.parser.skip(why);
                        self.pending.push_back(event);
                    }
                }
            }
            OutputEvent::Dropped(why) => {
                let event = self.parser.dropped(why);
                self.pending.push_back(event);
            }
            OutputEvent::ReadFailed(detail) => log::warn!("codex's output could not be read: {detail}"),
            OutputEvent::End => self.finish(),
        }
    }

    /// A notification: its events, and what it changes for the session.
    fn notified(&mut self, method: &str, params: &Value) {
        if method == "account/login/completed" && params.get("success").and_then(Value::as_bool) == Some(false) {
            self.sign_in_problem = Some(AgentError::reported(ErrorKind::InvalidCredential, "codex refused the sign-in"));
        }
        if method == "account/updated" && params.get("authMode").is_some_and(Value::is_null) {
            log::warn!("codex reports that it is no longer signed in");
        }
        for event in self.parser.notification(method, params) {
            match &event {
                SessionEvent::MessageDone { text } => self.history.push(history::Role::Assistant, text.clone()),
                SessionEvent::TurnFinished { .. } => {
                    self.turn_closed();
                }
                _ => {}
            }
            self.pending.push_back(event);
        }
    }

    /// Answers a request from Codex with a decline, and logs it.
    fn decline(&mut self, id: &Value, method: &str) {
        log::info!("codex asked for {}; Archforge declined", parse::kind_name(method));
        if let Err(e) = self.process.send_line(&decline_line(id, method)) {
            log::warn!("codex's request could not be answered: {e}");
        }
    }

    /// The answer to a request of the session.
    fn answered(&mut self, purpose: Purpose, method: &'static str, result: Result<Value, RpcError>) {
        if matches!(purpose, Purpose::ThreadStart | Purpose::Inject) {
            self.awaiting = false;
        }
        match (purpose, result) {
            (Purpose::Initialize, Ok(value)) => {
                if let Err(e) = check_agent(&self.runtime, &value) {
                    self.fail(e);
                }
            }
            (Purpose::Initialize, Err(e)) => self.fail(refused(method, &e, ErrorKind::MissingRuntime)),
            (Purpose::Login, Ok(_)) => log::debug!("codex took the API key"),
            (Purpose::Login, Err(e)) => self.sign_in_problem = Some(refused(method, &e, ErrorKind::InvalidCredential)),
            (Purpose::AccountRead, Ok(value)) => {
                if value.get("account").is_none_or(Value::is_null) && self.sign_in_problem.is_none() {
                    let detail = match self.sign_in {
                        SignInKind::ApiKey => "codex holds no API key",
                        SignInKind::ChatGpt => "codex is not signed in with ChatGPT; sign in under Settings",
                    };
                    self.sign_in_problem = Some(AgentError::reported(ErrorKind::InvalidCredential, detail));
                }
                self.ready = true;
                self.proceed_or_fail();
            }
            (Purpose::AccountRead, Err(e)) => self.fail(refused(method, &e, ErrorKind::Other)),
            (Purpose::ThreadStart, Ok(value)) => match self.parser.thread_started(&value) {
                Ok(started) => {
                    if let SessionEvent::Started { session_id, .. } = &started {
                        self.thread = Thread::Ready(session_id.clone());
                    }
                    self.pending.push_back(started);
                    self.proceed_or_fail();
                }
                Err(why) => {
                    self.thread = Thread::None;
                    let event = self.parser.skip(why);
                    self.pending.push_back(event);
                    self.fail_turn(AgentError::reported(ErrorKind::Other, "codex started a thread without naming it"));
                }
            },
            (Purpose::ThreadStart, Err(e)) => {
                self.thread = Thread::None;
                self.fail_turn(refused(method, &e, ErrorKind::Other));
            }
            (Purpose::Inject, Ok(_)) => {
                self.inject = false;
                self.proceed_or_fail();
            }
            (Purpose::Inject, Err(e)) => {
                let error = if e.unknown_method() {
                    refused(method, &e, ErrorKind::Other)
                } else {
                    AgentError::reported(ErrorKind::Other, &format!("codex did not take the conversation's earlier messages: {}", e.message))
                };
                self.fail_turn(error);
            }
            (Purpose::TurnStart, Ok(value)) => {
                if let Some(id) = value.get("turn").and_then(|t| t.get("id")).and_then(Value::as_str) {
                    self.parser.turn_started(id);
                }
                if self.stop_requested {
                    self.interrupt();
                }
            }
            (Purpose::TurnStart, Err(e)) => self.fail_turn(refused(method, &e, ErrorKind::Other)),
            (Purpose::Interrupt, Ok(_)) => log::debug!("codex took the stop request"),
            (Purpose::Interrupt, Err(e)) => log::warn!("codex refused the stop request: {}", e.message),
        }
    }

    /// Requests that waited past their deadline.
    fn expire(&mut self, now: Instant) {
        for (_, method, purpose) in self.requests.expired(now) {
            if matches!(purpose, Purpose::ThreadStart | Purpose::Inject) {
                self.awaiting = false;
            }
            let error = AgentError::reported(ErrorKind::Other, &format!("codex did not answer {method} within {} ms", self.timeout.as_millis()));
            match purpose {
                Purpose::Initialize | Purpose::Login | Purpose::AccountRead => self.fail(error),
                Purpose::ThreadStart => {
                    self.thread = Thread::None;
                    self.fail_turn(error);
                }
                Purpose::Inject | Purpose::TurnStart => self.fail_turn(error),
                Purpose::Interrupt => log::warn!("{error}"),
            }
        }
    }

    /// Takes the next step of the waiting message; a step that cannot be sent fails the turn.
    fn proceed_or_fail(&mut self) {
        if let Err(e) = self.proceed() {
            self.fail_turn(e);
        }
    }

    /// The next step of the waiting message: nothing until the handshake is done; a turn that
    /// cannot sign in fails before any thread starts; then the thread, the earlier messages on a
    /// resume, and the turn.
    fn proceed(&mut self) -> Result<(), AgentError> {
        if !self.ready || !self.turn_open || self.turn_requested {
            return Ok(());
        }
        if self.awaiting {
            return Ok(());
        }
        if self.stop_requested {
            self.finish_turn(TurnOutcome::Stopped);
            return Ok(());
        }
        if let Some(problem) = self.sign_in_problem.clone() {
            self.fail_turn(problem);
            return Ok(());
        }
        match self.thread.clone() {
            Thread::Starting => Ok(()),
            Thread::None => {
                let mut params =
                    json!({ "cwd": self.folder.work().to_string_lossy(), "approvalPolicy": "on-request", "sandbox": "read-only", "ephemeral": true });
                if let Some(model) = &self.model {
                    params["model"] = json!(model.as_str());
                }
                if let Some(text) = &self.instructions {
                    params["developerInstructions"] = json!(text);
                }
                self.request("thread/start", Purpose::ThreadStart, params)?;
                self.thread = Thread::Starting;
                self.awaiting = true;
                Ok(())
            }
            Thread::Ready(thread) if self.inject => {
                let budget = self.max_line_bytes.saturating_sub(1024);
                // The message waiting is the newest entry: it goes in the turn, not before it.
                let items = self.history.items_before_last(budget);
                if items.is_empty() {
                    self.inject = false;
                    return self.proceed();
                }
                self.request("thread/inject_items", Purpose::Inject, json!({ "threadId": thread, "items": items }))?;
                self.awaiting = true;
                Ok(())
            }
            Thread::Ready(thread) => {
                let text = self.queued.take().unwrap_or_default();
                let mut params = json!({ "threadId": thread, "input": [{ "type": "text", "text": text, "text_elements": [] }] });
                if let Some(model) = &self.model {
                    params["model"] = json!(model.as_str());
                }
                self.request("turn/start", Purpose::TurnStart, params)?;
                self.turn_requested = true;
                Ok(())
            }
        }
    }

    /// Sends `turn/interrupt` for the running turn, once.
    fn interrupt(&mut self) {
        if self.interrupt_sent {
            return;
        }
        let (Some(thread), Some(turn)) = (self.thread_id().map(str::to_owned), self.parser.turn_id().map(str::to_owned)) else { return };
        match self.request("turn/interrupt", Purpose::Interrupt, json!({ "threadId": thread, "turnId": turn })) {
            Ok(_) => self.interrupt_sent = true,
            Err(e) => log::warn!("codex's stop request could not be sent: {e}"),
        }
    }

    /// The turn ended: the session takes the next message, and the copy is written.
    fn turn_closed(&mut self) {
        self.parser.reset_turn();
        self.turn_open = false;
        self.turn_requested = false;
        self.awaiting = false;
        self.stop_requested = false;
        self.interrupt_sent = false;
        self.queued = None;
        if let Err(e) = self.history.write(&self.folder) {
            log::warn!("the Codex conversation copy could not be written: {e}");
        }
    }

    /// Ends the open turn with `outcome`.
    fn finish_turn(&mut self, outcome: TurnOutcome) {
        if self.turn_open {
            self.turn_closed();
            self.pending.push_back(SessionEvent::TurnFinished { outcome, usage: Default::default() });
        }
    }

    /// Fails the open turn with `error`.
    fn fail_turn(&mut self, error: AgentError) {
        self.finish_turn(TurnOutcome::Failed(error));
    }

    /// A failure the session cannot go on from: the open turn fails, and the runtime's input is
    /// closed so it exits; [`SessionEvent::Ended`] carries the error.
    fn fail(&mut self, error: AgentError) {
        if self.fatal.is_none() {
            log::warn!("the Codex session cannot go on: {error}");
            self.fail_turn(error.clone());
            self.fatal = Some(error);
            self.process.close_input();
        }
    }

    /// The runtime's output closed: an open turn fails, then the session ends.
    fn finish(&mut self) {
        self.ended = true;
        let exit = self.exit_status();
        for (_, method, _) in self.requests.drain() {
            log::debug!("codex ended without answering {method}");
        }
        let error = if self.shutting_down {
            None
        } else if let Some(fatal) = self.fatal.take() {
            Some(fatal)
        } else {
            let tail = self.process.stderr_tail();
            Some(if !self.lines_seen && (tail.contains("unknown configuration field") || tail.contains("unexpected argument")) {
                // A runtime too old for a setting or a switch stops before its first line.
                AgentError::reported(ErrorKind::MissingRuntime, &format!("codex does not know a setting Archforge needs; update it. {}", tail.trim()))
            } else {
                match exit {
                    Some(exit) => self.process.exit_error(exit),
                    None => AgentError::reported(ErrorKind::Other, "codex closed its output and its exit was not seen"),
                }
            })
        };
        if self.turn_open {
            let failed = error.clone().unwrap_or_else(|| AgentError::reported(ErrorKind::Other, "codex ended before the turn did"));
            self.fail_turn(failed);
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
                    log::warn!("codex's exit could not be checked: {e}");
                    return None;
                }
            }
        }
    }
}

impl AgentSession for CodexSession {
    /// Starts a turn: the message waits for the handshake, then starts the thread (first message),
    /// gives it the earlier messages (on a resume), and goes out as `turn/start`.
    fn send(&mut self, text: &str) -> Result<(), AgentError> {
        if self.ended {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        if let Some(fatal) = &self.fatal {
            return Err(fatal.clone());
        }
        if self.turn_open {
            return Err(AgentError::TurnRunning { runtime: RuntimeKind::Codex });
        }
        self.turn_open = true;
        self.turn_requested = false;
        self.stop_requested = false;
        self.interrupt_sent = false;
        self.queued = Some(text.to_owned());
        let entry = self.process.redact(text.to_owned());
        self.history.push(history::Role::User, entry);
        if let Err(e) = self.proceed() {
            self.turn_open = false;
            self.queued = None;
            self.history.pop();
            return Err(e);
        }
        Ok(())
    }

    /// Sends `turn/interrupt` once Codex has named the turn (at once when it has); a message still
    /// waiting for its thread is dropped and its turn ends as stopped. Codex stays up. There is no
    /// signal fallback: SIGINT ends `codex app-server` instead of the turn (spike).
    fn stop(&mut self) -> Result<(), AgentError> {
        if !self.turn_open || self.ended {
            return Ok(());
        }
        self.stop_requested = true;
        self.parser.stop_requested();
        if self.turn_requested {
            self.interrupt();
        } else if !self.awaiting {
            self.finish_turn(TurnOutcome::Stopped);
        }
        Ok(())
    }

    /// Reads output lines until one means an event, the timeout passes, or a call's budget of lines
    /// is spent; requests past their deadline are failed on the way. When the output closes, the
    /// wait for the exit status may add up to half a second.
    fn next_event(&mut self, timeout: Duration) -> Option<SessionEvent> {
        let deadline = Instant::now().checked_add(timeout);
        let mut budget = LINES_PER_CALL;
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            if self.ended {
                return None;
            }
            self.expire(Instant::now());
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            let now = Instant::now();
            if budget == 0 || deadline.is_some_and(|d| now >= d) {
                return None;
            }
            budget -= 1;
            let mut wait = deadline.map_or(timeout, |d| d.saturating_duration_since(now));
            if let Some(due) = self.requests.next_deadline() {
                wait = wait.min(due.saturating_duration_since(now));
            }
            if let Some(event) = self.process.next_event(wait) {
                self.take(event);
            }
        }
    }

    fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        self.shutting_down = true;
        self.process.shutdown(grace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TOML strings cannot be ended or broken by any value.
    #[test]
    fn toml_strings_are_escaped() {
        assert_eq!(toml_string("Chat 1"), "\"Chat 1\"");
        assert_eq!(toml_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(toml_string("x\ny\u{7f}\u{0}"), "\"x\\u000Ay\\u007F\\u0000\"");
        assert_eq!(toml_string("é ü"), "\"é ü\"");
    }

    /// The version comes from the user agent's first word after the client name.
    #[test]
    fn the_agent_version_is_read_strictly() {
        assert_eq!(agent_version("archforge/0.166.0 (Mac OS 27.2.0; arm64) unknown (archforge; 0.13.0)"), Some(Version::new(0, 166, 0)));
        assert_eq!(agent_version("archforge/0.170.2"), Some(Version::new(0, 170, 2)));
        for bad in ["", "codex/0.166.0", "archforge/0.166", "archforge/v0.166.0 x", "archforge0.166.0"] {
            assert_eq!(agent_version(bad), None, "{bad}");
        }
    }

    /// Each approval gets the decline its schema takes; anything else an error answer.
    #[test]
    fn requests_from_codex_are_declined() {
        let id = json!(4);
        assert_eq!(decline_line(&id, "item/fileChange/requestApproval"), r#"{"id":4,"result":{"decision":"decline"}}"#);
        assert!(decline_line(&id, "mcpServer/elicitation/request").contains(r#""action":"decline""#));
        assert!(decline_line(&id, "execCommandApproval").contains("denied"));
        assert!(decline_line(&id, "item/tool/call").contains("-32601"));
        assert!(decline_line(&id, "x\ny").contains("does not handle xy"));
    }
}
