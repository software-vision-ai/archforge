//! The Codex side of the Settings page: reading the account, signing in with ChatGPT (in the browser
//! or with a device code), cancelling a sign-in, signing out, giving an API key, the rate limits and
//! the models. Each [`CodexAccounts`] runs one `codex app-server` in the account folder; no thread
//! is ever started, so none of these reaches a model or costs anything.
//!
//! Every call waits for its own answer at most the request timeout and belongs on a worker thread,
//! never the interface thread. What Codex says on its own (a sign-in that finished, the account
//! changing, new rate limits) comes from [`CodexAccounts::next_event`].

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::parse::kind_name;
use super::rpc::{self, Incoming, capped};
use super::{SignIn, SignInKind, api_key_login_line, check_agent, check_timeout, command_line, decline_line, initialize_params};
use crate::{AgentError, ConversationFolder, ErrorKind, Exit, Limits, OutputEvent, Runtime, RuntimeKind, RuntimeProcess, SpawnRequest, spawn};

/// The longest name, id or plan kept from an answer, in bytes.
const MAX_FIELD_BYTES: usize = 256;
/// The longest model description kept, in bytes.
const MAX_DESCRIPTION_BYTES: usize = 1024;
/// The longest sign-in address accepted, in bytes.
const MAX_URL_BYTES: usize = 8 * 1024;
/// The longest device code accepted, in characters.
const MAX_USER_CODE: usize = 64;
/// The most pages of models read, and the most models kept.
const MAX_MODEL_PAGES: usize = 10;
const MAX_MODELS: usize = 1000;
/// The most output lines one `next_event` call reads while none means an event.
const LINES_PER_CALL: u32 = 256;
/// The hosts a sign-in address may be on (and their subdomains): Archforge opens no other.
pub const SIGN_IN_HOSTS: [&str; 2] = ["openai.com", "chatgpt.com"];

/// Who Codex is signed in as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountStatus {
    /// Not signed in.
    SignedOut,
    /// With an API key (in this process's memory only).
    ApiKey,
    /// With ChatGPT: the address, the plan and the workspace where Codex names them.
    ChatGpt { email: Option<String>, plan: Option<String>, workspace: Option<String> },
    /// A ChatGPT sign-in that can no longer be renewed: sign in again.
    Expired { email: Option<String>, plan: Option<String> },
    /// Another kind of account Archforge does not offer (named by its type).
    Other { kind: String },
}

/// What `account/read` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    pub status: AccountStatus,
    /// Whether the model provider needs an OpenAI sign-in at all.
    pub requires_openai_auth: bool,
}

/// A ChatGPT sign-in that has begun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginStarted {
    /// Open `auth_url` in the browser; Codex waits for the answer on its local callback.
    Browser { login_id: String, auth_url: String },
    /// Show `user_code` and open `verification_url`, where the person enters it.
    DeviceCode { login_id: String, verification_url: String, user_code: String },
}

impl LoginStarted {
    /// The sign-in's id, for [`CodexAccounts::cancel_login`].
    pub fn login_id(&self) -> &str {
        match self {
            LoginStarted::Browser { login_id, .. } | LoginStarted::DeviceCode { login_id, .. } => login_id,
        }
    }
}

/// What a cancelled sign-in found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelStatus {
    /// It was waiting and is cancelled.
    Canceled,
    /// No sign-in of that id was waiting.
    NotFound,
    /// Codex answered without saying which (as the documentation's example does).
    Unstated,
}

/// One model Codex offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
}

/// One rate-limit window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitWindow {
    pub used_percent: Option<u64>,
    pub window_minutes: Option<u64>,
    /// When the window resets, in seconds since 1970.
    pub resets_at: Option<i64>,
}

/// The account's rate limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimits {
    pub ordinary_usage_allowed: Option<bool>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub plan: Option<String>,
    /// Which limit was reached, when one was.
    pub reached: Option<String>,
}

/// What Codex said on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountEvent {
    /// A sign-in began: the address to open, or the device code to show.
    LoginStarted(LoginStarted),
    /// A sign-in finished; `error` says why one failed (a cancel among them).
    LoginCompleted { login_id: Option<String>, success: bool, error: Option<String> },
    /// The account changed: how it is signed in now (`apikey`, `chatgpt`, or `None` when signed
    /// out) and the plan.
    AccountChanged { auth_mode: Option<String>, plan: Option<String> },
    /// New rate limits.
    RateLimits(RateLimits),
    /// The account process ended; `error` says why when it was not shut down.
    Ended { exit: Option<Exit>, error: Option<AgentError> },
}

