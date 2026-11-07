//! A short-lived `codex app-server` for Settings: what account Codex is signed in with, signing in
//! with an API key or ChatGPT (in the browser or with a device code), signing out, and the models
//! offered. It runs in a scratch folder of its own (`codex-<pid>-<n>` in the workspace, removed when
//! it is dropped) with Archforge's private `CODEX_HOME`, where the ChatGPT sign-in is kept.
//!
//! Every call blocks its caller up to the smaller of its `timeout` and the request timeout for each
//! request it makes, so it belongs on a worker thread; no call waits past its caller's deadline.

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::rpc::{Conn, Incoming, LINES_PER_CALL, RpcError, deadline_in};
use super::{CodexHome, CredentialStore, checked_api_key, initialize, timeout_check};
use crate::folder::ScratchFolder;
use crate::{AgentError, ErrorKind, Exit, Limits, Runtime, RuntimeKind, SpawnRequest, Workspace, spawn};

/// The most events an account process keeps for [`CodexAccount::next_event`]; past it the oldest go
/// (logged).
const MAX_QUEUED_EVENTS: usize = 256;
/// The longest sign-in address taken from Codex, in bytes.
pub const MAX_URL_BYTES: usize = 8 * 1024;
/// The longest device code taken from Codex, in bytes.
pub const MAX_USER_CODE_BYTES: usize = 32;
/// The longest model list cursor passed back to Codex, in bytes.
pub const MAX_CURSOR_BYTES: usize = 64 * 1024;
/// The most model list pages read for one [`CodexAccount::models`].
pub const MAX_MODEL_PAGES: usize = 64;
/// The most models one [`CodexAccount::models`] answers.
pub const MAX_MODELS: usize = 4096;
/// The longest email, plan or model text kept, in bytes.
const MAX_TEXT_BYTES: usize = 320;
/// The most sign-ins whose start timed out that are remembered, to cancel one that starts late.
const MAX_LATE_LOGINS: usize = 16;
/// The error text Codex 0.166.0 gives a sign-in cancelled with `account/login/cancel`.
pub const LOGIN_NOT_COMPLETED: &str = "Login was not completed";
/// The error text Codex 0.166.0 gives a browser sign-in cancelled with `account/login/cancel`: its
/// login server's error, prefixed (`codex-rs/app-server` and `codex-rs/login/src/server.rs`).
pub const BROWSER_LOGIN_NOT_COMPLETED: &str = "Login server error: Login was not completed";

/// The id Codex gives a sign-in in progress: a UUID, checked before it is ever sent back.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LoginId(String);

impl LoginId {
    /// `raw` when it is a UUID (`8-4-4-4-12` hexadecimal digits, either case), as Codex makes its
    /// login ids; anything else is `None`. The value is kept exactly as given, never cut.
    pub fn checked(raw: &str) -> Option<LoginId> {
        let groups: Vec<&str> = raw.split('-').collect();
        let lengths = [8, 4, 4, 4, 12];
        let shaped = raw.len() == 36
            && groups.len() == lengths.len()
            && groups.iter().zip(lengths).all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()));
        shaped.then(|| LoginId(raw.to_owned()))
    }

    /// The id as Codex gave it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for LoginId {
    /// Writes the id's length only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LoginId({} bytes)", self.0.len())
    }
}

/// A ChatGPT sign-in in the browser: open `auth_url`; Codex receives the answer on its local port
/// and reports the end with [`AccountEvent::LoginCompleted`].
#[derive(Clone, PartialEq, Eq)]
pub struct BrowserLogin {
    pub login_id: LoginId,
    pub auth_url: String,
}

impl fmt::Debug for BrowserLogin {
    /// Writes lengths only: the address carries the sign-in's state.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserLogin").field("login_id", &self.login_id).field("auth_url_bytes", &self.auth_url.len()).finish()
    }
}

/// A ChatGPT sign-in with a device code: show `verification_url` and `user_code`; Codex reports the
/// end with [`AccountEvent::LoginCompleted`].
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceLogin {
    pub login_id: LoginId,
    pub verification_url: String,
    pub user_code: String,
}

impl fmt::Debug for DeviceLogin {
    /// Writes lengths only: the code signs the person in.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceLogin")
            .field("login_id", &self.login_id)
            .field("verification_url_bytes", &self.verification_url.len())
            .field("user_code_bytes", &self.user_code.len())
            .finish()
    }
}

/// What Codex is signed in with.
#[derive(Clone, PartialEq, Eq)]
pub enum Account {
    /// An API key.
    ApiKey,
    /// A ChatGPT account, its email and plan as Codex says them (cut short).
    ChatGpt { email: Option<String>, plan: Option<String> },
    /// Another kind Archforge does not offer; `kind` is its name, cleaned.
    Other { kind: String },
}

