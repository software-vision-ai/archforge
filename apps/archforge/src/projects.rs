//! The host's projects: creates and opens them on a worker thread, keeps those that are open (one
//! per tab of the window, at most [`MAX_OPEN`], each with its own server), and writes their models
//! as the user works (owner decision archforge-gm2.17) on a writer thread: each project's newest
//! rows a moment after its last change, all of them in one transaction; Save writes what waits and
//! commits it. A failed write keeps the rows and tries again with the next change or Save; the
//! status bar says why meanwhile.
//!
//! One creation or opening runs at a time, and a project joins the open ones only once it is
//! ready, so a failure changes nothing that is open. Rows and saves name their project: any that
//! arrive for one no longer open are dropped, never written into another. Closing a project runs
//! as an opening does, one at a time, and writes what waits for it first; when that fails the
//! project stays open and the window is told, so its tab stays. Dolt is found on the worker the
//! first time, never on the interface thread. On exit what waits is written, the running work is cancelled and waited for, then every
//! open project's server is stopped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::rows::Rows;
use archforge_project::{Hooks, ModelSchema, Project};
use archforge_ui_egui::{CreateError, Opened, ProjectHost, Writing};

/// How long the writer waits after a change for the next one before it writes.
const SETTLE: Duration = Duration::from_millis(300);
/// How long a closing, or the exit, waits for pending writes.
const FLUSH_BUDGET: Duration = Duration::from_secs(10);
/// The most projects open at once, one per tab: each runs a database server.
const MAX_OPEN: usize = archforge_ui_egui::MAX_TABS;

/// The writer's work: a project's rows to write; a save (write what waits, then commit the
/// project named, or only write when none is) with its answer; or a flush of one project (write
/// what waits, answer whether that project's rows reached it).
enum Job {
    Write(PathBuf, Rows),
    Save(Option<PathBuf>, Sender<Result<bool, String>>),
    Flush(PathBuf, Sender<Result<(), String>>),
}

pub struct Projects {
    inner: Arc<Inner>,
}

struct Inner {
    /// The application's settings folder: Dolt's home and the server records go there.
    settings: PathBuf,
    /// Who creates projects, for their settings: the version line.
    created_by: String,
    /// Held for a whole creation, opening or closing, so two never race.
    job: Mutex<()>,
    /// The open projects, one per tab.
    open: Mutex<Vec<Project>>,
    dolt: Mutex<Option<Dolt>>,
    cancel: Arc<AtomicBool>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// The model's tables, described once.
    schema: Mutex<Option<Arc<ModelSchema>>>,
    /// The writer thread's inbox, started with the first write.
    writer: Mutex<Option<Sender<Job>>>,
    /// How writing stands, per project with rows waiting or a failed write.
    writing: Mutex<BTreeMap<PathBuf, Writing>>,
    /// The closings under way, waited for at exit.
    closers: Mutex<Vec<std::thread::JoinHandle<()>>>,
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
                open: Mutex::new(Vec::new()),
                dolt: Mutex::new(None),
                cancel: Arc::new(AtomicBool::new(false)),
                worker: Mutex::new(None),
                schema: Mutex::new(None),
                writer: Mutex::new(None),
                writing: Mutex::new(BTreeMap::new()),
                closers: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Cancels a running creation and waits for it within the stop budget (std's join has no
    /// timeout, so it polls), then stops every open project's server. Runs on exit, on the interface
    /// thread: it may hold it for up to about twice the stop budget.
    pub fn shutdown(&self) {
        // Whatever waits to be written reaches the project before it closes.
        if let Err(e) = flush(&self.inner) {
            log::error!("at exit: {e:?}");
        }
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
        // A closing still stopping its server finishes within the same budget.
        for closer in std::mem::take(&mut *lock(&self.inner.closers)) {
            while !closer.is_finished() && started.elapsed() < budget {
                std::thread::sleep(Duration::from_millis(20));
            }
            if closer.is_finished() {
                let _ = closer.join();
            }
        }
        let open: Vec<Project> = std::mem::take(&mut *lock(&self.inner.open));
        for project in open {
            let path = project.path().display().to_string();
            if let Err(e) = project.close() {
                log::error!("the project {path} did not close cleanly: {e}");
            }
        }
    }
}

impl ProjectHost for Projects {
    fn open(&self, path: &Path) -> Receiver<Result<Opened, CreateError>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let path = path.to_path_buf();
        let reply = tx.clone();
        match std::thread::Builder::new().name("archforge-open".into()).spawn(move || open_job(&inner, &path, &reply)) {
            Ok(handle) => *lock(&self.inner.worker) = Some(handle),
            Err(e) => {
                let _ = tx.send(Err(CreateError::Failed { detail: format!("the work could not start: {e}") }));
            }
        }
        rx
    }