/// Settings for an account process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountOptions {
    pub limits: Limits,
    /// How long each call waits for its answer.
    pub request_timeout: Duration,
}

impl Default for AccountOptions {
    /// The default limits and [`super::DEFAULT_REQUEST_TIMEOUT`].
    fn default() -> AccountOptions {
        AccountOptions { limits: Limits::default(), request_timeout: super::DEFAULT_REQUEST_TIMEOUT }
    }
}

/// A `codex app-server` that reads and changes the account, without ever starting a thread.
pub struct CodexAccounts {
    process: RuntimeProcess,
    /// Dropped after the process (fields drop in order), so Codex has ended before its state goes.
    _scratch: Scratch,
    kind: SignInKind,
    next_id: u64,
    events: VecDeque<AccountEvent>,
    timeout: Duration,
    max_line_bytes: usize,
    skipped: u64,
    last_chatgpt: Option<(Option<String>, Option<String>)>,
    logging_out: bool,
    shutting_down: bool,
    ended: bool,
}

impl std::fmt::Debug for CodexAccounts {
    /// Writes the state; never an address, a code or a secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexAccounts")
            .field("process", &self.process)
            .field("kind", &self.kind)
            .field("events", &self.events.len())
            .field("skipped", &self.skipped)
            .field("ended", &self.ended)
            .finish()
    }
}

impl CodexAccounts {
    /// Starts `codex app-server` in the `account` folder ([`crate::Workspace::account`]) and shakes
    /// hands. With [`SignIn::ChatGpt`] Codex uses the keyring, where it keeps the ChatGPT sign-in;
    /// with [`SignIn::ApiKey`] it uses the in-memory store and is given the key over standard
    /// input at once, so a refused key is an `InvalidCredential` error here and nothing is ever
    /// written. Waits for the answers, at most the request timeout each.
    pub fn open(runtime: &Runtime, account: &ConversationFolder, sign_in: SignIn, options: AccountOptions) -> Result<CodexAccounts, AgentError> {
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
        options.limits.check()?;
        check_timeout(options.request_timeout)?;
        let kind = sign_in.kind();
        let (vars, key) = match sign_in {
            SignIn::ApiKey(key) => (Vec::new(), Some(key)),
            SignIn::ChatGpt(keyring) => (keyring.vars, None),
        };
        // The key is sent once, below; its login line is made before the key goes to the scrub list.
        let login = key.as_ref().map(|k| api_key_login_line(2, k)).transpose()?;
        let scratch = Scratch::new(account)?;
        let request = SpawnRequest {
            args: command_line(kind, Some(&scratch.state), None)?,
            vars,
            scrub: key.into_iter().collect(),
            config_home: Some(account.config().to_path_buf()),
            limits: options.limits,
            ..SpawnRequest::default()
        };
        let process = spawn(runtime, account, request)?;
        let mut accounts = CodexAccounts {
            process,
            _scratch: scratch,
            kind,
            next_id: 1,
            events: VecDeque::new(),
            timeout: options.request_timeout,
            max_line_bytes: options.limits.max_line_bytes,
            skipped: 0,
            last_chatgpt: None,
            logging_out: false,
            shutting_down: false,
            ended: false,
        };
        let hello = accounts.call("initialize", Some(initialize_params())).map_err(|e| missing_if_refused(e, "initialize"))?;
        check_agent(runtime, &hello)?;
        accounts.process.send_line(&rpc::notification_line("initialized"))?;
        if let Some(line) = login {
            // Id 2: the login line was made with it before the start.
            accounts.next_id = 2;
            accounts.call_line("account/login/start", &line).map_err(|e| match e {
                AgentError::Reported { kind: ErrorKind::Other, detail } => AgentError::Reported { kind: ErrorKind::InvalidCredential, detail },
                other => other,
            })?;
        }
        Ok(accounts)
    }

    /// How this process is signed in.
    pub fn kind(&self) -> SignInKind {
        self.kind
    }

    /// How many output lines were skipped so far.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// The runtime process, for its id and the end of its error stream.
    pub fn process(&self) -> &RuntimeProcess {
        &self.process
    }