impl fmt::Debug for Account {
    /// Writes the kind and plan; the email by its length only.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Account::ApiKey => f.write_str("ApiKey"),
            Account::ChatGpt { email, plan } => {
                f.debug_struct("ChatGpt").field("email_bytes", &email.as_ref().map(String::len)).field("plan", plan).finish()
            }
            Account::Other { kind } => f.debug_struct("Other").field("kind", kind).finish(),
        }
    }
}

/// The answer to `account/read`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    /// `None` when signed out.
    pub account: Option<Account>,
    /// Whether the model provider needs an OpenAI sign-in at all.
    pub requires_openai_auth: bool,
}

/// How a sign-in ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    /// Signed in.
    SignedIn,
    /// Cancelled with [`CodexAccount::cancel_login`] (Codex says [`LOGIN_NOT_COMPLETED`] for a device
    /// code, [`BROWSER_LOGIN_NOT_COMPLETED`] for the browser).
    Cancelled,
    /// Failed; `detail` is Codex's words, cut short, for the log.
    Failed { detail: String },
}

/// What `account/login/cancel` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelStatus {
    /// The sign-in was cancelled.
    Canceled,
    /// No sign-in with that id was running.
    NotFound,
}

/// A model Codex offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    /// The id to ask for.
    pub id: String,
    /// The name to show.
    pub display_name: String,
    /// Whether Codex hides it from its own picker.
    pub hidden: bool,
    /// Whether it is Codex's default.
    pub is_default: bool,
}

/// What an account process reports between calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountEvent {
    /// A sign-in ended (`account/login/completed`); `login_id` is `None` for an API key's.
    LoginCompleted { login_id: Option<LoginId>, outcome: LoginOutcome },
    /// The account changed (`account/updated`): `auth_mode` is how Codex is now signed in (`apikey`,
    /// `chatgpt`, …, cleaned), `None` when signed out.
    AccountUpdated { auth_mode: Option<String> },
    /// The process ended; `error` says why when the end was not asked for with `shutdown`.
    Ended { exit: Option<Exit>, error: Option<AgentError> },
}

/// A running account process.
pub struct CodexAccount {
    // Declared first, so the process is ended before its folder is removed.
    conn: Conn,
    store: CredentialStore,
    request_timeout: Duration,
    events: VecDeque<AccountEvent>,
    late_logins: VecDeque<u64>,
    skipped: u64,
    ended: bool,
    /// Why the process ended, for every call after the end.
    end_reason: Option<AgentError>,
    scratch: ScratchFolder,
}

impl fmt::Debug for CodexAccount {
    /// Writes the process and counts, never a line or a value Codex sent.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodexAccount")
            .field("conn", &self.conn)
            .field("store", &self.store)
            .field("queued_events", &self.events.len())
            .field("skipped", &self.skipped)
            .field("ended", &self.ended)
            .finish()
    }
}

impl CodexAccount {
    /// Starts `codex app-server` with `store` and Archforge's private `CODEX_HOME` in a new scratch
    /// folder of `workspace`, and makes the handshake: one request (`initialize`, waited for up to
    /// the smaller of `timeout` and `request_timeout`, counted from the call with the folder and the
    /// start), then the `initialized` notification. A Codex whose handshake names a version outside
    /// the accepted range is refused; a newer one than tested is logged. Any failure ends the
    /// process (it is dropped) and removes the folder.
    pub fn start(
        runtime: &Runtime,
        workspace: &Workspace,
        home: &CodexHome,
        store: CredentialStore,
        limits: Limits,
        request_timeout: Duration,
        timeout: Duration,
    ) -> Result<CodexAccount, AgentError> {
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
        timeout_check(request_timeout)?;
        limits.check()?;
        // The setup below counts against the caller's time too.
        let deadline = deadline_in(timeout.min(request_timeout));
        let scratch = workspace.scratch("codex")?;
        let folder = scratch
            .folder()
            .ok_or_else(|| AgentError::Folder { path: workspace.root().to_path_buf(), problem: "has an unfinished scratch folder".to_owned() })?;
        let request =
            SpawnRequest { args: super::command_line(store, None)?, config_dir: Some(home.path().to_path_buf()), limits, ..SpawnRequest::default() };
        let process = spawn(runtime, folder, request)?;
        let mut account = CodexAccount {
            conn: Conn::new(process),
            store,
            request_timeout,
            events: VecDeque::new(),
            late_logins: VecDeque::new(),
            skipped: 0,
            ended: false,
            end_reason: None,
            scratch,
        };
        let answer = account.call("initialize", Some(initialize::params()), deadline, None)?;
        initialize::check(&answer)?;
        account.conn.notify("initialized")?;
        Ok(account)
    }

    /// The scratch folder the process runs in.
    pub fn folder(&self) -> Option<&crate::ConversationFolder> {
        self.scratch.folder()
    }

