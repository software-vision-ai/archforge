//! The OpenAI Codex adapter: `codex app-server` sessions and sign-in over JSON-RPC (AR-202, after
//! the AR-196 spike in `docs/agent-runtimes/README.md`).
//!
//! - [`CodexSession`] drives one conversation: it starts `codex app-server` in the conversation's
//!   private folder, with Archforge's bridge as its only tool server, makes the handshake (signing in
//!   with the API key when the conversation uses one), and runs its turns on ephemeral threads: a
//!   thread serves turn after turn until a timeout abandons it, and the next turn starts another.
//!   Its text messages are kept in the conversation's copy ([`history`]), so a resumed conversation
//!   starts a new thread and hands it the copy.
//! - [`CodexAccount`] is a short-lived app-server for Settings: the account, ChatGPT sign-in in the
//!   browser or with a device code, an API key's check, sign-out, and the models.
//!
//! Where the sign-in lives (owner decisions): an API key goes to Codex over its standard input in
//! `account/login/start`, with `cli_auth_credentials_store="ephemeral"`, so it is never written to a
//! file, the keychain or the command line, and every process signs in again. A ChatGPT sign-in is
//! kept in Codex's own files (`"file"`) inside Archforge's private `CODEX_HOME` ([`CodexHome`], a
//! 0700 folder in the workspace), on every system (AR-236): Codex runs with a private `HOME` and an
//! emptied environment, where macOS finds no default keychain and the Secret Service's session bus
//! address is not passed on. Every process of a person shares that `CODEX_HOME`, as it holds the
//! sign-in; each conversation keeps its own home, working and temporary folders.

pub mod account;
pub mod history;
pub mod parse;
pub mod rpc;

use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use archforge_secrets::Secret;
use serde_json::{Value, json};
use zeroize::Zeroizing;

pub use account::{AccountEvent, AccountState, BrowserLogin, CancelStatus, CodexAccount, DeviceLogin, LoginId, LoginOutcome, ModelInfo};
pub use history::History;
pub use parse::{SignedInWith, kind_of};
pub use rpc::{EXPIRED_SIGN_IN_MESSAGES, RpcError};

pub use crate::claude::{Bridge, Model, Start};
use crate::session::{AgentSession, SessionEvent, Skipped, ToolServer, TurnOutcome};
use crate::{AgentError, ConversationFolder, ErrorKind, Exit, Limits, Runtime, RuntimeKind, SpawnRequest, Workspace, spawn};
use rpc::{Conn, Incoming, LINES_PER_CALL};

/// The folder in the workspace that is Archforge's private `CODEX_HOME`.
pub const HOME_FOLDER: &str = "codex-home";
/// The name of Archforge's tool server in Codex's configuration.
pub const SERVER_NAME: &str = "archforge";
/// The label of the API key in errors (the key never enters the environment).
pub const KEY_LABEL: &str = "OPENAI_API_KEY";
/// The shortest request timeout accepted.
pub const MIN_REQUEST_TIMEOUT: Duration = Duration::from_millis(10);
/// The longest request timeout accepted.
pub const MAX_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
/// The default request timeout.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The shortest line limit a session accepts: the longest message in a `turn/start` or an injected
/// item, written as JSON, must fit one line.
pub const MIN_SESSION_LINE_BYTES: usize = 8 * 1024 * 1024;
/// The longest instructions given to a thread, in bytes.
pub const MAX_INSTRUCTIONS_BYTES: usize = 64 * 1024;
/// How many request timeouts a timed-out `thread/start` or `turn/start` is remembered, so a late
/// answer's thread is still released.
pub const LATE_FACTOR: u32 = 10;
/// The most timed-out `thread/start` requests, and the most threads left by a timed-out
/// `turn/start`, remembered at once; past it the oldest is released or forgotten (logged).
pub const MAX_LATE: usize = 16;
/// The most tool server states held while the thread they may name is not answered yet.
pub const MAX_HELD_STATUSES: usize = 64;
/// The most bytes of held tool server states.
pub const MAX_HELD_BYTES: usize = 1024 * 1024;
/// The features `codex app-server` is started without (the spike's list): Archforge's bridge is the
/// model's only tool.
const DISABLED: [&str; 14] = [
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
];

/// Where Codex keeps a sign-in (`cli_auth_credentials_store`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStore {
    /// In the process's memory only: for an API key, which every process is given again.
    Ephemeral,
    /// Codex's own files in Archforge's private `CODEX_HOME` (mode 0700): for a ChatGPT sign-in.
    File,
}

impl CredentialStore {
    /// The value Codex takes.
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialStore::Ephemeral => "ephemeral",
            CredentialStore::File => "file",
        }
    }
}

/// How a session signs in.
#[derive(Debug)]
pub enum Auth {
    /// With an API key, sent in `account/login/start` during the handshake; the ephemeral store.
    ApiKey(Secret),
    /// With the ChatGPT sign-in already kept in Codex's own files in the private `CODEX_HOME`.
    ChatGpt,
}

impl Auth {
    /// The credential store the session's Codex is started with: the ephemeral store for an API key,
    /// Codex's own files for a ChatGPT sign-in, whatever secure store Archforge itself has.
    pub fn store(&self) -> CredentialStore {
        match self {
            Auth::ApiKey(_) => CredentialStore::Ephemeral,
            Auth::ChatGpt => CredentialStore::File,
        }
    }

    /// How the session is signed in, which decides what a refused credential means.
    pub fn signed_in_with(&self) -> SignedInWith {
        match self {
            Auth::ApiKey(_) => SignedInWith::ApiKey,
            Auth::ChatGpt => SignedInWith::ChatGpt,
        }
    }
}

/// Archforge's private `CODEX_HOME`: `codex-home` in the workspace, a real folder of this user
/// with mode 0700, shared by every Codex process of the person, as it holds the ChatGPT sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHome {
    path: PathBuf,
}

