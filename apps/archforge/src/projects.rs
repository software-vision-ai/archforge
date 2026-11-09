//! The host's projects: creates and opens them on a worker thread, keeps those that are open (one
//! per tab of the window, at most [`MAX_OPEN`], each with its own server), and writes their models
//! as the user works (owner decision) on a writer thread: each project's newest
//! rows a moment after its last change, all of them in one transaction; Save writes what waits and
//! commits it, under a description when one is given (AR-247). A failed write keeps the rows and tries again with the next change or Save; the
//! status bar says why meanwhile.
//!
//! One creation or opening runs at a time, and a project joins the open ones only once it is
//! ready, so a failure changes nothing that is open. Rows and saves name their project: any that
//! arrive for one no longer open are dropped, never written into another. Closing a project runs
//! as an opening does, one at a time, and writes what waits for it first; when that fails the
//! project stays open and the window is told, so its tab stays. Dolt is found on the worker the
//! first time, never on the interface thread.
//!
//! The project's conversations (AR-206) are written on the same writer, in the order they are
//! asked with the model's rows, Save and Don't save: a chat job runs against the open project
//! there. Opening marks turns left running as interrupted and commits what of the conversations
//! was not; closing commits what waits; Don't save runs on the writer too, so no chat write can
//! come between the conversations being kept and the model being put back. On exit what waits is written, the running work is cancelled and waited for, then every
//! open project's server is stopped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::rows::Rows;
use archforge_project::{Author, DESCRIPTION_CHARS, DescriptionProblem, Hooks, ModelSchema, Project, VersionReader, check_description};
use archforge_ui_egui::i18n::{fmt, tr};
use archforge_ui_egui::{CreateError, Opened, ProjectHost, SaveError, Saved, SavedBy, VersionAfter, VersionEntry, VersionsPage, Writing};

/// How long the writer waits after a change for the next one before it writes.
const SETTLE: Duration = Duration::from_millis(300);
/// How long a closing, or the exit, waits for pending writes.
const FLUSH_BUDGET: Duration = Duration::from_secs(10);
/// The most reads of one project's versions that run at once.
const MOST_READS: usize = 4;
/// The most projects open at once, one per tab: each runs a database server.
const MAX_OPEN: usize = archforge_ui_egui::MAX_TABS;

/// The writer's work: a project's rows to write; a save (write what waits, then commit the
/// project named, recorded as who asked, under the description given or an automatic one, or only
/// write when no project is named) with its answer; or a flush of one project (write what waits, answer whether that
/// project's rows reached it).
enum Job {
    Write(PathBuf, Rows),
    Save(Option<(PathBuf, SavedBy)>, Option<Described>, Sender<Result<Saved, SaveError>>),
    Flush(PathBuf, Sender<Result<(), String>>),
    /// Forget what waits for a project without writing it (Close without writing), then answer.
    Drop(PathBuf, Sender<()>),
    /// Don't save: keep a project's conversations and put its model back to its last save, then
    /// forget what waits for it and answer. The flag is the claim: whoever sets it first, the writer
    /// to run the job or the closing that stopped waiting for it, decides, so a job given up on is
    /// never run late.
    Discard(PathBuf, Arc<AtomicBool>, Sender<Result<(), String>>),
    /// Work on a project's conversations: run against the project while it is open (none when it
    /// is not), in order with every other job; it answers through its own channel.
    Chat(PathBuf, ChatJob),
}

/// A save's description, and how many sets of rows its project had been handed when the save was
/// asked: its key check runs off the writer, so rows handed over meanwhile can reach the writer
/// first, and they are not the state being saved.
struct Described {
    text: String,
    handed: u64,
}

/// A job on a project's conversations, run on the writer thread.
type ChatJob = Box<dyn FnOnce(Option<&Project>) + Send>;

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
    /// The closings and the reads of versions under way, waited for at exit.
    closers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    /// A reader of each open project's versions. Apart from `open`, which the writer holds while
    /// it writes, so that a read is held up by no write and holds up none.
    readers: Mutex<BTreeMap<PathBuf, Arc<OpenReader>>>,
    /// How many sets of rows each project has been handed, counted as each goes to the writer.
    handed: Mutex<BTreeMap<PathBuf, u64>>,
    /// Who checks a description against the kept API keys; while nobody does, a description is refused.
    keys: OnceLock<Arc<crate::agents::Agents>>,
}

/// An open project's reader, and how many reads are using it now. The count is the cap's and the
/// closing's only measure: a clone of this held for any other reason changes neither.
struct OpenReader {
    reader: VersionReader,
    under_way: AtomicUsize,
}

/// One read under way, counted from when it was let in until this is dropped, on every path.
struct ReadUnderWay(Arc<OpenReader>);

impl Drop for ReadUnderWay {
    /// Gives the read's place back.
    fn drop(&mut self) {
        self.0.under_way.fetch_sub(1, Ordering::SeqCst);
    }
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
                readers: Mutex::new(BTreeMap::new()),
                handed: Mutex::new(BTreeMap::new()),
                keys: OnceLock::new(),
            }),
        }
    }

    /// Who creates projects and conversations: the version line.
    pub fn created_by(&self) -> &str {
        &self.inner.created_by
    }

    /// Names who checks a save's description against the kept API keys. Only the first call counts.
    pub fn check_descriptions_with(&self, agents: Arc<crate::agents::Agents>) {
        let _ = self.inner.keys.set(agents);
    }

    /// Runs `job` on the writer against the open project at `path`, after every job sent before
    /// it, and answers with what it gives; a project no longer open is an error.
    pub fn on_writer<T: Send + 'static>(
        &self,
        path: &Path,
        job: impl FnOnce(&Project) -> Result<T, String> + Send + 'static,
    ) -> Receiver<Result<T, String>> {
        on_writer(&self.inner, path, job)
    }

    /// Runs `job` on the writer with the open project at `path`, or none when it is not open, after
    /// every job sent before it, and answers with what it gives; nothing when the writer stopped.
    pub fn on_writer_open<T: Send + 'static>(&self, path: &Path, job: impl FnOnce(Option<&Project>) -> T + Send + 'static) -> Receiver<T> {
        let (tx, rx) = channel();
        let run: ChatJob = Box::new(move |project| {
            let _ = tx.send(job(project));
        });
        if let Err(e) = writer(&self.inner).send(Job::Chat(path.to_path_buf(), run)) {
            log::warn!("a job on the conversations could not reach the writer: {e}");
        }
        rx
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
        lock(&self.inner.readers).clear();
        let open: Vec<Project> = std::mem::take(&mut *lock(&self.inner.open));
        for project in open {
            let path = project.path().display().to_string();
            if let Err(e) = project.close() {
                log::error!("the project {path} did not close cleanly: {e}");
            }
        }
    }
}

impl Projects {
    /// Lets one more read of `project` in, counted under the lock that finds its reader, so no two
    /// can take the last place. Refused past [`MOST_READS`] and for a project no longer open.
    fn take_a_read(&self, project: &Path) -> Result<ReadUnderWay, String> {
        let readers = lock(&self.inner.readers);
        let Some(open) = readers.get(project) else {
            return Err(format!("{} is no longer open", project.display()));
        };
        if open.under_way.load(Ordering::SeqCst) >= MOST_READS {
            return Err(tr("Too many versions are being read at once. Try again in a moment."));
        }
        open.under_way.fetch_add(1, Ordering::SeqCst);
        Ok(ReadUnderWay(open.clone()))
    }