    /// Who Codex is signed in as (`account/read`). With `refresh`, Codex renews a ChatGPT sign-in
    /// first, which reaches the sign-in service but no model.
    pub fn read(&mut self, refresh: bool) -> Result<AccountState, AgentError> {
        let params = if refresh { json!({ "refreshToken": true }) } else { json!({}) };
        let value = self.call("account/read", Some(params))?;
        let state = account_state(&value);
        if let AccountStatus::ChatGpt { email, plan, .. } = &state.status {
            self.last_chatgpt = Some((email.clone(), plan.clone()));
        }
        Ok(state)
    }

    /// The free check: the account as Codex reads it, and for a ChatGPT sign-in whether it can still
    /// be renewed: a renewal that is refused for an expired or revoked sign-in reads as
    /// [`AccountStatus::Expired`]. (A metered test, a real turn, is a conversation's.)
    pub fn check(&mut self) -> Result<AccountState, AgentError> {
        let state = self.read(false)?;
        if !matches!(state.status, AccountStatus::ChatGpt { .. }) {
            return Ok(state);
        }
        match self.read(true) {
            Ok(renewed) => Ok(renewed),
            Err(AgentError::Reported { detail, .. }) if expired_words(&detail) => {
                let (email, plan) = self.last_chatgpt.clone().unwrap_or_default();
                Ok(AccountState { status: AccountStatus::Expired { email, plan }, requires_openai_auth: state.requires_openai_auth })
            }
            Err(e) => Err(e),
        }
    }

    /// Begins a ChatGPT sign-in in the browser: the address to open (on a host of
    /// [`SIGN_IN_HOSTS`], else refused). Also queued as [`AccountEvent::LoginStarted`]. Keep this
    /// process running until [`AccountEvent::LoginCompleted`]: Codex receives the answer.
    pub fn login_chatgpt(&mut self) -> Result<LoginStarted, AgentError> {
        self.chatgpt_only("a ChatGPT sign-in")?;
        let value = self.call("account/login/start", Some(json!({ "type": "chatgpt" })))?;
        let login_id = field(&value, "loginId").ok_or_else(|| unreadable("account/login/start", "no sign-in id"))?;
        let auth_url = checked_url(value.get("authUrl").and_then(Value::as_str).unwrap_or_default())?;
        let started = LoginStarted::Browser { login_id: checked_id(&login_id)?, auth_url };
        self.events.push_back(AccountEvent::LoginStarted(started.clone()));
        Ok(started)
    }

    /// Begins a ChatGPT sign-in with a device code, for when the browser cannot reach this
    /// computer: the code to show and the address where the person enters it. Also queued as
    /// [`AccountEvent::LoginStarted`].
    pub fn login_device_code(&mut self) -> Result<LoginStarted, AgentError> {
        self.chatgpt_only("a ChatGPT sign-in")?;
        let value = self.call("account/login/start", Some(json!({ "type": "chatgptDeviceCode" })))?;
        let login_id = field(&value, "loginId").ok_or_else(|| unreadable("account/login/start", "no sign-in id"))?;
        let verification_url = checked_url(value.get("verificationUrl").and_then(Value::as_str).unwrap_or_default())?;
        let user_code = checked_code(value.get("userCode").and_then(Value::as_str).unwrap_or_default())?;
        let started = LoginStarted::DeviceCode { login_id: checked_id(&login_id)?, verification_url, user_code };
        self.events.push_back(AccountEvent::LoginStarted(started.clone()));
        Ok(started)
    }

    /// Cancels the sign-in `login_id`.
    pub fn cancel_login(&mut self, login_id: &str) -> Result<CancelStatus, AgentError> {
        let id = checked_id(login_id)?;
        let value = self.call("account/login/cancel", Some(json!({ "loginId": id })))?;
        Ok(match value.get("status").and_then(Value::as_str) {
            Some("canceled") => CancelStatus::Canceled,
            Some("notFound") => CancelStatus::NotFound,
            Some(other) => {
                log::warn!("codex answered a cancelled sign-in with the unknown status {}", kind_name(other));
                CancelStatus::Unstated
            }
            None => CancelStatus::Unstated,
        })
    }

    /// Signs out: Codex removes the ChatGPT sign-in from the keyring (or forgets the key).
    pub fn logout(&mut self) -> Result<(), AgentError> {
        self.logging_out = true;
        let done = self.call("account/logout", None).map(|_| ());
        self.last_chatgpt = None;
        done
    }