impl CodexHome {
    /// The folder in `workspace`, made with mode 0700 when missing and checked when present (a link,
    /// another user's folder or one others may enter is refused, never changed).
    pub fn open(workspace: &Workspace) -> Result<CodexHome, AgentError> {
        let path = workspace.root().join(HOME_FOLDER);
        crate::folder::ensure_private(&path, crate::sys::current_uid())?;
        Ok(CodexHome { path })
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Everything a Codex session is started with, but the sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConfig {
    /// [`Start::New`] for a new conversation (its folder must hold no copy yet), [`Start::Resume`]
    /// to continue the copy in its folder.
    pub start: Start,
    /// The model to ask for; Codex's default when `None`.
    pub model: Option<Model>,
    /// Archforge's tool server; none when the conversation has no agent access (or for a test
    /// message): Codex then starts with no tool server at all.
    pub bridge: Option<Bridge>,
    /// The thread's developer instructions, at most [`MAX_INSTRUCTIONS_BYTES`].
    pub instructions: Option<String>,
    /// The caps on the process's input and output; `max_line_bytes` at least
    /// [`MIN_SESSION_LINE_BYTES`].
    pub limits: Limits,
    /// How long each request may wait for its answer, [`MIN_REQUEST_TIMEOUT`] to
    /// [`MAX_REQUEST_TIMEOUT`].
    pub request_timeout: Duration,
}

/// The command line after `codex`: `app-server --strict-config`, the credential store, no history
/// file, no analytics, no web search, the spike's features switched off, and (for a session with
/// agent access) the bridge as the tool server `archforge` through `-c mcp_servers.archforge.*`: its
/// command, its arguments (`mcp --bridge <port> --token-file <file> [--read-only] --origin
/// <label>`) and the spike's timeouts. Refused: a relative or non-UTF-8 path, a port of 0, an origin
/// label `archforge-cli` would refuse, and a control character in any value.
pub fn command_line(store: CredentialStore, bridge: Option<&Bridge>) -> Result<Vec<OsString>, AgentError> {
    let mut args: Vec<String> = vec!["app-server".into(), "--strict-config".into()];
    for value in [
        format!("cli_auth_credentials_store={}", toml_string("credential store", store.as_str())?),
        "history.persistence=\"none\"".to_owned(),
        "analytics.enabled=false".to_owned(),
        "web_search=\"disabled\"".to_owned(),
    ] {
        args.push("-c".into());
        args.push(value);
    }
    for feature in DISABLED {
        args.push("--disable".into());
        args.push(feature.into());
    }
    if let Some(bridge) = bridge {
        for value in bridge_overrides(bridge)? {
            args.push("-c".into());
            args.push(value);
        }
    }
    Ok(args.into_iter().map(OsString::from).collect())
}

/// The `-c` values that give Codex Archforge's tool server.
fn bridge_overrides(bridge: &Bridge) -> Result<Vec<String>, AgentError> {
    let (cli, bridge_args) = crate::claude::bridge_command(bridge)?;
    let mut quoted = Vec::new();
    for arg in &bridge_args {
        quoted.push(toml_string("bridge argument", arg)?);
    }
    Ok(vec![
        format!("mcp_servers.{SERVER_NAME}.command={}", toml_string("archforge-cli path", &cli)?),
        format!("mcp_servers.{SERVER_NAME}.args=[{}]", quoted.join(",")),
        format!("mcp_servers.{SERVER_NAME}.startup_timeout_sec=10"),
        format!("mcp_servers.{SERVER_NAME}.tool_timeout_sec=60"),
    ])
}

/// `text` as a TOML basic string; a control character is refused (TOML would need it escaped and
/// none belongs in a path, a label or a name).
fn toml_string(what: &'static str, text: &str) -> Result<String, AgentError> {
    if text.chars().any(char::is_control) {
        return Err(setting(what, text, "it holds a control character"));
    }
    // With no control character left, JSON's escaping (of `"` and `\` alone) is TOML's.
    serde_json::to_string(text).map_err(|e| setting(what, text, &e.to_string()))
}

/// A refused setting, named by its length only: the value is never repeated.
fn setting(what: &'static str, value: &str, reason: &str) -> AgentError {
    AgentError::InvalidSetting { setting: what, bytes: value.len(), reason: reason.to_owned() }
}

/// Refuses a request timeout outside [`MIN_REQUEST_TIMEOUT`] to [`MAX_REQUEST_TIMEOUT`].
pub(crate) fn timeout_check(timeout: Duration) -> Result<(), AgentError> {
    if timeout < MIN_REQUEST_TIMEOUT || timeout > MAX_REQUEST_TIMEOUT {
        return Err(AgentError::InvalidLimit {
            limit: "request timeout (ms)",
            value: timeout.as_millis(),
            accepted: format!("{} to {}", MIN_REQUEST_TIMEOUT.as_millis(), MAX_REQUEST_TIMEOUT.as_millis()),
        });
    }
    Ok(())
}

/// Refuses an API key that holds anything but printable ASCII other than `"` and `\`, so it goes
/// into its JSON line exactly as it is, with no escaping and no copy; the key is never repeated.
pub(crate) fn checked_api_key(key: &Secret) -> Result<(), AgentError> {
    let fits = key.expose().bytes().all(|b| (0x21..=0x7e).contains(&b) && b != b'"' && b != b'\\');
    if fits {
        Ok(())
    } else {
        Err(AgentError::InvalidSetting {
            setting: "Codex API key",
            bytes: key.expose().len(),
            reason: "a key is printable ASCII without quotes or backslashes".to_owned(),
        })
    }
}

/// The API key's `account/login/start` line for request `id`, in a buffer of its final size that is
/// overwritten when dropped. The key must have passed [`checked_api_key`].
pub(crate) fn api_key_line(id: u64, key: &Secret) -> Zeroizing<String> {
    let mut line = Zeroizing::new(String::with_capacity(account::key_line_len(id, key.expose())));
    account::write_key_line(&mut line, id, key.expose());
    line
}

/// The handshake's `initialize`.
pub(crate) mod initialize {
    use serde_json::{Value, json};

    use crate::{AgentError, ErrorKind, Fit, RuntimeKind, Version};

    /// `initialize`'s parameters: Archforge's name and version, no experimental API.
    pub(crate) fn params() -> Value {
        json!({
            "clientInfo": { "name": "archforge", "title": "Archforge", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": false, "requestAttestation": false },
        })
    }

    /// Checks `initialize`'s answer: its `userAgent` (`archforge/<codex version> …`) must be text;
    /// the version it names must be in Codex's accepted range (a version newer than the tested one
    /// is logged, one outside the range refused as a missing runtime). A version that cannot be
    /// read is logged; `find` checked the program's own version already.
    pub(crate) fn check(answer: &Value) -> Result<(), AgentError> {
        let agent = answer.get("userAgent").and_then(Value::as_str).ok_or_else(|| AgentError::Unreadable {
            runtime: RuntimeKind::Codex,
            request: "initialize",
            reason: "its userAgent is missing or not text".to_owned(),
        })?;
        let version = agent.split_once('/').and_then(|(_, rest)| rest.split(' ').next()).and_then(Version::parse);
        let Some(version) = version else {
            log::warn!("codex's handshake names no version Archforge can read");
            return Ok(());
        };
        let range = RuntimeKind::Codex.range();
        match range.fit(version) {
            Fit::Tested => Ok(()),
            Fit::NewerThanTested => {
                log::warn!("codex {version} is newer than {}, the version Archforge was tested with", range.tested);
                Ok(())
            }
            Fit::TooOld | Fit::TooNew => Err(AgentError::reported(
                ErrorKind::MissingRuntime,
                &format!("codex says it is version {version}, outside the versions Archforge works with ({} up to {})", range.floor, range.below),
            )),
        }
    }
}

/// A request waiting for its answer, and what its answer or its timeout means.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Initialize,
    Login,
    ThreadStart,
    Inject { thread: String },
    TurnStart { turn: u64, thread: String },
    Interrupt { turn: u64 },
    Unsubscribe { thread: String },
    LateInterrupt { thread: String },
}

impl Call {
    /// The method the request was.
    fn method(&self) -> &'static str {
        match self {
            Call::Initialize => "initialize",
            Call::Login => "account/login/start",
            Call::ThreadStart => "thread/start",
            Call::Inject { .. } => "thread/inject_items",
            Call::TurnStart { .. } => "turn/start",
            Call::Interrupt { .. } | Call::LateInterrupt { .. } => "turn/interrupt",
            Call::Unsubscribe { .. } => "thread/unsubscribe",
        }
    }
}

/// A request in flight with its deadline.
#[derive(Debug)]
struct Pending {
    call: Call,
    sent: Instant,
    deadline: Instant,
}

/// The handshake's progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handshake {
    Initializing,
    SigningIn,
    Done,
}

/// The thread turns run on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ThreadState {
    /// None yet, or the last one was released.
    None,
    /// `thread/start` is in flight.
    Starting,
    /// Answered: the copy is being handed over, from message `next` on, in batches.
    Injecting { id: String, model: String, next: usize },
    /// Ready for turns.
    Ready { id: String },
}

/// The stop request of the open turn: at most one a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    None,
    /// Asked for while the turn has no id yet; sent once it is named.
    Wanted {
        at: Instant,
    },
    /// `turn/interrupt` is in flight.
    Sent,
    /// Refused or timed out: the turn has until `deadline` to end before the session is ended.
    Failed {
        deadline: Instant,
    },
}