    fn write(&self, project: &Path, rows: Rows) {
        lock(&self.inner.writing).entry(project.to_path_buf()).or_default().pending = true;
        if let Err(e) = writer(&self.inner).send(Job::Write(project.to_path_buf(), rows)) {
            log::error!("the model could not be handed to the writer: {e}");
            lock(&self.inner.writing).entry(project.to_path_buf()).or_default().error = Some("the writer stopped".into());
        }
    }

    fn save(&self, project: &Path) -> Receiver<Result<bool, String>> {
        let (tx, rx) = channel();
        if let Err(e) = writer(&self.inner).send(Job::Save(Some(project.to_path_buf()), tx.clone())) {
            let _ = tx.send(Err(format!("the writer stopped: {e}")));
        }
        rx
    }

    fn writing(&self, project: &Path) -> Writing {
        lock(&self.inner.writing).get(project).cloned().unwrap_or_default()
    }

    fn close(&self, project: &Path) -> Receiver<Result<(), String>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let path = project.to_path_buf();
        let reply = tx.clone();
        // Its server stops off the interface thread; what waits for it is written first.
        match std::thread::Builder::new().name("archforge-close".into()).spawn(move || close_job(&inner, &path, &reply)) {
            Ok(handle) => {
                let mut closers = lock(&self.inner.closers);
                closers.retain(|c| !c.is_finished());
                closers.push(handle);
            }
            Err(e) => {
                let _ = tx.send(Err(format!("the work could not start: {e}")));
            }
        }
        rx
    }

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
    if let Err(e) = room(inner) {
        let _ = reply.send(Err(e));
        return;
    }
    let result = dolt(inner).and_then(|dolt| {
        let opts = ServerOptions::new(inner.settings.join("servers"));
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_else(|e| {
            log::warn!("the clock reads before 1970 ({e}); the project's creation time says 1970");
            0
        });
        let hooks = Hooks { cancel: Some(inner.cancel.clone()), ..Hooks::default() };
        let project =
            Project::create_with(&dolt, &opts, folder, name, &inner.created_by, &archforge_project::timestamp(now), &hooks).map_err(kind)?;
        // The model's tables come with the project; its rows follow with the first change.
        match schema(inner).and_then(|s| project.prepare_model(&s).map_err(kind)) {
            Ok(()) => Ok(project),
            Err(e) => {
                let _ = project.close();
                Err(e)
            }
        }
    });
    let answer = match result {
        Ok(project) if inner.cancel.load(Ordering::SeqCst) => {
            // Quitting while it ran: the project exists and is closed again, not kept open.
            let _ = project.close();
            Err(CreateError::Failed { detail: "Archforge is quitting".into() })
        }
        Ok(project) => {
            let opened = Opened { name: project.settings().name.clone(), path: project.path().to_path_buf(), ..Default::default() };
            log::info!("created the project {}", opened.path.display());
            adopt(inner, project).map(|()| opened)
        }
        Err(e) => {
            log::warn!("a project could not be created: {e:?}");
            Err(e)
        }
    };
    let _ = reply.send(answer);
}