    /// The account's rate limits (`account/rateLimits/read`); Codex refuses without a sign-in.
    pub fn rate_limits(&mut self) -> Result<RateLimits, AgentError> {
        let value = self.call("account/rateLimits/read", None)?;
        Ok(rate_limits(value.get("rateLimits"), value.get("ordinaryUsageAllowed")))
    }

    /// The models Codex offers (`model/list`, every page, hidden ones left out), which needs no
    /// sign-in; what an account may really use shows only on a turn.
    pub fn models(&mut self) -> Result<Vec<ModelInfo>, AgentError> {
        let mut models = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_MODEL_PAGES {
            let mut params = json!({ "limit": 100, "includeHidden": false });
            if let Some(c) = &cursor {
                params["cursor"] = json!(c);
            }
            let value = self.call("model/list", Some(params))?;
            for model in value.get("data").and_then(Value::as_array).into_iter().flatten() {
                if models.len() >= MAX_MODELS {
                    log::warn!("codex lists more than {MAX_MODELS} models; the rest are left out");
                    return Ok(models);
                }
                if let Some(info) = model_info(model) {
                    models.push(info);
                }
            }
            cursor = value.get("nextCursor").and_then(Value::as_str).filter(|c| !c.is_empty() && c.len() <= MAX_DESCRIPTION_BYTES).map(str::to_owned);
            if cursor.is_none() {
                return Ok(models);
            }
        }
        log::warn!("codex lists models on more than {MAX_MODEL_PAGES} pages; the rest are left out");
        Ok(models)
    }

    /// The next thing Codex said on its own, waiting at most `timeout`; `None` when nothing came,
    /// and for every call after [`AccountEvent::Ended`].
    pub fn next_event(&mut self, timeout: Duration) -> Option<AccountEvent> {
        let deadline = Instant::now().checked_add(timeout);
        let mut budget = LINES_PER_CALL;
        loop {
            if let Some(event) = self.events.pop_front() {
                return Some(event);
            }
            let now = Instant::now();
            if self.ended || budget == 0 || deadline.is_some_and(|d| now >= d) {
                return None;
            }
            budget -= 1;
            let wait = deadline.map_or(timeout, |d| d.saturating_duration_since(now));
            match self.process.next_event(wait) {
                Some(event) => {
                    let _ = self.take(event, None);
                }
                None => return None,
            }
        }
    }