    /// Runs `read` with the project's reader on a thread of its own and gives its answer. Only the
    /// short lock on the readers is taken here, so the read waits for no write and no write waits
    /// for it. Refused past [`MOST_READS`] reads under way, and for a project no longer open: its
    /// reader has left the map, so a later read cannot reach it.
    fn on_a_reader<T: Send + 'static>(
        &self,
        project: &Path,
        read: impl FnOnce(&VersionReader) -> Result<T, String> + Send + 'static,
    ) -> Receiver<Result<T, String>> {
        let (tx, rx) = channel();
        let slot = match self.take_a_read(project) {
            Ok(slot) => slot,
            Err(why) => {
                let _ = tx.send(Err(why));
                return rx;
            }
        };
        let answer = tx.clone();
        let work = move || {
            // In this order, so that a panic gives the place back before the sender goes.
            let (answer, slot) = (answer, slot);
            let result = read(&slot.0.reader);
            drop(slot);
            let _ = answer.send(result);
        };
        match std::thread::Builder::new().name("archforge-versions".into()).spawn(work) {
            Ok(handle) => {
                let mut closers = lock(&self.inner.closers);
                closers.retain(|c| !c.is_finished());
                closers.push(handle);
            }
            Err(e) => {
                let _ = tx.send(Err(format!("the read could not start: {e}")));
            }
        }
        rx
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

    /// Hands the writer `project`'s newest rows, counted for any described save still being checked.
    fn write(&self, project: &Path, rows: Rows) {
        lock(&self.inner.writing).entry(project.to_path_buf()).or_default().pending = true;
        // Counted and sent under one lock, so the count always agrees with the writer's queue.
        let mut handed = lock(&self.inner.handed);
        *handed.entry(project.to_path_buf()).or_default() += 1;
        let sent = writer(&self.inner).send(Job::Write(project.to_path_buf(), rows));
        drop(handed);
        if let Err(e) = sent {
            log::error!("the model could not be handed to the writer: {e}");
            lock(&self.inner.writing).entry(project.to_path_buf()).or_default().error = Some("the writer stopped".into());
        }
    }

    /// Save, under `description` when one is given. A description of the wrong shape is refused
    /// here; a good one is checked against the kept API keys first, and only then asked of the
    /// writer, which answers [`SaveError::Changed`] when more rows came in during the check.
    fn save(&self, project: &Path, description: Option<String>, by: SavedBy) -> Receiver<Result<Saved, SaveError>> {
        let (tx, rx) = channel();
        match description {
            None => send_save(&self.inner, project, by, None, tx),
            Some(text) => match check_description(&text) {
                Ok(()) => save_described(&self.inner, project, by, text, tx),
                Err(problem) => {
                    let _ = tx.send(Err(SaveError::Failed(refusal(problem))));
                }
            },
        }
        rx
    }

    /// A page of the project's versions, read on a thread of its own ([`Projects::on_a_reader`]).
    fn versions(&self, project: &Path, after: Option<String>, limit: usize) -> Receiver<Result<VersionsPage, String>> {
        self.on_a_reader(project, move |reader| reader.versions(after.as_deref(), limit).map(page_of).map_err(|e| e.to_string()))
    }

    /// The model a version of the project holds, read on a thread of its own ([`Projects::on_a_reader`]).
    fn read_version(&self, project: &Path, commit: String) -> Receiver<Result<Arc<Rows>, String>> {
        let inner = self.inner.clone();
        self.on_a_reader(project, move |reader| {
            let schema = schema(&inner).map_err(|e| format!("{e:?}"))?;
            reader.read_model_at(&schema, &commit).map(Arc::new).map_err(|e| e.to_string())
        })
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

    fn discard_and_close(&self, project: &Path) -> Receiver<Result<(), String>> {
        self.close_unwritten(project, true)
    }

    fn close_without_writing(&self, project: &Path) -> Receiver<Result<(), String>> {
        self.close_unwritten(project, false)
    }

    fn find_dolt(&self) -> Receiver<Result<(), String>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let reply = tx.clone();
        // `dolt version` runs off the interface thread; one that is found is kept for every project.
        let looked = std::thread::Builder::new().name("archforge-dolt".into()).spawn(move || {
            let answer = dolt(&inner).map(|_| ()).map_err(|e| match e {
                CreateError::DoltMissing { detail } | CreateError::Failed { detail } | CreateError::FolderUnusable { detail } => detail,
                other => format!("{other:?}"),
            });
            let _ = reply.send(answer);
        });
        if let Err(e) = looked {
            let _ = tx.send(Err(format!("the look for dolt could not start: {e}")));
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
        // The project records which model it holds; each table comes with the first rows that go there.
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
            // Damaged conversations never keep the model from opening: the chat panel says why.
            if let Err(e) = project.open_chats(&now()) {
                log::warn!("the conversations of {} were not prepared: {e}", project.path().display());
            }
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
    let reader = OpenReader { reader: project.version_reader(), under_way: AtomicUsize::new(0) };
    lock(&inner.readers).insert(project.path().to_path_buf(), Arc::new(reader));
    open.push(project);
    Ok(())
}

/// Closes the project at `path`, one job at a time: what waits for it is written, then its server
/// stops. When its changes cannot be written it stays open and the answer says why, so the window
/// keeps its tab and nothing is lost.
fn close_job(inner: &Arc<Inner>, path: &Path, reply: &Sender<Result<(), String>>) {
    let _job = lock(&inner.job);
    if let Err(e) = flush_one(inner, path) {
        log::error!("{} stays open: {e}", path.display());
        let _ = reply.send(Err(e));
        return;
    }
    keep_chats(inner, path);
    let project = {
        let mut open = lock(&inner.open);
        open.iter().position(|p| p.path() == path).map(|i| open.remove(i))
    };
    settle_reads(inner, path, FLUSH_BUDGET);
    lock(&inner.writing).remove(path);
    if let Some(project) = project {
        log::info!("closed the project {}", path.display());
        if let Err(e) = project.close() {
            log::warn!("the project {} did not close cleanly: {e}", path.display());
        }
    }
    let _ = reply.send(Ok(()));
}

/// Closes a project without writing what waits for it: the writer forgets its waiting rows, then
/// the project closes. With `reset` (Don't save) it is first put back to its last save, and a
/// reset that fails keeps it open, saying why; without (Close without writing) what was written
/// stays, unsaved, for the next opening. A writer too busy to let go keeps it open too, so no
/// forgotten rows can reach it later.
fn discard_job(inner: &Arc<Inner>, path: &Path, reset: bool, reply: &Sender<Result<(), String>>) {
    let _job = lock(&inner.job);
    let busy = || {
        log::warn!("the writer did not let go of {} in time; it stays open", path.display());
        "the changes are still being written; try again in a moment".to_string()
    };
    if reset {
        // On the writer, after every job sent before it: what waits is forgotten, the conversations
        // are kept, then the model goes back to its last save.
        let (done, answer) = channel();
        let claim = Arc::new(AtomicBool::new(false));
        let undone = match writer(inner).send(Job::Discard(path.to_path_buf(), claim.clone(), done)) {
            Ok(()) => match answer.recv_timeout(FLUSH_BUDGET) {
                Ok(answer) => answer,
                // Given up on before the writer took it: it will not run.
                Err(_) if claim.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() => Err(busy()),
                // The writer has taken it: its answer comes once the reset ends.
                Err(_) => answer.recv().unwrap_or_else(|_| Err("the writer stopped".into())),
            },
            Err(_) => Err(busy()),
        };
        if let Err(e) = undone {
            log::warn!("the project {} could not be put back to its last save: {e}", path.display());
            let _ = reply.send(Err(format!("it could not be put back to its last save ({e})")));
            return;
        }
        log::info!("put the project {} back to its last save", path.display());
    } else {
        let writer = lock(&inner.writer).clone();
        if let Some(tx) = writer {
            let (done, answer) = channel();
            if tx.send(Job::Drop(path.to_path_buf(), done)).is_err() || answer.recv_timeout(FLUSH_BUDGET).is_err() {
                let _ = reply.send(Err(busy()));
                return;
            }
        }
        keep_chats(inner, path);
    }
    let project = {
        let mut open = lock(&inner.open);
        open.iter().position(|p| p.path() == path).map(|i| open.remove(i))
    };
    settle_reads(inner, path, FLUSH_BUDGET);
    lock(&inner.writing).remove(path);
    if let Some(project) = project
        && let Err(e) = project.close()
    {
        log::warn!("the project {} did not close cleanly: {e}", path.display());
    }
    let _ = reply.send(Ok(()));
}

impl Projects {
    /// Closes `project` without writing what waits for it, off the interface thread; with `reset`
    /// put back to its last save first.
    fn close_unwritten(&self, project: &Path, reset: bool) -> Receiver<Result<(), String>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let path = project.to_path_buf();
        let reply = tx.clone();
        match std::thread::Builder::new().name("archforge-discard".into()).spawn(move || discard_job(&inner, &path, reset, &reply)) {
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
}

#[cfg(test)]
impl Projects {
    /// Waits until what waits for `project` is written, as a closing does first.
    fn flush_for_test(&self, project: &Path) -> Result<(), String> {
        flush_one(&self.inner, project)
    }
}

/// The model's tables, described once from every notation's storage.
fn schema(inner: &Inner) -> Result<Arc<ModelSchema>, CreateError> {
    let mut held = lock(&inner.schema);
    if let Some(s) = held.as_ref() {
        return Ok(s.clone());
    }
    let failed = |detail: String| CreateError::Failed { detail };
    let universe = archforge_notations::store(Box::new(|| archforge_mof::Uuid([0; 16]))).map_err(failed)?.universe().clone();
    let (tables, junctions) = archforge_notations::storage();
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
    // How many sets of rows each project's queue has delivered, to hold against `Described::handed`.
    let mut seen: BTreeMap<PathBuf, u64> = BTreeMap::new();
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
                *seen.entry(project.clone()).or_default() += 1;
                waiting.insert(project, rows);
                failed = false;
            }
            None => {
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
            }
            Some(Job::Save(project, described, reply)) => {
                // Before anything is written: the rows that came in stay waiting, as any others do.
                if let (Some((p, _)), Some(d)) = (&project, &described)
                    && seen.get(p).copied().unwrap_or(0) != d.handed
                {
                    let _ = reply.send(Err(SaveError::Changed));
                    continue;
                }
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
                let answer = match project {
                    // A save needs its own project's rows written; a flush needs all.
                    Some((p, _)) if waiting.contains_key(&p) => Err(SaveError::Failed(unwritten(inner, &p))),
                    Some((p, by)) => commit(inner, &p, described.map(|d| d.text), &by),
                    None => match waiting.keys().next() {
                        Some(p) => Err(SaveError::Failed(unwritten(inner, p))),
                        None => Ok(Saved::NothingNew),
                    },
                };
                let _ = reply.send(answer);
            }
            Some(Job::Drop(project, reply)) => {
                waiting.remove(&project);
                failed = false;
                let _ = reply.send(());
            }
            Some(Job::Discard(project, claim, reply)) => {
                if claim.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                    // The closing stopped waiting and kept the project open: nothing changes.
                    continue;
                }
                let answer = match lock(&inner.open).iter().find(|p| p.path() == project) {
                    Some(p) => p.discard(&now()).map_err(|e| e.to_string()),
                    None => Err(format!("{} is no longer open", project.display())),
                };
                // The rows waiting are forgotten only once the model is back at its save: a refused
                // Don't save keeps the project open with every change it shows.
                if answer.is_ok() {
                    waiting.remove(&project);
                    failed = !waiting.is_empty();
                }
                let _ = reply.send(answer);
            }
            Some(Job::Chat(project, job)) => {
                let open = lock(&inner.open);
                // A failing job never stops the writer: the model's rows still get written.
                let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(open.iter().find(|p| p.path() == project))));
                if ran.is_err() {
                    log::error!("a job on the conversations of {} failed", project.display());
                }
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

/// Hands a save to the writer; a writer that has stopped is the save's answer.
fn send_save(inner: &Arc<Inner>, project: &Path, by: SavedBy, described: Option<Described>, reply: Sender<Result<Saved, SaveError>>) {
    if let Err(e) = writer(inner).send(Job::Save(Some((project.to_path_buf(), by)), described, reply.clone())) {
        let _ = reply.send(Err(SaveError::Failed(format!("the writer stopped: {e}"))));
    }
}

/// Why a description of the wrong shape is refused, in the language shown.
fn refusal(problem: DescriptionProblem) -> String {
    match problem {
        DescriptionProblem::Empty => tr("A description can't be empty."),
        DescriptionProblem::NotOneLine => tr("A description has to fit on one line."),
        DescriptionProblem::TooLong { chars } => fmt(
            &tr("A description is at most {max} characters; this one has {n}."),
            &[("max", &DESCRIPTION_CHARS.to_string()), ("n", &chars.to_string())],
        ),
    }
}

/// A save under `text`, once the text holds no kept API key: reading a key may wait for the system
/// to ask the person, so that runs on the agents host's thread and the writer is never held up.
/// Text that cannot be checked is not saved: it would stay in the project's history. Both refusals
/// are put in words here, since `tr` reads the language of the thread that calls it.
fn save_described(inner: &Arc<Inner>, project: &Path, by: SavedBy, text: String, reply: Sender<Result<Saved, SaveError>>) {
    let handed = lock(&inner.handed).get(project).copied().unwrap_or(0);
    let held = tr("This description holds one of your saved API keys, so nothing was saved.");
    let unchecked = tr("This description could not be checked against your saved API keys, so nothing was saved.");
    let Some(agents) = inner.keys.get() else {
        log::warn!("a description was refused: nothing checks it against the kept API keys");
        let _ = reply.send(Err(SaveError::Failed(unchecked)));
        return;
    };
    let checked = agents.check_keys(text.clone());
    let (inner, path, answer, words) = (inner.clone(), project.to_path_buf(), reply.clone(), unchecked.clone());
    let started = std::thread::Builder::new().name("archforge-save-keys".into()).spawn(move || {
        match checked.recv().unwrap_or_else(|_| Err("the agents host stopped".into())) {
            Ok(None) => send_save(&inner, &path, by, Some(Described { text, handed }), answer),
            Ok(Some(_)) => {
                let _ = answer.send(Err(SaveError::Failed(held)));
            }
            Err(why) => {
                log::warn!("a description could not be checked against the kept API keys: {why}");
                let _ = answer.send(Err(SaveError::Failed(words)));
            }
        }
    });
    if let Err(e) = started {
        log::warn!("a description could not be checked: {e}");
        let _ = reply.send(Err(SaveError::Failed(unchecked)));
    }
}

/// Save: a commit of everything written to the project at `target`, under `description` or else
/// the automatic description, the time it was made, recorded as `by`; it says what that did to
/// the history.
fn commit(inner: &Inner, target: &Path, description: Option<String>, by: &SavedBy) -> Result<Saved, SaveError> {
    let author = match by {
        SavedBy::Person(name) => Author::person(name),
        SavedBy::Agent(label) => Author::agent(label.as_deref()),
    };
    let message = description.unwrap_or_else(|| format!("Saved {}", now()));
    match lock(&inner.open).iter().find(|p| p.path() == target) {
        Some(project) => match project.save_version(&message, &author) {
            Ok(archforge_project::Saved::Version { hash }) => Ok(Saved::Version { hash }),
            Ok(archforge_project::Saved::NothingNew) => Ok(Saved::NothingNew),
            Err(failure) => Err(commit_error(failure)),
        },
        None => Err(SaveError::Failed(format!("{} is no longer open", target.display()))),
    }
}

/// Takes `path`'s reader away and waits, within `budget`, for the reads already under way, so its
/// server is not stopped under them. New reads are let in only through the map, so once the reader
/// has left it the count of reads under way can only fall. A read that outlasts the
/// budget fails when the server stops: a closing is never held up for longer.
fn settle_reads(inner: &Inner, path: &Path, budget: Duration) {
    let Some(open) = lock(&inner.readers).remove(path) else { return };
    let running = || open.under_way.load(Ordering::SeqCst) > 0;
    let started = Instant::now();
    while running() && started.elapsed() < budget {
        std::thread::sleep(Duration::from_millis(20));
    }
    if running() {
        log::warn!("a read of the versions of {} outlasted the closing", path.display());
    }
}

/// A page of versions in the shell's terms.
fn page_of(page: archforge_project::Versions) -> VersionsPage {
    let versions = page.versions.into_iter().map(|v| VersionEntry {
        hash: v.hash,
        description: v.description,
        author: v.author,
        by_agent: v.by_agent,
        date: v.date,
        previous: v.previous,
    });
    VersionsPage { versions: versions.collect(), more: page.more }
}

/// A commit's failure in the shell's terms, keeping what is known about a version.
fn commit_error(failure: archforge_project::SaveFailure) -> SaveError {
    let version = match failure.version {
        archforge_project::AfterFailure::NoVersion => VersionAfter::None,
        archforge_project::AfterFailure::Version(hash) => VersionAfter::Made(hash),
        archforge_project::AfterFailure::CannotTell => VersionAfter::Unknown,
    };
    SaveError::Commit { why: failure.error.to_string(), version }
}

/// The time now, as the project stores times.
fn now() -> String {
    archforge_project::timestamp(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

/// Runs `job` against the open project at `path` on the writer thread, after every job sent before
/// it, and answers with what it gives; a project no longer open is an error.
fn on_writer<T: Send + 'static>(
    inner: &Arc<Inner>,
    path: &Path,
    job: impl FnOnce(&Project) -> Result<T, String> + Send + 'static,
) -> Receiver<Result<T, String>> {
    let (tx, rx) = channel();
    let reply = tx.clone();
    let shown = path.display().to_string();
    let run: ChatJob = Box::new(move |project| {
        let _ = reply.send(project.map_or_else(|| Err(format!("{shown} is no longer open")), job));
    });
    if let Err(e) = writer(inner).send(Job::Chat(path.to_path_buf(), run)) {
        let _ = tx.send(Err(format!("the writer stopped: {e}")));
    }
    rx
}

/// Before a project closes: commits what of its conversations waits, on the writer. A failure only
/// goes to the log: the rows stay in the project's working set, and the next opening commits them.
fn keep_chats(inner: &Arc<Inner>, path: &Path) {
    let answer = on_writer(inner, path, |p| p.commit_chats().map_err(|e| e.to_string())).recv_timeout(FLUSH_BUDGET);
    match answer {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => log::warn!("the conversations of {} wait for the next opening: {e}", path.display()),
        Err(_) => log::warn!("the conversations of {} were not committed in time; they wait for the next opening", path.display()),
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
    if tx.send(Job::Save(None, None, reply)).is_err() {
        return Err(unwritten("the writer stopped".into()));
    }
    match answer.recv_timeout(FLUSH_BUDGET) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(unwritten(e.to_string())),
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

    /// Removes a test's folder once the servers that wrote in it have stopped. A file can still
    /// appear in it just after: the removal is tried again for a few seconds, and
    /// what stayed is named when it never empties.
    fn remove_settled(base: &Path) {
        let started = Instant::now();
        loop {
            match std::fs::remove_dir_all(base) {
                Ok(()) => return,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
                Err(e) if started.elapsed() < Duration::from_secs(5) => {
                    eprintln!("removing {} again after: {e}; left: {:?}", base.display(), left_in(base));
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => panic!("{} would not empty: {e}; left: {:?}", base.display(), left_in(base)),
            }
        }
    }

    /// The files under `dir`, at most twenty.
    fn left_in(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut todo = vec![dir.to_path_buf()];
        while let Some(d) = todo.pop() {
            for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = entry.path();
                if p.is_dir() {
                    todo.push(p);
                } else if out.len() < 20 {
                    out.push(p);
                }
            }
        }
        out
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
        remove_settled(&base);
    }

    #[test]
    fn dont_save_puts_a_project_back_to_its_last_save_and_closes_it() {
        let base = std::env::temp_dir().join(format!("af-discard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = Projects::new(&base.join("settings"), "Archforge test".into());
        let folder = base.join("Documents/Archforge");
        let path = wait(&host.create(&folder, "Commerce platform")).unwrap().path;
        let shop = model("shop");
        host.write(&path, shop.clone());
        assert_eq!(saved(&host, &path), Ok(true));
        // Changes written since the save, rows in a table first used since (UML's), and rows
        // still waiting, are all thrown away.
        host.write(&path, with_uml_package("shop"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        host.write(&path, model("draft"));
        let answer = host.discard_and_close(&path).recv_timeout(Duration::from_secs(60)).expect("the closing answers");
        assert_eq!(answer, Ok(()));
        assert!(open_paths(&host).is_empty());
        let back = wait(&host.open(&path)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&shop), "back to its last save, the newer table gone");
        assert!(!back.unsaved, "nothing unsaved is left");
        // Close without writing keeps what was written, unsaved, and forgets only what waits.
        let crm = model("crm");
        host.write(&path, crm.clone());
        assert_eq!(host.flush_for_test(&path), Ok(()));
        host.write(&path, model("draft"));
        let answer = host.close_without_writing(&path).recv_timeout(Duration::from_secs(60)).expect("the closing answers");
        assert_eq!(answer, Ok(()));
        let back = wait(&host.open(&path)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&crm), "what was written stays");
        assert!(back.unsaved);
        close(&host, &path).unwrap();
        // A project never saved goes back to how it was created: it opens as one never touched.
        let untouched = wait(&host.create(&folder, "Untouched")).unwrap().path;
        close(&host, &untouched).unwrap();
        let as_created = wait(&host.open(&untouched)).unwrap();
        close(&host, &untouched).unwrap();
        let fresh = wait(&host.create(&folder, "Fresh")).unwrap().path;
        host.write(&fresh, with_uml_package("x"));
        assert_eq!(host.flush_for_test(&fresh), Ok(()));
        let answer = host.discard_and_close(&fresh).recv_timeout(Duration::from_secs(60)).expect("the closing answers");
        assert_eq!(answer, Ok(()));
        let back = wait(&host.open(&fresh)).unwrap();
        assert!(back.model.as_deref().is_none_or(|rows| rows.is_empty()), "no model rows are left");
        assert_eq!(back.unsaved, as_created.unsaved, "as a project never touched opens");
        close(&host, &fresh).unwrap();
        host.shutdown();
        remove_settled(&base);
    }

    #[test]
    fn dont_save_through_the_host_keeps_the_conversations_written_on_the_writer() {
        use archforge_project::{NewChat, NewQuestion, NewTurn, Provider, TurnEnd, TurnOutcome};
        let base = std::env::temp_dir().join(format!("af-chats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = Projects::new(&base.join("settings"), "Archforge test".into());
        let path = wait(&host.create(&base.join("Documents/Archforge"), "Commerce platform")).unwrap().path;
        let shop = model("shop");
        host.write(&path, shop.clone());
        assert_eq!(saved(&host, &path), Ok(true));
        let id = |n: u8| archforge_mof::Uuid::v7(1_762_400_000_000, [n; 10]);
        let chat = id(1);
        let answer = |rx: Receiver<Result<(), String>>| rx.recv_timeout(Duration::from_secs(60)).expect("the writer answers");
        // A conversation and a turn, written on the writer in order with the model's rows.
        let made = on_writer(&host.inner, &path, move |p| {
            let now = archforge_project::timestamp(1_762_437_731);
            let new = NewChat {
                id: chat,
                origin: "Chat 1".into(),
                title: "Add a schema".into(),
                provider: Provider::Codex,
                model: "gpt-5-codex".into(),
                created_by: "Archforge test".into(),
                now: now.clone(),
            };
            p.create_chat(&new, &archforge_secrets::MemoryStore::new()).map_err(|e| e.to_string())?;
            let turn = NewTurn { id: id(2), chat, model: "gpt-5-codex".into(), allow_changes: true, now: now.clone() };
            let question = NewQuestion { id: id(3), body: "Add a crm schema".into(), anchors: Vec::new() };
            p.start_turn(&turn, &question, &archforge_secrets::MemoryStore::new()).map_err(|e| e.to_string())?;
            p.end_turn(&TurnEnd { turn: id(2), outcome: TurnOutcome::Completed, error: None, now, changes: None }).map_err(|e| e.to_string())
        });
        assert_eq!(answer(made), Ok(()));
        host.write(&path, model("crm"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        let answer = host.discard_and_close(&path).recv_timeout(Duration::from_secs(60)).expect("the closing answers");
        assert_eq!(answer, Ok(()));
        let back = wait(&host.open(&path)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&shop), "the model is back at its save");
        assert!(!back.unsaved, "the conversation never makes the project unsaved");
        let listed = on_writer(&host.inner, &path, |p| p.list_chats().map_err(|e| e.to_string()));
        let listed = listed.recv_timeout(Duration::from_secs(60)).expect("the writer answers").unwrap();
        let summary = listed[0].entry.as_ref().unwrap();
        assert_eq!((listed.len(), summary.chat.provider, summary.messages), (1, Provider::Codex, 1), "the conversation stayed");
        close(&host, &path).unwrap();
        host.shutdown();
        remove_settled(&base);
    }

    /// The host's store of conversations (`crate::chats`), over a real project and a secure store
    /// in memory holding a canary key: a question asked, failed, sent again in place, edited and
    /// sent again, its turn kept once; the chips removed kept; the conversation kept, the project
    /// closed and opened again, and read back whole; a question holding the key refused with
    /// nothing written.
    #[test]
    fn the_hosts_chat_store_runs_a_failed_turn_again_in_place_keeps_conversations_and_refuses_a_key() {
        use archforge_secrets::{MemoryStore, Secret, SecretName, SecretStore};
        use archforge_ui_egui::ai_agents::Provider;
        use archforge_ui_egui::chat::FailureKind;
        use archforge_ui_egui::chat_panel::Chip;
        use archforge_ui_egui::chat_store::{Anchor, AnchorRole, Answer, ChatStore, Outcome, Refusal, StoredBody, Write};
        const CANARY: &str = "sk-ant-api03-canary-chat-store-0000-4F2a";
        let base = std::env::temp_dir().join(format!("af-chat-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = std::sync::Arc::new(Projects::new(&base.join("settings"), "Archforge test".into()));
        let keys = MemoryStore::new();
        keys.set(&SecretName::ANTHROPIC_API_KEY, &Secret::new(CANARY.to_string()).unwrap()).unwrap();
        let agents = std::sync::Arc::new(crate::agents::Agents::with(&base.join("settings"), Box::new(keys), archforge_agents::HostEnv::default()));
        let chats = crate::chats::Chats::new(host.clone(), agents.clone());
        let path = wait(&host.create(&base.join("Documents/Archforge"), "Commerce platform")).unwrap().path;
        host.write(&path, model("shop"));
        assert_eq!(saved(&host, &path), Ok(true));
        let answer = |rx: Receiver<Result<Answer, Refusal>>| rx.recv_timeout(Duration::from_secs(60)).expect("the store answers");
        let id = |n: u8| archforge_mof::Uuid::v7(1_762_400_000_000, [n; 10]);
        let now = 1_762_437_731;
        let chat = id(1);
        // A question holding the kept key is refused before anything is written, conversation and all.
        let pasted = format!("Why is {CANARY} refused?");
        let refused =
            answer(chats.write(
                &path,
                Write::Create { chat, title: pasted.clone(), provider: Provider::Claude, model: "sonnet".into(), skip: Vec::new(), now },
            ));
        assert_eq!(refused, Err(Refusal::HeldKey { provider: Some(Provider::Claude) }));
        let listed = chats.list(&path).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        assert!(listed.is_empty(), "nothing was written");
        // Made, asked, failed on a lost connection.
        let made = answer(chats.write(
            &path,
            Write::Create {
                chat,
                title: "Which classes have no description yet?".into(),
                provider: Provider::Claude,
                model: "sonnet".into(),
                skip: Vec::new(),
                now,
            },
        ));
        assert_eq!(made, Ok(Answer::Created { origin: "Chat 1".into() }));
        let chip = Anchor { element: id(9), role: AnchorRole::Selection, metaclass: "uml_class".into(), name: Some("Order".into()) };
        let ask = |turn, question, text: &str, now| Write::Ask {
            chat,
            turn,
            question,
            text: text.into(),
            chips: vec![chip.clone()],
            model: "sonnet".into(),
            allow_changes: false,
            now,
        };
        assert_eq!(answer(chats.write(&path, ask(id(2), id(3), "Which classes have no description yet?", now + 1))), Ok(Answer::Asked { number: 1 }));
        let key_again = answer(chats.write(
            &path,
            Write::Rerun { turn: id(2), model: "sonnet".into(), allow_changes: false, question: Some((pasted.clone(), Vec::new())), now: now + 2 },
        ));
        assert_eq!(key_again, Err(Refusal::HeldKey { provider: Some(Provider::Claude) }), "an edited question is checked too");
        let failed = |now| Write::End { turn: id(2), outcome: Outcome::Failed, error: Some(FailureKind::Network), changes: None, now };
        assert_eq!(answer(chats.write(&path, failed(now + 3))), Ok(Answer::Done));
        // Sent again as it was, and failed again; then edited and sent again, and answered.
        let rerun =
            |question: Option<(String, Vec<Anchor>)>, now| Write::Rerun { turn: id(2), model: "opus".into(), allow_changes: false, question, now };
        assert_eq!(answer(chats.write(&path, rerun(None, now + 4))), Ok(Answer::Done));
        assert_eq!(answer(chats.write(&path, failed(now + 5))), Ok(Answer::Done));
        assert_eq!(
            answer(chats.write(&path, rerun(Some(("Which classes lack a description?".into(), vec![chip.clone()])), now + 6))),
            Ok(Answer::Done)
        );
        assert_eq!(
            answer(
                chats.write(
                    &path,
                    Write::Reply { id: id(4), turn: id(2), text: "Five of them.".into(), complete: true, shortened: false, now: now + 7 }
                )
            ),
            Ok(Answer::Done)
        );
        assert_eq!(
            answer(chats.write(&path, Write::End { turn: id(2), outcome: Outcome::Completed, error: None, changes: None, now: now + 8 })),
            Ok(Answer::Done)
        );
        // The chips the person removed (AR-227), its last activity left alone.
        let removed = std::collections::BTreeSet::from([Chip::Diagram, Chip::Project]);
        assert_eq!(answer(chats.write(&path, Write::Chips { chat, removed: removed.clone() })), Ok(Answer::Done));
        // Closed and opened again: the conversation is there, whole, its turn kept once.
        close(&host, &path).unwrap();
        let back = wait(&host.open(&path)).unwrap();
        assert!(!back.unsaved, "conversations never make a project unsaved");
        let listed = chats.list(&path).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        let row = listed[0].entry.as_ref().unwrap();
        assert_eq!(
            (listed.len(), row.origin.as_str(), row.title.as_str(), row.messages, row.updated_at),
            (1, "Chat 1", "Which classes have no description yet?", 2, now + 8)
        );
        assert_eq!(row.removed, removed, "its chips removed came back with it");
        let stored = chats.load(&path, chat).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        assert_eq!(stored.turns.len(), 1, "one stored row per turn");
        assert_eq!((stored.turns[0].outcome, stored.turns[0].model.as_str()), (Outcome::Completed, "opus"));
        let questions: Vec<&StoredBody> = stored.messages.iter().map(|m| &m.body).filter(|b| matches!(b, StoredBody::Question { .. })).collect();
        assert_eq!(
            questions,
            [&StoredBody::Question { text: "Which classes lack a description?".into(), chips: vec![chip.clone()] }],
            "the edited question replaced the kept one, its chips with it"
        );
        assert!(matches!(&stored.messages[1].body, StoredBody::Reply { text, complete: true, .. } if text == "Five of them."));
        let raw = host.on_writer(&path, |p| p.load_chat(archforge_mof::Uuid::v7(1_762_400_000_000, [1; 10])).map_err(|e| e.to_string()));
        let raw = raw.recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        assert!(raw.messages.iter().all(|m| !m.message.body.as_deref().unwrap_or_default().contains(CANARY)), "the key is nowhere");
        close(&host, &path).unwrap();
        host.shutdown();
        agents.shutdown();
        remove_settled(&base);
    }

    /// The host's store deletes a conversation on the writer after the writes asked before it (a
    /// running turn ended stopped, its reply so far kept), answers with the commit before the
    /// deletion, gives a new conversation a label past the deleted one's while it can come back,
    /// and brings it back whole from that commit, once; a forged commit is refused (AR-217).
    #[test]
    fn the_hosts_chat_store_deletes_after_the_writes_before_it_and_brings_a_conversation_back_from_its_commit() {
        use archforge_ui_egui::ai_agents::Provider;
        use archforge_ui_egui::chat_store::{Answer, ChatStore, Outcome, Refusal, StoredBody, Write};
        let base = std::env::temp_dir().join(format!("af-chat-delete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = std::sync::Arc::new(Projects::new(&base.join("settings"), "Archforge test".into()));
        let agents = std::sync::Arc::new(crate::agents::Agents::with(
            &base.join("settings"),
            Box::new(archforge_secrets::MemoryStore::new()),
            archforge_agents::HostEnv::default(),
        ));
        let chats = crate::chats::Chats::new(host.clone(), agents.clone());
        let path = wait(&host.create(&base.join("Documents/Archforge"), "Commerce platform")).unwrap().path;
        let answer = |rx: Receiver<Result<Answer, Refusal>>| rx.recv_timeout(Duration::from_secs(60)).expect("the store answers");
        let id = |n: u8| archforge_mof::Uuid::v7(1_762_400_000_000, [n; 10]);
        let now = 1_762_437_731;
        let (chat, other) = (id(1), id(5));
        let create = |chat, skip: Vec<String>| Write::Create {
            chat,
            title: "Review the checkout package".into(),
            provider: Provider::Codex,
            model: "default".into(),
            skip,
            now,
        };
        assert_eq!(answer(chats.write(&path, create(chat, Vec::new()))), Ok(Answer::Created { origin: "Chat 1".into() }));
        let ask = Write::Ask {
            chat,
            turn: id(2),
            question: id(3),
            text: "Review it".into(),
            chips: Vec::new(),
            model: "default".into(),
            allow_changes: false,
            now: now + 1,
        };
        assert_eq!(answer(chats.write(&path, ask)), Ok(Answer::Asked { number: 1 }));
        // The turn still running is ended stopped, its reply so far kept, and deleted: asked one
        // after the other without waiting, they run in that order on the writer.
        let reply =
            chats.write(&path, Write::Reply { id: id(4), turn: id(2), text: "So far".into(), complete: false, shortened: false, now: now + 2 });
        let end = chats.write(&path, Write::End { turn: id(2), outcome: Outcome::Stopped, error: None, changes: None, now: now + 3 });
        let deleted = chats.write(&path, Write::Delete { chat });
        assert_eq!((answer(reply), answer(end)), (Ok(Answer::Done), Ok(Answer::Done)));
        let Ok(Answer::Deleted { before }) = answer(deleted) else { panic!("deleted") };
        assert!(chats.list(&path).recv_timeout(Duration::from_secs(60)).unwrap().unwrap().is_empty());
        // A new conversation skips the label of the one that can still come back.
        assert_eq!(answer(chats.write(&path, create(other, vec!["Chat 1".into()]))), Ok(Answer::Created { origin: "Chat 2".into() }));
        // Brought back whole, from the commit the deletion named; a forged one and a second time refused.
        let forged = answer(chats.write(&path, Write::Restore { chat, from: Some("0".repeat(32)) }));
        assert!(matches!(forged, Err(Refusal::Failed(why)) if why.contains("not a commit")), "a forged commit");
        assert_eq!(answer(chats.write(&path, Write::Restore { chat, from: Some(before.clone()) })), Ok(Answer::Done));
        let again = answer(chats.write(&path, Write::Restore { chat, from: Some(before) }));
        assert_eq!(again, Err(Refusal::Listed), "listed already");
        // Undo pressed before the deletion answered: the restore names no commit, and takes the
        // one the deletion just before it on the writer gave.
        let other_deleted = chats.write(&path, Write::Delete { chat: other });
        let undone = chats.write(&path, Write::Restore { chat: other, from: None });
        assert!(matches!(answer(other_deleted), Ok(Answer::Deleted { .. })));
        assert_eq!(answer(undone), Ok(Answer::Done));
        assert_eq!(
            answer(chats.write(&path, Write::Restore { chat: id(9), from: None })),
            Err(Refusal::Failed("no deletion of it was asked in this project".into()))
        );
        // A commit kept from an earlier deletion never serves a later one: deleted for good, a
        // second deletion of it fails, and an Undo of that brings nothing back.
        assert!(matches!(answer(chats.write(&path, Write::Delete { chat: other })), Ok(Answer::Deleted { .. })));
        assert!(matches!(answer(chats.write(&path, Write::Delete { chat: other })), Err(Refusal::Failed(_))));
        assert_eq!(
            answer(chats.write(&path, Write::Restore { chat: other, from: None })),
            Err(Refusal::Failed("no deletion of it was asked in this project".into()))
        );
        let listed = chats.list(&path).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        assert_eq!(listed.len(), 1);
        let stored = chats.load(&path, chat).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        assert_eq!((stored.row.origin.as_str(), stored.turns[0].outcome), ("Chat 1", Outcome::Stopped));
        assert!(matches!(&stored.messages[1].body, StoredBody::Reply { text, complete: false, .. } if text == "So far"), "the reply so far");
        close(&host, &path).unwrap();
        host.shutdown();
        agents.shutdown();
        remove_settled(&base);
    }

    /// A model's rows with a schema and a UML package: rows in UML's tables too.
    fn with_uml_package(schema: &str) -> Rows {
        let mut n = 0u64;
        let ids = Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_800_000, r)
        });
        let mut s = archforge_notations::new_project_session("Commerce platform", ids).unwrap();
        s.execute("er.schema.add", &serde_json::json!({ "name": schema })).unwrap();
        s.execute("uml.package.add", &serde_json::json!({ "name": "Domain" })).unwrap();
        let (tables, junctions) = archforge_notations::storage();
        archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap()
    }

    /// Runs `read` while another thread holds the open-projects lock, which also stops the writer.
    /// The holder lets go when told or after a bounded wait, so a read that waits for the lock
    /// fails here instead of hanging the test.
    fn with_open_lock_held<T: Send>(host: &Projects, read: impl FnOnce() -> T) -> T {
        std::thread::scope(|scope| {
            let (locked, is_locked) = channel();
            let (release, released) = channel::<()>();
            let holder = scope.spawn(move || {
                let _open = lock(&host.inner.open);
                let _ = locked.send(());
                released.recv_timeout(Duration::from_secs(15)).is_ok()
            });
            is_locked.recv_timeout(Duration::from_secs(60)).expect("the lock is held");
            let answer = read();
            let _ = release.send(());
            assert!(holder.join().unwrap(), "the read waited for the open-projects lock");
            answer
        })
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
        let mut s = archforge_notations::new_project_session("Commerce platform", ids).unwrap();
        s.execute("er.schema.add", &serde_json::json!({ "name": schema })).unwrap();
        let (tables, junctions) = archforge_notations::storage();
        archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap()
    }

    /// A save of `project` without a description, and its answer.
    fn saved(host: &Projects, project: &Path) -> Result<bool, SaveError> {
        made(host.save(project, None, nobody()).recv_timeout(Duration::from_secs(60)).expect("the save answers"))
    }

    /// A person with no name set, who asks for most of these tests' saves.
    fn nobody() -> SavedBy {
        SavedBy::Person(String::new())
    }

    /// A save's answer as whether it made a version.
    fn made(answer: Result<Saved, SaveError>) -> Result<bool, SaveError> {
        answer.map(|saved| matches!(saved, Saved::Version { .. }))
    }

    /// A save of `project` under `text`, and its answer.
    fn described(host: &Projects, project: &Path, text: &str) -> Result<bool, SaveError> {
        made(host.save(project, Some(text.to_string()), nobody()).recv_timeout(Duration::from_secs(60)).expect("the save answers"))
    }

    /// A host in a folder of its own (`create_dir` refuses one that exists), checking descriptions
    /// against `keys` when given. Dropped, even by a failing test, it stops what it started and
    /// the folder goes.
    struct Sandbox {
        base: PathBuf,
        host: Projects,
        agents: Option<Arc<crate::agents::Agents>>,
    }

    impl Sandbox {
        /// The host, with its folder created and `keys` wired in.
        fn new(keys: Option<Box<dyn archforge_secrets::SecretStore>>) -> Sandbox {
            let base = (0..1000)
                .map(|n| std::env::temp_dir().join(format!("af-d{}-{n}", std::process::id())))
                .find(|p| std::fs::create_dir(p).is_ok())
                .expect("a folder of the test's own");
            let host = Projects::new(&base.join("settings"), "Archforge test".into());
            let agents = keys.map(|k| Arc::new(crate::agents::Agents::with(&base.join("settings"), k, archforge_agents::HostEnv::default())));
            if let Some(a) = &agents {
                host.check_descriptions_with(a.clone());
            }
            Sandbox { base, host, agents }
        }

        /// A project made in the sandbox.
        fn project(&self) -> PathBuf {
            wait(&self.host.create(&self.base.join("p"), "Shop")).unwrap().path
        }

        /// The project's commit messages, newest first.
        fn messages(&self) -> Vec<String> {
            lock(&self.host.inner.open)[0].newest_commit_messages(50).unwrap()
        }
    }

    impl Drop for Sandbox {
        /// Stops the servers and threads, then removes the folder, retrying while a stopping server
        /// still writes. It never panics, so it is safe while a failed test unwinds.
        fn drop(&mut self) {
            self.host.shutdown();
            if let Some(a) = &self.agents {
                a.shutdown();
            }
            for _ in 0..50 {
                if std::fs::remove_dir_all(&self.base).is_ok() || !self.base.exists() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    /// A store holding [`KEY`] as the Anthropic key.
    fn store_with_key() -> Box<dyn archforge_secrets::SecretStore> {
        use archforge_secrets::{MemoryStore, Secret, SecretName, SecretStore};
        let keys = MemoryStore::new();
        keys.set(&SecretName::ANTHROPIC_API_KEY, &Secret::new(KEY.to_string()).unwrap()).unwrap();
        Box::new(keys)
    }

    /// A key no real account has.
    const KEY: &str = "sk-ant-canary-0123456789";

    /// The description given is the commit message as typed, and a save without one keeps its time.
    #[test]
    fn a_description_becomes_the_saved_message_exactly_and_a_save_without_one_stays_automatic() {
        let sandbox = Sandbox::new(Some(store_with_key()));
        let (host, path) = (&sandbox.host, sandbox.project());
        host.write(&path, model("a"));
        // Bound as a value: quotes and other languages come back as typed.
        let typed = "Client's \"orders\" renamed · 顧客";
        assert_eq!(described(host, &path, typed), Ok(true));
        assert_eq!(sandbox.messages()[0], typed);
        // A text shaped like one of the commit's options is still only the text: it makes a new
        // version and amends nothing.
        let before = sandbox.messages().len();
        host.write(&path, model("b"));
        assert_eq!(described(host, &path, "--amend"), Ok(true));
        assert_eq!(sandbox.messages()[0], "--amend");
        assert_eq!(sandbox.messages().len(), before + 1);
        // Without a description a save is described by its time, as before.
        host.write(&path, model("d"));
        assert_eq!(saved(host, &path), Ok(true));
        let automatic = sandbox.messages()[0].clone();
        let time = automatic.strip_prefix("Saved ").unwrap_or_else(|| panic!("automatic: {automatic}"));
        assert!(time.len() == 20 && time.ends_with('Z'), "an RFC 3339 time: {automatic}");
        // Nothing new to save makes no version, described or not.
        let before = sandbox.messages();
        assert_eq!(described(host, &path, "Nothing changed"), Ok(false));
        assert_eq!(sandbox.messages(), before);
    }

    /// Every refusal says its own reason, makes no version and leaves the work for the next save.
    #[test]
    fn a_refused_description_makes_no_version_and_the_work_stays_to_be_saved() {
        let sandbox = Sandbox::new(Some(store_with_key()));
        let (host, path) = (&sandbox.host, sandbox.project());
        host.write(&path, model("a"));
        assert_eq!(saved(host, &path), Ok(true));
        host.write(&path, model("b"));
        let before = sandbox.messages();
        let too_long = "x".repeat(DESCRIPTION_CHARS + 1);
        let refusals = [
            ("   ", "A description can't be empty.".to_string()),
            ("two\nlines", "A description has to fit on one line.".to_string()),
            (too_long.as_str(), "A description is at most 200 characters; this one has 201.".to_string()),
            (&format!("see {KEY} here"), "This description holds one of your saved API keys, so nothing was saved.".to_string()),
        ];
        for (text, words) in refusals {
            assert_eq!(described(host, &path, text), Err(SaveError::Failed(words)));
            assert_eq!(sandbox.messages(), before, "no version from {text:?}");
        }
        // The refusal is in the language shown, though the check ran on another thread.
        archforge_ui_egui::i18n::set_language("de");
        let german = described(host, &path, KEY);
        let expected = tr("This description holds one of your saved API keys, so nothing was saved.");
        archforge_ui_egui::i18n::set_language("en");
        assert_eq!(german, Err(SaveError::Failed(expected.clone())));
        assert_ne!(expected, "This description holds one of your saved API keys, so nothing was saved.");
        // What was being saved is still there for the next save, and the key is in no message.
        assert_eq!(described(host, &path, "After the refusals"), Ok(true));
        let after = sandbox.messages();
        assert_eq!(after[0], "After the refusals");
        assert!(after.iter().all(|m| !m.contains(KEY)));
    }

    /// A save answers with the new version's hash or nothing new, and a failure keeps what is known.
    #[test]
    fn a_save_answers_with_its_hash_and_a_failed_commit_keeps_what_is_known() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        host.write(&path, model("a"));
        let ask = || host.save(&path, None, nobody()).recv_timeout(Duration::from_secs(60)).expect("the save answers");
        let answer = ask();
        assert!(matches!(&answer, Ok(Saved::Version { hash: Some(h) }) if h.len() == 32), "a version with a hash: {answer:?}");
        assert_eq!(ask(), Ok(Saved::NothingNew));

        use archforge_project::{AfterFailure as After, Error, SaveFailure};
        let told = |version| commit_error(SaveFailure { error: Error::Damaged { path: "/p".into(), why: "x".into() }, version });
        let why = || "the project /p is damaged: x".to_string();
        assert_eq!(told(After::NoVersion), SaveError::Commit { why: why(), version: VersionAfter::None });
        assert_eq!(told(After::Version("abc".into())), SaveError::Commit { why: why(), version: VersionAfter::Made("abc".into()) });
        assert_eq!(told(After::CannotTell), SaveError::Commit { why: why(), version: VersionAfter::Unknown });
    }

    /// Who asked for a save reaches the version, whether or not the save is described.
    #[test]
    fn a_save_records_who_asked_for_it() {
        let sandbox = Sandbox::new(Some(store_with_key()));
        let (host, path) = (&sandbox.host, sandbox.project());
        let author = |by: SavedBy, schema: &str, text: Option<&str>| {
            host.write(&path, model(schema));
            let answer = host.save(&path, text.map(str::to_string), by).recv_timeout(Duration::from_secs(60)).expect("the save answers");
            assert!(matches!(answer, Ok(Saved::Version { .. })), "{answer:?}");
            lock(&host.inner.open)[0].newest_commit_author().unwrap()
        };
        let person = (String::from("Ada Lovelace"), String::from("archforge@localhost"));
        assert_eq!(author(SavedBy::Person("Ada (Countess) Lovelace".into()), "a", None), (String::from("Ada Countess Lovelace"), person.1.clone()));
        assert_eq!(author(SavedBy::Person("Ada Lovelace".into()), "b", Some("Described")), person);
        assert_eq!(author(SavedBy::Person("李雷".into()), "e", None), (String::from("李雷"), person.1.clone()));
        assert_eq!(author(SavedBy::Person(String::new()), "f", None), (String::from("Archforge"), person.1.clone()));
        assert_eq!(author(SavedBy::Agent(Some("Chat 3".into())), "c", None), ("Chat 3".to_string(), "agent@localhost".to_string()));
        assert_eq!(author(SavedBy::Agent(None), "d", None), ("Agent".to_string(), "agent@localhost".to_string()));
    }

    /// Versions come back while the open-projects lock is held, which also stops the writer, so
    /// the read uses neither; the reader goes with the project and comes back with it.
    #[test]
    fn versions_are_read_without_the_writer_or_the_open_projects() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let read = |after: Option<String>, limit| host.versions(&path, after, limit).recv_timeout(Duration::from_secs(30)).expect("the read answers");
        host.write(&path, model("a"));
        assert_eq!(saved(host, &path), Ok(true));
        host.write(&path, model("b"));
        let by_agent = host.save(&path, None, SavedBy::Agent(Some("Chat 3".into()))).recv_timeout(Duration::from_secs(60)).expect("the save answers");
        assert!(matches!(by_agent, Ok(Saved::Version { .. })), "{by_agent:?}");

        let page = with_open_lock_held(host, || read(None, 1)).expect("the versions, with the lock held");
        let newest = &page.versions[0];
        assert!(page.more);
        assert_eq!((newest.author.as_deref(), newest.by_agent, newest.description.as_ref()), (Some("Chat 3"), true, None));
        assert_eq!(newest.date.len(), 20, "{}", newest.date);

        let older = read(Some(newest.hash.clone()), 1).unwrap();
        assert_eq!((older.versions.len(), older.more), (1, false));
        assert_eq!(newest.previous.as_ref(), Some(&older.versions[0].hash));
        assert_eq!((older.versions[0].author.as_ref(), older.versions[0].by_agent), (None, false));

        let closed = Err(format!("{} is no longer open", path.display()));
        close(host, &path).unwrap();
        assert_eq!(read(None, 1), closed, "the reader goes with the project");
        wait(&host.open(&path)).unwrap();
        assert_eq!(read(None, 1).unwrap().versions[0].hash, newest.hash, "and comes back with it, on the new server");
    }

    /// A version's model comes back while the open-projects lock is held, whatever came after it;
    /// a reader goes with its project and a new one comes with the project opened again.
    #[test]
    fn a_version_is_read_without_the_writer_and_goes_with_the_project() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let read = |commit: &str| host.read_version(&path, commit.to_string()).recv_timeout(Duration::from_secs(30)).expect("the read answers");
        host.write(&path, model("a"));
        assert_eq!(saved(host, &path), Ok(true));
        let first = host.versions(&path, None, 1).recv_timeout(Duration::from_secs(30)).unwrap().unwrap().versions[0].hash.clone();
        host.write(&path, model("b"));
        assert_eq!(saved(host, &path), Ok(true));
        assert_eq!(*with_open_lock_held(host, || read(&first)).expect("the version, with the lock held"), model("a"));
        assert!(read("0123456789abcdefghijklmnopqrstuv").unwrap_err().contains("not a version"), "a commit it does not hold");
        let thread = host.on_a_reader(&path, |_| Ok(std::thread::current().name().map(str::to_string)));
        assert_eq!(thread.recv_timeout(Duration::from_secs(30)).unwrap(), Ok(Some("archforge-versions".to_string())), "a thread of its own");

        // A reader kept past the closing, as a read still running would be, reads nothing more.
        let stale = lock(&host.inner.readers).get(&path).unwrap().reader.clone();
        close(host, &path).unwrap();
        let shape = schema(&host.inner).unwrap();
        assert!(matches!(stale.read_model_at(&shape, &first), Err(archforge_project::Error::Server(_) | archforge_project::Error::Sql { .. })));
        assert_eq!(read(&first), Err(format!("{} is no longer open", path.display())), "and a new read does not reach it");
        wait(&host.open(&path)).unwrap();
        assert_eq!(*read(&first).unwrap(), model("a"), "the project opened again has a reader of its own");
    }

    /// Reads past the cap are refused until one ends, and the cap is each project's own.
    #[test]
    fn reads_of_one_project_are_capped() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let other = wait(&host.create(&sandbox.base.join("p"), "Other")).unwrap().path;
        let read = |project: &Path| host.versions(project, None, 1).recv_timeout(Duration::from_secs(30)).expect("the read answers");
        // A clone held for another reason takes no place.
        let _kept: Vec<_> = (0..2 * MOST_READS).map(|_| lock(&host.inner.readers).get(&path).cloned().unwrap()).collect();
        let mut under_way: Vec<_> = (0..MOST_READS).map(|_| host.take_a_read(&path).expect("a place")).collect();
        let too_many = "Too many versions are being read at once. Try again in a moment.".to_string();
        assert_eq!(read(&path), Err(too_many.clone()));
        assert_eq!(host.read_version(&path, "any".into()).recv_timeout(Duration::from_secs(30)).unwrap(), Err(too_many), "the models read count too");
        assert!(read(&other).is_ok(), "another project is not held back");
        under_way.pop();
        assert!(read(&path).is_ok(), "one ended, so one may start");
    }

    /// A read gives its place back however it ends, and a clone held outside a read holds up nothing.
    #[test]
    fn a_read_gives_its_place_back_on_every_path_and_a_clone_holds_no_closing_up() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let open = lock(&host.inner.readers).get(&path).cloned().expect("the open project's reader");
        let failed = host.read_version(&path, "not a hash".into()).recv_timeout(Duration::from_secs(30)).expect("the read answers");
        assert!(failed.is_err() && open.under_way.load(Ordering::SeqCst) == 0, "a failed read holds no place");
        let panicked = host.on_a_reader(&path, |_| -> Result<(), String> { panic!("a read that panics") });
        assert!(panicked.recv_timeout(Duration::from_secs(30)).is_err() && open.under_way.load(Ordering::SeqCst) == 0, "nor does one that panics");
        assert_eq!(host.close(&path).recv_timeout(Duration::from_secs(5)).expect("the closing does not wait for the clone"), Ok(()));
    }

    /// Closing waits for a read under way, and for no longer than its budget.
    #[test]
    fn closing_waits_for_the_reads_under_way_within_a_budget() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let open = lock(&host.inner.readers).get(&path).cloned().expect("the open project's reader");
        let under_way = host.take_a_read(&path).expect("a read");

        // A read that does not end is given up on when the budget runs out. On a thread of its own,
        // so that a wait with no bound fails here instead of hanging the test.
        lock(&host.inner.readers).insert(PathBuf::from("/held"), open.clone());
        let (inner, (done, settled)) = (host.inner.clone(), channel());
        std::thread::spawn(move || {
            settle_reads(&inner, Path::new("/held"), Duration::from_millis(50));
            let _ = done.send(());
        });
        settled.recv_timeout(Duration::from_secs(30)).expect("the budget ends the wait");
        assert!(!lock(&host.inner.readers).contains_key(Path::new("/held")));

        let closing = host.close(&path);
        assert!(closing.recv_timeout(Duration::from_secs(2)).is_err(), "the closing waits for the read");
        assert!(under_way.0.reader.versions(None, 1).is_ok(), "and the server still answers it");
        drop(under_way);
        assert_eq!(closing.recv_timeout(Duration::from_secs(60)).expect("the closing answers"), Ok(()));
    }

    /// A store that is locked: no key can be read from it.
    struct Locked;

    impl archforge_secrets::SecretStore for Locked {
        /// Refuses every read, as a locked store does.
        fn get(&self, name: &archforge_secrets::SecretName) -> Result<archforge_secrets::Secret, archforge_secrets::SecretError> {
            Err(archforge_secrets::SecretError::Denied { name: name.to_string(), detail: "locked".into() })
        }
        /// Not used here.
        fn set(&self, _: &archforge_secrets::SecretName, _: &archforge_secrets::Secret) -> Result<(), archforge_secrets::SecretError> {
            Ok(())
        }
        /// Not used here.
        fn delete(&self, _: &archforge_secrets::SecretName) -> Result<archforge_secrets::Deleted, archforge_secrets::SecretError> {
            Ok(archforge_secrets::Deleted::WasAbsent)
        }
    }

    /// A description whose keys cannot be checked is refused, whether the store or the checker is missing.
    #[test]
    fn a_description_that_cannot_be_checked_for_keys_is_not_saved() {
        let unchecked =
            Err(SaveError::Failed("This description could not be checked against your saved API keys, so nothing was saved.".to_string()));
        // Keys that cannot be read, and a host told of no one to check them.
        for keys in [Some(Box::new(Locked) as Box<dyn archforge_secrets::SecretStore>), None] {
            let sandbox = Sandbox::new(keys);
            let (host, path) = (&sandbox.host, sandbox.project());
            host.write(&path, model("a"));
            let before = sandbox.messages();
            assert_eq!(described(host, &path, "Renamed the client"), unchecked);
            assert_eq!(sandbox.messages(), before);
            // A save without a description needs no check.
            assert_eq!(saved(host, &path), Ok(true));
        }
    }

    /// Holds every key read until released, so a test can act while a key check is under way.
    #[derive(Default)]
    struct KeyGate {
        /// Whether reads are held, and how many reads have arrived.
        state: Mutex<(bool, usize)>,
        changed: std::sync::Condvar,
    }

    impl KeyGate {
        /// Holds or lets through the reads to come.
        fn hold(&self, held: bool) {
            *lock(&self.state) = (held, 0);
            self.changed.notify_all();
        }

        /// Waits until a read has arrived and is held.
        fn wait_for_a_read(&self) {
            let guard = lock(&self.state);
            let (state, timeout) = self.changed.wait_timeout_while(guard, Duration::from_secs(60), |s| s.1 == 0).unwrap();
            assert!(!timeout.timed_out() && state.0, "the key check reached the store");
        }
    }

    /// A store with no keys whose reads pass through a [`KeyGate`].
    struct Gated(Arc<KeyGate>);

    impl archforge_secrets::SecretStore for Gated {
        /// Waits at the gate, then finds no key.
        fn get(&self, name: &archforge_secrets::SecretName) -> Result<archforge_secrets::Secret, archforge_secrets::SecretError> {
            let mut state = lock(&self.0.state);
            state.1 += 1;
            self.0.changed.notify_all();
            let _released = self.0.changed.wait_while(state, |s| s.0).unwrap();
            Err(archforge_secrets::SecretError::NotFound { name: name.to_string() })
        }
        /// Not used here.
        fn set(&self, _: &archforge_secrets::SecretName, _: &archforge_secrets::Secret) -> Result<(), archforge_secrets::SecretError> {
            Ok(())
        }
        /// Not used here.
        fn delete(&self, _: &archforge_secrets::SecretName) -> Result<archforge_secrets::Deleted, archforge_secrets::SecretError> {
            Ok(archforge_secrets::Deleted::WasAbsent)
        }
    }

    /// Rows handed over during a described save's key check answer "changed" and commit nothing;
    /// rows for another project do not, as they are not this save's.
    #[test]
    fn rows_handed_over_during_the_key_check_stop_a_described_save() {
        let gate = Arc::new(KeyGate::default());
        let sandbox = Sandbox::new(Some(Box::new(Gated(gate.clone()))));
        let (host, path) = (&sandbox.host, sandbox.project());
        let other = wait(&host.create(&sandbox.base.join("p"), "Other")).unwrap().path;
        let answer = |rx: Receiver<Result<Saved, SaveError>>| made(rx.recv_timeout(Duration::from_secs(60)).expect("the save answers"));

        host.write(&path, model("a"));
        gate.hold(true);
        let asked = host.save(&path, Some("Another project moved".into()), nobody());
        gate.wait_for_a_read();
        host.write(&other, model("z"));
        gate.hold(false);
        assert_eq!(answer(asked), Ok(true));
        assert_eq!(sandbox.messages()[0], "Another project moved");

        host.write(&path, model("b"));
        let before = sandbox.messages();
        gate.hold(true);
        let asked = host.save(&path, Some("This project moved".into()), nobody());
        gate.wait_for_a_read();
        host.write(&path, model("c"));
        gate.hold(false);
        assert_eq!(answer(asked), Err(SaveError::Changed));
        assert_eq!(sandbox.messages(), before, "no version of a state nobody reviewed");

        // The edit that stopped it is kept, and the next save carries it.
        assert_eq!(described(host, &path, "Reviewed again"), Ok(true));
        assert_eq!(sandbox.messages()[0], "Reviewed again");
    }

    /// The application's services wire the key check, so a description reaches the writer.
    #[test]
    fn the_native_services_give_the_host_its_key_check() {
        let sandbox = Sandbox::new(Some(Box::new(archforge_secrets::MemoryStore::new())));
        let host = Arc::new(Projects::new(&sandbox.base.join("wired"), "Archforge test".into()));
        let _services = crate::services::native(host.clone(), sandbox.agents.clone().unwrap());
        assert!(host.inner.keys.get().is_some(), "a description would be refused in the application");
    }

    #[test]
    fn a_project_whose_class_skill_overrides_a_project_skill_saves_and_opens_again() {
        // The project saved, but its model would not open again.
        let base = std::env::temp_dir().join(format!("af-skills-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let host = Projects::new(&base.join("settings"), "Archforge test".into());
        let path = wait(&host.create(&base.join("Documents/Archforge"), "MVPFlowReview")).unwrap().path;
        let mut n = 0u64;
        let ids = move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_800_000, r)
        };
        let mut s = archforge_notations::new_project_session("MVPFlowReview", Box::new(ids)).unwrap();
        let mut id = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap().to_string();
        id("skill.add", serde_json::json!({ "name": "Review export conventions", "body": "Everywhere." }));
        let package = id("uml.package.add", serde_json::json!({ "name": "Review domain" }));
        let invoice = id("uml.class.add", serde_json::json!({ "package": package, "name": "Invoice" }));
        let diagram = id("diagram.add", serde_json::json!({ "kind": "uml.class", "element": package, "name": "Review domain" }));
        id("diagram.place", serde_json::json!({ "diagram": diagram, "element": invoice, "x": 100, "y": 120 }));
        id("skill.add", serde_json::json!({ "subject": invoice, "name": "Review export conventions", "body": "Invoices only." }));
        let (tables, junctions) = archforge_notations::storage();
        let rows = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
        host.write(&path, rows.clone());
        assert_eq!(saved(&host, &path), Ok(true));
        close(&host, &path).unwrap();
        // Opened again, its rows make a model as the window makes it, with both skills and the class's
        // box on its diagram.
        let back = wait(&host.open(&path)).unwrap();
        assert_eq!(back.model.as_deref(), Some(&rows));
        let more = Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [7; 10]));
        let session = archforge_notations::session_from_rows(back.model.as_deref().unwrap(), more).expect("the model opens");
        let st = session.doc();
        let bodies: Vec<String> = archforge_core::skills::skills(st)
            .into_iter()
            .map(|k| st.get(k, archforge_core::skill::BODY).unwrap().as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(bodies, ["Everywhere.", "Invoices only."]);
        let d = st.by_uuid(archforge_mof::Uuid::parse(&diagram).unwrap()).unwrap();
        assert_eq!(st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), 1);
        close(&host, &path).unwrap();
        host.shutdown();
        remove_settled(&base);
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
        remove_settled(&base);
    }
}
