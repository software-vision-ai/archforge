//! The one guard every test that starts a real project host goes through: it owns the host, its
//! agents if any, and the test's folder, and on drop stops them the way the application does at
//! exit and removes the folder, so a test that panics or fails an assertion leaves no database
//! server running and no folder behind.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::projects::Projects;

/// A real project host in a folder of the test's own, stopped and removed on drop.
pub(crate) struct Sandbox {
    pub(crate) base: PathBuf,
    pub(crate) host: Arc<Projects>,
    pub(crate) agents: Option<Arc<crate::agents::Agents>>,
}

impl Sandbox {
    /// The host, with its folder created and `keys` wired in.
    pub(crate) fn new(keys: Option<Box<dyn archforge_secrets::SecretStore>>) -> Sandbox {
        Sandbox::named("d", keys)
    }

    /// The same, its folder named after `name` (`af-<name>-<process>-<n>`) so one left behind
    /// says which test made it.
    pub(crate) fn named(name: &str, keys: Option<Box<dyn archforge_secrets::SecretStore>>) -> Sandbox {
        let base = (0..1000)
            .map(|n| std::env::temp_dir().join(format!("af-{name}-{}-{n}", std::process::id())))
            .find(|p| std::fs::create_dir(p).is_ok())
            .expect("a folder of the test's own");
        let host = Arc::new(Projects::new(&base.join("settings"), "Archforge test".into()));
        let agents = keys.map(|k| Arc::new(crate::agents::Agents::with(&base.join("settings"), k, archforge_agents::HostEnv::default())));
        if let Some(a) = &agents {
            host.check_descriptions_with(a.clone());
        }
        Sandbox { base, host, agents }
    }
}

impl Drop for Sandbox {
    /// Stops the servers and threads through the application's own exit path (each server is asked
    /// to stop, waited for within its stop budget, then killed), then removes the folder, retrying
    /// while a stopping server still writes. It never panics, so it is safe while a failed test
    /// unwinds; what it could not do goes to the test's output.
    fn drop(&mut self) {
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.host.shutdown();
            if let Some(a) = &self.agents {
                a.shutdown();
            }
        }));
        if stopped.is_err() {
            eprintln!("the test host in {} did not shut down cleanly", self.base.display());
        }
        for _ in 0..50 {
            match std::fs::remove_dir_all(&self.base) {
                Ok(()) => return,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        eprintln!("the test folder {} could not be removed", self.base.display());
    }
}