    /// Ends the account process (closing its input, then terminating, then killing, each step
    /// given at most `grace`). A sign-in still waiting is abandoned.
    pub fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        self.shutting_down = true;
        self.process.shutdown(grace)
    }

    /// Refuses a ChatGPT operation on a process that runs with the in-memory store.
    fn chatgpt_only(&self, what: &str) -> Result<(), AgentError> {
        match self.kind {
            SignInKind::ChatGpt => Ok(()),
            SignInKind::ApiKey => Err(AgentError::InvalidSetting {
                setting: "sign-in mode",
                bytes: 0,
                reason: format!("{what} needs an account process opened for ChatGPT, which keeps the sign-in in the keyring"),
            }),
        }
    }

    /// Sends `method` and waits for its answer.
    fn call(&mut self, method: &'static str, params: Option<Value>) -> Result<Value, AgentError> {
        let line = rpc::request_line(self.next_id, method, params);
        self.call_line(method, &line)
    }

    /// Sends `line`, the request `method` under the next id, and waits at most the request timeout
    /// for its answer, reading what else Codex says on the way.
    fn call_line(&mut self, method: &'static str, line: &str) -> Result<Value, AgentError> {
        if self.ended {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or_else(|| AgentError::reported(ErrorKind::Other, "the request ids are used up"))?;
        self.process.send_line(line)?;
        let deadline = Instant::now().checked_add(self.timeout).ok_or(AgentError::InvalidLimit {
            limit: "request timeout (ms)",
            value: self.timeout.as_millis(),
            accepted: "a duration the clock can hold".to_owned(),
        })?;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(AgentError::reported(ErrorKind::Other, &format!("codex did not answer {method} within {} ms", self.timeout.as_millis())));
            }
            let Some(event) = self.process.next_event(deadline.saturating_duration_since(now)) else { continue };
            if let Some(answer) = self.take(event, Some(id)) {
                return answer.map_err(|e| {
                    if e.unknown_method() {
                        AgentError::reported(ErrorKind::MissingRuntime, &format!("codex does not know {method}, which Archforge needs; update it"))
                    } else {
                        AgentError::reported(ErrorKind::Other, &format!("codex refused {method}: {}", e.message))
                    }
                });
            }
            if self.ended {
                return Err(AgentError::reported(ErrorKind::Other, &format!("codex ended before it answered {method}")));
            }
        }
    }

    /// Reads one output event; answers the answer to `waiting` when this is it.
    fn take(&mut self, event: OutputEvent, waiting: Option<u64>) -> Option<Result<Value, rpc::RpcError>> {
        match event {
            OutputEvent::Line(line) => {
                let line = self.process.redact(line);
                match rpc::read_line(&line, self.max_line_bytes) {
                    Incoming::Response { id, result } if Some(id) == waiting => return Some(result),
                    Incoming::Response { id, .. } => {
                        log::debug!("codex answered a request that is not waiting (id {id})");
                        self.skipped = self.skipped.saturating_add(1);
                    }
                    Incoming::Notification { method, params } => self.notified(&method, &params),
                    Incoming::Request { id, method, .. } => {
                        log::info!("codex asked for {}; Archforge declined", kind_name(&method));
                        if let Err(e) = self.process.send_line(&decline_line(&id, &method)) {
                            log::warn!("codex's request could not be answered: {e}");
                        }
                    }
                    Incoming::Skipped(_) => self.skipped = self.skipped.saturating_add(1),
                }
            }
            OutputEvent::Dropped(_) => self.skipped = self.skipped.saturating_add(1),
            OutputEvent::ReadFailed(detail) => log::warn!("codex's output could not be read: {detail}"),
            OutputEvent::End => self.finish(),
        }
        None
    }

    /// A notification as an event, where it means one.
    fn notified(&mut self, method: &str, params: &Value) {
        match method {
            "account/login/completed" => self.events.push_back(AccountEvent::LoginCompleted {
                login_id: field(params, "loginId"),
                success: params.get("success").and_then(Value::as_bool).unwrap_or(false),
                error: params.get("error").and_then(Value::as_str).map(|e| capped(e, MAX_DESCRIPTION_BYTES)),
            }),
            "account/updated" => {
                let auth_mode = field(params, "authMode").map(|m| kind_name(&m));
                if auth_mode.is_none() {
                    if !self.logging_out && self.last_chatgpt.is_some() {
                        log::warn!("codex dropped the ChatGPT sign-in on its own");
                    }
                    self.logging_out = false;
                }
                self.events.push_back(AccountEvent::AccountChanged { auth_mode, plan: field(params, "planType").map(|p| kind_name(&p)) });
            }
            "account/rateLimits/updated" => self.events.push_back(AccountEvent::RateLimits(rate_limits(params.get("rateLimits"), None))),
            other if super::QUIET_NOTIFICATIONS.contains(&other) => {}
            other => log::debug!("codex sent the notification {} to the account process; it means nothing there", kind_name(other)),
        }
    }

    /// The process's output closed.
    fn finish(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;
        let exit = self.process.status().ok().flatten();
        let error = if self.shutting_down {
            None
        } else {
            Some(match exit {
                Some(exit) => self.process.exit_error(exit),
                None => AgentError::reported(ErrorKind::Other, "codex closed its output"),
            })
        };
        self.events.push_back(AccountEvent::Ended { exit, error });
    }
}

/// The state and log folders of one account process, in the account folder's temporary folder and
/// removed with everything in them when it drops: nothing an account process logs outlives it.
struct Scratch {
    path: std::path::PathBuf,
    state: super::StateFolders,
}

impl Scratch {
    /// A new private folder `codex-<pid>-<n>` with its state and log folders.
    fn new(account: &ConversationFolder) -> Result<Scratch, AgentError> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = account.tmp().join(format!("codex-{}-{n}", std::process::id()));
        let uid = crate::sys::current_uid();
        crate::folder::ensure_private(&path, uid)?;
        let state = super::StateFolders { sqlite_home: path.join(super::STATE_FOLDER), log_dir: path.join(super::LOG_FOLDER) };
        super::ensure_state(&state)?;
        Ok(Scratch { path, state })
    }
}

impl Drop for Scratch {
    /// Removes the folder; a failure is logged, as nothing can be returned from here.
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.path) {
            log::warn!("the Codex account process's folder {} stays: {e}", self.path.display());
        }
    }
}