    /// How many output lines were not understood so far.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// The runtime process, for its id and the end of its error stream.
    pub fn process(&self) -> &crate::RuntimeProcess {
        self.conn.process()
    }

    /// `account/read`, one request: what Codex is signed in with. `refresh` asks Codex to refresh a
    /// ChatGPT sign-in first, which goes on the network. `requiresOpenaiAuth` must be a boolean, else
    /// the answer is unreadable; a missing or null `account` means signed out. An expired ChatGPT
    /// sign-in is [`ErrorKind::SignInExpired`] (only Codex's exact refresh sentences, code -32603).
    pub fn read(&mut self, refresh: bool, timeout: Duration) -> Result<AccountState, AgentError> {
        let deadline = self.deadline(timeout);
        let answer = self.call("account/read", Some(json!({ "refreshToken": refresh })), deadline, None)?;
        account_state(&answer)
    }

    /// `account/login/start` with an API key, one request: the key goes to Codex in that request
    /// alone, over its standard input, in a buffer overwritten once written; it is never written to a
    /// file, the keyring or the command line. Only for an account process with the ephemeral store
    /// (owner decision), refused otherwise; a key with characters JSON would have to escape is
    /// refused before anything is sent. A refusal by Codex is [`ErrorKind::InvalidCredential`].
    pub fn login_api_key(&mut self, key: &archforge_secrets::Secret, timeout: Duration) -> Result<(), AgentError> {
        if self.store != CredentialStore::Ephemeral {
            return Err(AgentError::InvalidSetting {
                setting: "credential store",
                bytes: 0,
                reason: "an API key goes only to a Codex started with the ephemeral store, so it is never stored".to_owned(),
            });
        }
        checked_api_key(key)?;
        let deadline = self.deadline(timeout);
        // Scrubbed from everything Codex writes from now on, as Codex may echo what it was sent.
        let scrub = archforge_secrets::Secret::new(key.expose().to_owned()).map_err(|e| AgentError::InvalidSetting {
            setting: "Codex API key",
            bytes: 0,
            reason: e.to_string(),
        })?;
        self.conn.add_input_secret(scrub);
        if !self.conn.is_open() {
            return Err(self.closed(deadline));
        }
        let id = self.conn.request_secret(|id| super::api_key_line(id, key))?;
        let answer = self.wait(id, "account/login/start", deadline, Some(ErrorKind::InvalidCredential), false)?;
        match answer.get("type").and_then(Value::as_str) {
            Some("apiKey") => Ok(()),
            _ => Err(unreadable("account/login/start", "its type is not apiKey")),
        }
    }

    /// A ChatGPT sign-in in the browser: `account/login/start` with `chatgpt`. Its address must be an
    /// `https` address of printable characters with a host and no user part, at most
    /// [`MAX_URL_BYTES`]; its login id a UUID. When Codex's answer fails that check, the sign-in is
    /// cancelled with a second request, `account/login/cancel`, but only when the login id Codex gave
    /// passes [`LoginId::checked`] (it is sent back exactly as given, never a cut or shown form), and
    /// the call fails. So it makes one request, or two when Archforge refuses the answer. Refused for
    /// the ephemeral store, where a sign-in would end with the process.
    pub fn login_chatgpt(&mut self, timeout: Duration) -> Result<BrowserLogin, AgentError> {
        self.check_chatgpt_store()?;
        let deadline = self.deadline(timeout);
        let answer = self.call_with("account/login/start", Some(json!({ "type": "chatgpt" })), deadline, None, true)?;
        let raw_id = answer.get("loginId").and_then(Value::as_str).and_then(LoginId::checked);
        let checked = (|| {
            if answer.get("type").and_then(Value::as_str) != Some("chatgpt") {
                return Err("its type is not chatgpt");
            }
            let login_id = raw_id.clone().ok_or("its login id is not a UUID")?;
            let auth_url = checked_url(answer.get("authUrl"))?;
            Ok(BrowserLogin { login_id, auth_url })
        })();
        self.refused_login(checked, raw_id, deadline)
    }