/// The open turn.
struct Turn {
    /// The turn's number among this session's turns, which its stop request is tied to.
    number: u64,
    text: String,
    /// The thread `turn/start` went to, once sent.
    thread: Option<String>,
    sent: bool,
    accepted: bool,
    /// When Codex accepted the turn.
    accepted_at: Option<Instant>,
    /// Codex's id for the turn, once it named it.
    id: Option<String>,
    stop: Stop,
    last_error: Option<AgentError>,
    retries: u32,
}

impl fmt::Debug for Turn {
    /// Writes the state and the message's length, never the message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Turn")
            .field("number", &self.number)
            .field("text_bytes", &self.text.len())
            .field("sent", &self.sent)
            .field("accepted", &self.accepted)
            .field("named", &self.id.is_some())
            .field("stop", &self.stop)
            .finish()
    }
}

impl Turn {
    /// When a stop that waits for this turn's name gives up: one request timeout after the later of
    /// the stop and the turn's acceptance; `None` unless the turn is accepted and unnamed.
    fn unnamed_stop_due(&self, timeout: Duration) -> Option<Instant> {
        match (self.stop, self.accepted_at, &self.id) {
            (Stop::Wanted { at }, Some(accepted), None) => at.max(accepted).checked_add(timeout),
            _ => None,
        }
    }
}

/// A `thread/start` that timed out: a late answer's thread is released until `forget_at`.
#[derive(Debug)]
struct LateStart {
    request: u64,
    forget_at: Instant,
}

/// A thread left by a `turn/start` that timed out: kept until a late turn on it is interrupted,
/// released when the request is forgotten at `forget_at`.
#[derive(Debug)]
struct Retired {
    thread: String,
    request: u64,
    forget_at: Instant,
}

/// A running Codex conversation.
pub struct CodexSession {
    conn: Conn,
    copy_path: PathBuf,
    work: String,
    model: Option<Model>,
    instructions: Option<String>,
    signed_in_with: SignedInWith,
    request_timeout: Duration,
    max_line_bytes: usize,
    key: Option<Secret>,
    handshake: Handshake,
    thread: ThreadState,
    history: History,
    damaged_copy: Option<PathBuf>,
    calls: HashMap<u64, Pending>,
    late_starts: VecDeque<LateStart>,
    retired: VecDeque<Retired>,
    held: Vec<(Option<String>, ToolServer)>,
    held_bytes: usize,
    servers: Vec<ToolServer>,
    /// The model the ready thread runs, as Codex named it.
    thread_model: String,
    /// Why the last save of the copy failed, until one succeeds.
    unsaved: Option<AgentError>,
    turn: Option<Turn>,
    turns: u64,
    events: VecDeque<SessionEvent>,
    skipped: u64,
    ended: bool,
}

impl fmt::Debug for CodexSession {
    /// Writes the process and the state by counts and kinds, never a message, a key or a line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodexSession")
            .field("conn", &self.conn)
            .field("handshake", &self.handshake)
            .field("thread_ready", &matches!(self.thread, ThreadState::Ready { .. }))
            .field("history", &self.history)
            .field("calls", &self.calls.len())
            .field("turn", &self.turn.as_ref().map(|t| t.number))
            .field("key_held", &self.key.is_some())
            .field("skipped", &self.skipped)
            .field("ended", &self.ended)
            .finish()
    }
}