/// An `initialize` that Codex does not take means a Codex Archforge cannot use.
fn missing_if_refused(error: AgentError, method: &str) -> AgentError {
    match error {
        AgentError::Reported { kind: ErrorKind::Other, detail } if detail.starts_with("codex refused") => {
            AgentError::reported(ErrorKind::MissingRuntime, &format!("codex refused {method}, so Archforge cannot use it: {detail}"))
        }
        other => other,
    }
}

/// Whether a refused renewal's words say the sign-in itself is gone (expired, revoked, or its
/// renewal token refused), rather than that the service could not be reached.
fn expired_words(detail: &str) -> bool {
    let lower = detail.to_lowercase();
    ["expired", "revoked", "already used", "invalid_grant", "sign in again", "log in again"].iter().any(|w| lower.contains(w))
}

/// A text field of an answer, cut to [`MAX_FIELD_BYTES`].
fn field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(|s| capped(s, MAX_FIELD_BYTES))
}

/// An answer that could not be read.
fn unreadable(method: &str, what: &str) -> AgentError {
    AgentError::reported(ErrorKind::Other, &format!("codex answered {method} with {what}"))
}

/// The account of an `account/read` answer.
fn account_state(value: &Value) -> AccountState {
    let requires_openai_auth = value.get("requiresOpenaiAuth").and_then(Value::as_bool).unwrap_or(true);
    let status = match value.get("account") {
        None | Some(Value::Null) => AccountStatus::SignedOut,
        Some(account) => match account.get("type").and_then(Value::as_str) {
            Some("apiKey") => AccountStatus::ApiKey,
            Some("chatgpt") => AccountStatus::ChatGpt {
                email: field(account, "email"),
                plan: field(account, "planType").map(|p| kind_name(&p)),
                workspace: field(account, "workspaceName"),
            },
            other => AccountStatus::Other { kind: kind_name(other.unwrap_or_default()) },
        },
    };
    AccountState { status, requires_openai_auth }
}

/// The rate limits of a `rateLimits` object.
fn rate_limits(limits: Option<&Value>, allowed: Option<&Value>) -> RateLimits {
    let window = |key: &str| {
        limits.and_then(|l| l.get(key)).filter(|w| w.is_object()).map(|w| RateLimitWindow {
            used_percent: w.get("usedPercent").and_then(Value::as_u64),
            window_minutes: w.get("windowDurationMins").and_then(Value::as_u64),
            resets_at: w.get("resetsAt").and_then(Value::as_i64),
        })
    };
    RateLimits {
        ordinary_usage_allowed: allowed.and_then(Value::as_bool),
        primary: window("primary"),
        secondary: window("secondary"),
        plan: limits.and_then(|l| field(l, "planType")).map(|p| kind_name(&p)),
        reached: limits.and_then(|l| field(l, "rateLimitReachedType")).map(|r| kind_name(&r)),
    }
}

/// One model of a `model/list` answer; one without an id is left out.
fn model_info(model: &Value) -> Option<ModelInfo> {
    let id = field(model, "id").filter(|id| !id.is_empty())?;
    Some(ModelInfo {
        model: field(model, "model").unwrap_or_else(|| id.clone()),
        display_name: field(model, "displayName").unwrap_or_else(|| id.clone()),
        description: model.get("description").and_then(Value::as_str).map(|d| capped(d, MAX_DESCRIPTION_BYTES)).unwrap_or_default(),
        hidden: model.get("hidden").and_then(Value::as_bool).unwrap_or(false),
        is_default: model.get("isDefault").and_then(Value::as_bool).unwrap_or(false),
        reasoning_efforts: model
            .get("supportedReasoningEfforts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(16)
            .filter_map(|e| e.get("reasoningEffort").and_then(Value::as_str).map(kind_name))
            .collect(),
        default_reasoning_effort: model.get("defaultReasoningEffort").and_then(Value::as_str).map(kind_name),
        id,
    })
}

/// A sign-in address Archforge may open: `https`, on a host of [`SIGN_IN_HOSTS`] or below one, no
/// user part, printable ASCII without a backslash, at most [`MAX_URL_BYTES`]. Refused as a typed error
/// naming the host, never opened.
pub fn checked_url(url: &str) -> Result<String, AgentError> {
    let refuse = |why: String| AgentError::reported(ErrorKind::Other, &format!("codex offered a sign-in address Archforge does not open: {why}"));
    if url.is_empty() || url.len() > MAX_URL_BYTES {
        return Err(refuse(format!("{} bytes long", url.len())));
    }
    // Printable ASCII only: no space, control, bidirectional or zero-width character, and no
    // backslash, which browsers read as a slash and so could end the host early.
    if !url.bytes().all(|b| b.is_ascii_graphic() && b != b'\\') {
        return Err(refuse("it holds a character other than printable ASCII, or a backslash".to_owned()));
    }
    let Some(rest) = url.strip_prefix("https://") else { return Err(refuse("it is not an https address".to_owned())) };
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(refuse("it names a user".to_owned()));
    }
    let host = authority.split(':').next().unwrap_or_default().to_ascii_lowercase();
    let allowed = SIGN_IN_HOSTS.iter().any(|h| host == *h || host.strip_suffix(h).is_some_and(|front| front.ends_with('.')));
    if !allowed {
        return Err(refuse(format!("its host {} is not one of {}", kind_name(&host.replace('.', "-")), SIGN_IN_HOSTS.join(", "))));
    }
    Ok(url.to_owned())
}

