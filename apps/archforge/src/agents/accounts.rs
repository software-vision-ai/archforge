//! What talks to a provider or runs the codex program for Settings: the test message, Codex's
//! account, its ChatGPT sign-in and sign-out. Each runs on a tracked thread of the host's, stops
//! when Archforge quits, and is joined then.
//!
//! The test message runs without agent access: no tool server at all, in a scratch conversation
//! folder (`test-<16 hex>` in the private workspace) removed when it ends. The account calls each
//! start a short-lived `codex app-server` with the ChatGPT sign-in's store, Codex's own files in
//! the private `CODEX_HOME` ([`archforge_agents::codex::CredentialStore::File`]), in a scratch
//! folder the adapter removes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use archforge_agents::claude::{ClaudeConfig, ClaudeSession, Model, SessionId, Start};
use archforge_agents::codex::account::Account;
use archforge_agents::codex::{AccountEvent, Auth, CodexAccount as Program, CodexConfig, CodexHome, CodexSession, LoginOutcome};
use archforge_agents::{AgentError, AgentSession, ConversationId, ErrorKind, Limits, RuntimeKind, SessionEvent, TurnOutcome, Workspace};
use archforge_ui_egui::ai_agents::{AccountError, CodexAccount, CodexSignIn, LoginEvent, LoginWay, Provider};

use super::Inner;

/// The test message: one short message, asking for one word back.
const TEST_MESSAGE: &str = "Reply with the single word OK.";
/// How long the test message may take, and each account request.
const TEST_BUDGET: Duration = Duration::from_secs(120);
const REQUEST: Duration = Duration::from_secs(10);
/// How long a sign-in waits for the person to finish in the browser.
pub(super) const SIGN_IN_BUDGET: Duration = Duration::from_secs(15 * 60);
/// How long one wait for an event lasts, between looks at the quitting and cancel flags.
const TICK: Duration = Duration::from_millis(100);
/// The minutes a device code lasts, as Codex gives them.
const DEVICE_CODE_MINUTES: u32 = 15;

/// An adapter's failure as the page shows it: a refused key is the provider's refusal, anything
/// else in the adapter's words.
fn account_error(e: &AgentError) -> AccountError {
    match e.kind() {
        ErrorKind::InvalidCredential => AccountError::KeyRefused { detail: e.to_string() },
        _ => AccountError::Failed { detail: e.to_string() },
    }
}

/// A scratch conversation folder for one test message, removed when dropped.
struct Scratch {
    workspace: Workspace,
    id: ConversationId,
}

impl Scratch {
    /// A new folder `test-<16 hex>` in the private workspace.
    fn new(inner: &Inner) -> Result<(Scratch, archforge_agents::ConversationFolder), AccountError> {
        let failed = |e: AgentError| AccountError::Failed { detail: e.to_string() };
        let workspace = Workspace::open(&inner.workspace).map_err(failed)?;
        let id = ConversationId::new(&format!("test-{}", hex(8)?)).map_err(failed)?;
        let folder = workspace.conversation(&id).map_err(failed)?;
        Ok((Scratch { workspace, id }, folder))
    }
}

impl Drop for Scratch {
    /// Removes the folder and everything in it.
    fn drop(&mut self) {
        if let Err(e) = self.workspace.remove_conversation(&self.id) {
            log::warn!("the test message's folder {:?} could not be removed: {e}", self.id.as_str());
        }
    }
}

/// `bytes` random bytes as lowercase hexadecimal.
fn hex(bytes: usize) -> Result<String, AccountError> {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b).map_err(|e| AccountError::Failed { detail: format!("the system's random source failed: {e}") })?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// A random version-4 UUID, a test message's session id.
fn uuid() -> Result<SessionId, AccountError> {
    let h = hex(16)?;
    let text = format!(
        "{}-{}-4{}-8{}-{}",
        h.get(0..8).unwrap_or_default(),
        h.get(8..12).unwrap_or_default(),
        h.get(13..16).unwrap_or_default(),
        h.get(17..20).unwrap_or_default(),
        h.get(20..32).unwrap_or_default()
    );
    SessionId::new(&text).map_err(|e| AccountError::Failed { detail: e.to_string() })
}

