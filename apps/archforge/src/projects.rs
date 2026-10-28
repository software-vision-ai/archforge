//! The host's projects: creates them on a worker thread and keeps the one that is open.
//!
//! One creation runs at a time. A new project replaces the open one only once it exists, so a
//! failed creation leaves the open project as it was. Dolt is found on the worker the first time,
//! never on the interface thread. On exit the running creation is cancelled and waited for, then the
//! open project's server is stopped.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_project::{Hooks, Project};
use archforge_ui_egui::{CreateError, Opened, ProjectHost};

pub struct Projects {
    inner: Arc<Inner>,
}

struct Inner {
    /// The application's settings folder: Dolt's home and the server records go there.
    settings: PathBuf,
    /// Who creates projects, for their settings: the version line.
    created_by: String,
    /// Held for a whole creation, so two never race.
    job: Mutex<()>,
    open: Mutex<Option<Project>>,
    dolt: Mutex<Option<Dolt>>,
    cancel: Arc<AtomicBool>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// A lock that a panic elsewhere left poisoned is taken over: the data behind it is still whole.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Projects {
    pub fn new(settings: &Path, created_by: String) -> Projects {
        Projects {
            inner: Arc::new(Inner {
                settings: settings.to_path_buf(),
                created_by,
                job: Mutex::new(()),
                open: Mutex::new(None),
                dolt: Mutex::new(None),
                cancel: Arc::new(AtomicBool::new(false)),
                worker: Mutex::new(None),
            }),
        }
    }

    /// Cancels a running creation and waits for it within the stop budget (std's join has no
    /// timeout, so it polls), then stops the open project's server. Runs on exit, on the interface
    /// thread: it may hold it for up to about twice the stop budget.
    pub fn shutdown(&self) {
        self.inner.cancel.store(true, Ordering::SeqCst);
        // A start in progress can outlast this wait (its ready budget is longer): it is then left,
        // and the marker and recovery cover what it leaves.
        let budget = ServerOptions::new(self.inner.settings.join("servers")).stop_budget;
        let started = Instant::now();
        if let Some(worker) = lock(&self.inner.worker).take() {
            while !worker.is_finished() && started.elapsed() < budget {
                std::thread::sleep(Duration::from_millis(20));
            }
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                log::warn!("a project creation was still running at exit; the next start recovers what it left");
            }
        }
        if let Some(project) = lock(&self.inner.open).take() {
            let path = project.path().display().to_string();
            if let Err(e) = project.close() {
                log::error!("the project {path} did not close cleanly: {e}");
            }
        }
    }
}

impl ProjectHost for Projects {
    fn create(&self, folder: &Path, name: &str) -> Receiver<Result<Opened, CreateError>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let (folder, name) = (folder.to_path_buf(), name.to_string());
        let reply = tx.clone();
        match std::thread::Builder::new().name("archforge-create".into()).spawn(move || job(&inner, &folder, &name, &reply)) {
            Ok(handle) => {
                // An earlier worker has finished or waits on the job lock; exit waits for the newest.
                *lock(&self.inner.worker) = Some(handle);
            }
            Err(e) => {
                let _ = tx.send(Err(CreateError::Failed { detail: format!("the work could not start: {e}") }));
            }
        }
        rx
    }
}