/// A device code to show: 1 to 64 ASCII letters, digits and hyphens.
fn checked_code(code: &str) -> Result<String, AgentError> {
    if code.is_empty() || code.chars().count() > MAX_USER_CODE || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(AgentError::reported(ErrorKind::Other, "codex offered a device code that is not letters, digits and hyphens"));
    }
    Ok(code.to_owned())
}

/// A sign-in id: 1 to 256 printable ASCII characters without spaces.
fn checked_id(id: &str) -> Result<String, AgentError> {
    if id.is_empty() || id.len() > MAX_FIELD_BYTES || !id.chars().all(|c| c.is_ascii_graphic()) {
        return Err(AgentError::InvalidSetting {
            setting: "sign-in id",
            bytes: id.len(),
            reason: "an id is 1 to 256 printable ASCII characters".to_owned(),
        });
    }
    Ok(id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only https addresses on the sign-in hosts pass.
    #[test]
    fn sign_in_addresses_are_checked() {
        for good in [
            "https://auth.openai.com/oauth/authorize?x=1",
            "https://auth.openai.com/codex/device",
            "https://chatgpt.com/x",
            "https://AUTH.OpenAI.com:443/a",
        ] {
            assert!(checked_url(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "http://auth.openai.com/",
            "https://evil.example/",
            "https://openai.com.evil.example/",
            "https://evilopenai.com/",
            "https://auth.openai.com@evil.example/",
            "https://auth.openai.com/a b",
            "https://auth.openai.com/\n",
            "https://evil.example\\.openai.com/",
            "https://auth.openai.com/\u{202E}x",
            "https://auth.openai.com/\u{200B}",
            "javascript:alert(1)",
        ] {
            assert!(checked_url(bad).is_err(), "{bad}");
        }
        assert_eq!(checked_code("ABCD-EFGH").unwrap(), "ABCD-EFGH");
        assert!(checked_code("ABCD EFGH").is_err() && checked_code("").is_err() && checked_code(&"A".repeat(65)).is_err());
        assert!(checked_id("login-1").is_ok() && checked_id("a b").is_err() && checked_id("").is_err());
    }

    /// Each account shape reads as its status; odd shapes never panic.
    #[test]
    fn accounts_are_read() {
        let read = |v: Value| account_state(&v).status;
        assert_eq!(read(json!({"account": null, "requiresOpenaiAuth": true})), AccountStatus::SignedOut);
        assert_eq!(read(json!({"account": {"type": "apiKey"}})), AccountStatus::ApiKey);
        assert_eq!(
            read(json!({"account": {"type": "chatgpt", "email": "person@example.com", "planType": "plus", "workspaceName": null}})),
            AccountStatus::ChatGpt { email: Some("person@example.com".into()), plan: Some("plus".into()), workspace: None }
        );
        assert_eq!(read(json!({"account": {"type": "gcp"}})), AccountStatus::Other { kind: "gcp".into() });
        assert_eq!(read(json!({"account": 7})), AccountStatus::Other { kind: "<unnamed>".into() });
        assert!(expired_words("codex refused account/read: refresh token was revoked"));
        assert!(!expired_words("codex refused account/read: error sending request"));
    }
}
