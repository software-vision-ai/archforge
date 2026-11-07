//! The host's side of AI Agents and Chat, one service ([`AgentHost`]) for the shell: finding the
//! agent programs, keeping their API keys in the system's secure store, the test message, Codex's
//! account and sign-in, and Chat's conversations with their agent access. Every call returns at
//! once; the work runs on threads of the host's own, each tracked, which stop when Archforge quits
//! and are joined then, within a budget ([`Agents::shutdown`]): a thread still waiting past it (on
//! the secure store's prompt, or a codex program that does not answer) is named in the log and its
//! program ends with Archforge's process, as its input closes.
//!
//! - The look for the programs runs at start; the store is opened only when the keys are first
//!   needed, never at launch. Codex's account is read only when the shell first asks (the AI
//!   Agents page, or a Codex conversation): reading it starts `codex app-server`.
//! - Keys come from `archforge_secrets` as `Secret` alone, and go to a program only through its
//!   adapter; they are never logged, and nothing here shows them in a `Debug`.
//! - A ChatGPT sign-in is kept by the codex program in the system keyring when the secure store
//!   opens (the check `KeyringStore::open` makes), and otherwise in its own files in Archforge's
//!   private `CODEX_HOME` ([`chatgpt_store`]).
//! - Conversations ([`sessions`]) and agent access ([`access`]) are described in their modules.

mod access;
mod accounts;
mod sessions;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, OnceLock};

use archforge_agents::codex::ChatGptStore;
use archforge_agents::{Discovery, HostEnv, RuntimeKind, Workspace};
use archforge_secrets::{KeyringStore, Secret, SecretError, SecretName, SecretStore};
use archforge_ui_egui::ai_agents::{
    AccountError, AgentHost, CodexAccount, CodexSignIn, Key, KeyText, Keys, LoginEvent, LoginWay, Look, Program, Provider, Store, System, last_four,
};
use archforge_ui_egui::chat::{ChatRefusal, ChatStart, EventSink};

pub use access::{Window, start_control};

/// How long quitting waits for the host's threads: a conversation's program gets twice
/// `sessions::GRACE` to end, an account call its request time.
const QUIT_BUDGET: std::time::Duration = std::time::Duration::from_secs(12);

/// A secure store as the host keeps it: the system's, or another for the tests.
type Opened = Result<Box<dyn SecretStore>, SecretError>;

/// What opens the secure store: the system's ([`KeyringStore::open`]), or a test's.
type Opener = Arc<dyn Fn() -> Opened + Send + Sync>;

/// Where the command-line tool agents run as their tool server is found.
type CliPath = Arc<dyn Fn() -> Result<PathBuf, String> + Send + Sync>;

/// What the host's threads share.
pub(crate) struct Inner {
    /// The folder the programs run in, private to this user (`agents` in the settings folder).
    workspace: PathBuf,
    /// The environment the programs are looked for in: this process's unless given.
    env: Option<HostEnv>,
    /// The secure store, opened once, on the first thread that needs it: opening may block, and on
    /// Linux its answer holds until Archforge restarts.
    store: OnceLock<Opened>,
    opener: Opener,
    cli: CliPath,
    /// Set when Archforge quits: no thread starts after it, and every running one stops.
    quitting: AtomicBool,
    /// Every thread the host started, joined on quit.
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
    /// Where the conversations' events go, once the window connected.
    sink: Mutex<Option<EventSink>>,
    /// Chat's agent access.
    access: Mutex<access::Access>,
    /// The running conversations, by id.
    sessions: Mutex<HashMap<String, sessions::Handle>>,
    /// The conversations whose private folders are being removed: none of them starts meanwhile,
    /// and a start refused keeps the folder (AR-218). Taken before `sessions` when both are held.
    forgetting: Mutex<HashMap<String, sessions::Removing>>,
    /// For the tests: run by a removal once it decided, before the folder goes, to put a start in
    /// that exact place.
    #[cfg(test)]
    as_folder_goes: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// The sign-in under way: its cancel flag.
    login: Mutex<Option<Arc<AtomicBool>>>,
    /// How long a sign-in waits for the person to finish, in milliseconds (`accounts::SIGN_IN_BUDGET`;
    /// the tests shorten it).
    sign_in_millis: AtomicU64,
}