/// Sends the test message to `provider` with `model`, without agent access, and waits for its
/// answer: within [`TEST_BUDGET`], given up at once when Archforge quits.
pub fn test_message(inner: &Arc<Inner>, provider: Provider, model: &str, sign_in: CodexSignIn) -> Result<(), AccountError> {
    let failed = |e: AgentError| account_error(&e);
    let runtime = inner.runtime(super::kind(provider)).map_err(failed)?;
    let (scratch, folder) = Scratch::new(inner)?;
    // The key's read may have waited on the store's prompt past a quit: nothing starts then.
    let closing = || if inner.quitting() { Err(AccountError::Failed { detail: "Archforge is closing".into() }) } else { Ok(()) };
    // No key, or no store to hold one: there is nothing to test with. A store that refused (locked,
    // a prompt declined) says so.
    let key = |p: Provider| {
        inner.key(p).map_err(|e| match e {
            archforge_secrets::SecretError::NotFound { .. } | archforge_secrets::SecretError::NoSecureStore { .. } => AccountError::NoCredential,
            e => AccountError::Failed { detail: format!("the {} API key could not be read: {e}", p.name()) },
        })
    };
    let mut program: Box<dyn AgentSession> = match provider {
        Provider::Claude => {
            let model = Model::new(model).map_err(failed)?;
            let config = ClaudeConfig { session: uuid()?, start: Start::New, model, bridge: None, limits: Limits::default() };
            let key = key(provider)?;
            closing()?;
            Box::new(ClaudeSession::start(&runtime, &folder, config, key).map_err(failed)?)
        }
        Provider::Codex => {
            let home = CodexHome::open(&scratch.workspace).map_err(failed)?;
            let auth = match sign_in {
                CodexSignIn::ApiKey => Auth::ApiKey(key(provider)?),
                CodexSignIn::ChatGpt => Auth::ChatGpt,
            };
            let config = CodexConfig {
                start: Start::New,
                model: if model == "default" { None } else { Some(Model::new(model).map_err(failed)?) },
                bridge: None,
                instructions: None,
                limits: Limits::default(),
                request_timeout: archforge_agents::codex::DEFAULT_REQUEST_TIMEOUT,
            };
            closing()?;
            Box::new(CodexSession::start(&runtime, &folder, &home, config, auth).map_err(failed)?)
        }
    };
    let answer = turn(inner, program.as_mut());
    if let Err(e) = program.shutdown(Duration::from_secs(2)) {
        log::warn!("the test message's program did not shut down cleanly: {e}");
    }
    drop(program);
    drop(scratch);
    log::info!("the test message to {} {}", provider.name(), if answer.is_ok() { "was answered" } else { "failed" });
    answer
}

/// One turn of the test message: sent, then its events read until the turn ends.
fn turn(inner: &Inner, program: &mut dyn AgentSession) -> Result<(), AccountError> {
    program.send(TEST_MESSAGE).map_err(|e| account_error(&e))?;
    let deadline = Instant::now() + TEST_BUDGET;
    while Instant::now() < deadline {
        if inner.quitting() {
            return Err(AccountError::Failed { detail: "Archforge is closing".into() });
        }
        match program.next_event(TICK) {
            Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Completed, .. }) => return Ok(()),
            Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. }) => return Err(account_error(&e)),
            Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Stopped, .. }) => {
                return Err(AccountError::Failed { detail: "the test message was stopped".into() });
            }
            Some(SessionEvent::Ended { error, .. }) => {
                return Err(
                    error.map_or_else(|| AccountError::Failed { detail: "the program ended without an answer".into() }, |e| account_error(&e))
                );
            }
            _ => {}
        }
    }
    Err(AccountError::Failed { detail: format!("no answer within {} seconds", TEST_BUDGET.as_secs()) })
}

/// A short-lived codex program for the account, with the ChatGPT sign-in's store (Codex's own files
/// in the private `CODEX_HOME`, never the secure store, which is not opened); none once Archforge
/// quits.
fn account_program(inner: &Inner) -> Result<Program, AccountError> {
    if inner.quitting() {
        return Err(AccountError::Failed { detail: "Archforge is closing".into() });
    }
    let failed = |e: AgentError| AccountError::Failed { detail: e.to_string() };
    let runtime = inner.runtime(RuntimeKind::Codex).map_err(failed)?;
    let workspace = Workspace::open(&inner.workspace).map_err(failed)?;
    let home = CodexHome::open(&workspace).map_err(failed)?;
    // The program's look (its version probe) may have run past a quit: no program starts then.
    if inner.quitting() {
        return Err(AccountError::Failed { detail: "Archforge is closing".into() });
    }
    Program::start(&runtime, &workspace, &home, Auth::ChatGpt.store(), Limits::default(), REQUEST, REQUEST).map_err(failed)
}

/// Ends a short-lived codex program.
fn finish(mut program: Program) {
    if let Err(e) = program.shutdown(Duration::from_secs(1)) {
        log::warn!("the codex account program did not shut down cleanly: {e}");
    }
}