    /// A ChatGPT sign-in with a device code: `account/login/start` with `chatgptDeviceCode`. Its
    /// address is checked as [`CodexAccount::login_chatgpt`] checks it, its code must be 1 to
    /// [`MAX_USER_CODE_BYTES`] ASCII letters, digits and hyphens, its login id a UUID; a refused
    /// answer is cancelled the same way. One request, or two when Archforge refuses the answer.
    pub fn login_device_code(&mut self, timeout: Duration) -> Result<DeviceLogin, AgentError> {
        self.check_chatgpt_store()?;
        let deadline = self.deadline(timeout);
        let answer = self.call_with("account/login/start", Some(json!({ "type": "chatgptDeviceCode" })), deadline, None, true)?;
        let raw_id = answer.get("loginId").and_then(Value::as_str).and_then(LoginId::checked);
        let checked = (|| {
            if answer.get("type").and_then(Value::as_str) != Some("chatgptDeviceCode") {
                return Err("its type is not chatgptDeviceCode");
            }
            let login_id = raw_id.clone().ok_or("its login id is not a UUID")?;
            let verification_url = checked_url(answer.get("verificationUrl"))?;
            let user_code = answer.get("userCode").and_then(Value::as_str).ok_or("it has no device code")?;
            let fits =
                !user_code.is_empty() && user_code.len() <= MAX_USER_CODE_BYTES && user_code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
            if !fits {
                return Err("its device code is not 1 to 32 letters, digits and hyphens");
            }
            Ok(DeviceLogin { login_id, verification_url, user_code: user_code.to_owned() })
        })();
        self.refused_login(checked, raw_id, deadline)
    }

    /// `account/login/cancel`, one request: stops the sign-in `login_id`. Its end then comes as
    /// [`AccountEvent::LoginCompleted`] with [`LoginOutcome::Cancelled`].
    pub fn cancel_login(&mut self, login_id: &LoginId, timeout: Duration) -> Result<CancelStatus, AgentError> {
        let deadline = self.deadline(timeout);
        let answer = self.call("account/login/cancel", Some(json!({ "loginId": login_id.as_str() })), deadline, None)?;
        match answer.get("status").and_then(Value::as_str) {
            Some("canceled") => Ok(CancelStatus::Canceled),
            Some("notFound") => Ok(CancelStatus::NotFound),
            _ => Err(unreadable("account/login/cancel", "its status is neither canceled nor notFound")),
        }
    }

    /// `account/logout`, one request: signs Codex out (a ChatGPT sign-in leaves the keyring or
    /// Codex's files).
    pub fn logout(&mut self, timeout: Duration) -> Result<(), AgentError> {
        let deadline = self.deadline(timeout);
        self.call("account/logout", None, deadline, None).map(drop)
    }

    /// The models Codex offers, from `model/list` page by page: one request a page, each within the
    /// request timeout, all within `timeout`, at most [`MAX_MODEL_PAGES`] pages and [`MAX_MODELS`]
    /// models. A missing or null `nextCursor` ends the list; any other cursor is passed back exactly
    /// as given. An empty, non-text, repeated or oversized (past [`MAX_CURSOR_BYTES`]) cursor, too
    /// many pages or models, or a page that cannot be read fails the whole call: never a quiet
    /// partial list.
    pub fn models(&mut self, timeout: Duration) -> Result<Vec<ModelInfo>, AgentError> {
        let deadline = self.deadline_total(timeout);
        let mut models = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..MAX_MODEL_PAGES {
            let mut params = json!({ "includeHidden": false });
            if let (Some(c), Some(object)) = (&cursor, params.as_object_mut()) {
                object.insert("cursor".to_owned(), json!(c));
            }
            let page_deadline = deadline.min(Instant::now().checked_add(self.request_timeout).unwrap_or(deadline));
            let answer = self.call("model/list", Some(params), page_deadline, None)?;
            let data = answer.get("data").and_then(Value::as_array).ok_or_else(|| unreadable("model/list", "it has no list of models"))?;
            for model in data {
                if models.len() >= MAX_MODELS {
                    return Err(unreadable("model/list", "it lists more models than Archforge takes"));
                }
                models.push(model_info(model)?);
            }
            match answer.get("nextCursor") {
                None | Some(Value::Null) => return Ok(models),
                Some(Value::String(next)) if !next.is_empty() && next.len() <= MAX_CURSOR_BYTES => {
                    if !seen.insert(next.clone()) {
                        return Err(unreadable("model/list", "its next cursor repeats an earlier one"));
                    }
                    cursor = Some(next.clone());
                }
                Some(Value::String(next)) if next.is_empty() => return Err(unreadable("model/list", "its next cursor is empty")),
                Some(Value::String(_)) => return Err(unreadable("model/list", "its next cursor is longer than Archforge passes back")),
                Some(_) => return Err(unreadable("model/list", "its next cursor is not text")),
            }
        }
        Err(unreadable("model/list", "it has more pages than Archforge reads"))
    }

    /// The next event, reading at most [`LINES_PER_CALL`] lines and never waiting past `timeout`:
    /// with a zero timeout every line already there is read (within that budget) without waiting, so
    /// an event behind lines that mean nothing still comes. `None` when nothing came in time, and for
    /// every call after [`AccountEvent::Ended`].
    pub fn next_event(&mut self, timeout: Duration) -> Option<AccountEvent> {
        let deadline = Instant::now().checked_add(timeout).unwrap_or_else(Instant::now);
        let mut budget = LINES_PER_CALL;
        loop {
            if let Some(event) = self.events.pop_front() {
                return Some(event);
            }
            if self.ended {
                self.conn.reap();
                return None;
            }
            let incoming = self.conn.poll(deadline, &mut budget)?;
            self.take(incoming);
        }
    }