/// One opening, on the worker thread: the project's server, its model's tables and rows.
fn open_job(inner: &Inner, path: &Path, reply: &Sender<Result<Opened, CreateError>>) {
    let _job = lock(&inner.job);
    // A project already open is read where it is (the window shows its tab when it has one),
    // after what waits for it was written.
    let same = |p: &&Project| std::fs::canonicalize(path).is_ok_and(|c| c == p.path() || std::fs::canonicalize(p.path()).is_ok_and(|q| q == c));
    let held = lock(&inner.open).iter().find(same).map(|p| p.path().to_path_buf());
    if let Some(held) = held
        && let Err(e) = flush_one(inner, &held)
    {
        let _ = reply.send(Err(CreateError::Failed { detail: e }));
        return;
    }
    let open = lock(&inner.open);
    if let Some(project) = open.iter().find(same) {
        let answer = schema(inner).and_then(|s| {
            let rows = project.read_model(&s).map_err(kind)?;
            let unsaved = project.has_unsaved().map_err(kind)?;
            Ok(Opened { name: project.settings().name.clone(), path: project.path().to_path_buf(), model: Some(Arc::new(rows)), unsaved })
        });
        drop(open);
        let _ = reply.send(answer);
        return;
    }
    drop(open);
    if let Err(e) = room(inner) {
        let _ = reply.send(Err(e));
        return;
    }
    let result = dolt(inner).and_then(|dolt| {
        let opts = ServerOptions::new(inner.settings.join("servers"));
        let project = Project::open(&dolt, &opts, path).map_err(kind)?;
        let read = schema(inner).and_then(|s| {
            project.prepare_model(&s).map_err(kind)?;
            let rows = project.read_model(&s).map_err(kind)?;
            let unsaved = project.has_unsaved().map_err(kind)?;
            Ok((rows, unsaved))
        });
        match read {
            Ok((rows, unsaved)) => Ok((project, rows, unsaved)),
            Err(e) => {
                let _ = project.close();
                Err(e)
            }
        }
    });
    let answer = match result {
        Ok((project, _, _)) if inner.cancel.load(Ordering::SeqCst) => {
            let _ = project.close();
            Err(CreateError::Failed { detail: "Archforge is quitting".into() })
        }
        Ok((project, rows, unsaved)) => {
            let opened = Opened { name: project.settings().name.clone(), path: project.path().to_path_buf(), model: Some(Arc::new(rows)), unsaved };
            log::info!("opened the project {}", opened.path.display());
            adopt(inner, project).map(|()| opened)
        }
        Err(e) => {
            log::warn!("a project could not be opened: {e:?}");
            Err(e)
        }
    };
    let _ = reply.send(answer);
}

/// Refuses another project when [`MAX_OPEN`] are open.
fn room(inner: &Inner) -> Result<(), CreateError> {
    if lock(&inner.open).len() >= MAX_OPEN {
        return Err(CreateError::Failed { detail: format!("{MAX_OPEN} projects are open; close one to open another") });
    }
    Ok(())
}

/// A project ready to be open joins the open ones.
fn adopt(inner: &Inner, project: Project) -> Result<(), CreateError> {
    let mut open = lock(&inner.open);
    if open.len() >= MAX_OPEN {
        drop(open);
        let _ = project.close();
        return Err(CreateError::Failed { detail: format!("{MAX_OPEN} projects are open; close one to open another") });
    }
    open.push(project);
    Ok(())
}

/// Closes the project at `path`, one job at a time: what waits for it is written, then its server
/// stops. When its changes cannot be written it stays open and the answer says why, so the window
/// keeps its tab and nothing is lost.
fn close_job(inner: &Inner, path: &Path, reply: &Sender<Result<(), String>>) {
    let _job = lock(&inner.job);
    if let Err(e) = flush_one(inner, path) {
        log::error!("{} stays open: {e}", path.display());
        let _ = reply.send(Err(e));
        return;
    }
    let project = {
        let mut open = lock(&inner.open);
        open.iter().position(|p| p.path() == path).map(|i| open.remove(i))
    };
    lock(&inner.writing).remove(path);
    if let Some(project) = project {
        log::info!("closed the project {}", path.display());
        if let Err(e) = project.close() {
            log::warn!("the project {} did not close cleanly: {e}", path.display());
        }
    }
    let _ = reply.send(Ok(()));
}