/// A lock a panicking thread poisoned is taken over: what it guards stays usable.
fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Inner {
    /// Runs `work` on a named thread the host tracks; refused once Archforge quits, or when the
    /// system cannot start a thread (said in the log).
    fn spawn(self: &Arc<Self>, name: &str, work: impl FnOnce() + Send + 'static) -> bool {
        let mut threads = locked(&self.threads);
        // Checked under the lock quitting takes too, so no thread starts after the quit took the
        // threads it joins.
        if self.quitting.load(Ordering::SeqCst) {
            return false;
        }
        // Finished threads are joined as new ones start, so the list holds only live ones.
        threads.retain(|t| !t.is_finished());
        match std::thread::Builder::new().name(name.into()).spawn(work) {
            Ok(t) => {
                threads.push(t);
                true
            }
            Err(e) => {
                log::warn!("the {name} thread could not start: {e}");
                false
            }
        }
    }

    /// Runs `work` on a tracked thread and sends its answer; a thread that cannot start sends
    /// nothing, which the page reads as an answer that never came.
    fn answer<T: Send + 'static>(self: &Arc<Self>, name: &str, work: impl FnOnce(&Arc<Inner>) -> T + Send + 'static) -> Receiver<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        let inner = self.clone();
        self.spawn(name, move || {
            let _ = tx.send(work(&inner));
        });
        rx
    }

    /// Whether Archforge is quitting.
    fn quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }

    /// The secure store, opened the first time.
    fn store(&self) -> &Opened {
        self.store.get_or_init(|| (self.opener)())
    }

    /// The window's event sink, once it connected.
    fn sink(&self) -> Option<EventSink> {
        locked(&self.sink).clone()
    }

    /// The program of `kind` on this computer, looked for in the private workspace.
    fn runtime(&self, kind: RuntimeKind) -> Result<archforge_agents::Runtime, archforge_agents::AgentError> {
        let env = self.env.clone().unwrap_or_else(HostEnv::from_process);
        let workspace = Workspace::open(&self.workspace)?;
        archforge_agents::find(kind, &env, &workspace)?.into_result()
    }

    /// The API key of `provider` from the secure store, as a `Secret`.
    fn key(&self, provider: Provider) -> Result<Secret, SecretError> {
        self.store().as_ref().map_err(Clone::clone)?.get(&secret_name(provider))
    }
}

/// Where the codex program keeps a ChatGPT sign-in (owner decision): the system keyring when the
/// secure store opens, the same check `KeyringStore::open` makes; otherwise its own files in
/// Archforge's private `CODEX_HOME`.
fn chatgpt_store(opened: &Opened) -> ChatGptStore {
    if opened.is_ok() { ChatGptStore::Keyring } else { ChatGptStore::File }
}

/// The agent programs, keys, accounts and conversations of this computer.
pub struct Agents {
    inner: Arc<Inner>,
}

/// The system's secure store, opened as `archforge_secrets` opens it.
fn open_system_store() -> Opened {
    KeyringStore::open().map(|s| Box::new(s) as Box<dyn SecretStore>)
}

/// The command-line tool beside the application, as `archforge_control` finds it.
fn system_cli() -> Result<PathBuf, String> {
    archforge_control::cli_path().map_err(|e| e.to_string())
}

impl Agents {
    /// The host over the system's secure store and this process's environment; token files go in
    /// `settings`, the programs run in `settings/agents`, and requests reach the shell through
    /// `window`.
    pub fn new(settings: &Path, window: Window) -> Agents {
        Agents::build(settings, Arc::new(open_system_store), None, Arc::new(system_cli), window)
    }

    /// The host over `store` and `env` in place of the system's, with no tool server to give:
    /// for the tests, which never touch the real store or an installed program.
    #[cfg(test)]
    pub fn with(settings: &Path, store: Box<dyn SecretStore>, env: HostEnv) -> Agents {
        let (window, _) = Window::new();
        let agents = Agents::build(
            settings,
            Arc::new(|| Err(SecretError::NoSecureStore { reason: "not used".into() })),
            Some(env),
            Arc::new(|| Err("no tool here".into())),
            window,
        );
        let _ = agents.inner.store.set(Ok(store));
        agents
    }