    /// Ends the process as [`crate::RuntimeProcess::shutdown`] does: its input is closed, then it is
    /// terminated, then killed, each step given at most `grace`, so up to twice `grace`.
    pub fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        self.conn.shutdown(grace)
    }

    /// The deadline of one request made now: the smaller of `timeout` and the request timeout.
    fn deadline(&self, timeout: Duration) -> Instant {
        deadline_in(timeout.min(self.request_timeout))
    }

    /// The deadline of a call made of several requests: `timeout` from now.
    fn deadline_total(&self, timeout: Duration) -> Instant {
        deadline_in(timeout)
    }

    /// A ChatGPT sign-in needs a store that outlives the process.
    fn check_chatgpt_store(&self) -> Result<(), AgentError> {
        if self.store == CredentialStore::Ephemeral {
            return Err(AgentError::InvalidSetting {
                setting: "credential store",
                bytes: 0,
                reason: "a ChatGPT sign-in is kept in the keyring or Codex's own files; the ephemeral store would lose it".to_owned(),
            });
        }
        Ok(())
    }

    /// The end of a sign-in start: `checked` when Archforge took Codex's answer; otherwise the
    /// sign-in Codex started is cancelled (a second request, waited for until `deadline`) when its
    /// raw id passed the check, and the call fails as unreadable.
    fn refused_login<T>(&mut self, checked: Result<T, &'static str>, raw_id: Option<LoginId>, deadline: Instant) -> Result<T, AgentError> {
        let why = match checked {
            Ok(login) => return Ok(login),
            Err(why) => why,
        };
        match raw_id {
            Some(id) => match self.call("account/login/cancel", Some(json!({ "loginId": id.as_str() })), deadline, None) {
                Ok(_) => log::warn!("codex's sign-in answer was refused ({why}); the sign-in it started was cancelled"),
                Err(e) => log::warn!("codex's sign-in answer was refused ({why}); cancelling the sign-in it started failed: {e}"),
            },
            None => log::warn!(
                "codex's sign-in answer was refused ({why}); its login id is not a UUID, so it is not sent back and the sign-in ends on Codex's own timeout"
            ),
        }
        Err(unreadable("account/login/start", why))
    }

    /// Sends one request and waits for its answer until `deadline`.
    fn call(&mut self, method: &'static str, params: Option<Value>, deadline: Instant, refused: Option<ErrorKind>) -> Result<Value, AgentError> {
        self.call_with(method, params, deadline, refused, false)
    }

    /// [`CodexAccount::call`]; with `late_login`, a ChatGPT sign-in start that times out is
    /// remembered, so a sign-in Codex starts late is cancelled. Once Codex is ending, the call
    /// sends nothing and fails with the end's reason (read until `deadline`).
    fn call_with(
        &mut self,
        method: &'static str,
        params: Option<Value>,
        deadline: Instant,
        refused: Option<ErrorKind>,
        late_login: bool,
    ) -> Result<Value, AgentError> {
        if !self.conn.is_open() {
            return Err(self.closed(deadline));
        }
        let id = self.conn.request(method, params)?;
        self.wait(id, method, deadline, refused, late_login)
    }

    /// The error of a call made once Codex is ending: the end's reason, read (keeping the events
    /// read meanwhile) until the end comes or `deadline` passes; the input closed otherwise.
    fn closed(&mut self, deadline: Instant) -> AgentError {
        while !self.ended {
            let mut budget = LINES_PER_CALL;
            match self.conn.poll(deadline, &mut budget) {
                Some(incoming) => self.take(incoming),
                None if Instant::now() >= deadline => break,
                None => {}
            }
        }
        self.end_reason.clone().unwrap_or(AgentError::InputClosed { runtime: RuntimeKind::Codex })
    }

    /// Reads until the answer to request `id` comes, keeping every event read meanwhile, and never
    /// past `deadline`. A refusal is the request's error (`refused` names its category, if any);
    /// when Codex ends first, the call fails with the end's reason (the exit code or signal and the
    /// error stream's tail), and the end stays queued for [`CodexAccount::next_event`].
    fn wait(&mut self, id: u64, method: &'static str, deadline: Instant, refused: Option<ErrorKind>, late_login: bool) -> Result<Value, AgentError> {
        let started = Instant::now();
        loop {
            if self.ended {
                return Err(self.end_reason.clone().unwrap_or(AgentError::InputClosed { runtime: RuntimeKind::Codex }));
            }
            let mut budget = LINES_PER_CALL;
            let Some(incoming) = self.conn.poll(deadline, &mut budget) else {
                if Instant::now() >= deadline {
                    if late_login {
                        self.remember_late_login(id);
                    }
                    return Err(AgentError::TimedOut { runtime: RuntimeKind::Codex, request: method, after_ms: started.elapsed().as_millis() });
                }
                continue;
            };
            match incoming {
                Incoming::Response { id: answered, result } if answered == id => {
                    return result.map_err(|e: RpcError| e.into_error(method, refused));
                }
                Incoming::Ended { exit, error } => {
                    self.end(exit, error);
                    return Err(self.end_reason.clone().unwrap_or(AgentError::InputClosed { runtime: RuntimeKind::Codex }));
                }
                other => self.take(other),
            }
        }
    }

    /// Keeps what one incoming message means: an event, a skipped line, a late answer.
    fn take(&mut self, incoming: Incoming) {
        match incoming {
            Incoming::Notification { method, params } => self.notification(&method, &params),
            Incoming::Request { id, method } => {
                log::warn!("codex asked the account process {}, which it does not handle; refused", super::rpc::kind_name(&method));
                if let Err(e) = self.conn.respond_error(&id, super::rpc::METHOD_NOT_FOUND, "Archforge's account process does not handle this request")
                {
                    log::warn!("codex's request could not be answered: {e}");
                }
            }
            Incoming::Response { id, result } => self.late_answer(id, result),
            Incoming::Skipped(why) => {
                log::debug!("codex wrote a line the account process skipped: {why:?}");
                self.skipped = self.skipped.saturating_add(1);
            }
            Incoming::Ended { exit, error } => self.end(exit, error),
        }
    }

    /// An answer to a request no call waits for any more: a sign-in that started after its call
    /// timed out is cancelled when its id passes the check; anything else is logged.
    fn late_answer(&mut self, id: u64, result: Result<Value, RpcError>) {
        let Some(at) = self.late_logins.iter().position(|late| *late == id) else {
            log::debug!("codex answered request {id} after its call gave up; the answer is dropped");
            return;
        };
        self.late_logins.remove(at);
        let Ok(answer) = result else { return };
        match answer.get("loginId").and_then(Value::as_str).and_then(LoginId::checked) {
            Some(login) => {
                if let Err(e) = self.conn.request("account/login/cancel", Some(json!({ "loginId": login.as_str() }))) {
                    log::warn!("a sign-in that started late could not be cancelled: {e}");
                }
            }
            None => log::warn!("a sign-in started late without a usable login id; it ends on Codex's own timeout"),
        }
    }

    /// Codex has ended: the end is kept as the reason of every later call and queued as an event.
    fn end(&mut self, exit: Option<Exit>, error: Option<AgentError>) {
        self.ended = true;
        self.end_reason = Some(error.clone().unwrap_or_else(|| AgentError::reported(ErrorKind::Other, "codex ended before it answered")));
        self.push(AccountEvent::Ended { exit, error });
    }

    /// Remembers a sign-in start that timed out, so one that starts late is cancelled.
    fn remember_late_login(&mut self, id: u64) {
        if self.late_logins.len() >= MAX_LATE_LOGINS {
            self.late_logins.pop_front();
        }
        self.late_logins.push_back(id);
    }

    /// Keeps what a notification means as an event.
    fn notification(&mut self, method: &str, params: &Value) {
        match method {
            "account/login/completed" => {
                let login_id = params.get("loginId").and_then(Value::as_str).and_then(LoginId::checked);
                let outcome = match (params.get("success").and_then(Value::as_bool), params.get("error").and_then(Value::as_str)) {
                    (Some(true), _) => LoginOutcome::SignedIn,
                    (_, Some(LOGIN_NOT_COMPLETED | BROWSER_LOGIN_NOT_COMPLETED)) => LoginOutcome::Cancelled,
                    (_, error) => LoginOutcome::Failed {
                        detail: cut(&self.conn.redact(error.unwrap_or("no reason given").to_owned()), crate::MAX_DETAIL_BYTES),
                    },
                };
                self.push(AccountEvent::LoginCompleted { login_id, outcome });
            }
            "account/updated" => {
                let auth_mode = params.get("authMode").and_then(Value::as_str).map(super::rpc::kind_name);
                self.push(AccountEvent::AccountUpdated { auth_mode });
            }
            "remoteControl/status/changed" | "account/rateLimits/updated" | "configWarning" | "deprecationNotice" | "warning" => {}
            other => {
                log::debug!("codex sent the account process {}; it was skipped", super::rpc::kind_name(other));
                self.skipped = self.skipped.saturating_add(1);
            }
        }
    }

    /// Queues an event, dropping the oldest past [`MAX_QUEUED_EVENTS`].
    fn push(&mut self, event: AccountEvent) {
        if self.events.len() >= MAX_QUEUED_EVENTS {
            log::warn!("the account process's events are not read; the oldest was dropped");
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

impl Drop for CodexAccount {
    /// Ends the process (its own drop kills its group and collects it) before the scratch folder
    /// goes with the fields that follow.
    fn drop(&mut self) {
        self.conn.reap();
    }
}

/// `text` cut to at most `max` bytes at a character boundary, with an ellipsis after the cut.
fn cut(text: &str, max: usize) -> String {
    if text.len() > max {
        log::debug!("codex wrote a text of {} bytes to the account process; the first {max} are kept", text.len());
    }
    crate::error::capped_to(text.to_owned(), max)
}

/// An unreadable answer to `request`.
fn unreadable(request: &'static str, reason: &str) -> AgentError {
    AgentError::Unreadable { runtime: RuntimeKind::Codex, request, reason: reason.to_owned() }
}

/// The answer to `account/read`: `requiresOpenaiAuth` must be a boolean; `account` missing or null
/// is signed out, an object is read by its `type`, anything else is unreadable.
pub(crate) fn account_state(answer: &Value) -> Result<AccountState, AgentError> {
    let requires_openai_auth = answer
        .get("requiresOpenaiAuth")
        .and_then(Value::as_bool)
        .ok_or_else(|| unreadable("account/read", "requiresOpenaiAuth is missing or not a boolean"))?;
    let account = match answer.get("account") {
        None | Some(Value::Null) => None,
        Some(Value::Object(object)) => Some(match object.get("type").and_then(Value::as_str) {
            Some("apiKey") => Account::ApiKey,
            Some("chatgpt") => Account::ChatGpt {
                email: object.get("email").and_then(Value::as_str).map(|e| cut(e, MAX_TEXT_BYTES)),
                plan: object.get("planType").and_then(Value::as_str).map(super::rpc::kind_name),
            },
            Some(other) => Account::Other { kind: super::rpc::kind_name(other) },
            None => return Err(unreadable("account/read", "its account has no type")),
        }),
        Some(_) => return Err(unreadable("account/read", "its account is not an object")),
    };
    Ok(AccountState { account, requires_openai_auth })
}

/// One model of `model/list`: its `id` (checked as an id), its name to show and two flags.
fn model_info(model: &Value) -> Result<ModelInfo, AgentError> {
    let id = super::rpc::checked_id(model.get("id")).ok_or_else(|| unreadable("model/list", "a model's id is not readable"))?;
    let display_name = model.get("displayName").and_then(Value::as_str).map_or_else(|| id.clone(), |n| cut(n, MAX_TEXT_BYTES));
    let hidden = model.get("hidden").and_then(Value::as_bool).unwrap_or(false);
    let is_default = model.get("isDefault").and_then(Value::as_bool).unwrap_or(false);
    Ok(ModelInfo { id, display_name, hidden, is_default })
}

/// A sign-in address from Codex, taken only when it is an `https` address of printable ASCII (no
/// spaces), at most [`MAX_URL_BYTES`], with a host and without a user part.
fn checked_url(value: Option<&Value>) -> Result<String, &'static str> {
    let url = value.and_then(Value::as_str).ok_or("it has no address")?;
    if url.len() > MAX_URL_BYTES || !url.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err("its address is too long or holds a character an address cannot");
    }
    let rest = url.strip_prefix("https://").ok_or("its address is not https")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if host.is_empty() || host.contains('@') {
        return Err("its address has no host, or a user part");
    }
    Ok(url.to_owned())
}