/// Who Codex is signed in as, from a short-lived codex program; sends nothing.
pub fn read(inner: &Arc<Inner>) -> Result<CodexAccount, AccountError> {
    let mut program = account_program(inner)?;
    let state = program.read(false, REQUEST);
    finish(program);
    let account = match state {
        Ok(state) => match state.account {
            Some(Account::ChatGpt { email, plan }) => CodexAccount::SignedIn { email: email.unwrap_or_default(), plan },
            // An API key kept by the codex program is not a ChatGPT sign-in.
            Some(Account::ApiKey | Account::Other { .. }) | None => CodexAccount::NotSignedIn,
        },
        Err(e) if e.kind() == ErrorKind::SignInExpired => CodexAccount::Expired { email: String::new() },
        Err(e) => return Err(AccountError::Failed { detail: e.to_string() }),
    };
    // Whether Codex is signed in, never who.
    log::info!("Codex's account was read: {}", if matches!(account, CodexAccount::SignedIn { .. }) { "signed in" } else { "not signed in" });
    Ok(account)
}

/// Signs Codex in with ChatGPT: the browser page or the device code goes to the window, then the
/// sign-in's end is awaited within [`SIGN_IN_BUDGET`]. The window hears how it ended: signed in,
/// or why not (not finished in time, cancelled by the codex program, or another failure); only a
/// cancel the person asked for, or Archforge quitting, ends it without a word, as nobody waits.
pub fn sign_in(inner: &Arc<Inner>, way: LoginWay, cancel: &AtomicBool, tx: &Sender<LoginEvent>) {
    let result = signed_in(inner, way, cancel, tx);
    let event = match result {
        Ok(Some(account)) => LoginEvent::Done(account),
        Ok(None) => return,
        Err(e) => LoginEvent::Failed(e),
    };
    let _ = tx.send(event);
}

/// [`sign_in`]'s work: the account once signed in, none when cancelled.
fn signed_in(inner: &Arc<Inner>, way: LoginWay, cancel: &AtomicBool, tx: &Sender<LoginEvent>) -> Result<Option<CodexAccount>, AccountError> {
    let failed = |e: AgentError| AccountError::Failed { detail: e.to_string() };
    let mut program = account_program(inner)?;
    let login_id = match way {
        LoginWay::Browser => {
            let login = program.login_chatgpt(REQUEST).map_err(failed)?;
            let _ = tx.send(LoginEvent::OpenBrowser { url: login.auth_url.clone() });
            login.login_id
        }
        LoginWay::DeviceCode => {
            let login = program.login_device_code(REQUEST).map_err(failed)?;
            let _ =
                tx.send(LoginEvent::DeviceCode { code: login.user_code.clone(), url: login.verification_url.clone(), minutes: DEVICE_CODE_MINUTES });
            login.login_id
        }
    };
    let budget = Duration::from_millis(inner.sign_in_millis.load(Ordering::SeqCst));
    let deadline = Instant::now() + budget;
    // Asked to stop here: by the person's cancel, or Archforge quitting.
    let stopped = || cancel.load(Ordering::SeqCst) || inner.quitting();
    let outcome = loop {
        let timed_out = Instant::now() >= deadline;
        if stopped() || timed_out {
            let _ = program.cancel_login(&login_id, REQUEST);
            finish(program);
            if timed_out && !stopped() {
                log::info!("the ChatGPT sign-in was not finished within {} s and was cancelled", budget.as_secs());
                return Err(AccountError::SignInTimedOut);
            }
            log::info!("the ChatGPT sign-in was cancelled");
            return Ok(None);
        }
        match program.next_event(TICK) {
            Some(AccountEvent::LoginCompleted { outcome, .. }) => break outcome,
            Some(AccountEvent::Ended { error, .. }) => {
                return Err(error.map_or_else(|| AccountError::Failed { detail: "the codex program ended".into() }, failed));
            }
            _ => {}
        }
    };
    let answer = match outcome {
        LoginOutcome::SignedIn => {
            let state = program.read(false, REQUEST).map_err(failed);
            state.map(|s| match s.account {
                Some(Account::ChatGpt { email, plan }) => Some(CodexAccount::SignedIn { email: email.unwrap_or_default(), plan }),
                _ => Some(CodexAccount::NotSignedIn),
            })
        }
        // A cancel nobody here asked for is the codex program's: the window is told.
        LoginOutcome::Cancelled if stopped() => Ok(None),
        LoginOutcome::Cancelled => Err(AccountError::SignInCancelled),
        LoginOutcome::Failed { detail } => Err(AccountError::Failed { detail }),
    };
    finish(program);
    log::info!(
        "the ChatGPT sign-in ended: {}",
        if matches!(answer, Ok(Some(CodexAccount::SignedIn { .. }))) { "signed in" } else { "not signed in" }
    );
    answer
}

/// Signs Codex out.
pub fn sign_out(inner: &Arc<Inner>) -> Result<(), AccountError> {
    let mut program = account_program(inner)?;
    let done = program.logout(REQUEST).map_err(|e| AccountError::Failed { detail: e.to_string() });
    finish(program);
    log::info!("Codex was signed out: {}", done.is_ok());
    done
}