/// The model's tables, described once from the ER crate's storage.
fn schema(inner: &Inner) -> Result<Arc<ModelSchema>, CreateError> {
    let mut held = lock(&inner.schema);
    if let Some(s) = held.as_ref() {
        return Ok(s.clone());
    }
    let failed = |detail: String| CreateError::Failed { detail };
    let universe = archforge_er::store(Box::new(|| archforge_mof::Uuid([0; 16]))).map_err(failed)?.universe().clone();
    let (tables, junctions) = archforge_er::storage();
    let s = Arc::new(ModelSchema::new(&universe, tables, junctions).map_err(failed)?);
    *held = Some(s.clone());
    Ok(s)
}

/// The writer's inbox, starting the writer with the first job.
fn writer(inner: &Arc<Inner>) -> Sender<Job> {
    let mut held = lock(&inner.writer);
    if let Some(tx) = held.as_ref() {
        return tx.clone();
    }
    let (tx, rx) = channel::<Job>();
    let worker = inner.clone();
    if let Err(e) = std::thread::Builder::new().name("archforge-write".into()).spawn(move || write_loop(&worker, &rx)) {
        log::error!("the writer could not start: {e}");
    }
    *held = Some(tx.clone());
    tx
}

/// The writer: each project's newest rows once changes settle; a save writes what waits, then
/// commits. Rows that failed to be written are tried again with the next change or Save, not on a
/// timer. What waits is one set of rows per open project at most.
fn write_loop(inner: &Inner, rx: &Receiver<Job>) {
    let mut waiting: BTreeMap<PathBuf, Rows> = BTreeMap::new();
    let mut failed = false;
    loop {
        let job = if !waiting.is_empty() && !failed {
            match rx.recv_timeout(SETTLE) {
                Ok(job) => Some(job),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    write_waiting(inner, &mut waiting);
                    return;
                }
            }
        } else {
            match rx.recv() {
                Ok(job) => Some(job),
                Err(_) => return,
            }
        };
        match job {
            Some(Job::Write(project, rows)) => {
                waiting.insert(project, rows);
                failed = false;
            }
            None => {
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
            }
            Some(Job::Save(project, reply)) => {
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
                let answer = match project {
                    // A save needs its own project's rows written; a flush needs all.
                    Some(p) if waiting.contains_key(&p) => Err(unwritten(inner, &p)),
                    Some(p) => commit(inner, &p),
                    None => match waiting.keys().next() {
                        Some(p) => Err(unwritten(inner, p)),
                        None => Ok(false),
                    },
                };
                let _ = reply.send(answer);
            }
            Some(Job::Flush(project, reply)) => {
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
                let _ = reply.send(if waiting.contains_key(&project) { Err(unwritten(inner, &project)) } else { Ok(()) });
            }
        }
    }
}

/// Why `project`'s rows could not be written, as the last write said.
fn unwritten(inner: &Inner, project: &Path) -> String {
    lock(&inner.writing).get(project).and_then(|w| w.error.clone()).unwrap_or_else(|| "the changes could not be written".into())
}

/// Writes the waiting rows to their projects; those that fail stay waiting, and each project's
/// state says how its own writing stands. Rows for a project that is no longer open are dropped.
fn write_waiting(inner: &Inner, waiting: &mut BTreeMap<PathBuf, Rows>) {
    let targets: Vec<PathBuf> = waiting.keys().cloned().collect();
    for target in targets {
        let open = lock(&inner.open);
        let Some(project) = open.iter().find(|p| p.path() == target) else {
            log::warn!("changes for {} arrived after it closed and are dropped", target.display());
            waiting.remove(&target);
            drop(open);
            lock(&inner.writing).remove(&target);
            continue;
        };
        let Some(rows) = waiting.get(&target) else { continue };
        let result = schema(inner).map_err(|e| format!("{e:?}")).and_then(|s| project.write_model(&s, rows).map_err(|e| e.to_string()));
        drop(open);
        let mut states = lock(&inner.writing);
        match result {
            Ok(()) => {
                waiting.remove(&target);
                states.remove(&target);
            }
            Err(e) => {
                let state = states.entry(target.clone()).or_default();
                // The same failure again is said once.
                if state.error.as_deref() != Some(e.as_str()) {
                    log::error!("the model of {} could not be written: {e}", target.display());
                }
                state.pending = true;
                state.error = Some(e);
            }
        }
    }
}