/// The API key's request line: `account/login/start` with the key, built in one buffer of its final
/// size (no growth leaves a copy behind), overwritten when dropped.
pub(crate) fn key_line_len(id: u64, key: &str) -> usize {
    format!("{{\"id\":{id},\"method\":\"account/login/start\",\"params\":{{\"apiKey\":\"\",\"type\":\"apiKey\"}}}}").len().saturating_add(key.len())
}

/// Writes the API key's request line into `line`, which has room for it.
pub(crate) fn write_key_line(line: &mut Zeroizing<String>, id: u64, key: &str) {
    line.push_str(&format!("{{\"id\":{id},\"method\":\"account/login/start\",\"params\":{{\"apiKey\":\""));
    line.push_str(key);
    line.push_str("\",\"type\":\"apiKey\"}}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `requiresOpenaiAuth` must be a boolean; a missing or null account is signed out.
    #[test]
    fn account_read_requires_the_boolean_and_reads_a_missing_account_as_signed_out() {
        let read = |v: Value| account_state(&v);
        assert_eq!(read(json!({"account": null, "requiresOpenaiAuth": true})).unwrap(), AccountState { account: None, requires_openai_auth: true });
        assert_eq!(read(json!({"requiresOpenaiAuth": false})).unwrap(), AccountState { account: None, requires_openai_auth: false });
        for bad in [
            json!({"account": null}),
            json!({"account": null, "requiresOpenaiAuth": "true"}),
            json!({"account": null, "requiresOpenaiAuth": null}),
            json!({}),
        ] {
            assert!(matches!(read(bad.clone()), Err(AgentError::Unreadable { request: "account/read", .. })), "{bad}");
        }
        assert!(matches!(read(json!({"account": 7, "requiresOpenaiAuth": true})), Err(AgentError::Unreadable { .. })));
        assert!(matches!(read(json!({"account": {}, "requiresOpenaiAuth": true})), Err(AgentError::Unreadable { .. })));
        assert_eq!(read(json!({"account": {"type": "apiKey"}, "requiresOpenaiAuth": true})).unwrap().account, Some(Account::ApiKey));
        let chatgpt =
            read(json!({"account": {"type": "chatgpt", "email": "person@example.com", "planType": "plus"}, "requiresOpenaiAuth": true})).unwrap();
        assert_eq!(chatgpt.account, Some(Account::ChatGpt { email: Some("person@example.com".into()), plan: Some("plus".into()) }));
        assert_eq!(
            read(json!({"account": {"type": "gcp"}, "requiresOpenaiAuth": true})).unwrap().account,
            Some(Account::Other { kind: "gcp".into() })
        );
    }

    /// Login ids are UUIDs, kept exactly as given.
    #[test]
    fn login_ids_are_uuids() {
        assert_eq!(LoginId::checked("019a0000-0000-7000-8000-00000000c002").unwrap().as_str(), "019a0000-0000-7000-8000-00000000c002");
        assert!(LoginId::checked("019A0000-0000-7000-8000-00000000C002").is_some());
        for bad in [
            "",
            "019a0000-0000-7000-8000-00000000c00",
            "019a0000-0000-7000-8000-00000000c0022",
            "019a0000_0000-7000-8000-00000000c002",
            "g19a0000-0000-7000-8000-00000000c002",
            "login-1",
        ] {
            assert!(LoginId::checked(bad).is_none(), "{bad}");
        }
    }

    /// Only https addresses of printable characters with a host pass.
    #[test]
    fn sign_in_addresses_are_checked() {
        assert!(checked_url(Some(&json!("https://auth.openai.com/oauth/authorize?x=1"))).is_ok());
        for bad in [
            json!("http://auth.openai.com/"),
            json!("https://"),
            json!("https:///path"),
            json!("https://user@evil.example/"),
            json!("https://a b"),
            json!("javascript:alert(1)"),
            json!(7),
            json!(format!("https://a/{}", "x".repeat(MAX_URL_BYTES))),
        ] {
            assert!(checked_url(Some(&bad)).is_err(), "{bad}");
        }
        assert!(checked_url(None).is_err());
    }

    /// The key's line is built at its final size and is valid JSON holding the key.
    #[test]
    fn the_key_line_is_built_at_its_final_size() {
        let key = "sk-test-0123456789";
        let mut line = Zeroizing::new(String::with_capacity(key_line_len(42, key)));
        let capacity = line.capacity();
        write_key_line(&mut line, 42, key);
        assert_eq!(line.len(), key_line_len(42, key));
        assert_eq!(line.capacity(), capacity, "no growth");
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value, json!({"id": 42, "method": "account/login/start", "params": {"apiKey": key, "type": "apiKey"}}));
    }

    /// `Debug` of everything that carries a sign-in address, a device code or an email shows
    /// lengths only.
    #[test]
    fn debug_output_never_shows_sign_in_values() {
        let login_id = LoginId::checked("019a0000-0000-7000-8000-00000000c001").unwrap();
        let browser = BrowserLogin { login_id: login_id.clone(), auth_url: "https://auth.openai.com/oauth/authorize?state=SECRETSTATE".into() };
        let device = DeviceLogin {
            login_id: login_id.clone(),
            verification_url: "https://auth.openai.com/codex/device?SECRETURL".into(),
            user_code: "SECRET-CODE".into(),
        };
        let account = Account::ChatGpt { email: Some("secret-person@example.com".into()), plan: Some("plus".into()) };
        let event = AccountEvent::LoginCompleted { login_id: Some(login_id), outcome: LoginOutcome::SignedIn };
        for shown in [
            format!("{browser:?}"),
            format!("{browser:#?}"),
            format!("{device:?}"),
            format!("{device:#?}"),
            format!("{account:?}"),
            format!("{account:#?}"),
            format!("{event:?}"),
            format!("{event:#?}"),
        ] {
            for secret in ["SECRETSTATE", "SECRETURL", "SECRET-CODE", "secret-person", "019a0000"] {
                assert!(!shown.contains(secret), "{secret} in {shown}");
            }
        }
    }
}