    /// The host with everything given: for [`Agents::new`] and the tests.
    fn build(settings: &Path, opener: Opener, env: Option<HostEnv>, cli: CliPath, window: Window) -> Agents {
        Agents {
            inner: Arc::new(Inner {
                workspace: settings.join("agents"),
                env,
                store: OnceLock::new(),
                opener,
                cli,
                quitting: AtomicBool::new(false),
                threads: Mutex::new(Vec::new()),
                sink: Mutex::new(None),
                access: Mutex::new(access::Access::new(settings, window)),
                sessions: Mutex::new(HashMap::new()),
                forgetting: Mutex::new(HashMap::new()),
                #[cfg(test)]
                as_folder_goes: Mutex::new(None),
                login: Mutex::new(None),
                sign_in_millis: AtomicU64::new(u64::try_from(accounts::SIGN_IN_BUDGET.as_millis()).unwrap_or(u64::MAX)),
            }),
        }
    }

    /// At quit: no thread starts any more, agent access closes and stays closed (every token
    /// revoked, held connections answered and closed), the sign-in under way is cancelled, every
    /// conversation is ended and its program shut down, and the host's threads are joined, so the
    /// programs have ended when it returns. The wait is bounded by [`QUIT_BUDGET`]: a thread still
    /// waiting past it (on the secure store's own prompt, or on a codex program that does not
    /// answer within its request time) is left and named in the log.
    pub fn shutdown(&self) {
        let inner = &self.inner;
        let threads = {
            let mut threads = locked(&inner.threads);
            inner.quitting.store(true, Ordering::SeqCst);
            std::mem::take(&mut *threads)
        };
        locked(&inner.access).set(false, inner.sink().as_ref());
        if let Some(cancel) = locked(&inner.login).take() {
            cancel.store(true, Ordering::SeqCst);
        }
        for handle in locked(&inner.sessions).values() {
            handle.end();
        }
        let deadline = std::time::Instant::now() + QUIT_BUDGET;
        while threads.iter().any(|t| !t.is_finished()) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut left = Vec::new();
        for t in threads {
            if t.is_finished() {
                let _ = t.join();
            } else {
                left.push(t.thread().name().unwrap_or("agents").to_string());
            }
        }
        if left.is_empty() {
            log::info!("the agents host stopped: every program ended and every thread joined");
        } else {
            log::warn!(
                "the agents host stopped with {} thread(s) still waiting after {} s, left to end with Archforge: {left:?}",
                left.len(),
                QUIT_BUDGET.as_secs()
            );
        }
    }
}

/// The store's name for the page; a missing store says why.
fn store_kind(store: &Opened) -> Store {
    match store {
        Ok(_) if cfg!(target_os = "macos") => Store::MacKeychain,
        Ok(_) if cfg!(target_os = "linux") => Store::SecretService,
        Ok(_) => Store::Other,
        Err(e) => Store::Missing { why: e.to_string() },
    }
}

/// The name `provider`'s API key is kept under in the secure store.
fn secret_name(p: Provider) -> SecretName {
    match p {
        Provider::Claude => SecretName::ANTHROPIC_API_KEY,
        Provider::Codex => SecretName::OPENAI_API_KEY,
    }
}

/// The runtime that drives `provider`.
fn kind(p: Provider) -> RuntimeKind {
    match p {
        Provider::Claude => RuntimeKind::Claude,
        Provider::Codex => RuntimeKind::Codex,
    }
}

/// A key as the page shows it: saved with its last four, absent, or why it could not be read.
/// The secret itself is overwritten as it drops here. A store that turns out to have no system
/// behind it answers the reason, so the page says there is no secure store.
fn key_of(store: &Opened, p: Provider) -> (Key, Option<String>) {
    let Ok(store) = store else { return (Key::Absent, None) };
    match store.get(&secret_name(p)) {
        Ok(secret) => (Key::Saved { last4: last_four(secret.expose()) }, None),
        Err(SecretError::NotFound { .. }) => (Key::Absent, None),
        Err(e @ SecretError::NoSecureStore { .. }) => (Key::Absent, Some(e.to_string())),
        Err(e) => (Key::Unreadable { why: e.to_string() }, None),
    }
}

