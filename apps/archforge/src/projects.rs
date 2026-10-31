//! The host's projects: creates and opens them on a worker thread, keeps the one that is open, and
//! writes its model as the user works (owner decision archforge-gm2.17) on a writer thread: the
//! newest rows a moment after the last change, all of them in one transaction; Save writes what
//! waits and commits it. A failed write keeps the rows and tries again with the next change or
//! Save; the status bar says why meanwhile.
//!
//! One creation or opening runs at a time. A project replaces the open one only once it is ready,
//! after the open one's pending writes reached it, so a failure leaves the open project as it was.
//! Rows and saves name their project: any that arrive for one no longer open are dropped, never
//! written into the next. Dolt is found on the worker the first time, never on the interface
//! thread. On exit what waits is written, the running work is cancelled and waited for, then the
//! open project's server is stopped.

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
/// How long a creation or an opening waits for the old project's pending writes.
const FLUSH_BUDGET: Duration = Duration::from_secs(10);

/// The writer's work: a project's rows to write, or a save (write what waits, then commit the
/// project named, or only write when none is) with its answer.
enum Job {
    Write(PathBuf, Rows),
    Save(Option<PathBuf>, Sender<Result<bool, String>>),
}

pub struct Projects {
    inner: Arc<Inner>,
}

struct Inner {
    /// The application's settings folder: Dolt's home and the server records go there.
    settings: PathBuf,
    /// Who creates projects, for their settings: the version line.
    created_by: String,
    /// Held for a whole creation or opening, so two never race.
    job: Mutex<()>,
    open: Mutex<Option<Project>>,
    dolt: Mutex<Option<Dolt>>,
    cancel: Arc<AtomicBool>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// The model's tables, described once.
    schema: Mutex<Option<Arc<ModelSchema>>>,
    /// The writer thread's inbox, started with the first write.
    writer: Mutex<Option<Sender<Job>>>,
    writing: Mutex<Writing>,
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
                schema: Mutex::new(None),
                writer: Mutex::new(None),
                writing: Mutex::new(Writing::default()),
            }),
        }
    }

    /// Cancels a running creation and waits for it within the stop budget (std's join has no
    /// timeout, so it polls), then stops the open project's server. Runs on exit, on the interface
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
        if let Some(project) = lock(&self.inner.open).take() {
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
        lock(&self.inner.writing).pending = true;
        if let Err(e) = writer(&self.inner).send(Job::Write(project.to_path_buf(), rows)) {
            log::error!("the model could not be handed to the writer: {e}");
            lock(&self.inner.writing).error = Some("the writer stopped".into());
        }
    }

    fn save(&self, project: &Path) -> Receiver<Result<bool, String>> {
        let (tx, rx) = channel();
        if let Err(e) = writer(&self.inner).send(Job::Save(Some(project.to_path_buf()), tx.clone())) {
            let _ = tx.send(Err(format!("the writer stopped: {e}")));
        }
        rx
    }

    fn writing(&self) -> Writing {
        lock(&self.inner.writing).clone()
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
    if let Err(e) = flush(inner) {
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
    if let Err(e) = flush(inner) {
        let _ = reply.send(Err(e));
        return;
    }
    // The open project chosen again is read where it is.
    let same = |p: &Project| std::fs::canonicalize(path).is_ok_and(|c| c == p.path() || std::fs::canonicalize(p.path()).is_ok_and(|q| q == c));
    if lock(&inner.open).as_ref().is_some_and(same) {
        let open = lock(&inner.open);
        let answer = match open.as_ref() {
            Some(project) => schema(inner).and_then(|s| {
                let rows = project.read_model(&s).map_err(kind)?;
                let unsaved = project.has_unsaved().map_err(kind)?;
                Ok(Opened { name: project.settings().name.clone(), path: project.path().to_path_buf(), model: Some(Arc::new(rows)), unsaved })
            }),
            None => Err(CreateError::Failed { detail: "the project closed while it was read".into() }),
        };
        drop(open);
        let _ = reply.send(answer);
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

/// A project ready to be the open one replaces it, once the open one's pending writes reached it;
/// when they could not, the open one stays and the new one is closed again, so no edit is lost.
fn adopt(inner: &Inner, project: Project) -> Result<(), CreateError> {
    if let Err(e) = flush(inner) {
        let _ = project.close();
        return Err(e);
    }
    let old = lock(&inner.open).replace(project);
    *lock(&inner.writing) = Writing::default();
    if let Some(old) = old
        && let Err(e) = old.close()
    {
        log::warn!("the project open before did not close cleanly: {e}");
    }
    Ok(())
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

/// The writer: the newest rows once changes settle; a save writes what waits, then commits. Rows
/// that failed to be written are tried again with the next change or Save, not on a timer.
fn write_loop(inner: &Inner, rx: &Receiver<Job>) {
    let mut waiting: Option<(PathBuf, Rows)> = None;
    let mut failed = false;
    loop {
        let job = if waiting.is_some() && !failed {
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
                waiting = Some((project, rows));
                failed = false;
            }
            None => {
                write_waiting(inner, &mut waiting);
                failed = waiting.is_some();
            }
            Some(Job::Save(project, reply)) => {
                write_waiting(inner, &mut waiting);
                failed = waiting.is_some();
                let answer = if waiting.is_some() {
                    Err(lock(&inner.writing).error.clone().unwrap_or_else(|| "the changes could not be written".into()))
                } else {
                    project.map_or(Ok(false), |p| commit(inner, &p))
                };
                let _ = reply.send(answer);
            }
        }
    }
}

/// Writes the waiting rows to their project; on a failure they stay waiting. Rows for a project
/// that is no longer open are dropped.
fn write_waiting(inner: &Inner, waiting: &mut Option<(PathBuf, Rows)>) {
    let Some((target, rows)) = waiting.as_ref() else { return };
    let open = lock(&inner.open);
    let Some(project) = open.as_ref().filter(|p| p.path() == target) else {
        log::warn!("changes for {} arrived after it closed and are dropped", target.display());
        *waiting = None;
        *lock(&inner.writing) = Writing::default();
        return;
    };
    let result = schema(inner).map_err(|e| format!("{e:?}")).and_then(|s| project.write_model(&s, rows).map_err(|e| e.to_string()));
    drop(open);
    let mut state = lock(&inner.writing);
    match result {
        Ok(()) => {
            *waiting = None;
            *state = Writing::default();
        }
        Err(e) => {
            // The same failure again is said once.
            if state.error.as_deref() != Some(e.as_str()) {
                log::error!("the model could not be written: {e}");
            }
            state.pending = true;
            state.error = Some(e);
        }
    }
}

/// Save: a commit of everything written to the project at `target`, with the time it was made.
fn commit(inner: &Inner, target: &Path) -> Result<bool, String> {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let message = format!("Saved {}", archforge_project::timestamp(now));
    match lock(&inner.open).as_ref().filter(|p| p.path() == target) {
        Some(project) => project.commit(&message).map_err(|e| e.to_string()),
        None => Err(format!("{} is no longer open", target.display())),
    }
}

/// Waits until the writer has written what it holds, without a commit, within the flush budget;
/// fails when it could not, or did not answer in time. Nothing waits when no writer runs.
fn flush(inner: &Inner) -> Result<(), CreateError> {
    let Some(tx) = lock(&inner.writer).clone() else { return Ok(()) };
    let (reply, answer) = channel();
    let unwritten =
        |why: String| CreateError::Failed { detail: format!("the open project's last changes could not be written ({why}); it stays open") };
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
    fn the_host_writes_saves_and_reopens_a_model_and_drops_rows_for_a_closed_project() {
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
        assert!(host.writing().pending, "the rows wait for the changes to settle");
        assert_eq!(saved(&host, &first), Ok(true));
        assert_eq!(host.writing(), Writing::default());
        let second = wait(&host.create(&folder, "Library")).unwrap().path;
        let back = wait(&host.open(&first)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&shop), "the newest rows were written, the earlier ones replaced");
        assert!(!back.unsaved);
        // Rows written but never saved come back as unsaved.
        let crm = model("crm");
        host.write(&first, crm.clone());
        wait(&host.open(&second)).unwrap();
        let back = wait(&host.open(&first)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&crm));
        assert!(back.unsaved);
        // Rows and a save for a project no longer open never reach the open one.
        host.write(&second, model("stray"));
        assert!(saved(&host, &second).is_err());
        assert_eq!(saved(&host, &first), Ok(true));
        let again = wait(&host.open(&first)).unwrap();
        assert_eq!(again.model.as_deref(), Some(&crm));
        // Rows that cannot be written keep the open project open: another is not opened over them.
        host.write(&first, Rows::default());
        assert!(saved(&host, &first).is_err());
        assert!(host.writing().error.is_some(), "the status bar says why");
        assert!(wait(&host.open(&second)).is_err(), "opening another would lose the changes");
        assert_eq!(lock(&host.inner.open).as_ref().map(|p| p.path().to_path_buf()), Some(first.clone()));
        // Rows that can be written clear the failure, and the other project opens.
        host.write(&first, crm.clone());
        assert_eq!(saved(&host, &first), Ok(false), "the same rows again leave nothing to commit");
        assert_eq!(host.writing(), Writing::default());
        wait(&host.open(&second)).unwrap();
        host.shutdown();
        std::fs::remove_dir_all(&base).unwrap();
    }
}
