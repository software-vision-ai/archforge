//! The host's AI Agents services: finding the agent programs and keeping their API keys in the
//! system's secure store, each on a thread of its own so the window never waits. Nothing here
//! sends anything over the network. The store is opened only when the keys are first needed,
//! never at launch.
//!
//! The test message and Codex's ChatGPT sign-in come with the agents' adapters (AR-202, AR-203);
//! until then the accounts service answers that they are not available yet.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, OnceLock};

use archforge_agents::{Discovery, HostEnv, RuntimeKind, Workspace};
use archforge_secrets::{KeyringStore, Secret, SecretError, SecretName, SecretStore};
use archforge_ui_egui::ai_agents::{AgentHost, Key, KeyText, Keys, Look, Program, Provider, Store, System, last_four};

/// A secure store as the host keeps it: the system's, or another for the tests.
type Opened = Result<Box<dyn SecretStore>, SecretError>;

/// The agent programs and keys of this computer.
pub struct Agents {
    /// The folder the version probes run in: private to this user.
    workspace: PathBuf,
    /// The environment the programs are looked for in: this process's unless given.
    env: Option<HostEnv>,
    /// The secure store, opened once, on the first thread that needs it: opening may block, and on
    /// Linux its answer holds until Archforge restarts.
    store: Arc<OnceLock<Opened>>,
}

fn open_system_store() -> Opened {
    KeyringStore::open().map(|s| Box::new(s) as Box<dyn SecretStore>)
}

impl Agents {
    pub fn new(settings: &std::path::Path) -> Agents {
        Agents { workspace: settings.join("agents"), env: None, store: Arc::new(OnceLock::new()) }
    }

    /// Agents over `store` and `env` in place of the system's: for the tests, which never touch
    /// the real store or an installed program.
    #[cfg(test)]
    pub fn with(settings: &std::path::Path, store: Box<dyn SecretStore>, env: HostEnv) -> Agents {
        let lock = OnceLock::new();
        let _ = lock.set(Ok(store));
        Agents { workspace: settings.join("agents"), env: Some(env), store: Arc::new(lock) }
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

fn secret_name(p: Provider) -> SecretName {
    match p {
        Provider::Claude => SecretName::ANTHROPIC_API_KEY,
        Provider::Codex => SecretName::OPENAI_API_KEY,
    }
}

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

/// Runs `work` on a named thread and sends its answer; a thread that cannot start sends nothing,
/// which the page reads as an answer that never came.
fn on_thread<T: Send + 'static>(name: &str, work: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Err(e) = std::thread::Builder::new().name(name.into()).spawn(move || {
        let _ = tx.send(work());
    }) {
        log::warn!("the {name} thread could not start: {e}");
    }
    rx
}

impl AgentHost for Agents {
    fn system(&self) -> System {
        if cfg!(any(target_os = "macos", target_os = "linux")) { System::Supported } else { System::NotYet }
    }

    fn look(&self) -> Receiver<Look> {
        let (root, env) = (self.workspace.clone(), self.env.clone());
        on_thread("agents-look", move || {
            let env = env.unwrap_or_else(HostEnv::from_process);
            let workspace = Workspace::open(&root).map_err(|e| e.to_string());
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

    fn read_keys(&self) -> Receiver<Keys> {
        let store = self.store.clone();
        on_thread("agents-keys", move || {
            let store = store.get_or_init(open_system_store);
            let (claude_key, gone) = key_of(store, Provider::Claude);
            let (codex_key, gone_too) = key_of(store, Provider::Codex);
            let keys = Keys {
                store: match gone.or(gone_too) {
                    Some(why) => Store::Missing { why },
                    None => store_kind(store),
                },
                claude_key,
                codex_key,
            };
            // The store's state and whether each key is saved; never a key, nor its last four.
            log::info!("secure store {:?}; keys saved: claude {}, codex {}", keys.store, keys.claude_key.saved(), keys.codex_key.saved());
            keys
        })
    }

    fn save_key(&self, provider: Provider, key: KeyText) -> Receiver<Result<Key, String>> {
        let store = self.store.clone();
        on_thread("agents-key", move || {
            let store = store.get_or_init(open_system_store).as_ref().map_err(|e| e.to_string())?;
            let last4 = last_four(key.expose());
            let secret = Secret::new(key.take()).map_err(|e| e.to_string())?;
            store.set(&secret_name(provider), &secret).map_err(|e| e.to_string())?;
            log::info!("the {} API key was saved in the secure store", provider.name());
            Ok(Key::Saved { last4 })
        })
    }

    fn remove_key(&self, provider: Provider) -> Receiver<Result<Key, String>> {
        let store = self.store.clone();
        on_thread("agents-key", move || {
            let store = store.get_or_init(open_system_store).as_ref().map_err(|e| e.to_string())?;
            store.delete(&secret_name(provider)).map_err(|e| e.to_string())?;
            log::info!("the {} API key was removed from the secure store", provider.name());
            Ok(Key::Absent)
        })
    }
}