/// What the search found of one program.
fn program_of(p: Provider, env: &HostEnv, workspace: &Result<Workspace, String>) -> Program {
    let workspace = match workspace {
        Ok(w) => w,
        Err(why) => return Program::Unusable { why: why.clone() },
    };
    match archforge_agents::find(kind(p), env, workspace) {
        Ok(Discovery::Found(runtime)) => Program::Found { version: runtime.version().to_string(), tested: runtime.tested() },
        Ok(Discovery::TooOld { version, floor, .. }) => Program::TooOld { version: version.to_string(), floor: floor.to_string() },
        Ok(Discovery::Unsupported { reason, .. }) => Program::Unusable { why: reason.to_string() },
        Ok(Discovery::Missing { .. }) => Program::Missing,
        Ok(Discovery::NotAvailable { .. }) => Program::Unusable { why: "agent programs are not available on this system yet".into() },
        Err(e) => Program::Unusable { why: e.to_string() },
    }
}

impl Agents {
    /// Whether `text` holds one of the API keys the secure store keeps, checked on one of the
    /// host's threads (reading a key may wait for the system to ask the person): none when it holds
    /// none, else the provider of the key it holds when the key is one of theirs. A system without a
    /// secure store holds no key; a store that cannot be read is an error, as the text could then
    /// not be checked. Neither the text nor a key is in the answer.
    pub fn check_keys(&self, text: String) -> Receiver<Result<Option<Option<Provider>>, String>> {
        self.inner.answer("agents-check-keys", move |inner| match inner.store() {
            Err(SecretError::NoSecureStore { .. }) => Ok(None),
            Err(e) => Err(format!("the saved keys could not be read to check the question: {e}")),
            Ok(store) => match archforge_secrets::held_key_in(store.as_ref(), &text) {
                Ok(None) => Ok(None),
                Ok(Some(name)) if name == SecretName::OPENAI_API_KEY => Ok(Some(Some(Provider::Codex))),
                Ok(Some(name)) if name == SecretName::ANTHROPIC_API_KEY => Ok(Some(Some(Provider::Claude))),
                Ok(Some(_)) => Ok(Some(None)),
                Err(e) => Err(format!("the saved keys could not be read to check the question: {e}")),
            },
        })
    }
}

impl AgentHost for Agents {
    /// Agents run on macOS and Linux for now.
    fn system(&self) -> System {
        if cfg!(any(target_os = "macos", target_os = "linux")) { System::Supported } else { System::NotYet }
    }

    /// Looks for both programs on a thread of its own; the secure store is not touched.
    fn look(&self) -> Receiver<Look> {
        self.inner.answer("agents-look", |inner| {
            let env = inner.env.clone().unwrap_or_else(HostEnv::from_process);
            let workspace = Workspace::open(&inner.workspace).map_err(|e| e.to_string());
            // The programs only: the secure store is not touched here, so launching Archforge
            // never makes the system ask for access to it.
            let look = Look {
                claude: program_of(Provider::Claude, &env, &workspace),
                codex: program_of(Provider::Codex, &env, &workspace),
                ..Look::default()
            };
            log::info!("agent programs: claude {:?}, codex {:?}", look.claude, look.codex);
            look
        })
    }

    /// Opens the secure store the first time and reads which keys are saved, by their last four.
    fn read_keys(&self) -> Receiver<Keys> {
        self.inner.answer("agents-keys", |inner| {
            let store = inner.store();
            let (claude_key, gone) = key_of(store, Provider::Claude);
            let (codex_key, gone_too) = key_of(store, Provider::Codex);
            let keys = Keys {
                store: match gone.or(gone_too) {
                    Some(why) => Store::Missing { why },
                    None => store_kind(store),
                },
                claude_key,
                codex_key,
                sign_in_files: chatgpt_store(store) == ChatGptStore::File,
            };
            // The store's state and whether each key is saved; never a key, nor its last four.
            log::info!("secure store {:?}; keys saved: claude {}, codex {}", keys.store, keys.claude_key.saved(), keys.codex_key.saved());
            keys
        })
    }