/// Save: a commit of everything written to the project at `target`, with the time it was made.
fn commit(inner: &Inner, target: &Path) -> Result<bool, String> {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let message = format!("Saved {}", archforge_project::timestamp(now));
    match lock(&inner.open).iter().find(|p| p.path() == target) {
        Some(project) => project.commit(&message).map_err(|e| e.to_string()),
        None => Err(format!("{} is no longer open", target.display())),
    }
}

/// Waits until the writer has written what waits for `project`, within the flush budget; fails when
/// it could not, or did not answer in time. Nothing waits when no writer runs.
fn flush_one(inner: &Inner, project: &Path) -> Result<(), String> {
    let Some(tx) = lock(&inner.writer).clone() else { return Ok(()) };
    let (reply, answer) = channel();
    let unwritten = |why: String| format!("its last changes could not be written ({why})");
    if tx.send(Job::Flush(project.to_path_buf(), reply)).is_err() {
        return Err(unwritten("the writer stopped".into()));
    }
    match answer.recv_timeout(FLUSH_BUDGET) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(unwritten(e)),
        Err(_) => Err(unwritten(format!("writing took longer than {} seconds", FLUSH_BUDGET.as_secs()))),
    }
}

/// Waits until the writer has written what it holds, without a commit, within the flush budget;
/// fails when it could not, or did not answer in time. Nothing waits when no writer runs.
fn flush(inner: &Inner) -> Result<(), CreateError> {
    let Some(tx) = lock(&inner.writer).clone() else { return Ok(()) };
    let (reply, answer) = channel();
    let unwritten = |why: String| CreateError::Failed { detail: format!("the last changes could not be written ({why})") };
    if tx.send(Job::Save(None, reply)).is_err() {
        return Err(unwritten("the writer stopped".into()));
    }
    match answer.recv_timeout(FLUSH_BUDGET) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(unwritten(e)),
        Err(_) => Err(unwritten(format!("writing took longer than {} seconds", FLUSH_BUDGET.as_secs()))),
    }
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

    fn open_paths(host: &Projects) -> Vec<PathBuf> {
        lock(&host.inner.open).iter().map(|p| p.path().to_path_buf()).collect()
    }

    fn gone(pid: u32) -> bool {
        !std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Closes `path` and waits for the answer.
    fn close(host: &Projects, path: &Path) -> Result<(), String> {
        host.close(path).recv_timeout(Duration::from_secs(60)).expect("the closing answers")
    }

    #[test]
    fn the_host_keeps_several_projects_open_closes_one_on_request_and_all_on_exit() {
        let base = std::env::temp_dir().join(format!("af-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = Projects::new(&base.join("settings"), "Archforge test".into());
        let folder = base.join("Documents/Archforge");
        let opened = wait(&host.create(&folder, "Commerce platform")).unwrap();
        assert_eq!(opened.path, folder.canonicalize().unwrap().join("Commerce platform.archforge"));
        let first_pid = lock(&host.inner.open)[0].server().pid();
        // The same name again is refused, and the open project stays open.
        assert_eq!(wait(&host.create(&folder, "Commerce platform")), Err(CreateError::NameTaken));
        // A second project opens beside the first.
        let second = wait(&host.create(&folder, "Library")).unwrap();
        assert_eq!(second.name, "Library");
        assert_eq!(open_paths(&host), [opened.path.clone(), second.path.clone()]);
        assert!(!gone(first_pid), "the first project's server still runs");
        // Closing one stops its server and leaves the other open.
        close(&host, &opened.path).unwrap();
        let started = Instant::now();
        while !gone(first_pid) && started.elapsed() < Duration::from_secs(30) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(gone(first_pid), "the closed project's server stopped");
        assert_eq!(open_paths(&host), std::slice::from_ref(&second.path));
        let pid = lock(&host.inner.open)[0].server().pid();
        host.shutdown();
        assert!(gone(pid), "exit stopped every open project's server");
        assert!(open_paths(&host).is_empty());
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// A model's rows: a project with one schema of the given name.
    fn model(schema: &str) -> Rows {
        let mut n = 0u64;
        let ids = Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_800_000, r)
        });
        let mut s = archforge_er::new_project_session("Commerce platform", ids).unwrap();
        s.execute("er.schema.add", &serde_json::json!({ "name": schema })).unwrap();
        let (tables, junctions) = archforge_er::storage();
        archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap()
    }

    fn saved(host: &Projects, project: &Path) -> Result<bool, String> {
        host.save(project).recv_timeout(Duration::from_secs(60)).expect("the save answers")
    }

    #[test]
    fn the_host_writes_saves_and_reopens_models_and_drops_rows_for_a_closed_project() {
        let base = std::env::temp_dir().join(format!("af-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = Projects::new(&base.join("settings"), "Archforge test".into());
        let folder = base.join("Documents/Archforge");
        let first = wait(&host.create(&folder, "Commerce platform")).unwrap().path;
        // Rows written and saved come back on opening, with nothing unsaved.
        let shop = model("shop");
        host.write(&first, model("draft"));
        host.write(&first, shop.clone());
        assert!(host.writing(&first).pending, "the rows wait for the changes to settle");
        assert_eq!(saved(&host, &first), Ok(true));
        assert_eq!(host.writing(&first), Writing::default());
        let second = wait(&host.create(&folder, "Library")).unwrap().path;
        // Each project's rows wait apart: writes to the second never displace the first's.
        let crm = model("crm");
        host.write(&first, crm.clone());
        host.write(&second, model("books"));
        assert_eq!(saved(&host, &second), Ok(true));
        // Closed and opened again, the first shows its saved rows and the written ones as unsaved.
        close(&host, &first).unwrap();
        let back = wait(&host.open(&first)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&crm), "the newest rows were written before it closed");
        assert!(back.unsaved);
        // An open project chosen again is read where it is.
        let again = wait(&host.open(&first)).unwrap();
        assert_eq!(again.model.as_deref(), Some(&crm));
        assert_eq!(open_paths(&host).len(), 2);
        // Rows and a save for a project no longer open are dropped.
        close(&host, &second).unwrap();
        host.write(&second, model("stray"));
        assert!(saved(&host, &second).is_err());
        assert_eq!(saved(&host, &first), Ok(true));
        // Rows that cannot be written keep their project open, and the closing says why; the
        // failure is that project's own.
        let third = wait(&host.create(&folder, "Billing")).unwrap().path;
        host.write(&first, Rows::default());
        assert!(saved(&host, &first).is_err());
        assert!(host.writing(&first).error.is_some(), "the status bar says why");
        assert_eq!(host.writing(&third), Writing::default(), "another project's status is its own");
        assert!(close(&host, &first).is_err());
        assert!(open_paths(&host).contains(&first), "it stays open");
        // Another project closes meanwhile; its own rows reached it.
        host.write(&third, model("invoices"));
        close(&host, &third).unwrap();
        // Rows that can be written clear the failure, and it closes.
        host.write(&first, crm.clone());
        assert_eq!(saved(&host, &first), Ok(false), "the same rows again leave nothing to commit");
        assert_eq!(host.writing(&first), Writing::default());
        close(&host, &first).unwrap();
        assert!(open_paths(&host).is_empty());
        host.shutdown();
        std::fs::remove_dir_all(&base).unwrap();
    }
}