/// One creation, on the worker thread.
fn job(inner: &Inner, folder: &Path, name: &str, reply: &Sender<Result<Opened, CreateError>>) {
    let _job = lock(&inner.job);
    let result = dolt(inner).and_then(|dolt| {
        let opts = ServerOptions::new(inner.settings.join("servers"));
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_else(|e| {
            log::warn!("the clock reads before 1970 ({e}); the project's creation time says 1970");
            0
        });
        let hooks = Hooks { after_start: None, cancel: Some(inner.cancel.clone()) };
        Project::create_with(&dolt, &opts, folder, name, &inner.created_by, &archforge_project::timestamp(now), &hooks).map_err(kind)
    });
    let answer = match result {
        Ok(project) if inner.cancel.load(Ordering::SeqCst) => {
            // Quitting while it ran: the project exists and is closed again, not kept open.
            let _ = project.close();
            Err(CreateError::Failed { detail: "Archforge is quitting".into() })
        }
        Ok(project) => {
            let opened = Opened { name: project.settings().name.clone(), path: project.path().to_path_buf() };
            log::info!("created the project {}", opened.path.display());
            // The new project replaces the open one only now that it exists.
            let old = lock(&inner.open).replace(project);
            if let Some(old) = old
                && let Err(e) = old.close()
            {
                log::warn!("the project open before did not close cleanly: {e}");
            }
            Ok(opened)
        }
        Err(e) => {
            log::warn!("a project could not be created: {e:?}");
            Err(e)
        }
    };
    let _ = reply.send(answer);
}

/// Dolt, found once: its home is prepared in the settings folder, its binary checked.
fn dolt(inner: &Inner) -> Result<Dolt, CreateError> {
    let mut found = lock(&inner.dolt);
    if let Some(d) = found.as_ref() {
        return Ok(d.clone());
    }
    let missing = |e: archforge_dolt::Error| CreateError::DoltMissing { detail: e.to_string() };
    let home = Home::prepare(&inner.settings.join("dolt")).map_err(missing)?;
    let dolt = Dolt::find(&home).map_err(missing)?;
    log::info!("dolt {} at {}", dolt.version(), dolt.path().display());
    *found = Some(dolt.clone());
    Ok(dolt)
}

/// A project error as the kind the dialog explains, with the technical text as its detail.
fn kind(e: archforge_project::Error) -> CreateError {
    use archforge_dolt::Error as D;
    use archforge_project::Error as P;
    let detail = e.to_string();
    match e {
        P::BadName { .. } => CreateError::BadName,
        P::NameTaken { .. } => CreateError::NameTaken,
        P::Unfinished { .. } => CreateError::Unfinished,
        // A file-system failure in a creation comes from the folder steps (looking at the target,
        // making it, writing its marker).
        P::FolderUnusable { .. } | P::Io { .. } => CreateError::FolderUnusable { detail },
        P::Server(
            D::BinaryMissing { .. } | D::BinaryNotAbsolute { .. } | D::VersionRefused { .. } | D::VersionUnreadable { .. } | D::HomeUnsafe { .. },
        ) => CreateError::DoltMissing { detail },
        _ => CreateError::Failed { detail },
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn wait(rx: &Receiver<Result<Opened, CreateError>>) -> Result<Opened, CreateError> {
        rx.recv_timeout(Duration::from_secs(60)).expect("the host answers")
    }

    #[test]
    fn the_host_creates_a_project_keeps_it_open_refuses_its_name_again_and_stops_it_on_exit() {
        let base = std::env::temp_dir().join(format!("af-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = Projects::new(&base.join("settings"), "Archforge test".into());
        let folder = base.join("Documents/Archforge");
        let opened = wait(&host.create(&folder, "Commerce platform")).unwrap();
        assert_eq!(opened.path, folder.canonicalize().unwrap().join("Commerce platform.archforge"));
        let pid = lock(&host.inner.open).as_ref().map(|p| p.server().pid()).expect("the project is open");
        // The same name again is refused, and the open project stays open.
        assert_eq!(wait(&host.create(&folder, "Commerce platform")), Err(CreateError::NameTaken));
        assert!(lock(&host.inner.open).is_some());
        // A second project replaces the first, whose server stops.
        let second = wait(&host.create(&folder, "Library")).unwrap();
        assert_eq!(second.name, "Library");
        let gone = |pid: u32| {
            !std::process::Command::new("/bin/kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        };
        assert!(gone(pid), "the first project's server stopped");
        let pid = lock(&host.inner.open).as_ref().map(|p| p.server().pid()).unwrap();
        host.shutdown();
        assert!(gone(pid), "exit stopped the open project's server");
        assert!(lock(&host.inner.open).is_none());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