impl CodexSession {
    /// Starts `codex app-server` for the conversation in `folder` with Archforge's private
    /// `CODEX_HOME` (`home`) and `config`, signing in with `auth`, and sends `initialize`; it does
    /// not wait for anything. The rest of the handshake (`initialized`, then `account/login/start`
    /// with the key when `auth` is an API key) follows in [`AgentSession::next_event`]; the first
    /// thread starts with the first turn.
    ///
    /// [`Start::New`] refuses a folder that already holds a conversation copy (every new
    /// conversation gets a fresh folder); [`Start::Resume`] reads the copy there, setting a damaged
    /// one aside ([`CodexSession::damaged_copy`]).
    pub fn start(
        runtime: &Runtime,
        folder: &ConversationFolder,
        home: &CodexHome,
        config: CodexConfig,
        auth: Auth,
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
        if config.limits.max_line_bytes < MIN_SESSION_LINE_BYTES {
            return Err(AgentError::InvalidLimit {
                limit: "max_line_bytes for a Codex session",
                value: u128::try_from(config.limits.max_line_bytes).unwrap_or(u128::MAX),
                accepted: format!("{MIN_SESSION_LINE_BYTES} to {}", Limits::MAX_LINE_BYTES),
            });
        }
        timeout_check(config.request_timeout)?;
        if let Some(instructions) = &config.instructions
            && instructions.len() > MAX_INSTRUCTIONS_BYTES
        {
            return Err(setting("instructions", instructions, &format!("instructions are at most {MAX_INSTRUCTIONS_BYTES} bytes")));
        }
        let work = folder
            .work()
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| AgentError::Folder { path: folder.work().to_path_buf(), problem: "is not UTF-8 text, which Codex needs".to_owned() })?;
        if let Auth::ApiKey(key) = &auth {
            checked_api_key(key)?;
        }
        let copy_path = folder.path().join(history::COPY_FILE);
        let (history, damaged_copy) = match config.start {
            Start::New => match std::fs::symlink_metadata(&copy_path) {
                Ok(_) => {
                    return Err(AgentError::Folder {
                        path: copy_path,
                        problem:
                            "already holds a conversation copy; a new conversation starts in a fresh folder, and Start::Resume continues this one"
                                .to_owned(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (History::default(), None),
                Err(e) => return Err(AgentError::Folder { path: copy_path, problem: format!("cannot be checked for a conversation copy: {e}") }),
            },
            Start::Resume => History::load(&copy_path)?,
        };
        let store = auth.store();
        let signed_in_with = auth.signed_in_with();
        let (input_secrets, key) = match auth {
            Auth::ApiKey(key) => {
                // One copy scrubs the process's error tail; the other is sent once and dropped.
                let scrub = Secret::new(key.expose().to_owned()).map_err(|e| AgentError::InvalidSetting {
                    setting: "Codex API key",
                    bytes: 0,
                    reason: e.to_string(),
                })?;
                (vec![(KEY_LABEL.to_owned(), scrub)], Some(key))
            }
            Auth::ChatGpt => (Vec::new(), None),
        };
        let request = SpawnRequest {
            args: command_line(store, config.bridge.as_ref())?,
            input_secrets,
            config_dir: Some(home.path().to_path_buf()),
            limits: config.limits,
            ..SpawnRequest::default()
        };
        let process = spawn(runtime, folder, request)?;
        let mut session = CodexSession {
            conn: Conn::new(process),
            copy_path,
            work,
            model: config.model,
            instructions: config.instructions,
            signed_in_with,
            request_timeout: config.request_timeout,
            max_line_bytes: config.limits.max_line_bytes,
            key,
            handshake: Handshake::Initializing,
            thread: ThreadState::None,
            history,
            damaged_copy,
            calls: HashMap::new(),
            late_starts: VecDeque::new(),
            retired: VecDeque::new(),
            held: Vec::new(),
            held_bytes: 0,
            servers: Vec::new(),
            thread_model: String::new(),
            unsaved: None,
            turn: None,
            turns: 0,
            events: VecDeque::new(),
            skipped: 0,
            ended: false,
        };
        session.request(Call::Initialize, Some(initialize::params()))?;
        Ok(session)
    }

    /// Where a damaged copy found at the start was set aside; the conversation then went on without
    /// its earlier messages.
    pub fn damaged_copy(&self) -> Option<&Path> {
        self.damaged_copy.as_deref()
    }

    /// Why the last save of the conversation's copy failed, while no later save has succeeded:
    /// the copy on disk is behind the conversation then, and a resume would miss its newest
    /// messages.
    pub fn copy_error(&self) -> Option<&AgentError> {
        self.unsaved.as_ref()
    }

    /// The conversation's messages as the copy holds them.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// How many output lines were not understood so far.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Whether a turn is open (send is refused until it ends).
    pub fn turn_running(&self) -> bool {
        self.turn.is_some()
    }

    /// The runtime process, for its id and the end of its error stream.
    pub fn process(&self) -> &crate::RuntimeProcess {
        self.conn.process()
    }

    /// Sends request `call` with `params` and remembers it with its deadline.
    fn request(&mut self, call: Call, params: Option<Value>) -> Result<u64, AgentError> {
        let id = self.conn.request(call.method(), params)?;
        self.pending(id, call);
        Ok(id)
    }

    /// Remembers request `id` as `call`, due one request timeout from now.
    fn pending(&mut self, id: u64, call: Call) {
        let sent = Instant::now();
        let deadline = sent.checked_add(self.request_timeout).unwrap_or(sent);
        self.calls.insert(id, Pending { call, sent, deadline });
    }

    /// Ends the session for `reason`: an open turn fails with it at once, and Codex is ended within
    /// a bound ([`rpc::Conn::fail`]), its end carrying the reason.
    fn fail(&mut self, reason: AgentError) {
        if let Some(turn) = self.turn.take() {
            self.settle_history(&turn);
            self.events.push_back(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(reason.clone()), usage: Default::default() });
        }
        self.conn.fail(reason);
    }

    /// Keeps or restores the open turn's question as it ends, and saves the copy: a turn that was
    /// sent keeps it (Codex may have it), one never sent is restored.
    fn settle_history(&mut self, turn: &Turn) {
        if turn.sent || turn.accepted {
            self.history.keep();
            self.history.trim();
            self.save();
        } else {
            self.history.restore();
        }
    }

    /// Saves the copy. A failure is logged (the folder and the purpose, never the content) and kept
    /// for [`CodexSession::copy_error`] until a later save, at the next turn's acceptance or end,
    /// succeeds.
    fn save(&mut self) {
        match self.history.save(&self.copy_path) {
            Ok(()) => self.unsaved = None,
            Err(e) => {
                log::warn!("the conversation's copy could not be saved: {e}");
                self.unsaved = Some(e);
            }
        }
    }

    /// Ends the open turn with `outcome`.
    fn close_turn(&mut self, outcome: TurnOutcome) {
        if let Some(turn) = self.turn.take() {
            self.settle_history(&turn);
            self.events.push_back(SessionEvent::TurnFinished { outcome, usage: Default::default() });
        }
    }

    /// Moves the open turn on as far as it can go now: once the handshake is done, a thread is
    /// started when there is none, and the turn sent once one is ready. A request that cannot be
    /// sent is the error.
    fn advance(&mut self) -> Result<(), AgentError> {
        let waiting = self.turn.as_ref().is_some_and(|t| !t.sent);
        if !waiting || self.handshake != Handshake::Done {
            return Ok(());
        }
        match self.thread.clone() {
            ThreadState::None => {
                let mut params = json!({ "approvalPolicy": "on-request", "cwd": self.work, "ephemeral": true, "sandbox": "read-only" });
                if let Some(object) = params.as_object_mut() {
                    if let Some(model) = &self.model {
                        object.insert("model".to_owned(), json!(model.as_str()));
                    }
                    if let Some(instructions) = &self.instructions {
                        object.insert("developerInstructions".to_owned(), json!(instructions));
                    }
                }
                self.request(Call::ThreadStart, Some(params))?;
                self.thread = ThreadState::Starting;
                Ok(())
            }
            ThreadState::Starting | ThreadState::Injecting { .. } => Ok(()),
            ThreadState::Ready { id } => {
                let Some(turn) = self.turn.as_ref() else { return Ok(()) };
                let mut params = json!({ "threadId": id, "input": [{ "type": "text", "text": turn.text, "text_elements": [] }] });
                if let (Some(model), Some(object)) = (&self.model, params.as_object_mut()) {
                    object.insert("model".to_owned(), json!(model.as_str()));
                }
                let number = turn.number;
                self.request(Call::TurnStart { turn: number, thread: id.clone() }, Some(params))?;
                if let Some(turn) = self.turn.as_mut() {
                    turn.sent = true;
                    turn.thread = Some(id);
                }
                Ok(())
            }
        }
    }

    /// [`CodexSession::advance`] where nobody waits for its result: a failure fails the open turn,
    /// restoring the copy when it was never sent.
    fn advance_or_fail_turn(&mut self) {
        if let Err(e) = self.advance() {
            self.close_turn(TurnOutcome::Failed(e));
        }
    }

    /// The thread could not be started or prepared: it is forgotten with the tool server states
    /// held for it, and the turn waiting for it fails (its question was never sent, so the copy is
    /// restored).
    fn thread_failed(&mut self, error: AgentError) {
        self.thread = ThreadState::None;
        self.held.clear();
        self.held_bytes = 0;
        self.close_turn(TurnOutcome::Failed(error));
    }

    /// The thread is ready: its tool servers are reported with [`SessionEvent::Started`] and the
    /// waiting turn is sent.
    fn thread_ready(&mut self, id: String, model: String) {
        self.thread = ThreadState::Ready { id: id.clone() };
        self.thread_model = model.clone();
        self.events.push_back(SessionEvent::Started { session_id: id, model, tools: Vec::new(), servers: self.servers.clone() });
        self.advance_or_fail_turn();
    }

    /// Sends the next batch of the copy to thread `id` with `thread/inject_items`, from message
    /// `next`: as many messages as fit one line of half the line limit, at least one. When every
    /// message was sent, the thread is ready.
    fn inject(&mut self, id: String, model: String, next: usize) {
        let messages = self.history.before_turn();
        if next >= messages.len() {
            self.thread_ready(id, model);
            return;
        }
        let budget = self.max_line_bytes / 2;
        let mut items = Vec::new();
        let mut bytes = 0usize;
        let mut end = next;
        for message in messages.iter().skip(next) {
            let item = match message.role {
                history::Role::User => json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": message.text }] }),
                history::Role::Assistant => {
                    json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": message.text }] })
                }
            };
            let size = item.to_string().len();
            if !items.is_empty() && bytes.saturating_add(size) > budget {
                break;
            }
            bytes = bytes.saturating_add(size);
            items.push(item);
            end = end.saturating_add(1);
        }
        match self.request(Call::Inject { thread: id.clone() }, Some(json!({ "threadId": id, "items": items }))) {
            Ok(_) => self.thread = ThreadState::Injecting { id, model, next: end },
            Err(e) => {
                self.unsubscribe(&id);
                self.thread_failed(e);
            }
        }
    }

    /// Releases thread `id` with `thread/unsubscribe` (a failure to send is logged).
    fn unsubscribe(&mut self, id: &str) {
        if !self.conn.is_open() {
            return;
        }
        if let Err(e) = self.request(Call::Unsubscribe { thread: id.to_owned() }, Some(json!({ "threadId": id }))) {
            log::warn!("a codex thread could not be released: {e}");
        }
    }

    /// Sends the open turn's one stop request, `turn/interrupt`, tied to the turn's number. A
    /// request that cannot be sent counts as refused.
    fn send_interrupt(&mut self) {
        let Some(turn) = self.turn.as_ref() else { return };
        let (Some(thread), Some(turn_id)) = (turn.thread.clone(), turn.id.clone()) else { return };
        let number = turn.number;
        let sent = self.request(Call::Interrupt { turn: number }, Some(json!({ "threadId": thread, "turnId": turn_id })));
        if let Some(turn) = self.turn.as_mut() {
            turn.stop = match sent {
                Ok(_) => Stop::Sent,
                Err(e) => {
                    log::warn!("codex's stop request could not be sent: {e}");
                    Stop::Failed { deadline: deadline_after(self.request_timeout) }
                }
            };
        }
    }

    /// Whether `turn` is the open turn's number.
    fn is_current(&self, turn: u64) -> bool {
        self.turn.as_ref().is_some_and(|t| t.number == turn)
    }

    /// Handles everything due by `now`: requests past their deadline, a refused stop whose grace
    /// ran out, a stop waiting for an unnamed turn, and late requests to forget.
    fn tick(&mut self, now: Instant) {
        let due: Vec<u64> = self.calls.iter().filter(|(_, p)| p.deadline <= now).map(|(id, _)| *id).collect();
        for id in due {
            if let Some(pending) = self.calls.remove(&id) {
                self.timed_out(id, pending);
            }
        }
        if let Some((stop, unnamed_due)) = self.turn.as_ref().map(|t| (t.stop, t.unnamed_stop_due(self.request_timeout))) {
            match stop {
                Stop::Failed { deadline } if deadline <= now => {
                    self.fail(AgentError::reported(
                        ErrorKind::Other,
                        "codex did not stop the turn and may still be running it, so the session was ended; start it again with Start::Resume to go on with the conversation",
                    ));
                    return;
                }
                Stop::Wanted { .. } if unnamed_due.is_some_and(|due| due <= now) => {
                    // The turn never got a name to stop it by: its thread is released, which ends
                    // it, and the next turn starts a new one.
                    log::warn!("codex never named the turn being stopped; its thread is released");
                    if let ThreadState::Ready { id } = self.thread.clone() {
                        self.unsubscribe(&id);
                    }
                    self.thread = ThreadState::None;
                    self.close_turn(TurnOutcome::Stopped);
                }
                _ => {}
            }
        }
        while self.late_starts.front().is_some_and(|l| l.forget_at <= now) {
            self.late_starts.pop_front();
        }
        while let Some(at) = self.retired.iter().position(|r| r.forget_at <= now) {
            if let Some(retired) = self.retired.remove(at) {
                self.unsubscribe(&retired.thread);
            }
        }
    }

    /// The next moment [`CodexSession::tick`] has work, if any.
    fn next_timer(&self) -> Option<Instant> {
        let calls = self.calls.values().map(|p| p.deadline);
        let stop = self.turn.as_ref().and_then(|t| match t.stop {
            Stop::Failed { deadline } => Some(deadline),
            Stop::Wanted { .. } => t.unnamed_stop_due(self.request_timeout),
            _ => None,
        });
        let late = self.late_starts.iter().map(|l| l.forget_at);
        let retired = self.retired.iter().map(|r| r.forget_at);
        calls.chain(stop).chain(late).chain(retired).min()
    }

    /// What a request's timeout means.
    fn timed_out(&mut self, id: u64, pending: Pending) {
        let error = AgentError::TimedOut {
            runtime: RuntimeKind::Codex,
            request: pending.call.method(),
            after_ms: pending.deadline.saturating_duration_since(pending.sent).as_millis(),
        };
        match pending.call {
            Call::Initialize | Call::Login => self.fail(error),
            Call::ThreadStart => {
                if self.late_starts.len() >= MAX_LATE {
                    log::warn!("too many codex thread starts timed out; the oldest is forgotten, and a thread it starts late stays until codex ends");
                    self.late_starts.pop_front();
                }
                self.late_starts.push_back(LateStart { request: id, forget_at: late_deadline(self.request_timeout) });
                self.thread_failed(error);
            }
            Call::Inject { thread } => {
                self.unsubscribe(&thread);
                self.thread_failed(error);
            }
            Call::TurnStart { turn, thread } => {
                if !self.is_current(turn) {
                    return;
                }
                // Codex may have the turn: its question stays in the copy, and its thread is kept
                // only to stop a turn that starts late.
                if self.retired.len() >= MAX_LATE
                    && let Some(oldest) = self.retired.pop_front()
                {
                    self.unsubscribe(&oldest.thread);
                }
                self.retired.push_back(Retired { thread, request: id, forget_at: late_deadline(self.request_timeout) });
                self.thread = ThreadState::None;
                self.close_turn(TurnOutcome::Failed(error));
            }
            Call::Interrupt { turn } => {
                if let Some(open) = self.turn.as_mut()
                    && open.number == turn
                    && open.stop == Stop::Sent
                {
                    log::warn!("codex did not answer the stop request in time; the turn gets one more request timeout to end");
                    open.stop = Stop::Failed { deadline: deadline_after(self.request_timeout) };
                }
            }
            Call::Unsubscribe { .. } => log::warn!("codex did not answer a thread's release in time"),
            Call::LateInterrupt { thread } => self.unsubscribe(&thread),
        }
    }

    /// What one incoming message means.
    fn take(&mut self, incoming: Incoming) {
        match incoming {
            Incoming::Response { id, result } => self.answer(id, result),
            Incoming::Notification { method, params } => self.notification(&method, &params),
            Incoming::Request { id, method } => self.server_request(&id, &method),
            Incoming::Skipped(why) => {
                self.skipped = self.skipped.saturating_add(1);
                self.events.push_back(SessionEvent::Ignored(why));
            }
            Incoming::Ended { exit, error } => self.finish(exit, error),
        }
    }

    /// Codex has ended: an open turn fails with the end's reason, then [`SessionEvent::Ended`].
    fn finish(&mut self, exit: Option<Exit>, error: Option<AgentError>) {
        self.ended = true;
        self.calls.clear();
        if let Some(turn) = self.turn.take() {
            self.settle_history(&turn);
            let failed = error.clone().unwrap_or_else(|| AgentError::reported(ErrorKind::Other, "codex ended before the turn did"));
            self.events.push_back(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(failed), usage: Default::default() });
        }
        self.events.push_back(SessionEvent::Ended { exit, error });
    }

    /// What the answer to request `id` means.
    fn answer(&mut self, id: u64, result: Result<Value, RpcError>) {
        let Some(pending) = self.calls.remove(&id) else {
            self.late_answer(id, result);
            return;
        };
        let method = pending.call.method();
        match pending.call {
            Call::Initialize => match result.map_err(|e| e.into_error(method, None)).and_then(|answer| initialize::check(&answer)) {
                Ok(()) => self.initialized(),
                Err(e) => self.fail(e),
            },
            Call::Login => {
                let accepted = result.map_err(|e| e.into_error(method, Some(ErrorKind::InvalidCredential))).and_then(|answer| {
                    match answer.get("type").and_then(Value::as_str) {
                        Some("apiKey") => Ok(()),
                        _ => {
                            Err(AgentError::Unreadable { runtime: RuntimeKind::Codex, request: method, reason: "its type is not apiKey".to_owned() })
                        }
                    }
                });
                match accepted {
                    Ok(()) => {
                        self.handshake = Handshake::Done;
                        self.advance_or_fail_turn();
                    }
                    Err(e) => self.fail(e),
                }
            }
            Call::ThreadStart => match result {
                Ok(answer) => match rpc::checked_id(answer.get("thread").and_then(|t| t.get("id"))) {
                    Some(thread) => {
                        let model = answer.get("model").and_then(Value::as_str).map(|m| parse::capped(m, crate::claude::MAX_NAME_BYTES));
                        if model.is_none() {
                            log::warn!("codex's thread names no model; Started says none");
                        }
                        let model = model.unwrap_or_default();
                        let held = std::mem::take(&mut self.held);
                        self.held_bytes = 0;
                        self.servers.clear();
                        for (named, server) in held {
                            if named.as_deref() == Some(thread.as_str()) {
                                update_server(&mut self.servers, server);
                            }
                        }
                        self.inject(thread, model, 0);
                    }
                    None => self.thread_failed(AgentError::Unreadable {
                        runtime: RuntimeKind::Codex,
                        request: method,
                        reason: "its thread id is not readable".to_owned(),
                    }),
                },
                Err(e) => self.thread_failed(e.into_error(method, None)),
            },
            Call::Inject { thread } => match (result, self.thread.clone()) {
                (Ok(_), ThreadState::Injecting { id, model, next }) if id == thread => self.inject(id, model, next),
                (Ok(_), _) => log::debug!("an injection answer came for a thread no longer being prepared"),
                (Err(e), _) => {
                    self.unsubscribe(&thread);
                    self.thread_failed(e.into_error(method, None));
                }
            },
            Call::TurnStart { turn, thread } => {
                if !self.is_current(turn) {
                    return;
                }
                match result {
                    Ok(answer) => {
                        self.history.keep();
                        self.save();
                        let named = rpc::checked_id(answer.get("turn").and_then(|t| t.get("id")));
                        let mut interrupt = false;
                        if let Some(open) = self.turn.as_mut() {
                            open.accepted = true;
                            open.accepted_at = Some(Instant::now());
                            open.thread = Some(thread);
                            if open.id.is_none() {
                                open.id = named;
                            }
                            interrupt = matches!(open.stop, Stop::Wanted { .. }) && open.id.is_some();
                        }
                        if interrupt {
                            self.send_interrupt();
                        }
                    }
                    Err(e) => {
                        let error = e.into_error(method, None);
                        self.history.restore();
                        if let Some(open) = self.turn.as_mut() {
                            open.sent = false;
                        }
                        self.close_turn(TurnOutcome::Failed(error));
                    }
                }
            }
            Call::Interrupt { turn } => {
                let Err(e) = result else { return };
                if let Some(open) = self.turn.as_mut()
                    && open.number == turn
                    && open.stop == Stop::Sent
                {
                    log::warn!("codex refused the stop request ({}); the turn gets one more request timeout to end", e.message);
                    open.stop = Stop::Failed { deadline: deadline_after(self.request_timeout) };
                }
            }
            Call::Unsubscribe { .. } => {
                if let Err(e) = result {
                    log::debug!("codex refused a thread's release: {}", e.message);
                }
            }
            Call::LateInterrupt { thread } => self.unsubscribe(&thread),
        }
    }

    /// The handshake's `initialize` was answered: `initialized` goes out, then the key's sign-in
    /// when the session uses one.
    fn initialized(&mut self) {
        if let Err(e) = self.conn.notify("initialized") {
            self.fail(e);
            return;
        }
        match self.key.take() {
            Some(key) => {
                // The key is sent in this one line and then dropped, which overwrites it.
                match self.conn.request_secret(|id| api_key_line(id, &key)) {
                    Ok(id) => {
                        self.pending(id, Call::Login);
                        self.handshake = Handshake::SigningIn;
                    }
                    Err(e) => self.fail(e),
                }
            }
            None => {
                self.handshake = Handshake::Done;
                self.advance_or_fail_turn();
            }
        }
    }

    /// An answer to a request no longer waited for: the thread a late `thread/start` names is
    /// released; a late `turn/start` on a retired thread gets its turn stopped, then the thread is
    /// released (at once when the answer names no turn).
    fn late_answer(&mut self, id: u64, result: Result<Value, RpcError>) {
        if let Some(at) = self.late_starts.iter().position(|l| l.request == id) {
            self.late_starts.remove(at);
            if let Ok(answer) = result {
                match rpc::checked_id(answer.get("thread").and_then(|t| t.get("id"))) {
                    Some(thread) => self.unsubscribe(&thread),
                    None => log::warn!("a thread codex started late has no readable id, so it is not released"),
                }
            }
            return;
        }
        if let Some(at) = self.retired.iter().position(|r| r.request == id) {
            let named = result.ok().and_then(|answer| rpc::checked_id(answer.get("turn").and_then(|t| t.get("id"))));
            match named {
                Some(turn_id) => self.interrupt_retired(at, &turn_id),
                None => {
                    if let Some(retired) = self.retired.remove(at) {
                        self.unsubscribe(&retired.thread);
                    }
                }
            }
            return;
        }
        log::debug!("codex answered request {id} after it was given up; the answer is dropped");
    }

    /// Stops the late turn `turn_id` on the retired thread at `at`, which leaves the list, so it is
    /// stopped once; the thread is released when the stop is answered or times out.
    fn interrupt_retired(&mut self, at: usize, turn_id: &str) {
        let Some(retired) = self.retired.remove(at) else { return };
        let thread = retired.thread;
        if self.request(Call::LateInterrupt { thread: thread.clone() }, Some(json!({ "threadId": thread, "turnId": turn_id }))).is_err() {
            self.unsubscribe(&thread);
        }
    }

    /// Whether a notification for `thread` (and `turn`, where it names one) belongs to the open turn.
    fn for_open_turn(&self, thread: Option<&str>, turn: Option<&str>) -> bool {
        let Some(open) = &self.turn else { return false };
        let thread_matches = open.thread.as_deref().is_some_and(|t| Some(t) == thread);
        let turn_matches = match (&open.id, turn) {
            (Some(id), Some(turn)) => id == turn,
            _ => true,
        };
        open.sent && thread_matches && turn_matches
    }

    /// What a notification means.
    fn notification(&mut self, method: &str, params: &Value) {
        let thread = parse::thread_of(params);
        let turn_id = parse::turn_of(params);
        match method {
            "turn/started" => {
                if let Some(at) = self.retired.iter().position(|r| Some(r.thread.as_str()) == thread.as_deref()) {
                    if let Some(turn_id) = &turn_id {
                        self.interrupt_retired(at, turn_id);
                    }
                    return;
                }
                if self.for_open_turn(thread.as_deref(), turn_id.as_deref()) {
                    let mut interrupt = false;
                    if let Some(open) = self.turn.as_mut()
                        && open.id.is_none()
                    {
                        open.id = turn_id;
                        interrupt = matches!(open.stop, Stop::Wanted { .. }) && open.id.is_some();
                    }
                    if interrupt {
                        self.send_interrupt();
                    }
                }
            }
            "turn/completed" => {
                if let Some(at) = self.retired.iter().position(|r| Some(r.thread.as_str()) == thread.as_deref()) {
                    if let Some(retired) = self.retired.remove(at) {
                        self.unsubscribe(&retired.thread);
                    }
                    return;
                }
                if self.for_open_turn(thread.as_deref(), turn_id.as_deref()) {
                    let Some(open) = self.turn.as_mut() else { return };
                    let stop_requested = open.stop != Stop::None;
                    let last_error = open.last_error.take();
                    let outcome = parse::turn_completed(params.get("turn").unwrap_or(&Value::Null), last_error, stop_requested, self.signed_in_with);
                    open.accepted = true;
                    self.close_turn(outcome);
                }
            }
            "item/agentMessage/delta" => {
                if self.for_open_turn(thread.as_deref(), turn_id.as_deref())
                    && let Some(event) = parse::delta(params)
                {
                    self.events.push_back(event);
                }
            }
            "item/started" => {
                if self.for_open_turn(thread.as_deref(), turn_id.as_deref())
                    && let Some(event) = parse::item_started(params)
                {
                    self.events.push_back(event);
                }
            }
            "item/completed" => {
                if self.for_open_turn(thread.as_deref(), turn_id.as_deref())
                    && let Some((event, answer)) = parse::item_completed(params)
                {
                    if let Some(text) = answer
                        && self.turn.as_ref().is_some_and(|t| t.sent)
                    {
                        self.history.push_answer(&text);
                    }
                    self.events.push_back(event);
                }
            }
            "error" => {
                if !self.for_open_turn(thread.as_deref(), turn_id.as_deref()) {
                    return;
                }
                let Some(error) = params.get("error").and_then(|e| parse::failure(e, self.signed_in_with)) else { return };
                let Some(open) = self.turn.as_mut() else { return };
                if params.get("willRetry").and_then(Value::as_bool) == Some(true) {
                    open.retries = open.retries.saturating_add(1);
                    let kind = error.kind();
                    self.events.push_back(SessionEvent::Retrying { attempt: open.retries, max_attempts: None, delay_ms: None, kind });
                } else {
                    open.last_error = Some(error);
                }
            }
            "mcpServer/startupStatus/updated" => self.server_status(params),
            "account/login/completed" => {
                if params.get("success").and_then(Value::as_bool) == Some(false) {
                    self.fail(AgentError::reported(ErrorKind::InvalidCredential, "codex did not accept the API key"));
                }
            }
            "account/updated" => {
                if params.get("authMode").is_some_and(Value::is_null) {
                    log::warn!("codex says it is no longer signed in");
                }
            }
            "thread/started"
            | "thread/status/changed"
            | "thread/tokenUsage/updated"
            | "thread/name/updated"
            | "thread/settings/updated"
            | "thread/compacted"
            | "thread/closed"
            | "remoteControl/status/changed"
            | "account/rateLimits/updated"
            | "serverRequest/resolved"
            | "turn/diff/updated"
            | "turn/plan/updated"
            | "item/plan/delta"
            | "item/mcpToolCall/progress"
            | "item/reasoning/summaryTextDelta"
            | "item/reasoning/summaryPartAdded"
            | "item/reasoning/textDelta"
            | "item/commandExecution/outputDelta"
            | "item/fileChange/outputDelta"
            | "hook/started"
            | "hook/completed"
            | "model/rerouted"
            | "modelProvider/authRecoveryStarted"
            | "modelProvider/authRecoveryCompleted"
            | "warning"
            | "configWarning"
            | "deprecationNotice" => {}
            other => {
                let kind = rpc::kind_name(other);
                log::debug!("codex sent the notification {kind}, which Archforge does not read; it was skipped");
                self.skipped = self.skipped.saturating_add(1);
                self.events.push_back(SessionEvent::Ignored(Skipped::Unknown { kind }));
            }
        }
    }

    /// A tool server's state: held while `thread/start` is not answered yet (at most
    /// [`MAX_HELD_STATUSES`] and [`MAX_HELD_BYTES`], the rest dropped and logged), then delivered only
    /// when it names the answered thread; reported with [`SessionEvent::Started`] again once the
    /// thread is ready.
    fn server_status(&mut self, params: &Value) {
        let Some((named, server)) = parse::server_status(params) else {
            self.skipped = self.skipped.saturating_add(1);
            self.events
                .push_back(SessionEvent::Ignored(Skipped::Malformed { kind: "mcpServer/startupStatus/updated", reason: "it has no name or state" }));
            return;
        };
        match self.thread.clone() {
            ThreadState::Starting => {
                let size = server.name.len().saturating_add(server.status.len()).saturating_add(named.as_ref().map_or(0, String::len));
                if self.held.len() >= MAX_HELD_STATUSES || self.held_bytes.saturating_add(size) > MAX_HELD_BYTES {
                    log::warn!("too many tool server states came before codex's thread; one was dropped");
                    return;
                }
                self.held_bytes = self.held_bytes.saturating_add(size);
                self.held.push((named, server));
            }
            ThreadState::Injecting { id, .. } if named.as_deref() == Some(id.as_str()) => update_server(&mut self.servers, server),
            ThreadState::Ready { id } if named.as_deref() == Some(id.as_str()) => {
                update_server(&mut self.servers, server);
                let model = self.thread_model.clone();
                self.events.push_back(SessionEvent::Started { session_id: id, model, tools: Vec::new(), servers: self.servers.clone() });
            }
            _ => log::debug!("a tool server state for another thread was skipped"),
        }
    }

    /// Answers a request of Codex's own: approvals and elicitations are declined (Archforge's bridge
    /// is the only tool and runs read-only unless the person allowed changes), a permission request
    /// gets an empty grant, and anything else a JSON-RPC error.
    fn server_request(&mut self, id: &Value, method: &str) {
        let answered = match method {
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => self.conn.respond(id, json!({ "decision": "decline" })),
            "mcpServer/elicitation/request" => self.conn.respond(id, json!({ "action": "decline", "content": null, "_meta": null })),
            "item/permissions/requestApproval" => self.conn.respond(id, json!({ "permissions": {}, "scope": "turn" })),
            other => {
                log::warn!("codex asked {}, which Archforge does not handle; refused", rpc::kind_name(other));
                self.conn.respond_error(id, rpc::METHOD_NOT_FOUND, "Archforge does not handle this request")
            }
        };
        if let Err(e) = answered {
            log::warn!("codex's request could not be answered: {e}");
        }
    }
}

/// Replaces the state of the server with `server`'s name, or adds it (at most
/// [`MAX_HELD_STATUSES`] servers).
fn update_server(servers: &mut Vec<ToolServer>, server: ToolServer) {
    if let Some(known) = servers.iter_mut().find(|s| s.name == server.name) {
        known.status = server.status;
    } else if servers.len() < MAX_HELD_STATUSES {
        servers.push(server);
    } else {
        log::warn!("codex reports more tool servers than Archforge keeps; one was dropped");
    }
}

/// One request timeout from now.
fn deadline_after(timeout: Duration) -> Instant {
    let now = Instant::now();
    now.checked_add(timeout).unwrap_or(now)
}

/// [`LATE_FACTOR`] request timeouts from now.
fn late_deadline(timeout: Duration) -> Instant {
    deadline_after(timeout.saturating_mul(LATE_FACTOR))
}

impl AgentSession for CodexSession {
    /// Opens a turn with the person's message: the copy's snapshot is taken and the question added,
    /// then `turn/start` goes out at once when the thread is ready, or after the handshake and the
    /// thread's start (`thread/start`, then the copy in `thread/inject_items` batches) otherwise.
    /// Refused while a turn is open, once Codex has ended, and for an empty message or one past
    /// [`history::MAX_MESSAGE_BYTES`]. A request that cannot be sent restores the copy.
    fn send(&mut self, text: &str) -> Result<(), AgentError> {
        if self.ended || !self.conn.is_open() {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        if self.turn.is_some() {
            return Err(AgentError::TurnRunning { runtime: RuntimeKind::Codex });
        }
        if text.is_empty() {
            return Err(AgentError::InvalidSetting { setting: "message", bytes: 0, reason: "a message is not empty".to_owned() });
        }
        self.history.begin_turn(text)?;
        self.turns = self.turns.saturating_add(1);
        self.turn = Some(Turn {
            number: self.turns,
            text: text.to_owned(),
            thread: None,
            sent: false,
            accepted: false,
            accepted_at: None,
            id: None,
            stop: Stop::None,
            last_error: None,
            retries: 0,
        });
        if let Err(e) = self.advance() {
            self.history.restore();
            self.turn = None;
            return Err(e);
        }
        Ok(())
    }

    /// Asks Codex to stop the open turn with one `turn/interrupt`, tied to the turn's number; a
    /// second stop of the same turn does nothing more. A turn not sent yet ends at once as stopped;
    /// one Codex has not named yet is stopped as soon as it is named, or, when it stays unnamed for a
    /// request timeout after being accepted, its thread is released. When Codex refuses the stop or
    /// does not answer in time, the turn gets one more request timeout to end (send stays refused);
    /// then the session is ended, naming [`Start::Resume`]. A stop answer for a turn that is no
    /// longer open is ignored.
    fn stop(&mut self) -> Result<(), AgentError> {
        if self.ended {
            return Ok(());
        }
        let Some((stop, sent, named)) = self.turn.as_ref().map(|t| (t.stop, t.sent, t.id.is_some())) else { return Ok(()) };
        if stop != Stop::None {
            return Ok(());
        }
        if !sent {
            self.close_turn(TurnOutcome::Stopped);
        } else if named {
            self.send_interrupt();
        } else if let Some(turn) = self.turn.as_mut() {
            turn.stop = Stop::Wanted { at: Instant::now() };
        }
        Ok(())
    }

    /// The next event, never waiting past `timeout` (an exit wait is at most the smaller of half a
    /// second and the time left) and reading at most [`rpc::LINES_PER_CALL`] lines: with a zero
    /// timeout every line already there is read within that budget, without waiting, so an event
    /// behind lines that mean nothing still comes. Requests past their timeout are handled on the
    /// way. Once Codex has ended it is collected here without waiting. The one thing a timeout does
    /// not cut short: when a turn is accepted or ends, the conversation's copy is written within the
    /// call (one file of at most [`history::LARGEST_COPY_BYTES`], synchronised with its folder).
    fn next_event(&mut self, timeout: Duration) -> Option<SessionEvent> {
        let deadline = rpc::deadline_in(timeout);
        let mut budget = LINES_PER_CALL;
        loop {
            if let Some(event) = self.events.pop_front() {
                return Some(event);
            }
            if self.ended {
                self.conn.reap();
                return None;
            }
            let now = Instant::now();
            self.tick(now);
            if !self.events.is_empty() {
                continue;
            }
            let wake = self.next_timer().map_or(deadline, |t| t.max(now.checked_add(Duration::from_millis(1)).unwrap_or(now)).min(deadline));
            match self.conn.poll(wake, &mut budget) {
                Some(incoming) => self.take(incoming),
                None => {
                    if budget == 0 || Instant::now() >= deadline {
                        self.tick(Instant::now());
                        return self.events.pop_front();
                    }
                }
            }
        }
    }

    /// Ends Codex as [`crate::RuntimeProcess::shutdown`] does, waiting up to twice `grace`; an open
    /// turn then fails and [`SessionEvent::Ended`] follows from [`AgentSession::next_event`].
    fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        self.conn.shutdown(grace)
    }
}

impl Drop for CodexSession {
    /// Collects Codex if it has ended; otherwise the process's drop kills its group and collects it.
    fn drop(&mut self) {
        self.conn.reap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command line carries the store, the switches and the bridge, read-only unless changes
    /// are allowed, and refuses values TOML would need escaped.
    #[test]
    fn the_command_line_carries_the_store_and_the_bridge() {
        let bridge = Bridge {
            cli: PathBuf::from("/opt/archforge/bin/archforge-cli"),
            port: 47000,
            origin: "Chat 1".to_owned(),
            allow_changes: false,
            token_file: PathBuf::from("/Users/person/Library/Application Support/Archforge/control-47000-0123456789abcdef.token"),
        };
        let args: Vec<String> = command_line(Auth::ChatGpt.store(), Some(&bridge)).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert_eq!(&args[..4], ["app-server", "--strict-config", "-c", "cli_auth_credentials_store=\"file\""]);
        for feature in DISABLED {
            assert!(args.windows(2).any(|w| w[0] == "--disable" && w[1] == feature), "{feature}");
        }
        assert!(args.contains(&"mcp_servers.archforge.command=\"/opt/archforge/bin/archforge-cli\"".to_owned()), "{args:?}");
        assert!(
            args.contains(
                &"mcp_servers.archforge.args=[\"mcp\",\"--bridge\",\"47000\",\"--token-file\",\"/Users/person/Library/Application Support/Archforge/control-47000-0123456789abcdef.token\",\"--read-only\",\"--origin\",\"Chat 1\"]"
                    .to_owned()
            ),
            "{args:?}"
        );
        assert!(!args.iter().any(|a| a.contains("mcp_servers.archforge.env")), "the bridge needs no home of its own: {args:?}");
        let allowed = Bridge { allow_changes: true, ..bridge.clone() };
        let args: Vec<String> =
            command_line(CredentialStore::Ephemeral, Some(&allowed)).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(!args.iter().any(|a| a.contains("--read-only")));
        assert!(args.contains(&"cli_auth_credentials_store=\"ephemeral\"".to_owned()));
        let account: Vec<String> = command_line(CredentialStore::File, None).unwrap().into_iter().map(|a| a.into_string().unwrap()).collect();
        assert!(!account.iter().any(|a| a.contains("mcp_servers")));
        for bad in [
            Bridge { cli: PathBuf::from("archforge-cli"), ..bridge.clone() },
            Bridge { port: 0, ..bridge.clone() },
            Bridge { origin: "-x".into(), ..bridge.clone() },
            Bridge { token_file: PathBuf::from("control.token"), ..bridge.clone() },
            Bridge { cli: PathBuf::from("/opt/a\u{7}b"), ..bridge.clone() },
        ] {
            assert!(matches!(command_line(CredentialStore::File, Some(&bad)), Err(AgentError::InvalidSetting { .. })), "{bad:?}");
        }
        assert_eq!(toml_string("x", "a\"b\\c").unwrap(), "\"a\\\"b\\\\c\"");
    }

    /// Request timeouts out of range and keys JSON would escape are refused, never repeated.
    #[test]
    fn timeouts_and_keys_are_checked() {
        assert!(timeout_check(DEFAULT_REQUEST_TIMEOUT).is_ok());
        assert!(timeout_check(Duration::from_millis(1)).is_err());
        assert!(timeout_check(MAX_REQUEST_TIMEOUT + Duration::from_millis(1)).is_err());
        assert!(checked_api_key(&Secret::new("sk-proj-AbC_123-xyz".into()).unwrap()).is_ok());
        for bad in ["sk \"quoted\"", "sk\\x", "sk\nline", "sk é"] {
            let error = checked_api_key(&Secret::new(bad.into()).unwrap()).unwrap_err();
            assert!(!format!("{error:?}{error}").contains(bad), "{error:?}");
        }
    }

    /// The handshake's version check accepts the tested and newer versions and refuses others.
    #[test]
    fn the_handshake_version_is_checked() {
        let agent = |v: &str| json!({ "userAgent": format!("archforge/{v} (Mac OS 27.2.0; arm64) unknown (archforge; 0.13.0)") });
        assert!(initialize::check(&agent("0.166.0")).is_ok());
        assert!(initialize::check(&agent("0.170.2")).is_ok());
        assert_eq!(initialize::check(&agent("0.165.9")).unwrap_err().kind(), ErrorKind::MissingRuntime);
        assert_eq!(initialize::check(&agent("1.0.0")).unwrap_err().kind(), ErrorKind::MissingRuntime);
        assert!(initialize::check(&json!({ "userAgent": "something else" })).is_ok());
        assert!(matches!(initialize::check(&json!({})), Err(AgentError::Unreadable { .. })));
    }

    /// `Debug` of a session's sign-in never shows the key.
    #[test]
    fn debug_never_shows_the_key() {
        let auth = Auth::ApiKey(Secret::new("sk-SECRET-KEY-123".into()).unwrap());
        for shown in [format!("{auth:?}"), format!("{auth:#?}")] {
            assert!(!shown.contains("sk-SECRET"), "{shown}");
        }
    }
}