    /// Saves a key in the secure store; the key goes there whole and nowhere else.
    fn save_key(&self, provider: Provider, key: KeyText) -> Receiver<Result<Key, String>> {
        self.inner.answer("agents-key", move |inner| {
            let store = inner.store().as_ref().map_err(|e| e.to_string())?;
            let last4 = last_four(key.expose());
            let secret = Secret::new(key.take()).map_err(|e| e.to_string())?;
            store.set(&secret_name(provider), &secret).map_err(|e| e.to_string())?;
            log::info!("the {} API key was saved in the secure store", provider.name());
            Ok(Key::Saved { last4 })
        })
    }

    /// Removes a key from the secure store.
    fn remove_key(&self, provider: Provider) -> Receiver<Result<Key, String>> {
        self.inner.answer("agents-key", move |inner| {
            let store = inner.store().as_ref().map_err(|e| e.to_string())?;
            store.delete(&secret_name(provider)).map_err(|e| e.to_string())?;
            log::info!("the {} API key was removed from the secure store", provider.name());
            Ok(Key::Absent)
        })
    }

    /// The test message, on a tracked thread ([`accounts::test_message`]).
    fn test_message(&self, provider: Provider, model: &str, sign_in: CodexSignIn) -> Receiver<Result<(), AccountError>> {
        let model = model.to_string();
        self.inner.answer("agents-test", move |inner| accounts::test_message(inner, provider, &model, sign_in))
    }

    /// Codex's account, read by a short-lived codex program ([`accounts::read`]).
    fn codex_account_read(&self) -> Receiver<Result<CodexAccount, AccountError>> {
        self.inner.answer("agents-account", accounts::read)
    }

    /// Starts a ChatGPT sign-in ([`accounts::sign_in`]); one at a time, a new one cancelling the
    /// one before.
    fn codex_login_start(&self, way: LoginWay) -> Receiver<LoginEvent> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(before) = locked(&self.inner.login).replace(cancel.clone()) {
            before.store(true, Ordering::SeqCst);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let inner = self.inner.clone();
        self.inner.spawn("agents-sign-in", move || accounts::sign_in(&inner, way, &cancel, &tx));
        rx
    }

    /// Cancels the sign-in under way.
    fn codex_login_cancel(&self) {
        if let Some(cancel) = locked(&self.inner.login).take() {
            cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Signs Codex out ([`accounts::sign_out`]).
    fn codex_logout(&self) -> Receiver<Result<(), AccountError>> {
        self.inner.answer("agents-sign-out", accounts::sign_out)
    }

    /// Keeps the window's sink for the conversations' events.
    fn connect(&self, sink: EventSink) {
        *locked(&self.inner.sink) = Some(sink);
    }

    /// Let Chat use the model: off closes agent access at once.
    fn set_model_access(&self, on: bool) {
        let sink = self.inner.sink();
        locked(&self.inner.access).set(on, sink.as_ref());
        log::info!("Let Chat use the model is {}", if on { "on" } else { "off" });
    }

    /// Starts a conversation ([`sessions::start`]).
    fn start(&self, request: ChatStart) -> Result<(), ChatRefusal> {
        sessions::start(&self.inner, request)
    }

    /// Sends the person's message to a running conversation.
    fn send(&self, chat: &str, text: String) -> Result<(), ChatRefusal> {
        sessions::command(&self.inner, chat, sessions::Command::Send(text))
    }

    /// Stops a running conversation's turn.
    fn stop(&self, chat: &str) -> Result<(), ChatRefusal> {
        sessions::command(&self.inner, chat, sessions::Command::Stop)
    }

    /// Ends a running conversation.
    fn end(&self, chat: &str) -> Result<(), ChatRefusal> {
        sessions::command(&self.inner, chat, sessions::Command::End)
    }

    /// Removes a deleted conversation's private folder ([`sessions::forget`]).
    fn forget(&self, chat: &str) -> Receiver<Result<bool, String>> {
        sessions::forget(&self.inner, chat, Vec::new())
    }

    /// Removes it once every one of the project's answers lets it go ([`sessions::forget`]).
    fn forget_after(&self, chat: &str, answers: archforge_ui_egui::ai_agents::Awaited) -> Receiver<Result<bool, String>> {
        sessions::forget(&self.inner, chat, answers)
    }
}
