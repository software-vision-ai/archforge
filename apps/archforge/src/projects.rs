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
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::Uuid;
use archforge_mof::compare::CompareError;
use archforge_mof::rows::Rows;
use archforge_notations::differences::{ClassifyError, Difference};
use archforge_project::{
    Author, BranchCounts, BranchCreated, BranchCreationError, DESCRIPTION_CHARS, DeleteBranchError, DescriptionProblem, Hooks, ModelSchema, Project,
    RenameBranchError, SafeBranch, VersionReader, check_description,
};
use archforge_ui_egui::i18n::{fmt, tr};
use archforge_ui_egui::replace::{Job as ReplacementJob, JobAnswer, JobFailure};
use archforge_ui_egui::{
    BranchNotDeleted, BranchNotice, BranchSwitch, CreateError, Opened, ProjectHost, SaveError, Saved, SavedBy, VersionAfter, VersionEntry,
    VersionsPage, Writing,
};

/// How long the writer waits after a change for the next one before it writes.
const SETTLE: Duration = Duration::from_millis(300);
/// How long a closing, or the exit, waits for pending writes.
const FLUSH_BUDGET: Duration = Duration::from_secs(10);
/// The most a restore may take on the writer before its outcome counts as unclear.
const RESTORE_LIMIT: Duration = Duration::from_secs(5 * 60);
/// How often the wait for a restore looks whether Archforge is exiting.
const RESTORE_WAIT_STEP: Duration = Duration::from_millis(200);
/// The most reads of one project's versions that run at once.
const MOST_READS: usize = 4;
/// The most projects open at once, one per tab: each runs a database server.
const MAX_OPEN: usize = archforge_ui_egui::MAX_TABS;

/// The writer's work: a project's rows to write; a save (write what waits, then commit the
/// project named, recorded as who asked, under the description given or an automatic one, or only
/// write when no project is named) with its answer; or a flush of one project (write what waits, answer whether that
/// project's rows reached it).
///
/// The generation a job carries is its project's when the job was asked for, before any key check;
/// a job of another generation is refused rather than applied to a branch it was not made for.
enum Job {
    /// The rows, their generation, and the words to record if that generation has passed, put in
    /// the language shown while still on the interface thread.
    Write(PathBuf, Rows, u64, String),
    Save(Option<(PathBuf, SavedBy, u64)>, Option<Described>, Sender<Result<Saved, SaveError>>),
    Flush(PathBuf, Sender<Result<(), String>>),
    /// Forget what waits for a project without writing it (Close without writing), then answer.
    Drop(PathBuf, Sender<()>),
    /// Don't save: keep a project's conversations and put its model back to its last save, then
    /// forget what waits for it and answer. The flag is the claim: whoever sets it first, the writer
    /// to run the job or the closing that stopped waiting for it, decides, so a job given up on is
    /// never run late.
    Discard(PathBuf, Arc<AtomicBool>, Sender<Result<(), String>>),
    /// Work on a project's conversations: run against the project while it is open (none when it
    /// is not or its generation has passed), in order with every other job; it answers through its
    /// own channel.
    Chat(PathBuf, u64, ChatJob),
    /// Move a project to another branch; the generation is the project's when it was asked for.
    Switch(PathBuf, SafeBranch, (u64, u64), Arc<JobWatch>, Sender<Result<Switched, SwitchError>>),
    /// The first step of a switch: everything but the move. Generation and open instance as for
    /// [`Job::Switch`], whose watch keeps the switch under way until the writer is done with it.
    CheckSwitch(PathBuf, SafeBranch, (u64, u64), Sender<Result<CheckedBranch, SwitchError>>),
    /// Make a branch at a version, or from current work when none is named; the generation and the
    /// open instance are the project's when it was asked for.
    CreateBranch(PathBuf, SafeBranch, Option<String>, u64, u64, Sender<Result<BranchCreated, CreateBranchError>>),
    /// Tells a test whether the writer takes the last write to have left rows behind.
    #[cfg(test)]
    Peek(Sender<bool>),
    /// Delete a branch; the generation and the open instance are the project's when it was asked for.
    DeleteBranch(PathBuf, SafeBranch, u64, u64, Sender<Result<Vec<Uuid>, DeleteJobError>>),
    /// Rename a branch, from the name to the name; the generation and the open instance are the
    /// project's when it was asked for.
    RenameBranch(PathBuf, SafeBranch, SafeBranch, u64, u64, Sender<Result<(), RenameJobError>>),
    /// Restore a version of a project, the open instance and generation it was asked for with it;
    /// see [`JobWatch`] for what it shares with those waiting.
    Restore(PathBuf, RestoreRequest, u64, u64, Arc<JobWatch>, Sender<Result<Restored, RestoreError>>),
}

/// What a job on the writer shares with the wait for it and with its read-back; held in the mark
/// of a restore whose outcome is unknown, it also says which restore set that mark.
#[derive(Default)]
struct JobWatch {
    /// The claim, as for [`Job::Discard`]: whoever sets it first, the writer to run the job or the
    /// wait that gave up on it, decides, so a job given up on is never run late.
    claim: AtomicBool,
    /// The newest version just before the job's commit, and the branch the job ran on; none until
    /// the job has got that far.
    before_commit: Mutex<Option<(String, SafeBranch)>>,
    /// A switch only: the writer has finished the job, however it went.
    finished: AtomicBool,
    /// The read-back found this restore's version. Set and read under the lock on the marks, so
    /// that it and the writer's mark decide in either order.
    confirmed: AtomicBool,
}

/// A version to restore, and who asks.
pub struct RestoreRequest {
    /// The version's commit.
    pub commit: String,
    /// When the version was saved, RFC 3339 in UTC, for the new version's message.
    pub saved_at: String,
    pub by: SavedBy,
    /// What an opening does to a model, applied to both sides of the comparison.
    pub repair: fn(&mut archforge_mof::Store),
}

/// How a restore that did not fail went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// A new version holds the old model; `kept_work` says unsaved work was saved first.
    Version { kept_work: bool },
    /// Nothing differed from the version, so no version of it was made.
    Same { kept_work: bool },
}

/// Why a restore stopped.
#[derive(Debug)]
pub enum RestoreError {
    /// A switch moved the project after this restore was asked for.
    BranchChanged,
    /// The project was closed after this restore was asked for, though it may be open again.
    Reopened,
    NotOpen,
    /// What waits could not be written: the rows are kept for the next write.
    Unwritten(String),
    /// The version could not be read.
    Version(archforge_project::Error),
    /// It could not start: nothing changed.
    Failed(String),
    /// The unsaved work could not be kept as a version: no version was made and nothing changed.
    WorkNotKept(String),
    /// Whether the unsaved work was kept as a version cannot be told. The model did not change.
    WorkUnknown(String),
    /// The version was not restored and no version of it was made. `kept_work` says whether the
    /// unsaved work was kept as a version first. The work's rows were written back, or, when
    /// `written_back` is false, they wait to be written with the next change.
    NotRestored {
        why: String,
        kept_work: bool,
        written_back: bool,
    },
    /// Whether the version was committed cannot be told, so nothing was written over it. Further
    /// restores of this project are refused until it has been closed or this restore's read-back
    /// has found it finished.
    CannotTell {
        why: String,
        kept_work: bool,
    },
    /// An earlier restore of this project ended in [`RestoreError::CannotTell`].
    OutcomeUnknown,
}

impl std::fmt::Display for RestoreError {
    /// The reason in English, as the project's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestoreError::BranchChanged => f.write_str("a switch moved the project first"),
            RestoreError::Reopened => f.write_str("the project was closed after this was asked for"),
            RestoreError::NotOpen => f.write_str("the project is no longer open"),
            RestoreError::Unwritten(why) | RestoreError::Failed(why) => f.write_str(why),
            RestoreError::WorkNotKept(why) | RestoreError::WorkUnknown(why) => f.write_str(why),
            RestoreError::NotRestored { why, kept_work, written_back } => {
                let waits = if *written_back { "" } else { "; the work's rows wait to be written" };
                write!(f, "{why}{}{waits}", kept_note(*kept_work))
            }
            RestoreError::CannotTell { why, kept_work } => write!(f, "{why}{}", kept_note(*kept_work)),
            RestoreError::OutcomeUnknown => f.write_str("whether an earlier restore finished could not be told"),
            RestoreError::Version(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for RestoreError {}

/// What a failed restore's reason adds about the version of the unsaved work.
fn kept_note(kept_work: bool) -> &'static str {
    if kept_work { "; the unsaved work was kept as a version" } else { "" }
}

/// How a switch of branch went.
#[derive(Debug, Clone, PartialEq)]
pub enum Switched {
    /// The project is on the branch now, whose model this is.
    Moved(Arc<Rows>),
    /// It was on that branch already, under this or another case: nothing moved.
    AlreadyThere,
}

/// What the first step of a switch found on the target branch.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedBranch {
    pub rows: Arc<Rows>,
    /// Whether the branch holds work not saved as a version.
    pub unsaved: bool,
}

/// The two steps of a switch: the check, already asked of the writer, and the move.
pub struct SwitchSteps {
    pub check: Receiver<Result<CheckedBranch, SwitchError>>,
    pub moving: SwitchMove,
}

/// The second step of a switch, to run once the window has built the branch's model.
pub struct SwitchMove {
    inner: Arc<Inner>,
    path: PathBuf,
    target: SafeBranch,
    asked: (u64, u64),
    under_way: Arc<JobWatch>,
    limit: Duration,
}

impl SwitchMove {
    /// The move as the window runs it: it hands the move to the writer within the limit and reads
    /// back an answer that did not come.
    pub fn into_job(self) -> ReplacementJob {
        let SwitchMove { inner, path, target, asked, under_way, limit } = self;
        let run = {
            let (inner, path, target, under_way) = (inner.clone(), path.clone(), target.clone(), under_way.clone());
            move || start_move(inner, path, target, asked, under_way, limit)
        };
        let read_back = move || Projects { inner }.read_back_switch(&path, target, under_way);
        ReplacementJob { run: Box::new(run), read_back: Box::new(read_back) }
    }

    /// Hands the move to the writer and returns at once, its answer with its exact reason.
    #[cfg(test)]
    pub fn run(self) -> Receiver<Result<Switched, SwitchError>> {
        let (tx, rx) = channel();
        if let Err(e) = writer(&self.inner).send(Job::Switch(self.path, self.target, self.asked, self.under_way, tx.clone())) {
            let _ = tx.send(Err(SwitchError::Failed(format!("the writer stopped: {e}"))));
        }
        rx
    }
}

/// Why a project did not move to another branch: it stays where it was, with its work.
#[derive(Debug)]
pub enum SwitchError {
    /// Another switch moved the project after this one was asked for.
    BranchChanged,
    /// The project was closed after this was asked for, though it may be open again.
    Reopened,
    /// The project is on that branch already, under this or another case.
    AlreadyOnIt,
    /// Another switch of this project is under way.
    UnderWay,
    /// The step gave no answer in time; a move that had not started never will.
    NoAnswer,
    NotOpen,
    /// What waits could not be written: the rows are kept for the next write.
    Unwritten(String),
    /// The target branch did not pass what an opening checks.
    Checks(archforge_project::Error),
    /// The model's tables could not be described, or the writer has stopped.
    Failed(String),
}

impl std::fmt::Display for SwitchError {
    /// The reason in English, as the project's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchError::BranchChanged => f.write_str("another switch moved the project first"),
            SwitchError::Reopened => f.write_str("the project was closed after this was asked for"),
            SwitchError::AlreadyOnIt => f.write_str("the project is on that branch already"),
            SwitchError::UnderWay => f.write_str("another switch of this project is under way"),
            SwitchError::NoAnswer => f.write_str("the switch gave no answer in time"),
            SwitchError::NotOpen => f.write_str("the project is no longer open"),
            SwitchError::Unwritten(why) | SwitchError::Failed(why) => f.write_str(why),
            SwitchError::Checks(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SwitchError {}

impl SwitchError {
    /// What a person is told. The project stayed on its branch in every case, and nothing more is
    /// claimed, as the check may have upgraded the target's tables.
    pub fn words(&self) -> String {
        match self {
            SwitchError::AlreadyOnIt => tr("You are already on this branch."),
            SwitchError::BranchChanged | SwitchError::Reopened | SwitchError::UnderWay | SwitchError::NotOpen | SwitchError::NoAnswer => {
                tr("The project stayed on its branch because it changed or was busy.")
            }
            SwitchError::Unwritten(_) => tr("Your latest changes could not be written, so the project stayed on its branch."),
            SwitchError::Checks(e) => fmt(&tr("The project stayed on its branch: {why}"), &[("why", &e.to_string())]),
            SwitchError::Failed(why) => fmt(&tr("The project stayed on its branch: {why}"), &[("why", why)]),
        }
    }
}

/// Why the writer made no branch.
#[derive(Debug, PartialEq, Eq)]
pub enum CreateBranchError {
    /// A switch moved the project after this was asked for.
    BranchChanged,
    /// The project was closed after this was asked for, though it may be open again.
    Reopened,
    NotOpen,
    /// What waits could not be written: the rows are kept for the next write.
    Unwritten(String),
    /// The project's own refusal or failure.
    Creation(BranchCreationError),
    /// The model's tables could not be described, or the writer has stopped.
    Failed(String),
}

impl std::fmt::Display for CreateBranchError {
    /// The reason in English, as the project's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreateBranchError::BranchChanged => f.write_str("another switch moved the project first"),
            CreateBranchError::Reopened => f.write_str("the project was closed after this was asked for"),
            CreateBranchError::NotOpen => f.write_str("the project is no longer open"),
            CreateBranchError::Creation(e) => e.fmt(f),
            CreateBranchError::Unwritten(why) | CreateBranchError::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for CreateBranchError {}

/// Why the writer deleted no branch. Its `Display` text is English, for logs only: the screen
/// (AR-292) words each variant through `tr` and never shows `to_string()`.
#[derive(Debug, PartialEq, Eq)]
pub enum DeleteJobError {
    /// A switch moved the project after this was asked for.
    BranchChanged,
    /// The project was closed after this was asked for, though it may be open again.
    Reopened,
    NotOpen,
    /// The project's own refusal, failure or uncertainty.
    Delete(DeleteBranchError),
    /// The writer has stopped.
    Failed(String),
}

impl std::fmt::Display for DeleteJobError {
    /// The reason in English, as the project's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeleteJobError::BranchChanged => f.write_str("another switch moved the project first"),
            DeleteJobError::Reopened => f.write_str("the project was closed after this was asked for"),
            DeleteJobError::NotOpen => f.write_str("the project is no longer open"),
            DeleteJobError::Delete(e) => e.fmt(f),
            DeleteJobError::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for DeleteJobError {}

/// Why the writer renamed no branch.
#[derive(Debug, PartialEq, Eq)]
pub enum RenameJobError {
    /// A switch moved the project after this was asked for.
    BranchChanged,
    /// The project was closed after this was asked for, though it may be open again.
    Reopened,
    NotOpen,
    /// The project's own refusal or failure.
    Rename(RenameBranchError),
    /// The writer has stopped.
    Failed(String),
}

impl std::fmt::Display for RenameJobError {
    /// The reason in English, as the project's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenameJobError::BranchChanged => f.write_str("another switch moved the project first"),
            RenameJobError::Reopened => f.write_str("the project was closed after this was asked for"),
            RenameJobError::NotOpen => f.write_str("the project is no longer open"),
            RenameJobError::Rename(e) => e.fmt(f),
            RenameJobError::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for RenameJobError {}

/// Why a job on the conversations has no project to run against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoProject {
    NotOpen,
    /// The project's branch changed after the job was asked for.
    OtherBranch,
}

/// A save's description, and how many sets of rows its project had been handed when the save was
/// asked: its key check runs off the writer, so rows handed over meanwhile can reach the writer
/// first, and they are not the state being saved.
struct Described {
    text: String,
    handed: u64,
}

/// A job on a project's conversations, run on the writer thread.
type ChatJob = Box<dyn FnOnce(Result<&Project, NoProject>) + Send>;

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
    /// Each project's switch generation: absent is 0, and nothing has moved it yet.
    generations: Mutex<BTreeMap<PathBuf, u64>>,
    /// The switch asked for on each project, under way for as long as anything still holds it.
    switches: Mutex<BTreeMap<PathBuf, std::sync::Weak<JobWatch>>>,
    /// The projects a restore of which ended with its outcome unknown, each with that restore,
    /// until they are closed or that restore's read-back finds it finished.
    restores_unknown: Mutex<BTreeMap<PathBuf, Arc<JobWatch>>>,
    /// How many times each project has closed, as the instance a job names: absent is 0.
    instances: Mutex<BTreeMap<PathBuf, u64>>,
    /// Runs in both steps of a switch, once the target is checked, for a test to act just before
    /// the move or its first step's end; and in a switch's read-back between its two reads.
    #[cfg(test)]
    before_move: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// What is to go wrong in a restore, for a test: each fault is used once.
    #[cfg(test)]
    restore_faults: Mutex<Vec<Fault>>,
    /// Makes committing the waiting conversations fail, for a test.
    #[cfg(test)]
    fail_chats: AtomicBool,
    /// Makes the next description of the model's tables fail, once.
    #[cfg(test)]
    fail_schema: AtomicBool,
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
                generations: Mutex::new(BTreeMap::new()),
                #[cfg(test)]
                before_move: Mutex::new(None),
                #[cfg(test)]
                restore_faults: Mutex::new(Vec::new()),
                restores_unknown: Mutex::new(BTreeMap::new()),
                switches: Mutex::new(BTreeMap::new()),
                instances: Mutex::new(BTreeMap::new()),
                #[cfg(test)]
                fail_chats: AtomicBool::new(false),
                #[cfg(test)]
                fail_schema: AtomicBool::new(false),
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

    /// Starts moving `project` to `target` on the writer, and returns at once; see [`switch`]. The
    /// window calls this and the read-back below once it has its hold (AR-289).
    #[cfg(test)]
    pub fn switch_branch(&self, project: &Path, target: SafeBranch) -> Receiver<Result<Switched, SwitchError>> {
        let (tx, rx) = channel();
        let asked = (generation_of(&self.inner, project), instance_of(&self.inner, project));
        if let Err(e) = writer(&self.inner).send(Job::Switch(project.to_path_buf(), target, asked, Arc::new(JobWatch::default()), tx.clone())) {
            let _ = tx.send(Err(SwitchError::Failed(format!("the writer stopped: {e}"))));
        }
        rx
    }

    /// Starts making the branch `name` at the version `start` on the writer, and returns at once;
    /// see [`create_branch`]. The screen that asks for this comes with AR-292.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn create_branch(&self, project: &Path, name: SafeBranch, start: String) -> Receiver<Result<BranchCreated, CreateBranchError>> {
        self.ask_for_branch(project, name, Some(start))
    }

    /// Starts making the branch `name` from current work on the writer, and returns at once. The
    /// screen that asks for this comes with AR-292.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn create_branch_from_work(&self, project: &Path, name: SafeBranch) -> Receiver<Result<BranchCreated, CreateBranchError>> {
        self.ask_for_branch(project, name, None)
    }

    /// Reads, off the writer, what deleting `branch` would take with it, for its confirmation. An
    /// answer from before a switch is dropped, as any read of the versions is. The screen that asks
    /// for this comes with AR-292.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn branch_counts(&self, project: &Path, branch: SafeBranch) -> Receiver<Result<BranchCounts, String>> {
        self.on_a_reader(project, move |reader| reader.branch_counts(&branch).map_err(|e| e.to_string()))
    }

    /// Starts deleting `branch` on the writer, and returns at once; see [`Project::delete_branch`].
    /// The answer is the conversations whose folders can go. The screen comes with AR-292.
    pub fn delete_branch(&self, project: &Path, branch: SafeBranch) -> Receiver<Result<Vec<Uuid>, DeleteJobError>> {
        let (tx, rx) = channel();
        let asked = (generation_of(&self.inner, project), instance_of(&self.inner, project));
        if let Err(e) = writer(&self.inner).send(Job::DeleteBranch(project.to_path_buf(), branch, asked.0, asked.1, tx.clone())) {
            let _ = tx.send(Err(DeleteJobError::Failed(format!("the writer stopped: {e}"))));
        }
        rx
    }

    /// Starts renaming the branch `from` to `to` on the writer, and returns at once; see
    /// [`Project::rename_branch`]. The screen that asks for this comes with AR-292.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn rename_branch(&self, project: &Path, from: SafeBranch, to: SafeBranch) -> Receiver<Result<(), RenameJobError>> {
        let (tx, rx) = channel();
        let asked = (generation_of(&self.inner, project), instance_of(&self.inner, project));
        if let Err(e) = writer(&self.inner).send(Job::RenameBranch(project.to_path_buf(), from, to, asked.0, asked.1, tx.clone())) {
            let _ = tx.send(Err(RenameJobError::Failed(format!("the writer stopped: {e}"))));
        }
        rx
    }

    /// Hands the making of a branch to the writer, under the project's generation now.
    fn ask_for_branch(&self, project: &Path, name: SafeBranch, start: Option<String>) -> Receiver<Result<BranchCreated, CreateBranchError>> {
        let (tx, rx) = channel();
        let generation = generation_of(&self.inner, project);
        let instance = instance_of(&self.inner, project);
        if let Err(e) = writer(&self.inner).send(Job::CreateBranch(project.to_path_buf(), name, start, generation, instance, tx.clone())) {
            let _ = tx.send(Err(CreateBranchError::Failed(format!("the writer stopped: {e}"))));
        }
        rx
    }

    /// Asks for a switch of `project` to `target` in two steps, each within the limit of a job on
    /// the writer: the check goes to the writer now, and the move runs once the window has built
    /// the branch's model. See [`Projects::start_switch_within`].
    pub fn start_switch(&self, project: &Path, target: SafeBranch) -> Result<SwitchSteps, SwitchError> {
        self.start_switch_within(project, target, RESTORE_LIMIT)
    }

    /// [`Projects::start_switch`] within `limit` for each step. Refused while another switch of the
    /// project is under way, which holds from here until its move has ended or been dropped.
    fn start_switch_within(&self, project: &Path, target: SafeBranch, limit: Duration) -> Result<SwitchSteps, SwitchError> {
        let under_way = Arc::new(JobWatch::default());
        {
            let mut switches = lock(&self.inner.switches);
            if switches.get(project).is_some_and(|held| held.strong_count() > 0) {
                return Err(SwitchError::UnderWay);
            }
            switches.insert(project.to_path_buf(), Arc::downgrade(&under_way));
        }
        let asked = (generation_of(&self.inner, project), instance_of(&self.inner, project));
        let check = ask_for_check(self.inner.clone(), project.to_path_buf(), target.clone(), asked, limit);
        let moving = SwitchMove { inner: self.inner.clone(), path: project.to_path_buf(), target, asked, under_way, limit };
        Ok(SwitchSteps { check, moving })
    }

    /// Reads back, off the writer, whether a switch moved the project. It cannot be told until the
    /// writer has finished the job, as one still running may yet move it; whether it has is read
    /// before the branch, so that the branch is the one the job left.
    fn read_back_switch(&self, project: &Path, target: SafeBranch, watch: Arc<JobWatch>) -> Receiver<Result<bool, String>> {
        #[cfg(test)]
        let inner = self.inner.clone();
        self.on_a_reader(project, move |reader| {
            if !watch.finished.load(Ordering::SeqCst) {
                return Err("whether the switch finished could not be told".to_string());
            }
            #[cfg(test)]
            if let Some(step) = lock(&inner.before_move).take() {
                step();
            }
            let name = reader.active_branch().map_err(|e| e.to_string())?;
            Ok(SafeBranch::found(&name).map_err(|e| e.to_string())?.same_branch(&target))
        })
    }

    /// Reads back, off the writer, whether `project` is on `target`, for when a switch's answer was
    /// lost; an error says it cannot be told, and no answer at all means the same.
    #[cfg(test)]
    pub fn read_back_branch(&self, project: &Path, target: SafeBranch) -> Receiver<Result<bool, String>> {
        self.on_a_reader(project, move |reader| {
            let name = reader.active_branch().map_err(|e| e.to_string())?;
            Ok(SafeBranch::found(&name).map_err(|e| e.to_string())?.same_branch(&target))
        })
    }

    /// Starts restoring a version of `project` on the writer, and returns at once; see [`restore`].
    #[cfg(test)]
    pub fn restore_version(&self, project: &Path, request: RestoreRequest) -> Receiver<Result<Restored, RestoreError>> {
        let (tx, rx) = channel();
        let generation = generation_of(&self.inner, project);
        let (instance, watch) = (instance_of(&self.inner, project), Arc::new(JobWatch::default()));
        if let Err(e) = writer(&self.inner).send(Job::Restore(project.to_path_buf(), request, generation, instance, watch, tx.clone())) {
            let _ = tx.send(Err(RestoreError::Failed(format!("the writer stopped: {e}"))));
        }
        rx
    }

    /// The project's switch generation now, to take when a job is asked for.
    pub fn generation_of(&self, path: &Path) -> u64 {
        generation_of(&self.inner, path)
    }

    /// Runs `job` on the writer against the open project at `path`, or says why there is none, after
    /// every job sent before it, and answers with what it gives; nothing when the writer stopped.
    /// `generation` is the project's when the job was asked for.
    pub fn on_writer_open<T: Send + 'static>(
        &self,
        path: &Path,
        generation: u64,
        job: impl FnOnce(Result<&Project, NoProject>) -> T + Send + 'static,
    ) -> Receiver<T> {
        let (tx, rx) = channel();
        let run: ChatJob = Box::new(move |project| {
            let _ = tx.send(job(project));
        });
        if let Err(e) = writer(&self.inner).send(Job::Chat(path.to_path_buf(), generation, run)) {
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

    /// The job that restores a version within `limit`, the project's open instance and generation
    /// now, before the version is read.
    fn restore_job_within(&self, project: &Path, request: RestoreRequest, limit: Duration) -> ReplacementJob {
        let asked = (generation_of(&self.inner, project), instance_of(&self.inner, project));
        let message = restored_message(&request.saved_at);
        let (inner, path) = (self.inner.clone(), project.to_path_buf());
        let watch = Arc::new(JobWatch::default());
        let run = {
            let (inner, path, watch) = (inner.clone(), path.clone(), watch.clone());
            move || start_restore(inner, path, request, asked, watch, limit)
        };
        let read_back = move || Projects { inner }.read_back_restore(&path, message, asked, watch);
        ReplacementJob { run: Box::new(run), read_back: Box::new(read_back) }
    }

    /// Reads back, off the writer, whether this restore finished: the job got to its commit and the
    /// newest version is a new one, not the one before that commit, carrying `message`. The message
    /// alone would not do: an earlier restore of a version saved at the same time has the same text
    /// and may still be the newest. Anything else cannot be told, never that it did not finish, as
    /// a commit whose reply was lost may still be running; so too once the project has closed or
    /// its branch moved, the switch's own first step included. A restore found finished clears its own mark, never another restore's.
    fn read_back_restore(&self, project: &Path, message: String, asked: (u64, u64), watch: Arc<JobWatch>) -> Receiver<Result<bool, String>> {
        let (inner, path) = (self.inner.clone(), project.to_path_buf());
        self.on_a_reader(project, move |reader| {
            #[cfg(test)]
            hold_reading(&inner, ReadingAt::Start);
            // The baseline first: read after the page, it could be newer than the page, which
            // would then look like a new version though nothing has happened since.
            let before = lock(&watch.before_commit).clone();
            #[cfg(test)]
            hold_reading(&inner, ReadingAt::Baseline);
            let page = reader.versions(None, 1).map_err(|e| e.to_string())?;
            let newest = page.versions.first();
            let made =
                newest.is_some_and(|v| before.as_ref().is_some_and(|(b, _)| *b != v.hash) && v.description.as_deref() == Some(message.as_str()));
            #[cfg(test)]
            hold_reading(&inner, ReadingAt::Page);
            // After the page: a switch shows its target before its generation moves, so a page
            // read from the target is found here; one read before the switch is refused as well.
            let same_branch = reader.active_branch().ok().and_then(|name| SafeBranch::found(&name).ok());
            let same_branch = before.as_ref().is_some_and(|(_, ran_on)| same_branch.is_some_and(|now| now.same_branch(ran_on)));
            // Under the writer's lock, so that this and its mark decide in either order.
            let mut unknown = lock(&inner.restores_unknown);
            if !made || !same_branch || asked != (generation_of(&inner, &path), instance_of(&inner, &path)) {
                return Err("whether the restore finished could not be told".to_string());
            }
            watch.confirmed.store(true, Ordering::SeqCst);
            if unknown.get(&path).is_some_and(|mark| Arc::ptr_eq(mark, &watch)) {
                unknown.remove(&path);
            }
            Ok(true)
        })
    }

    /// Runs `read` with the project's reader on a thread of its own and gives its answer. Only the
    /// short lock on the readers is taken here, so the read waits for no write and no write waits
    /// for it. Refused past [`MOST_READS`] reads under way, and for a project no longer open: its
    /// reader has left the map, so a later read cannot reach it. An answer read across a change of
    /// the project's branch is dropped: the receiver sees no answer rather than another branch's.
    fn on_a_reader<T: Send + 'static>(
        &self,
        project: &Path,
        read: impl FnOnce(&VersionReader) -> Result<T, String> + Send + 'static,
    ) -> Receiver<Result<T, String>> {
        let (tx, rx) = channel();
        let generation = generation_of(&self.inner, project);
        let (inner, path) = (self.inner.clone(), project.to_path_buf());
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
            if generation_of(&inner, &path) == generation {
                let _ = answer.send(result);
            }
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
    /// The check's answer goes to the window from a thread of its own, worded in the language shown
    /// when the switch was asked for; the unsaved mark is set before the model goes.
    fn begin_branch_switch(&self, project: &Path, branch: String) -> Result<BranchSwitch, String> {
        let target = SafeBranch::found(&branch).map_err(|e| SwitchError::Failed(e.to_string()).words())?;
        let SwitchSteps { check, moving } = self.start_switch(project, target).map_err(|e| e.words())?;
        let language = archforge_ui_egui::i18n::language();
        let unsaved = Arc::new(AtomicBool::new(false));
        let (send, read) = channel();
        let marked = unsaved.clone();
        let forward = move || {
            archforge_ui_egui::i18n::set_language(language);
            let _ = send.send(match check.recv().unwrap_or(Err(SwitchError::NoAnswer)) {
                Ok(checked) => {
                    marked.store(checked.unsaved, Ordering::SeqCst);
                    Ok(checked.rows)
                }
                Err(error) => {
                    log::warn!("a branch check stopped: {error}");
                    Err(error.words())
                }
            });
        };
        spawn_wait(&self.inner, forward).map_err(|e| SwitchError::Failed(format!("the switch could not start: {e}")).words())?;
        Ok(BranchSwitch { read, unsaved, job: moving.into_job() })
    }

    /// Worded on the wait's own thread in the language shown when the job runs.
    fn restore_job(&self, project: &Path, commit: String, saved_at: String, by: SavedBy, repair: fn(&mut archforge_mof::Store)) -> ReplacementJob {
        self.restore_job_within(project, RestoreRequest { commit, saved_at, by, repair }, RESTORE_LIMIT)
    }

    /// The writer's delete, its answer put as the window takes it, on a thread of its own as only
    /// the writer can answer it.
    fn delete_branch(&self, path: &Path, name: &str) -> Receiver<Result<Vec<Uuid>, BranchNotDeleted>> {
        let (tx, rx) = channel();
        let job = match SafeBranch::found(name) {
            Ok(branch) => Projects::delete_branch(self, path, branch),
            Err(e) => {
                let _ = tx.send(Err(BranchNotDeleted::Failed(e.to_string())));
                return rx;
            }
        };
        let reply = tx.clone();
        let wait = move || {
            let answer = job.recv().unwrap_or_else(|e| Err(DeleteJobError::Failed(format!("the writer stopped: {e}"))));
            let _ = reply.send(answer.map_err(not_deleted));
        };
        if let Err(e) = std::thread::Builder::new().name("archforge-branch-delete".into()).spawn(wait) {
            let _ = tx.send(Err(BranchNotDeleted::Failed(format!("no thread could wait for the answer: {e}"))));
        }
        rx
    }

    /// Opens the project on `main`, with no branch stored for it.
    fn open(&self, path: &Path) -> Receiver<Result<Opened, CreateError>> {
        self.open_on(path, None)
    }

    /// Opens the project on its own thread, on the stored `branch` when that can still be opened;
    /// otherwise on `main`, and what is opened carries the reason as a notice.
    fn open_on(&self, path: &Path, branch: Option<&str>) -> Receiver<Result<Opened, CreateError>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let path = path.to_path_buf();
        let stored = branch.map(str::to_string);
        let reply = tx.clone();
        match std::thread::Builder::new().name("archforge-open".into()).spawn(move || open_job(&inner, &path, stored.as_deref(), &reply)) {
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
        let stale = tr("A change arrived after the branch changed, so it was not written.");
        let sent = writer(&self.inner).send(Job::Write(project.to_path_buf(), rows, generation_of(&self.inner, project), stale));
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
        let generation = generation_of(&self.inner, project);
        match description {
            None => send_save(&self.inner, project, by, generation, None, tx),
            Some(text) => match check_description(&text) {
                Ok(()) => save_described(&self.inner, project, by, generation, text, tx),
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
fn open_job(inner: &Inner, path: &Path, stored: Option<&str>, reply: &Sender<Result<Opened, CreateError>>) {
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
            Ok(Opened {
                name: project.settings().name.clone(),
                path: project.path().to_path_buf(),
                model: Some(Arc::new(rows)),
                unsaved,
                notice: None,
            })
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
        // Provisional, for recovery alone: told which branch it may open on, so that recovery keeps
        // it, before anything lists or reads branches. `open_on_stored` below settles the branch.
        // A record naming a stored branch that has gone is logged as protected, not as no branch;
        // the record is removed all the same.
        if let Some(wanted) = stored.and_then(|name| SafeBranch::found(name).ok()) {
            project.set_current_branch(wanted);
        }
        project.recover_creation();
        let read = schema(inner).and_then(|s| {
            let notice = stored.and_then(|name| project.open_on_stored(name, &s).err().map(|e| branch_notice(name, e)));
            project.prepare_model(&s).map_err(kind)?;
            let rows = project.read_model(&s).map_err(kind)?;
            // Damaged conversations never keep the model from opening: the chat panel says why.
            if let Err(e) = project.open_chats(&now()) {
                log::warn!("the conversations of {} were not prepared: {e}", project.path().display());
            }
            let unsaved = project.has_unsaved().map_err(kind)?;
            Ok((rows, unsaved, notice))
        });
        match read {
            Ok(read) => Ok((project, read)),
            Err(e) => {
                let _ = project.close();
                Err(e)
            }
        }
    });
    let answer = match result {
        Ok((project, _)) if inner.cancel.load(Ordering::SeqCst) => {
            let _ = project.close();
            Err(CreateError::Failed { detail: "Archforge is quitting".into() })
        }
        Ok((project, (rows, unsaved, notice))) => {
            let opened =
                Opened { name: project.settings().name.clone(), path: project.path().to_path_buf(), model: Some(Arc::new(rows)), unsaved, notice };
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

/// The window's words for why the branch `stored` did not open. A name made in Archforge is shown
/// as it was typed.
fn branch_notice(stored: &str, why: archforge_project::StoredBranch) -> BranchNotice {
    let name = SafeBranch::found(stored).map_or_else(|_| stored.to_string(), |b| b.shown());
    match why {
        archforge_project::StoredBranch::Gone => BranchNotice::Gone { name },
        archforge_project::StoredBranch::CannotOpen(why) => BranchNotice::CannotOpen { name, why },
    }
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
    lock(&inner.restores_unknown).remove(path);
    *lock(&inner.instances).entry(path.to_path_buf()).or_default() += 1;
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
    lock(&inner.restores_unknown).remove(path);
    *lock(&inner.instances).entry(path.to_path_buf()).or_default() += 1;
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
    #[cfg(test)]
    if inner.fail_schema.swap(false, Ordering::SeqCst) {
        return Err(CreateError::Failed { detail: "a test refused".into() });
    }
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
    // Whether the last write left rows behind, which then wait for the next change or Save. It
    // changes only when a write was tried, or new rows come in, which any write needs first: so
    // forgetting one project's rows says nothing of another's, and a value left over with nothing
    // waiting is cleared before it can matter.
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
            Some(Job::Write(project, rows, generation, stale)) => {
                *seen.entry(project.clone()).or_default() += 1;
                if generation != generation_of(inner, &project) {
                    log::warn!("changes for {} made before its branch changed are dropped", project.display());
                    let pending = waiting.contains_key(&project);
                    let mut states = lock(&inner.writing);
                    let state = states.entry(project).or_default();
                    (state.pending, state.error) = (pending, Some(stale));
                    continue;
                }
                waiting.insert(project, rows);
                failed = false;
            }
            None => {
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
            }
            Some(Job::Save(project, described, reply)) => {
                if let Some((p, _, generation)) = &project
                    && *generation != generation_of(inner, p)
                {
                    let _ = reply.send(Err(SaveError::BranchChanged));
                    continue;
                }
                // Before anything is written: the rows that came in stay waiting, as any others do.
                if let (Some((p, ..)), Some(d)) = (&project, &described)
                    && seen.get(p).copied().unwrap_or(0) != d.handed
                {
                    let _ = reply.send(Err(SaveError::Changed));
                    continue;
                }
                write_waiting(inner, &mut waiting);
                failed = !waiting.is_empty();
                let answer = match project {
                    // A save needs its own project's rows written; a flush needs all.
                    Some((p, ..)) if waiting.contains_key(&p) => Err(SaveError::Failed(unwritten(inner, &p))),
                    Some((p, by, _)) => commit(inner, &p, described.map(|d| d.text), &by),
                    None => match waiting.keys().next() {
                        Some(p) => Err(SaveError::Failed(unwritten(inner, p))),
                        None => Ok(Saved::NothingNew),
                    },
                };
                let _ = reply.send(answer);
            }
            #[cfg(test)]
            Some(Job::Peek(reply)) => {
                let _ = reply.send(failed);
            }
            Some(Job::Drop(project, reply)) => {
                waiting.remove(&project);
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
                }
                let _ = reply.send(answer);
            }
            Some(Job::Chat(project, generation, job)) => {
                let open = lock(&inner.open);
                let moved = generation != generation_of(inner, &project);
                // A failing job never stops the writer: the model's rows still get written.
                let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    job(if moved { Err(NoProject::OtherBranch) } else { open.iter().find(|p| p.path() == project).ok_or(NoProject::NotOpen) })
                }));
                if ran.is_err() {
                    log::error!("a job on the conversations of {} failed", project.display());
                }
            }
            Some(Job::CheckSwitch(project, target, asked, reply)) => {
                let _ = reply.send(check_switch(inner, &project, &target, asked, &mut waiting, &mut failed));
            }
            Some(Job::Switch(project, target, asked, under_way, reply)) => {
                if under_way.claim.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                    // The wait gave up on it before it started: it must not run late.
                    continue;
                }
                let answer = switch(inner, &project, target, asked, &mut waiting, &mut failed);
                under_way.finished.store(true, Ordering::SeqCst);
                // Released before the answer, so whoever has the answer can ask again at once.
                drop(under_way);
                let _ = reply.send(answer);
            }
            Some(Job::CreateBranch(project, name, start, generation, instance, reply)) => {
                let _ = reply.send(create_branch(inner, &project, &name, start.as_deref(), generation, instance, &mut waiting, &mut failed));
            }
            Some(Job::DeleteBranch(project, branch, generation, instance, reply)) => {
                let _ = reply.send(delete_branch(inner, &project, &branch, generation, instance));
            }
            Some(Job::RenameBranch(project, from, to, generation, instance, reply)) => {
                let _ = reply.send(rename_branch(inner, &project, &from, &to, generation, instance));
            }
            Some(Job::Restore(project, request, generation, instance, watch, reply)) => {
                if watch.claim.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                    // The wait gave up on it before it started: it must not run late.
                    continue;
                }
                let answer = restore(inner, &project, request, (generation, instance), &watch, &mut waiting, &mut failed);
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
        #[cfg(test)]
        let refused = take_fault(inner, |f| matches!(f, Fault::WriteBack)).is_some();
        #[cfg(not(test))]
        let refused = false;
        let result = if refused {
            Err("a test refused".to_string())
        } else {
            schema(inner).map_err(|e| format!("{e:?}")).and_then(|s| project.write_model(&s, rows).map_err(|e| e.to_string()))
        };
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

/// Moves `path` to `target`, on the writer, after everything [`checked_target`] does. Only then do
/// the connections move, the change ledger start again and, last, the generation move: a job
/// asked for before that last step has the old generation and is refused when the writer reaches
/// it. A branch the project is on already, in any case, moves nothing.
fn switch(
    inner: &Inner,
    path: &Path,
    target: SafeBranch,
    asked: (u64, u64),
    waiting: &mut BTreeMap<PathBuf, Rows>,
    failed: &mut bool,
) -> Result<Switched, SwitchError> {
    let moved = checked_target(inner, path, &target.clone(), asked, waiting, failed, |project, rows| {
        project.move_to_branch(target);
        *lock(&inner.generations).entry(path.to_path_buf()).or_default() += 1;
        Ok(Switched::Moved(Arc::new(rows)))
    });
    if matches!(moved, Err(SwitchError::AlreadyOnIt)) { Ok(Switched::AlreadyThere) } else { moved }
}

/// The first step of a switch, on the writer: what [`checked_target`] does, and no move. The
/// target's checks may upgrade its tables, which stays: the project stays on the branch it was on.
fn check_switch(
    inner: &Inner,
    path: &Path,
    target: &SafeBranch,
    asked: (u64, u64),
    waiting: &mut BTreeMap<PathBuf, Rows>,
    failed: &mut bool,
) -> Result<CheckedBranch, SwitchError> {
    checked_target(inner, path, target, asked, waiting, failed, |project, rows| {
        let unsaved = project.branch_has_unsaved(target).map_err(SwitchError::Checks)?;
        Ok(CheckedBranch { rows: Arc::new(rows), unsaved })
    })
}

/// What both steps of a switch do, in this order, on the writer: a job from before a close or
/// another switch is refused, and so is the branch the project is on, before anything is written;
/// what waits is written; the waiting conversations are committed, which does not stop the switch
/// when it fails, as their rows stay uncommitted on the branch left; the target is checked and its
/// model built; and `then` runs on the result. `failed` changes only once the write has been tried.
fn checked_target<T>(
    inner: &Inner,
    path: &Path,
    target: &SafeBranch,
    (generation, instance): (u64, u64),
    waiting: &mut BTreeMap<PathBuf, Rows>,
    failed: &mut bool,
    then: impl FnOnce(&Project, Rows) -> Result<T, SwitchError>,
) -> Result<T, SwitchError> {
    if instance != instance_of(inner, path) {
        return Err(SwitchError::Reopened);
    }
    if generation != generation_of(inner, path) {
        return Err(SwitchError::BranchChanged);
    }
    if lock(&inner.open).iter().find(|p| p.path() == path).ok_or(SwitchError::NotOpen)?.current_branch().same_branch(target) {
        return Err(SwitchError::AlreadyOnIt);
    }
    let schema = schema(inner).map_err(|e| SwitchError::Failed(format!("{e:?}")))?;
    write_waiting(inner, waiting);
    *failed = !waiting.is_empty();
    if waiting.contains_key(path) {
        return Err(SwitchError::Unwritten(unwritten(inner, path)));
    }
    let (open, at) = open_as_asked(inner, path, (generation, instance), (SwitchError::Reopened, SwitchError::BranchChanged, SwitchError::NotOpen))?;
    let project = &open[at];
    if let Err(e) = keep_conversations(inner, project) {
        log::warn!("the conversations of {} wait in their branch: {e}", path.display());
    }
    let rows = project.check_branch(target, &schema).map_err(SwitchError::Checks)?;
    #[cfg(test)]
    if let Some(step) = lock(&inner.before_move).take() {
        step();
    }
    then(project, rows)
}

/// Whether what `classified` found means the restore goes on. A comparison too large to finish
/// goes on, as the commit still finds out when nothing differs, at the cost of a possibly needless
/// version of the unsaved work.
fn differs(found: Result<Vec<Difference>, ClassifyError>) -> Result<bool, RestoreError> {
    match found {
        Ok(differences) => Ok(!differences.is_empty()),
        Err(ClassifyError::Compare(CompareError::TooLarge { .. })) => Ok(true),
        Err(e) => Err(RestoreError::Failed(e.to_string())),
    }
}

/// Restores a version of `path` as a new version, on the writer. In this order: what waits is
/// written; the version is compared with what is stored, and when nothing differs nothing more is
/// written or made; unsaved work is kept as a version; the version's rows are written in today's
/// table shape, which leaves `settings` and the conversations as they are; and they are committed.
///
/// The comparison is here, not in the window as step 6 of the design's order has it: this spares
/// the frame and compares what is really stored, and nothing has been written at this point.
///
/// | Fails at | Versions made | The working set | The answer |
/// |---|---|---|---|
/// | Keeping the work: no version | None | Unchanged | [`RestoreError::WorkNotKept`] |
/// | Keeping the work: cannot tell | Perhaps that one | Unchanged | [`RestoreError::WorkUnknown`] |
/// | Writing the rows, however it failed | Any of the work | The work's rows are written back | [`RestoreError::NotRestored`] |
/// | The commit: no version | Any of the work | The work's rows are written back | [`RestoreError::NotRestored`] |
/// | The commit: cannot tell | Any of the work, perhaps the restore | Left alone: nothing is written over it | [`RestoreError::CannotTell`] |
///
/// A commit that reported a failure but moved `HEAD` has finished. Rows written back are written as
/// autosave writes them, and wait for the next change if that fails too.
fn restore(
    inner: &Inner,
    path: &Path,
    request: RestoreRequest,
    (generation, instance): (u64, u64),
    watch: &Arc<JobWatch>,
    waiting: &mut BTreeMap<PathBuf, Rows>,
    failed: &mut bool,
) -> Result<Restored, RestoreError> {
    if instance != instance_of(inner, path) {
        return Err(RestoreError::Reopened);
    }
    if generation != generation_of(inner, path) {
        return Err(RestoreError::BranchChanged);
    }
    #[cfg(test)]
    if let Some(Fault::Hold(_, step)) = take_fault(inner, |f| matches!(f, Fault::Hold(false, _))) {
        step();
    }
    if lock(&inner.restores_unknown).contains_key(path) {
        return Err(RestoreError::OutcomeUnknown);
    }
    let schema = schema(inner).map_err(|e| RestoreError::Failed(format!("{e:?}")))?;
    write_waiting(inner, waiting);
    *failed = !waiting.is_empty();
    if waiting.contains_key(path) {
        return Err(RestoreError::Unwritten(unwritten(inner, path)));
    }
    let (open, at) =
        open_as_asked(inner, path, (generation, instance), (RestoreError::Reopened, RestoreError::BranchChanged, RestoreError::NotOpen))?;
    let project = &open[at];
    let version = project.read_model_at(&schema, &request.commit).map_err(RestoreError::Version)?;
    let current = project.read_model(&schema).map_err(|e| RestoreError::Failed(e.to_string()))?;
    if !differs(archforge_notations::differences::classified(&current, &version, request.repair))? {
        return Ok(Restored::Same { kept_work: false });
    }
    let replaced = replace(inner, project, &schema, &request, &version, watch);
    // While the project is still open: its closing, which forgets this, comes after.
    if matches!(replaced, Err(Stop::CannotTell { .. })) {
        let mut unknown = lock(&inner.restores_unknown);
        if !watch.confirmed.load(Ordering::SeqCst) {
            unknown.insert(path.to_path_buf(), watch.clone());
        }
    }
    drop(open);
    match replaced {
        Ok(done) => Ok(done),
        Err(Stop::Unchanged(error)) => Err(error),
        Err(Stop::WriteBack { why, kept_work }) => {
            waiting.insert(path.to_path_buf(), current);
            write_waiting(inner, waiting);
            *failed = !waiting.is_empty();
            Err(RestoreError::NotRestored { why, kept_work, written_back: !waiting.contains_key(path) })
        }
        Err(Stop::CannotTell { why, kept_work }) => Err(RestoreError::CannotTell { why, kept_work }),
    }
}

/// Hands a restore to the writer and returns its answer's channel at once, as a [`Job`]'s callback
/// must. The limit's clock starts first, and a thread of its own waits for the writer within it.
fn start_restore(
    inner: Arc<Inner>,
    path: PathBuf,
    request: RestoreRequest,
    (generation, instance): (u64, u64),
    watch: Arc<JobWatch>,
    limit: Duration,
) -> JobAnswer {
    let deadline = Instant::now() + limit;
    let language = archforge_ui_egui::i18n::language();
    let (answer, gives) = channel();
    let (reply, from_writer) = channel();
    let wait = {
        let (inner, watch, answer) = (inner.clone(), watch.clone(), answer.clone());
        move || {
            archforge_ui_egui::i18n::set_language(language);
            let _ = answer.send(forwarded(&from_writer, &watch.claim, deadline, &inner.cancel));
        }
    };
    if let Err(e) = spawn_wait(&inner, wait) {
        let _ = answer.send(Err(JobFailure::Failed(format!("the restore could not start: {e}"))));
        return gives;
    }
    // Should the writer have stopped, the reply's sender goes with the job, and the wait ends.
    let _ = writer(&inner).send(Job::Restore(path, request, generation, instance, watch, reply));
    gives
}

/// Waits for a restore's reply within `deadline` and gives the job's answer.
fn forwarded(
    from_writer: &Receiver<Result<Restored, RestoreError>>,
    claim: &AtomicBool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), JobFailure> {
    answered(waited(from_writer, deadline, cancel), claim, job_answer, not_going_ahead)
}

/// Waits for the writer's reply until `deadline` or until Archforge exits; none if neither brought
/// one, or the reply was lost.
fn waited<T>(from_writer: &Receiver<T>, deadline: Instant, cancel: &AtomicBool) -> Option<T> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match from_writer.recv_timeout(left.min(RESTORE_WAIT_STEP)) {
            Ok(answer) => return Some(answer),
            Err(RecvTimeoutError::Timeout) if !left.is_zero() && !cancel.load(Ordering::SeqCst) => {}
            Err(_) => return None,
        }
    }
}

/// A job's answer once [`waited`] has ended. With no reply the claim decides, in one step: a job
/// not started never will be, which is a plain failure; one that has started has an outcome nobody
/// can tell yet.
fn answered<T>(reply: Option<T>, claim: &AtomicBool, answer: fn(T) -> Result<(), JobFailure>, not_started: fn() -> String) -> Result<(), JobFailure> {
    match reply {
        Some(reply) => answer(reply),
        None if claim.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() => Err(JobFailure::Failed(not_started())),
        None => Err(JobFailure::Unclear),
    }
}

/// Runs `wait` on a thread of its own that a closing or an exit can wait for.
fn spawn_wait(inner: &Inner, wait: impl FnOnce() + Send + 'static) -> std::io::Result<()> {
    let handle = std::thread::Builder::new().name("archforge-job-wait".into()).spawn(wait)?;
    let mut closers = lock(&inner.closers);
    closers.retain(|c| !c.is_finished());
    closers.push(handle);
    Ok(())
}

/// Asks the writer for the first step of a switch and returns its answer's channel at once. An
/// answer that does not come is [`SwitchError::NoAnswer`]: a check that still runs moves nothing.
fn ask_for_check(
    inner: Arc<Inner>,
    path: PathBuf,
    target: SafeBranch,
    asked: (u64, u64),
    limit: Duration,
) -> Receiver<Result<CheckedBranch, SwitchError>> {
    let deadline = Instant::now() + limit;
    let (told, check) = channel();
    let (reply, from_writer) = channel();
    let (waiting, telling) = (inner.clone(), told.clone());
    let wait = move || {
        let _ = telling.send(waited(&from_writer, deadline, &waiting.cancel).unwrap_or(Err(SwitchError::NoAnswer)));
    };
    if let Err(e) = spawn_wait(&inner, wait) {
        let _ = told.send(Err(SwitchError::Failed(format!("the switch could not start: {e}"))));
        return check;
    }
    // Should the writer have stopped, the reply's sender goes with the job, and the wait ends.
    let _ = writer(&inner).send(Job::CheckSwitch(path, target, asked, reply));
    check
}

/// Hands the move of a switch to the writer and returns its answer's channel at once, as a
/// [`ReplacementJob`]'s callback must. The limit's clock starts first.
fn start_move(inner: Arc<Inner>, path: PathBuf, target: SafeBranch, asked: (u64, u64), under_way: Arc<JobWatch>, limit: Duration) -> JobAnswer {
    let deadline = Instant::now() + limit;
    let language = archforge_ui_egui::i18n::language();
    let (answer, gives) = channel();
    let (reply, from_writer) = channel();
    let wait = {
        let (inner, under_way, answer) = (inner.clone(), under_way.clone(), answer.clone());
        move || {
            archforge_ui_egui::i18n::set_language(language);
            let reply = waited(&from_writer, deadline, &inner.cancel);
            let outcome = answered(reply, &under_way.claim, switch_answer, || SwitchError::NoAnswer.words());
            // Released before the answer, as the writer does.
            drop(under_way);
            let _ = answer.send(outcome);
        }
    };
    if let Err(e) = spawn_wait(&inner, wait) {
        let _ = answer.send(Err(JobFailure::Failed(SwitchError::Failed(format!("the switch could not start: {e}")).words())));
        return gives;
    }
    let _ = writer(&inner).send(Job::Switch(path, target, asked, under_way, reply));
    gives
}

/// A switch's answer as a [`ReplacementJob`] gives it, in the language of the thread it is made on.
fn switch_answer(answer: Result<Switched, SwitchError>) -> Result<(), JobFailure> {
    answer.map(|_| ()).map_err(|error| {
        log::warn!("a switch stopped: {error}");
        JobFailure::Failed(error.words())
    })
}

/// What the log says of a restore that stopped: the exact reason, which the person's message may
/// put together with others.
fn stop_note(error: &RestoreError) -> String {
    format!("a restore stopped: {error}")
}

/// A restore's answer as a [`Job`] gives it, in the language of the thread it is made on. Only a
/// commit that cannot be told is unclear. Each message is one whole sentence for its translator; a
/// `{why}` in one is the project's own reason, in English, as its errors are wherever they show.
fn job_answer(answer: Result<Restored, RestoreError>) -> Result<(), JobFailure> {
    let error = match answer {
        Ok(Restored::Version { .. }) => return Ok(()),
        Ok(Restored::Same { kept_work: false }) => return Err(JobFailure::Failed(tr("This version is the same as your current work."))),
        Ok(Restored::Same { kept_work: true }) => {
            return Err(JobFailure::WorkSaved(tr("This version is the same as your current work. Your unsaved work was kept as a version.")));
        }
        Err(error) => error,
    };
    log::warn!("{}", stop_note(&error));
    let not_restored = |why: &str| fmt(&tr("The version could not be restored: {why}"), &[("why", why)]);
    Err(match error {
        RestoreError::CannotTell { .. } => JobFailure::Unclear,
        RestoreError::BranchChanged | RestoreError::Reopened | RestoreError::NotOpen => JobFailure::Failed(not_going_ahead()),
        RestoreError::Unwritten(_) | RestoreError::WorkNotKept(_) => {
            JobFailure::Failed(tr("Your unsaved work could not be kept, so nothing was restored."))
        }
        RestoreError::WorkUnknown(_) => {
            JobFailure::Failed(tr("Archforge could not tell whether your unsaved work was kept as a version, so nothing was restored."))
        }
        RestoreError::OutcomeUnknown => JobFailure::Failed(earlier_unknown()),
        RestoreError::Version(e) => JobFailure::Failed(not_restored(&e.to_string())),
        RestoreError::Failed(why) => JobFailure::Failed(not_restored(&why)),
        RestoreError::NotRestored { why, kept_work, written_back } => {
            let text = fmt(&not_restored_template(kept_work, written_back), &[("why", &why)]);
            if kept_work { JobFailure::WorkSaved(text) } else { JobFailure::Failed(text) }
        }
    })
}

/// The sentence for a restore that wrote its rows and could not finish, by what became of the work.
fn not_restored_template(kept_work: bool, written_back: bool) -> String {
    match (kept_work, written_back) {
        (false, true) => tr("The version could not be restored: {why}"),
        (false, false) => tr("The version could not be restored: {why}. Your work will be written with your next change."),
        (true, true) => tr("The version could not be restored: {why}. Your unsaved work was kept as a version."),
        (true, false) => kept_and_waiting(),
    }
}

/// The sentence for a restore whose work was kept and whose rows wait for the next change.
fn kept_and_waiting() -> String {
    tr("The version could not be restored: {why}. Your unsaved work was kept as a version, and it will be written with your next change.")
}

/// The message of a restore refused as an earlier one's outcome could not be told.
fn earlier_unknown() -> String {
    tr("Archforge could not tell whether an earlier restore finished. Close the project and open it again before restoring.")
}

/// The message of a restore that could not go ahead, whichever way the project had changed.
fn not_going_ahead() -> String {
    tr("The restore could not go ahead because the project changed or was busy, so nothing was restored.")
}

/// The message of the version a restore makes of the version saved at `saved_at`.
fn restored_message(saved_at: &str) -> String {
    format!("Restored the version of {saved_at}")
}

/// How a restore's writing stopped, and what is left to do about it.
enum Stop {
    /// The model did not change.
    Unchanged(RestoreError),
    /// The version's rows may be in the working set and are not committed: the work's go back.
    WriteBack { why: String, kept_work: bool },
    /// The commit's outcome cannot be told.
    CannotTell { why: String, kept_work: bool },
}

/// Keeps the unsaved work as a version, writes the version's rows and commits them.
fn replace(
    inner: &Inner,
    project: &Project,
    schema: &ModelSchema,
    request: &RestoreRequest,
    version: &Rows,
    watch: &JobWatch,
) -> Result<Restored, Stop> {
    use archforge_project::AfterFailure::{CannotTell, NoVersion, Version};
    let author = author_of(&request.by);
    let kept_work = match save_step(inner, project, &format!("Before restoring {}", now()), &author) {
        Ok(saved) => matches!(saved, archforge_project::Saved::Version { .. }),
        Err(f) => match f.version {
            Version(_) => true,
            NoVersion => return Err(Stop::Unchanged(RestoreError::WorkNotKept(f.error.to_string()))),
            CannotTell => return Err(Stop::Unchanged(RestoreError::WorkUnknown(f.error.to_string()))),
        },
    };
    if let Err(e) = write_step(inner, project, schema, version) {
        return Err(Stop::WriteBack { why: e.to_string(), kept_work });
    }
    *lock(&watch.before_commit) =
        project.versions(None, 1).ok().and_then(|page| page.versions.first().map(|v| (v.hash.clone(), project.current_branch())));
    match save_step(inner, project, &restored_message(&request.saved_at), &author) {
        Ok(saved) => Ok(restored(saved, kept_work)),
        Err(f) => match f.version {
            Version(_) => Ok(Restored::Version { kept_work }),
            NoVersion => Err(Stop::WriteBack { why: f.error.to_string(), kept_work }),
            CannotTell => Err(Stop::CannotTell { why: f.error.to_string(), kept_work }),
        },
    }
}

/// A save of a restore, and what a test made of it.
fn save_step(inner: &Inner, project: &Project, message: &str, author: &Author) -> Result<archforge_project::Saved, archforge_project::SaveFailure> {
    #[cfg(test)]
    if let Some(Fault::Commit(_, told, lands)) = take_fault(inner, |f| matches!(f, Fault::Commit(start, ..) if message.starts_with(start))) {
        let hash = if lands {
            project.save_version(message, author).map_err(|f| format!("{f:?}")).unwrap();
            "landed"
        } else {
            ""
        };
        if let Some(Fault::Hold(_, step)) = take_fault(inner, |f| matches!(f, Fault::Hold(true, _))) {
            step();
        }
        let version = told.after(hash);
        return Err(archforge_project::SaveFailure { error: refused_by_test(project), version });
    }
    let _ = inner;
    project.save_version(message, author)
}

/// A write of a restore's rows, and what a test made of it.
fn write_step(inner: &Inner, project: &Project, schema: &ModelSchema, rows: &Rows) -> Result<(), archforge_project::Error> {
    #[cfg(test)]
    if let Some(Fault::Write { lands }) = take_fault(inner, |f| matches!(f, Fault::Write { .. })) {
        if lands {
            project.write_model(schema, rows)?;
        }
        return Err(refused_by_test(project));
    }
    let _ = inner;
    project.write_model(schema, rows)
}

/// What goes wrong in a restore, for a test.
#[cfg(test)]
enum Fault {
    /// The commit whose message starts so reports a failure, having landed or not.
    Commit(&'static str, Told, bool),
    /// Writing the version's rows fails, having landed or not.
    Write { lands: bool },
    /// The next writing of what waits fails.
    WriteBack,
    /// Runs when the restore has started, or, when true, after a commit with a fault has landed.
    Hold(bool, Box<dyn FnOnce() + Send>),
    /// Runs at this point of a read-back.
    Reading(ReadingAt, Box<dyn FnOnce() + Send>),
    /// Runs just before a job picks its project, after any checks it made before writing.
    AtSelection(Box<dyn FnOnce() + Send>),
}

/// What a failed commit is judged to have left, for a test.
#[cfg(test)]
#[derive(Clone, Copy)]
enum Told {
    NoVersion,
    Landed,
    CannotTell,
}

#[cfg(test)]
impl Told {
    /// As the judgment of a commit whose new head is `hash`.
    fn after(self, hash: &str) -> archforge_project::AfterFailure {
        match self {
            Told::NoVersion => archforge_project::AfterFailure::NoVersion,
            Told::Landed => archforge_project::AfterFailure::Version(hash.to_string()),
            Told::CannotTell => archforge_project::AfterFailure::CannotTell,
        }
    }
}

/// The points of a read-back a test can hold it at.
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum ReadingAt {
    Start,
    Baseline,
    Page,
}

/// Runs the step a test has put at this point of a read-back.
#[cfg(test)]
fn hold_reading(inner: &Inner, here: ReadingAt) {
    if let Some(Fault::Reading(_, step)) = take_fault(inner, |f| matches!(f, Fault::Reading(at, _) if *at == here)) {
        step();
    }
}

/// Takes the first fault `wanted` accepts, using it up.
#[cfg(test)]
fn take_fault(inner: &Inner, wanted: impl Fn(&Fault) -> bool) -> Option<Fault> {
    let mut faults = lock(&inner.restore_faults);
    let at = faults.iter().position(wanted)?;
    Some(faults.remove(at))
}

/// The error a test's fault gives.
#[cfg(test)]
fn refused_by_test(project: &Project) -> archforge_project::Error {
    archforge_project::Error::Damaged { path: project.path().to_path_buf(), why: "a test refused".into() }
}

/// What a restore's commit says: no version when nothing differed after all, which a comparison
/// too large to finish leaves to it.
fn restored(saved: archforge_project::Saved, kept_work: bool) -> Restored {
    match saved {
        archforge_project::Saved::Version { .. } => Restored::Version { kept_work },
        archforge_project::Saved::NothingNew => Restored::Same { kept_work },
    }
}

/// Makes the branch `name` of the project at `path`, on the writer, unless it has closed or a
/// switch has moved it since this was asked for. From a version, what waits is left waiting, as it
/// does not reach a version; from current work (no `start`), it is written first, and rows that
/// cannot be written refuse it.
fn create_branch(
    inner: &Inner,
    path: &Path,
    name: &SafeBranch,
    start: Option<&str>,
    generation: u64,
    instance: u64,
    waiting: &mut BTreeMap<PathBuf, Rows>,
    failed: &mut bool,
) -> Result<BranchCreated, CreateBranchError> {
    if instance != instance_of(inner, path) {
        return Err(CreateBranchError::Reopened);
    }
    if generation != generation_of(inner, path) {
        return Err(CreateBranchError::BranchChanged);
    }
    let schema = schema(inner).map_err(|e| CreateBranchError::Failed(format!("{e:?}")))?;
    if start.is_none() {
        write_waiting(inner, waiting);
        *failed = !waiting.is_empty();
        if waiting.contains_key(path) {
            return Err(CreateBranchError::Unwritten(unwritten(inner, path)));
        }
    }
    // Checked again now that nothing can move: writing what waits may have taken a while.
    let (open, at) = open_as_asked(
        inner,
        path,
        (generation, instance),
        (CreateBranchError::Reopened, CreateBranchError::BranchChanged, CreateBranchError::NotOpen),
    )?;
    let project = &open[at];
    match start {
        Some(start) => project.create_branch(name, start, &schema),
        None => project.create_branch_from_work(name, &schema),
    }
    .map_err(CreateBranchError::Creation)
}

/// The open project at `path`, as the guard on the open projects and its place there, if it is
/// still what a job was asked for under: `asked` is the generation and the open instance then, and
/// `refusals` what to answer for a different instance, a different generation and no project.
///
/// All of it under the one lock, so that what passes is what the job gets. A closing removes the
/// project and only later moves the instance, but nothing can reopen it until that closing is
/// over, as both hold the job lock: so a project found here under the old instance is the old one,
/// and one removed but not yet counted is simply not found. Nothing takes these in the other order.
fn open_as_asked<'a, E>(
    inner: &'a Inner,
    path: &Path,
    (generation, instance): (u64, u64),
    (reopened, changed, not_open): (E, E, E),
) -> Result<(MutexGuard<'a, Vec<Project>>, usize), E> {
    #[cfg(test)]
    if let Some(Fault::AtSelection(step)) = take_fault(inner, |f| matches!(f, Fault::AtSelection(_))) {
        step();
    }
    let open = lock(&inner.open);
    if instance != instance_of(inner, path) {
        return Err(reopened);
    }
    if generation != generation_of(inner, path) {
        return Err(changed);
    }
    let at = open.iter().position(|p| p.path() == path).ok_or(not_open)?;
    Ok((open, at))
}

/// Deletes a branch of the project at `path`, on the writer, unless it has closed or a switch has
/// moved it since this was asked for.
fn delete_branch(inner: &Inner, path: &Path, branch: &SafeBranch, generation: u64, instance: u64) -> Result<Vec<Uuid>, DeleteJobError> {
    let (open, at) =
        open_as_asked(inner, path, (generation, instance), (DeleteJobError::Reopened, DeleteJobError::BranchChanged, DeleteJobError::NotOpen))?;
    open[at].delete_branch(branch).map_err(DeleteJobError::Delete)
}

/// Renames a branch of the project at `path`, on the writer, unless it has closed or a switch has
/// moved it since this was asked for. What waits follows, as writes go to the current branch.
fn rename_branch(inner: &Inner, path: &Path, from: &SafeBranch, to: &SafeBranch, generation: u64, instance: u64) -> Result<(), RenameJobError> {
    let (open, at) =
        open_as_asked(inner, path, (generation, instance), (RenameJobError::Reopened, RenameJobError::BranchChanged, RenameJobError::NotOpen))?;
    open[at].rename_branch(from, to).map_err(RenameJobError::Rename)
}

/// The writer's reason for deleting no branch, as the window takes it. None holds conversations.
fn not_deleted(error: DeleteJobError) -> BranchNotDeleted {
    match error {
        DeleteJobError::BranchChanged | DeleteJobError::Reopened | DeleteJobError::NotOpen => BranchNotDeleted::Moved,
        DeleteJobError::Delete(DeleteBranchError::CannotTell(why)) => BranchNotDeleted::Unclear(why),
        other => BranchNotDeleted::Failed(other.to_string()),
    }
}

/// Commits what of the conversations waits.
fn keep_conversations(inner: &Inner, project: &Project) -> Result<bool, archforge_project::Error> {
    #[cfg(test)]
    if inner.fail_chats.load(Ordering::SeqCst) {
        return Err(archforge_project::Error::Damaged { path: project.path().to_path_buf(), why: "a test refused".into() });
    }
    let _ = inner;
    project.commit_chats()
}

/// Hands a save to the writer; a writer that has stopped is the save's answer.
fn send_save(
    inner: &Arc<Inner>,
    project: &Path,
    by: SavedBy,
    generation: u64,
    described: Option<Described>,
    reply: Sender<Result<Saved, SaveError>>,
) {
    if let Err(e) = writer(inner).send(Job::Save(Some((project.to_path_buf(), by, generation)), described, reply.clone())) {
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
fn save_described(inner: &Arc<Inner>, project: &Path, by: SavedBy, generation: u64, text: String, reply: Sender<Result<Saved, SaveError>>) {
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
            Ok(None) => send_save(&inner, &path, by, generation, Some(Described { text, handed }), answer),
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
    let author = author_of(by);
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

/// Who a version is recorded as saved by.
fn author_of(by: &SavedBy) -> Author {
    match by {
        SavedBy::Person(name) => Author::person(name),
        SavedBy::Agent(label) => Author::agent(label.as_deref()),
    }
}

/// The time now, as the project stores times.
fn now() -> String {
    archforge_project::timestamp(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

/// The open instance of the project at `path` now: it moves each time the project closes.
fn instance_of(inner: &Inner, path: &Path) -> u64 {
    lock(&inner.instances).get(path).copied().unwrap_or(0)
}

/// The switch generation of the project at `path` now.
fn generation_of(inner: &Inner, path: &Path) -> u64 {
    lock(&inner.generations).get(path).copied().unwrap_or(0)
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
    let generation = generation_of(inner, path);
    let moved = tr("the branch changed while it was on its way");
    let run: ChatJob = Box::new(move |project| {
        let _ = reply.send(match project {
            Ok(project) => job(project),
            Err(NoProject::NotOpen) => Err(format!("{shown} is no longer open")),
            Err(NoProject::OtherBranch) => Err(moved),
        });
    });
    if let Err(e) = writer(inner).send(Job::Chat(path.to_path_buf(), generation, run)) {
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

    /// Ids counted from one, so that the same steps make the same ids.
    fn counted_ids() -> Box<dyn FnMut() -> archforge_mof::Uuid + Send> {
        let mut n = 0u64;
        Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_800_000, r)
        })
    }

    /// A model's rows: a project with one schema of the given name.
    fn model(schema: &str) -> Rows {
        let mut s = archforge_notations::new_project_session("Commerce platform", counted_ids()).unwrap();
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
        host: Arc<Projects>,
        agents: Option<Arc<crate::agents::Agents>>,
    }

    impl Sandbox {
        /// The host, with its folder created and `keys` wired in.
        fn new(keys: Option<Box<dyn archforge_secrets::SecretStore>>) -> Sandbox {
            let base = (0..1000)
                .map(|n| std::env::temp_dir().join(format!("af-d{}-{n}", std::process::id())))
                .find(|p| std::fs::create_dir(p).is_ok())
                .expect("a folder of the test's own");
            let host = Arc::new(Projects::new(&base.join("settings"), "Archforge test".into()));
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

    /// Moves `path`'s generation, as a switch of its branch will.
    fn switched(inner: &Inner, path: &Path) {
        *lock(&inner.generations).entry(path.to_path_buf()).or_default() += 1;
    }

    /// Holds the writer at a job until the sender is used or dropped, so jobs asked for meanwhile
    /// wait in its queue and a generation can move before it reaches them.
    fn hold_the_writer(host: &Projects, path: &Path) -> Sender<()> {
        let (release, wait) = channel::<()>();
        let (started, running) = channel::<()>();
        host.on_writer(path, move |_| {
            let _ = started.send(());
            let _ = wait.recv_timeout(Duration::from_secs(30));
            Ok(())
        });
        running.recv_timeout(Duration::from_secs(30)).expect("the writer reached the job");
        release
    }

    /// Rows from before a switch are dropped and said to be, and another project's are not touched.
    #[test]
    fn a_write_from_before_the_switch_is_dropped_and_recorded_for_that_project_only() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let other = wait(&host.create(&sandbox.base.join("p"), "Other")).unwrap().path;
        host.write(&path, model("a"));
        assert_eq!(saved(host, &path), Ok(true));
        let release = hold_the_writer(host, &path);
        host.write(&path, model("b"));
        host.write(&other, model("z"));
        switched(&host.inner, &path);
        drop(release);
        assert_eq!(host.flush_for_test(&path), Ok(()));
        let words = "A change arrived after the branch changed, so it was not written.".to_string();
        assert_eq!(host.writing(&path), Writing { pending: false, error: Some(words) });
        assert_eq!(saved(host, &path), Ok(false), "the stale rows did not reach the project");
        assert_eq!(saved(host, &other), Ok(true), "the other project's job was not refused");
        // The next change, of the generation now, is written and clears the word.
        host.write(&path, model("c"));
        assert_eq!(saved(host, &path), Ok(true));
        assert_eq!(host.writing(&path), Writing::default());
    }

    /// A save is refused by its reason, its generation taken before the key check it waits on.
    #[test]
    fn a_save_from_before_the_switch_is_refused_even_across_its_key_check() {
        let gate = Arc::new(KeyGate::default());
        let sandbox = Sandbox::new(Some(Box::new(Gated(gate.clone()))));
        let (host, path) = (&sandbox.host, sandbox.project());
        let other = wait(&host.create(&sandbox.base.join("p"), "Other")).unwrap().path;
        host.write(&path, model("a"));
        let release = hold_the_writer(host, &path);
        let queued = host.save(&path, None, nobody());
        switched(&host.inner, &path);
        drop(release);
        assert_eq!(made(queued.recv_timeout(Duration::from_secs(60)).unwrap()), Err(SaveError::BranchChanged));
        let before = sandbox.messages();
        // Across the key check: this project's move refuses it, another project's does not.
        for (moved, answer, next) in [(&other, Ok(true), "b"), (&path, Err(SaveError::BranchChanged), "c")] {
            gate.hold(true);
            let asked = host.save(&path, Some("Described".into()), nobody());
            gate.wait_for_a_read();
            switched(&host.inner, moved);
            gate.hold(false);
            assert_eq!(made(asked.recv_timeout(Duration::from_secs(60)).unwrap()), answer);
            host.write(&path, model(next));
        }
        assert_eq!(sandbox.messages().len(), before.len() + 1, "only the save in the other project's move made a version");
    }

    /// A conversation write is refused by its reason, its generation also taken before the key check.
    #[test]
    fn a_chat_write_from_before_the_switch_is_refused_by_its_reason() {
        use archforge_ui_egui::ai_agents::Provider;
        use archforge_ui_egui::chat_store::{ChatStore, Refusal, Write};
        let gate = Arc::new(KeyGate::default());
        let sandbox = Sandbox::new(Some(Box::new(Gated(gate.clone()))));
        let (host, path) = (&sandbox.host, sandbox.project());
        let chats = crate::chats::Chats::new(host.clone(), sandbox.agents.clone().unwrap());
        let chat = archforge_mof::Uuid::v7(1_762_400_000_000, [1; 10]);
        gate.hold(true);
        let asked = chats.write(
            &path,
            Write::Create { chat, title: "Hello".into(), provider: Provider::Claude, model: "m".into(), skip: Vec::new(), now: 1_762_437_731 },
        );
        gate.wait_for_a_read();
        switched(&host.inner, &path);
        gate.hold(false);
        assert_eq!(asked.recv_timeout(Duration::from_secs(60)).unwrap(), Err(Refusal::BranchChanged));
        assert!(chats.list(&path).recv_timeout(Duration::from_secs(60)).unwrap().unwrap().is_empty(), "nothing was written");
        // A job with no key check: its reason, and a project not open keeps its own.
        let stale = host.generation_of(&path).saturating_sub(1);
        let reason = host.on_writer_open(&path, stale, |p| p.err()).recv_timeout(Duration::from_secs(60)).unwrap();
        assert_eq!(reason, Some(NoProject::OtherBranch));
        let gone = host.on_writer_open(Path::new("/nowhere"), 0, |p| p.err()).recv_timeout(Duration::from_secs(60)).unwrap();
        assert_eq!(gone, Some(NoProject::NotOpen));
        let release = hold_the_writer(host, &path);
        let read = host.on_writer(&path, |_| Ok(()));
        switched(&host.inner, &path);
        drop(release);
        assert_eq!(read.recv_timeout(Duration::from_secs(60)).unwrap(), Err("the branch changed while it was on its way".to_string()));
    }

    /// An answer read across a switch is dropped; the same read with no switch answers.
    #[test]
    fn a_read_across_a_switch_gives_no_answer() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        assert_eq!(host.on_a_reader(&path, |_| Ok(1)).recv_timeout(Duration::from_secs(30)), Ok(Ok(1)));
        let (inner, moved) = (host.inner.clone(), path.clone());
        let across = host.on_a_reader(&path, move |_| {
            switched(&inner, &moved);
            Ok(2)
        });
        assert_eq!(across.recv_timeout(Duration::from_secs(30)), Err(std::sync::mpsc::RecvTimeoutError::Disconnected));
    }

    /// A saved project on `main` with the branches `elsewhere` (an earlier state) and `scripted`.
    fn branched() -> (Sandbox, PathBuf) {
        let sandbox = Sandbox::new(None);
        let path = sandbox.project();
        sandbox.host.write(&path, model("a"));
        assert_eq!(saved(&sandbox.host, &path), Ok(true));
        let run = |branch: &str, statement: &str| lock(&sandbox.host.inner.open)[0].run_on_branch(branch, statement).unwrap();
        run("main", "CALL DOLT_BRANCH('elsewhere')");
        run("main", "CALL DOLT_BRANCH('scripted')");
        run("scripted", "CREATE TRIGGER t BEFORE UPDATE ON settings FOR EACH ROW SET NEW.`value` = NEW.`value`");
        sandbox.host.write(&path, model("b"));
        assert_eq!(saved(&sandbox.host, &path), Ok(true));
        (sandbox, path)
    }

    /// The branch named `name`.
    fn branch(name: &str) -> SafeBranch {
        SafeBranch::found(name).unwrap()
    }

    /// Asks for a switch and gives its answer.
    fn switch_to(host: &Projects, path: &Path, name: &str) -> Result<Switched, SwitchError> {
        host.switch_branch(path, branch(name)).recv_timeout(Duration::from_secs(60)).expect("the switch answers")
    }

    /// The branch the project is on, by its name.
    fn on(host: &Projects) -> String {
        lock(&host.inner.open)[0].current_branch().stored().to_string()
    }

    /// A conversation with one change link, applied at the last save and undone since.
    fn undone_since_the_save(host: &Projects, path: &Path) -> (archforge_mof::Uuid, archforge_mof::Uuid) {
        use archforge_project::{
            ChangeKind, ChangeState, ChangedElement, NewChat, NewQuestion, NewTurn, Provider, TurnChanges, TurnEnd, TurnOutcome,
        };
        let id = |n: u8| archforge_mof::Uuid::v7(1_762_400_000_000, [n; 10]);
        let (chat, turn) = (id(1), id(2));
        let seeded = on_writer(&host.inner, path, move |p| {
            let (now, keys) = (now(), archforge_secrets::MemoryStore::new());
            let e = |e: archforge_project::Error| e.to_string();
            let new = NewChat {
                id: chat,
                origin: "Chat 1".into(),
                title: "T".into(),
                provider: Provider::Codex,
                model: "m".into(),
                created_by: "t".into(),
                now: now.clone(),
            };
            p.create_chat(&new, &keys).map_err(e)?;
            let asked = NewTurn { id: turn, chat, model: "m".into(), allow_changes: true, now: now.clone() };
            p.start_turn(&asked, &NewQuestion { id: id(3), body: "Q".into(), anchors: Vec::new() }, &keys).map_err(e)?;
            let changed = vec![ChangedElement { element: id(4), kind: ChangeKind::Added, metaclass: "er_schema".into(), name: None }];
            let changes = Some(TurnChanges { changes: 1, elements: 1, changed });
            p.end_turn(&TurnEnd { turn, outcome: TurnOutcome::Completed, error: None, now: now.clone(), changes }).map_err(e)?;
            p.commit("Saved").map_err(e)?;
            p.set_change_states(&[(turn, ChangeState::Undone)], &now).map_err(e)
        });
        assert_eq!(seeded.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
        (chat, turn)
    }

    /// What Don't save leaves the conversation's change link as: back at its saved state while the
    /// ledger still knows it, as it is once the ledger has started again.
    fn after_dont_save(host: &Projects, path: &Path, chat: archforge_mof::Uuid) -> Option<archforge_project::ChangeState> {
        let read = on_writer(&host.inner, path, move |p| {
            p.discard(&now()).map_err(|e| e.to_string())?;
            Ok(p.load_chat(chat).map_err(|e| e.to_string())?.turns[0].change.as_ref().map(|c| c.state))
        });
        read.recv_timeout(Duration::from_secs(60)).unwrap().unwrap()
    }

    /// The time the versions restored in these tests were saved at.
    const SAVED_AT: &str = "2026-11-08T14:02:11Z";

    /// Restores `commit` and gives the answer.
    fn restore_of(host: &Projects, path: &Path, commit: &str) -> Result<Restored, RestoreError> {
        host.restore_version(path, restoring(commit)).recv_timeout(Duration::from_secs(60)).expect("the restore answers")
    }

    /// A restore of `commit`, repaired as an opening repairs a model.
    fn restoring(commit: &str) -> RestoreRequest {
        RestoreRequest { commit: commit.into(), saved_at: SAVED_AT.into(), by: nobody(), repair: archforge_ui_egui::editing::fit_tables }
    }

    /// The commit of the `n`th newest version of the open project.
    fn version(host: &Projects, n: usize) -> String {
        lock(&host.inner.open)[0].versions(None, 50).unwrap().versions[n].hash.clone()
    }

    /// The model the open project's working set holds.
    fn work(host: &Projects) -> Rows {
        lock(&host.inner.open)[0].read_model(&schema(&host.inner).unwrap()).unwrap()
    }

    /// Whether the open project reads as unsaved.
    fn unsaved(host: &Projects) -> bool {
        lock(&host.inner.open)[0].has_unsaved().unwrap()
    }

    /// With unsaved work, a restore keeps it as a version, then makes the new version of the old
    /// model; the project's settings, changed since that version, stay as they are now.
    #[test]
    fn a_restore_keeps_unsaved_work_then_makes_a_version_of_the_old_model() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        lock(&host.inner.open)[0].run_on_branch("main", "UPDATE settings SET `value` = 'Renamed' WHERE `key` = 'name'").unwrap();
        // Handed over just before: the restore writes it first, and keeps it.
        host.write(&path, model("c"));
        let old = version(host, 1);
        assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Version { kept_work: true });
        let messages = sandbox.messages();
        assert_eq!(messages[0], format!("Restored the version of {SAVED_AT}"));
        assert!(messages[1].starts_with("Before restoring 20"), "{messages:?}");
        assert_eq!(work(host), model("a"));
        assert!(!unsaved(host));
        let before = version(host, 1);
        assert_eq!(lock(&host.inner.open)[0].read_model_at(&schema(&host.inner).unwrap(), &before).unwrap(), model("c"));
        assert_eq!(close(host, &path), Ok(()));
        wait(&host.open(&path)).unwrap();
        assert_eq!(lock(&host.inner.open)[0].settings().name, "Renamed");
    }

    /// With only a conversation's rows uncommitted, the restore makes its own version alone and
    /// leaves those rows uncommitted.
    #[test]
    fn a_restore_with_nothing_unsaved_makes_only_its_own_version_and_leaves_chats_alone() {
        use archforge_project::{NewChat, NewQuestion, NewTurn, Provider};
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let id = |n: u8| archforge_mof::Uuid::v7(1_762_400_000_000, [n; 10]);
        let new = NewChat {
            id: id(1),
            origin: "Chat 1".into(),
            title: "T".into(),
            provider: Provider::Codex,
            model: "m".into(),
            created_by: "t".into(),
            now: now(),
        };
        let turn = NewTurn { id: id(2), chat: id(1), model: "m".into(), allow_changes: true, now: now() };
        let created = on_writer(&host.inner, &path, move |p| {
            let keys = archforge_secrets::MemoryStore::new();
            p.create_chat(&new, &keys).map_err(|e| e.to_string())?;
            p.start_turn(&turn, &NewQuestion { id: id(3), body: "Q".into(), anchors: Vec::new() }, &keys).map(|_| ()).map_err(|e| e.to_string())
        });
        assert_eq!(created.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
        let (old, count) = (version(host, 1), sandbox.messages().len());
        assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Version { kept_work: false });
        let messages = sandbox.messages();
        assert_eq!(messages.len(), count + 1, "one version: {messages:?}");
        assert_eq!(messages[0], format!("Restored the version of {SAVED_AT}"));
        let chats = on_writer(&host.inner, &path, |p| p.commit_chats().map_err(|e| e.to_string()));
        assert_eq!(chats.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(true), "the conversation's rows were not committed");
    }

    /// A version that holds what the work holds makes no version and writes nothing, unsaved or not.
    #[test]
    fn a_version_the_same_as_the_work_makes_no_version_and_writes_nothing() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        host.write(&path, model("a"));
        let (old, messages) = (version(host, 1), sandbox.messages());
        assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Same { kept_work: false });
        assert_eq!(sandbox.messages(), messages);
        assert!(unsaved(host), "the unsaved work is as it was");
    }

    /// A table box too short for its columns, as an earlier version left it, and the same box as
    /// an opening fits it: the two differ only by that repair.
    fn short_and_fitted_box() -> (Rows, Rows) {
        let mut s = archforge_notations::new_project_session("Commerce platform", counted_ids()).unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let shop = id(s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let table = id(s.execute("er.entity.add", &serde_json::json!({ "schema": shop, "name": "customer" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": table, "name": "id", "type": "BIGINT", "primary_key": true })).unwrap();
        let diagram = id(s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": shop, "name": "Shop" })).unwrap().value);
        s.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": table, "x": 0, "y": 0, "width": 240, "height": 20 })).unwrap();
        let (tables, junctions) = archforge_notations::storage();
        let short = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
        let fitted = archforge_notations::session_from_rows_repaired(&short, counted_ids(), archforge_ui_egui::editing::fit_tables).unwrap();
        let fitted = archforge_mof::rows::snapshot(fitted.doc(), &tables, &junctions).unwrap();
        assert_ne!(short, fitted, "the repair changes the box");
        (short, fitted)
    }

    /// A layout only the repair changes is no difference: it makes no false new version.
    #[test]
    fn a_layout_only_the_repair_changes_makes_no_version() {
        let sandbox = Sandbox::new(None);
        let path = sandbox.project();
        let host = &sandbox.host;
        let (short, fitted) = short_and_fitted_box();
        host.write(&path, short);
        assert_eq!(saved(host, &path), Ok(true));
        host.write(&path, fitted.clone());
        assert_eq!(saved(host, &path), Ok(true));
        let (old, messages) = (version(host, 1), sandbox.messages());
        assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Same { kept_work: false });
        assert_eq!((sandbox.messages(), work(host)), (messages, fitted));
    }

    /// A restore asked for before the project's branch moved is refused and touches nothing, moved
    /// by hand while it waits or by a switch in flight when it was asked for.
    #[test]
    fn a_restore_from_before_a_switch_is_refused_by_its_reason() {
        let (sandbox, path) = branched();
        let host = sandbox.host.clone();
        let old = version(&host, 1);
        let (messages, unmoved) = (sandbox.messages(), work(&host));
        let release = hold_the_writer(&host, &path);
        let waiting = host.restore_version(&path, restoring(&old));
        switched(&host.inner, &path);
        drop(release);
        assert!(matches!(waiting.recv_timeout(Duration::from_secs(60)).unwrap(), Err(RestoreError::BranchChanged)));
        assert_eq!((sandbox.messages(), work(&host)), (messages.clone(), unmoved.clone()));

        let (late, late_path, commit) = (host.clone(), path.clone(), old.clone());
        let (asked, answer) = channel();
        *lock(&host.inner.before_move) = Some(Box::new(move || {
            let _ = asked.send(late.restore_version(&late_path, restoring(&commit)));
        }));
        assert!(matches!(switch_to(&host, &path, "elsewhere"), Ok(Switched::Moved(_))));
        let queued = answer.recv_timeout(Duration::from_secs(60)).unwrap();
        assert!(matches!(queued.recv_timeout(Duration::from_secs(60)).unwrap(), Err(RestoreError::BranchChanged)));
        assert_eq!(switch_to(&host, &path, "main").map(|_| sandbox.messages()).unwrap(), messages, "the branch left has no new version");
    }

    /// A commit that is no version, a project not open and rows that cannot be written each stop
    /// it, with no version made; rows that cannot be written stay waiting.
    #[test]
    fn a_restore_is_refused_by_its_reason() {
        use archforge_project::Error;
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (old, messages) = (version(host, 1), sandbox.messages());
        assert!(matches!(restore_of(host, &path, &"0".repeat(32)), Err(RestoreError::Version(Error::NotAVersion { .. }))));
        assert!(matches!(restore_of(host, &sandbox.base.join("nowhere"), &old), Err(RestoreError::NotOpen)));
        let mut broken = model("c");
        broken.tables.iter_mut().find(|t| !t.is_empty()).unwrap()[0].clear();
        host.write(&path, broken);
        assert!(matches!(restore_of(host, &path, &old), Err(RestoreError::Unwritten(_))));
        assert!(host.writing(&path).pending, "the rows are still waiting");
        assert_eq!(sandbox.messages(), messages);
    }

    /// A restore that stops before it tried to write leaves the writer's timer running.
    #[test]
    fn a_restore_that_wrote_nothing_leaves_the_rows_to_the_timer() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let old = version(host, 1);
        let release = hold_the_writer(host, &path);
        host.write(&path, model("c"));
        let nowhere = sandbox.base.join("nowhere");
        let asked = host.restore_version(&nowhere, restoring(&old));
        switched(&host.inner, &nowhere);
        drop(release);
        assert!(matches!(asked.recv_timeout(Duration::from_secs(60)).unwrap(), Err(RestoreError::BranchChanged)));
        let deadline = Instant::now() + Duration::from_secs(30);
        while host.writing(&path).pending {
            assert!(Instant::now() < deadline, "the rows are still waiting");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(work(host), model("c"));
    }

    /// Nothing found stops a restore, anything found goes on, too large to tell goes on, and a side
    /// that cannot be read is refused.
    #[test]
    fn a_comparison_decides_whether_a_restore_goes_on() {
        let too_large = ClassifyError::Compare(CompareError::TooLarge { work: 2, max: 1 });
        assert!(!differs(Ok(Vec::new())).unwrap());
        assert!(differs(Err(too_large)).unwrap());
        let unreadable = ClassifyError::Unreadable { side: "before", why: "x".into() };
        assert!(matches!(differs(Err(unreadable)), Err(RestoreError::Failed(_))));
        assert_eq!(restored(archforge_project::Saved::NothingNew, true), Restored::Same { kept_work: true });
    }

    /// A project with unsaved work, restored with `faults` waiting; the commits it held before.
    fn restored_with(faults: Vec<Fault>) -> (Sandbox, PathBuf, Vec<String>, Result<Restored, RestoreError>) {
        let (sandbox, path) = branched();
        let host = sandbox.host.clone();
        host.write(&path, model("c"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        let (old, messages) = (version(&host, 1), sandbox.messages());
        *lock(&host.inner.restore_faults) = faults;
        let answer = restore_of(&host, &path, &old);
        (sandbox, path, messages, answer)
    }

    /// Asserts that the work is what the working set holds, and whether it is saved.
    #[track_caller]
    fn the_work_is_back(host: &Projects, saved_first: bool) {
        assert_eq!(work(host), model("c"));
        assert_eq!(unsaved(host), !saved_first);
    }

    /// Whether the newest commit is a `Before restoring` version.
    fn before_on_top(messages: &[String]) -> bool {
        messages[0].starts_with("Before restoring ")
    }

    /// Keeping the work fails: nothing moves, and the answer says whether a version exists.
    #[test]
    fn when_the_work_cannot_be_kept_nothing_is_restored() {
        let before = "Before restoring";
        let (sandbox, _path, messages, answer) = restored_with(vec![Fault::Commit(before, Told::NoVersion, false)]);
        assert!(matches!(answer, Err(RestoreError::WorkNotKept(_))), "{answer:?}");
        assert_eq!(sandbox.messages(), messages);
        the_work_is_back(&sandbox.host, false);
        for lands in [false, true] {
            let (sandbox, _path, messages, answer) = restored_with(vec![Fault::Commit(before, Told::CannotTell, lands)]);
            assert!(matches!(answer, Err(RestoreError::WorkUnknown(_))), "{answer:?}");
            assert_eq!(sandbox.messages().len(), messages.len() + usize::from(lands));
            the_work_is_back(&sandbox.host, lands);
        }
        // A commit that reported a failure and moved the head has finished.
        let (sandbox, .., answer) = restored_with(vec![Fault::Commit(before, Told::Landed, true)]);
        assert_eq!(answer.unwrap(), Restored::Version { kept_work: true });
        assert_eq!(work(&sandbox.host), model("a"));
    }

    /// Writing the rows fails, whether or not they landed: the work's go back and its version stays.
    #[test]
    fn when_the_rows_cannot_be_written_the_work_goes_back() {
        for lands in [false, true] {
            let (sandbox, _path, _messages, answer) = restored_with(vec![Fault::Write { lands }]);
            assert!(matches!(answer, Err(RestoreError::NotRestored { kept_work: true, written_back: true, .. })), "{answer:?}");
            assert!(before_on_top(&sandbox.messages()));
            the_work_is_back(&sandbox.host, true);
        }
    }

    /// When the work's rows cannot go back either, they wait for the next write.
    #[test]
    fn rows_that_cannot_go_back_wait_for_the_next_write() {
        let (sandbox, path, _messages, answer) = restored_with(vec![Fault::Write { lands: true }, Fault::WriteBack]);
        assert!(matches!(answer, Err(RestoreError::NotRestored { kept_work: true, written_back: false, .. })), "{answer:?}");
        let said = answer.unwrap_err().to_string();
        assert!(said.contains("kept as a version") && said.contains("wait to be written"), "the reason says what exists: {said}");
        let host = &sandbox.host;
        // Not tried again on a timer: as after any failed write, the next change or flush tries it.
        std::thread::sleep(SETTLE * 3);
        assert_eq!((host.writing(&path).pending, work(host)), (true, model("a")), "still the version's rows");
        assert!(host.writing(&path).error.is_some());
        assert_eq!(host.flush_for_test(&path), Ok(()));
        the_work_is_back(host, true);
    }

    /// The final commit fails: the work goes back and a retry still makes the restore's version, so
    /// the history cannot be left without it behind an answer of "the same".
    #[test]
    fn a_failed_commit_writes_the_work_back_and_a_retry_makes_the_version() {
        let restoring = "Restored the version";
        let (sandbox, path, _messages, answer) = restored_with(vec![Fault::Commit(restoring, Told::NoVersion, false)]);
        assert!(matches!(answer, Err(RestoreError::NotRestored { kept_work: true, written_back: true, .. })), "{answer:?}");
        let host = &sandbox.host;
        assert!(before_on_top(&sandbox.messages()));
        the_work_is_back(host, true);
        let old = version(host, 2);
        assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Version { kept_work: false });
        assert_eq!((sandbox.messages()[0].as_str(), work(host)), (format!("Restored the version of {SAVED_AT}").as_str(), model("a")));
        // A commit that reported a failure and moved the head has finished.
        let (sandbox, .., answer) = restored_with(vec![Fault::Commit(restoring, Told::Landed, true)]);
        assert_eq!(answer.unwrap(), Restored::Version { kept_work: true });
        assert_eq!(work(&sandbox.host), model("a"));
    }

    /// A commit whose outcome cannot be read is never written over, and until the project closes
    /// a retry is refused rather than answering "the same" about a restore that may not exist.
    #[test]
    fn a_commit_that_cannot_be_told_is_not_written_over_and_stops_a_retry() {
        for lands in [false, true] {
            let (sandbox, path, _messages, answer) = restored_with(vec![Fault::Commit("Restored the version", Told::CannotTell, lands)]);
            assert!(matches!(answer, Err(RestoreError::CannotTell { kept_work: true, .. })), "{answer:?}");
            let host = &sandbox.host;
            assert_eq!(work(host), model("a"), "nothing was written over it");
            assert_eq!(unsaved(host), !lands);
            assert_eq!(sandbox.messages()[0].starts_with("Restored the version"), lands);
            let old = version(host, if lands { 3 } else { 2 });
            assert!(matches!(restore_of(host, &path, &old), Err(RestoreError::OutcomeUnknown)));
            let closing = if lands { host.close_without_writing(&path) } else { host.close(&path) };
            assert_eq!(closing.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Same { kept_work: false }, "reopened, it reads what is stored");
        }
    }

    /// The answer to a restore job's `run`, within the time a test waits.
    fn ran(answer: &JobAnswer) -> Result<(), JobFailure> {
        answer.recv_timeout(Duration::from_secs(60)).expect("the job answers")
    }

    /// A job restoring `commit`, within the real limit.
    fn job_of(host: &Projects, path: &Path, commit: &str) -> ReplacementJob {
        host.restore_job_within(path, restoring(commit), RESTORE_LIMIT)
    }

    /// Holds a restore at its start, or after its faulted commit has landed, and takes the same way
    /// out of the wait as its limit, with no clock to wait for: its answer is unclear. Dropping
    /// what this gives lets it go on.
    fn started_then_unclear(host: &Projects, run: Box<dyn FnOnce() -> JobAnswer>, after_commit: bool) -> Sender<()> {
        let (running, started) = channel();
        let (finish, hold) = channel::<()>();
        let step = move || {
            let _ = running.send(());
            let _ = hold.recv_timeout(Duration::from_secs(30));
        };
        lock(&host.inner.restore_faults).push(Fault::Hold(after_commit, Box::new(step)));
        let answer = run();
        started.recv_timeout(Duration::from_secs(60)).expect("the job started");
        host.inner.cancel.store(true, Ordering::SeqCst);
        assert!(matches!(ran(&answer), Err(JobFailure::Unclear)));
        host.inner.cancel.store(false, Ordering::SeqCst);
        finish
    }

    /// Waits for every thread a job started to have ended, so none outlives its answer.
    fn no_thread_left(host: &Projects) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while lock(&host.inner.closers).iter().any(|c| !c.is_finished()) {
            assert!(Instant::now() < deadline, "a thread is still waiting");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The reply decides; with none the claim does, whether it timed out, was lost or Archforge exits.
    #[test]
    fn a_job_with_no_reply_is_decided_by_its_claim() {
        let (no, past) = (AtomicBool::new(false), Instant::now());
        let far = Instant::now() + Duration::from_secs(3600);
        let (tx, waiting) = channel();
        tx.send(Ok(Restored::Version { kept_work: false })).unwrap();
        assert!(matches!(forwarded(&waiting, &AtomicBool::new(false), far, &no), Ok(())));
        // Timed out, and Archforge exiting, with the sender still there.
        for (deadline, exiting) in [(past, false), (far, true)] {
            let unclaimed = AtomicBool::new(false);
            assert!(matches!(forwarded(&waiting, &unclaimed, deadline, &AtomicBool::new(exiting)), Err(JobFailure::Failed(_))));
            assert!(unclaimed.load(Ordering::SeqCst), "the job can no longer start");
            assert!(matches!(forwarded(&waiting, &AtomicBool::new(true), deadline, &AtomicBool::new(exiting)), Err(JobFailure::Unclear)));
        }
        drop(tx);
        assert!(matches!(forwarded(&waiting, &AtomicBool::new(false), far, &no), Err(JobFailure::Failed(_))), "the reply was lost, not started");
        assert!(matches!(forwarded(&waiting, &AtomicBool::new(true), far, &no), Err(JobFailure::Unclear)), "the reply was lost, started");
    }

    /// Each answer becomes the failure it should; a commit that cannot be told is the only unclear
    /// one, and only the work having been kept makes a failure one that saved it.
    #[test]
    fn a_restores_answer_becomes_the_jobs() {
        let not_restored = |kept_work, written_back| RestoreError::NotRestored { why: "x".into(), kept_work, written_back };
        assert!(matches!(job_answer(Ok(Restored::Version { kept_work: true })), Ok(())));
        assert!(matches!(job_answer(Ok(Restored::Same { kept_work: false })), Err(JobFailure::Failed(_))));
        assert!(matches!(job_answer(Ok(Restored::Same { kept_work: true })), Err(JobFailure::WorkSaved(_))));
        assert!(matches!(job_answer(Err(RestoreError::CannotTell { why: "x".into(), kept_work: true })), Err(JobFailure::Unclear)));
        assert!(matches!(job_answer(Err(not_restored(true, true))), Err(JobFailure::WorkSaved(_))));
        assert!(matches!(job_answer(Err(not_restored(false, true))), Err(JobFailure::Failed(_))));
        let plain = [
            RestoreError::BranchChanged,
            RestoreError::Reopened,
            RestoreError::NotOpen,
            RestoreError::Unwritten("x".into()),
            RestoreError::WorkNotKept("x".into()),
            RestoreError::WorkUnknown("x".into()),
            RestoreError::OutcomeUnknown,
            RestoreError::Failed("x".into()),
        ];
        for error in plain {
            let said = format!("{error:?}");
            assert!(matches!(job_answer(Err(error)), Err(JobFailure::Failed(text)) if !text.is_empty()), "{said}");
        }
        assert!(matches!(job_answer(Err(not_restored(true, false))), Err(JobFailure::WorkSaved(_))));
        assert!(matches!(job_answer(Err(not_restored(false, false))), Err(JobFailure::Failed(_))));
        // Merged for the person, the three reasons stay apart in the log.
        let notes = [RestoreError::BranchChanged, RestoreError::Reopened, RestoreError::NotOpen].map(|e| stop_note(&e));
        assert!(notes[0] != notes[1] && notes[1] != notes[2] && notes[0] != notes[2], "{notes:?}");
    }

    /// In every language each outcome of a restore that could not finish has a sentence of its
    /// own, with the reason in its place and nothing left to fill in.
    #[test]
    fn each_way_a_restore_stops_has_a_whole_sentence_in_every_language() {
        use archforge_ui_egui::i18n::set_language;
        for language in ["en", "de", "es", "fr", "ja", "pt-PT", "zh-Hans"] {
            set_language(language);
            let said = [(false, true), (false, false), (true, true), (true, false)].map(|(kept_work, written_back)| {
                let answer = job_answer(Err(RestoreError::NotRestored { why: "reason-x".into(), kept_work, written_back }));
                let (Err(JobFailure::Failed(text)) | Err(JobFailure::WorkSaved(text))) = answer else { panic!("a failure") };
                text
            });
            set_language("en");
            assert!(said.iter().all(|text| text.contains("reason-x") && !text.contains("{why}")), "{language}: {said:?}");
            for (n, text) in said.iter().enumerate() {
                assert!(said[n + 1..].iter().all(|other| other != text), "{language}: {said:?}");
            }
        }
    }

    /// The words are made on the wait's own thread in the language the job was run under, not that
    /// thread's default.
    #[test]
    fn a_jobs_failure_comes_in_the_interfaces_language() {
        use archforge_ui_egui::i18n::set_language;
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let same = version(host, 0);
        let english = |_: ()| job_answer(Ok(Restored::Same { kept_work: false }));
        let word = |answer: Result<(), JobFailure>| match answer {
            Err(JobFailure::Failed(text)) => text,
            _ => panic!("a failure"),
        };
        set_language("en");
        let in_english = word(english(()));
        set_language("de");
        let in_german = word(english(()));
        let told = word(ran(&(job_of(host, &path, &same).run)()));
        set_language("en");
        assert_ne!(in_german, in_english, "the sentence is translated");
        assert_eq!(told, in_german);
    }

    /// A job taken for a branch the project has since left is refused, its generation taken when
    /// it was asked for; a project closed meanwhile is refused too, and neither leaves a thread.
    #[test]
    fn a_restore_job_from_before_a_switch_or_a_close_is_refused() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let old = version(host, 1);
        let (stale, closed) = (job_of(host, &path, &old), job_of(host, &path, &old));
        let messages = sandbox.messages();
        switched(&host.inner, &path);
        assert!(matches!(ran(&(stale.run)()), Err(JobFailure::Failed(why)) if why == not_going_ahead()));
        assert_eq!(sandbox.messages(), messages);
        let fresh = ProjectHost::restore_job(&**host, &path, old.clone(), SAVED_AT.into(), nobody(), archforge_ui_egui::editing::fit_tables);
        assert!(matches!(ran(&(fresh.run)()), Ok(())), "a job asked for now runs");
        assert_eq!(work(host), model("a"));
        assert_eq!(close(host, &path), Ok(()));
        assert!(matches!(ran(&(closed.run)()), Err(JobFailure::Failed(_))));
        no_thread_left(host);
    }

    /// Past the limit a job the writer has not reached is a plain failure and never runs late; both
    /// callbacks return while the writer is busy, and the read-back answers without it.
    #[test]
    fn a_restore_not_started_within_the_limit_never_runs() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (old, messages) = (version(host, 1), sandbox.messages());
        let job = host.restore_job_within(&path, restoring(&old), Duration::ZERO);
        let release = hold_the_writer(host, &path);
        assert!(matches!(ran(&(job.run)()), Err(JobFailure::Failed(why)) if why == not_going_ahead()));
        assert!((job.read_back)().recv_timeout(Duration::from_secs(60)).unwrap().is_err(), "the read-back needs no writer");
        drop(release);
        assert_eq!(host.flush_for_test(&path), Ok(()));
        assert_eq!((sandbox.messages(), work(host)), (messages, model("b")));
        no_thread_left(host);
    }

    /// A job that has started and passes its limit is unclear; when it finishes late the read-back
    /// finds its version.
    #[test]
    fn a_restore_started_but_late_is_unclear_and_read_back_when_it_lands() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let ReplacementJob { run, read_back } = job_of(host, &path, &version(host, 1));
        drop(started_then_unclear(host, run, false));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        assert_eq!(work(host), model("a"));
        assert_eq!(read_back().recv_timeout(Duration::from_secs(60)).unwrap(), Ok(true));
        no_thread_left(host);
    }

    /// An earlier restore of a version saved at the same time is still the newest version and has
    /// the same message: that is no sign this one finished, however far this one got.
    #[test]
    fn a_read_back_is_not_fooled_by_an_earlier_restore_with_the_same_message() {
        let (sandbox, path) = branched();
        let host = sandbox.host.clone();
        let (old, other) = (version(&host, 1), version(&host, 0));
        assert_eq!(restore_of(&host, &path, &old).unwrap(), Restored::Version { kept_work: false });
        // It reached its commit, which did not land. Only the head can tell the two apart.
        *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, false)];
        let job = job_of(&host, &path, &other);
        assert!(matches!(ran(&(job.run)()), Err(JobFailure::Unclear)));
        assert_eq!(sandbox.messages()[0], restored_message(SAVED_AT), "the earlier one is still on top");
        assert!((job.read_back)().recv_timeout(Duration::from_secs(60)).unwrap().is_err());

        // Another that has only started, its limit passed, has reached no commit at all.
        let (sandbox, path) = branched();
        let host = sandbox.host.clone();
        let old = version(&host, 1);
        assert_eq!(restore_of(&host, &path, &old).unwrap(), Restored::Version { kept_work: false });
        host.write(&path, model("c"));
        let ReplacementJob { run, read_back } = job_of(&host, &path, &old);
        let finish = started_then_unclear(&host, run, false);
        assert!(read_back().recv_timeout(Duration::from_secs(60)).unwrap().is_err());
        drop(finish);
        assert_eq!(host.flush_for_test(&path), Ok(()));
    }

    /// A read-back that confirms the restore before the writer has finished saying it cannot tell:
    /// its mark must not refuse later restores. The test below has the mark come first.
    #[test]
    fn a_read_back_that_confirms_first_stops_the_writers_mark() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, true)];
        let old = version(host, 1);
        let ReplacementJob { run, read_back } = job_of(host, &path, &old);
        let finish = started_then_unclear(host, run, true);
        assert_eq!(read_back().recv_timeout(Duration::from_secs(60)).unwrap(), Ok(true));
        drop(finish);
        assert_eq!(host.flush_for_test(&path), Ok(()));
        assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Same { kept_work: false });
    }

    /// A commit that cannot be told is unclear, and the writer marks it so. Read back afterwards, a
    /// version found finishes it and lets restores through again; none found keeps them refused.
    #[test]
    fn a_read_back_settles_an_unknown_restore_only_when_it_landed() {
        for lands in [false, true] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, lands)];
            let old = version(host, 1);
            let job = job_of(host, &path, &old);
            assert!(matches!(ran(&(job.run)()), Err(JobFailure::Unclear)));
            if !lands {
                // A newer version that is not this restore's says nothing about it.
                host.write(&path, model("z"));
                assert_eq!(saved(host, &path), Ok(true));
            }
            let read = (job.read_back)().recv_timeout(Duration::from_secs(60)).unwrap();
            assert_eq!(read.is_ok(), lands, "{read:?}");
            let again = restore_of(host, &path, &version(host, usize::from(lands)));
            assert_eq!(matches!(again, Err(RestoreError::OutcomeUnknown)), !lands, "{again:?}");
        }
        // Read back after the branch moved, it says nothing about the project as it is now.
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, true)];
        let job = job_of(host, &path, &version(host, 1));
        assert!(matches!(ran(&(job.run)()), Err(JobFailure::Unclear)));
        switched(&host.inner, &path);
        assert!((job.read_back)().recv_timeout(Duration::from_secs(60)).unwrap().is_err());
        assert!(matches!(restore_of(host, &path, &version(host, 1)), Err(RestoreError::OutcomeUnknown)));
    }

    /// The writer records its baseline while a read-back is under way, the read-back having taken
    /// the baseline or not yet. An earlier restore with the same message is the newest until then,
    /// and no commit of this one lands unless asked. Neither is taken for this restore finishing,
    /// and a baseline read as none says nothing even when the commit has landed since.
    #[test]
    fn a_read_back_is_not_fooled_while_the_writer_records_its_baseline() {
        for (at, lands) in [(ReadingAt::Start, false), (ReadingAt::Baseline, false), (ReadingAt::Baseline, true)] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let old = version(host, 1);
            assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Version { kept_work: false });
            host.write(&path, model("c"));
            let ReplacementJob { run, read_back } = job_of(host, &path, &old);
            let (reached, reading) = channel();
            let (go_on, hold) = channel::<()>();
            let step = move || {
                let _ = reached.send(());
                let _ = hold.recv_timeout(Duration::from_secs(30));
            };
            *lock(&host.inner.restore_faults) =
                vec![Fault::Commit("Restored the version", Told::CannotTell, lands), Fault::Reading(at, Box::new(step))];
            let read = read_back();
            reading.recv_timeout(Duration::from_secs(60)).expect("the read-back is under way");
            assert!(matches!(ran(&run()), Err(JobFailure::Unclear)));
            drop(go_on);
            assert!(read.recv_timeout(Duration::from_secs(60)).unwrap().is_err(), "at {at:?}, lands {lands}");
            assert!(matches!(restore_of(host, &path, &old), Err(RestoreError::OutcomeUnknown)), "the mark stays");
        }
    }

    /// A read-back during a switch, which shows its target before the generation moves, cannot
    /// tell: read from the target, where the same message is the newest, or from the job's branch
    /// just before the switch, where the restore did land.
    #[test]
    fn a_read_back_during_a_switch_cannot_tell() {
        for (at, lands) in [(ReadingAt::Baseline, false), (ReadingAt::Page, true)] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let message = format!("Restored the version of {SAVED_AT}");
            let on_target = format!("CALL DOLT_COMMIT('--allow-empty', '-m', '{message}', '--author', 'Archforge <archforge@localhost>')");
            lock(&host.inner.open)[0].run_on_branch("elsewhere", &on_target).unwrap();
            let ReplacementJob { run, read_back } = job_of(host, &path, &version(host, 1));
            *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, lands)];
            assert!(matches!(ran(&run()), Err(JobFailure::Unclear)));
            let (reached, reading) = channel();
            let (go_on, hold) = channel::<()>();
            let step = move || {
                let _ = reached.send(());
                let _ = hold.recv_timeout(Duration::from_secs(30));
            };
            lock(&host.inner.restore_faults).push(Fault::Reading(at, Box::new(step)));
            let read = read_back();
            reading.recv_timeout(Duration::from_secs(60)).expect("the read-back is under way");
            lock(&host.inner.open)[0].move_to_branch(branch("elsewhere"));
            drop(go_on);
            assert!(read.recv_timeout(Duration::from_secs(60)).unwrap().is_err(), "at {at:?}");
            lock(&host.inner.open)[0].move_to_branch(branch("main"));
            assert!(matches!(restore_of(host, &path, &version(host, 0)), Err(RestoreError::OutcomeUnknown)), "the mark stays");
        }
    }

    /// A restore that finishes late confirms itself and nothing else: another's mark stays.
    #[test]
    fn a_late_read_back_leaves_another_restores_mark() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (old, other) = (version(host, 1), version(host, 0));
        let ReplacementJob { run, read_back } = job_of(host, &path, &old);
        drop(started_then_unclear(host, run, false));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, false)];
        let another = job_of(host, &path, &other);
        assert!(matches!(ran(&(another.run)()), Err(JobFailure::Unclear)));
        assert_eq!(read_back().recv_timeout(Duration::from_secs(60)).unwrap(), Ok(true), "its own version is there");
        assert!(matches!(restore_of(host, &path, &old), Err(RestoreError::OutcomeUnknown)));
    }

    /// A read-back from before a close and reopen settles nothing about the project opened since.
    #[test]
    fn a_read_back_from_before_a_reopen_settles_nothing() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (old, other) = (version(host, 1), version(host, 0));
        *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, true)];
        let job = job_of(host, &path, &old);
        assert!(matches!(ran(&(job.run)()), Err(JobFailure::Unclear)));
        assert_eq!(close(host, &path), Ok(()));
        wait(&host.open(&path)).unwrap();
        *lock(&host.inner.restore_faults) = vec![Fault::Commit("Restored the version", Told::CannotTell, false)];
        let reopened = job_of(host, &path, &other);
        assert!(matches!(ran(&(reopened.run)()), Err(JobFailure::Unclear)));
        assert!((job.read_back)().recv_timeout(Duration::from_secs(60)).unwrap().is_err());
        assert!(matches!(restore_of(host, &path, &old), Err(RestoreError::OutcomeUnknown)));
    }

    /// Asks the writer for a restore of the project at `path`, naming the open instance `instance`.
    fn restore_of_instance(host: &Projects, path: &Path, commit: &str, instance: u64) -> Result<Restored, RestoreError> {
        let (reply, answer) = channel();
        let watch = Arc::new(JobWatch::default());
        writer(&host.inner)
            .send(Job::Restore(path.to_path_buf(), restoring(commit), generation_of(&host.inner, path), instance, watch, reply))
            .unwrap();
        answer.recv_timeout(Duration::from_secs(60)).expect("the restore answers")
    }

    /// Both of the routes by which a project closes.
    fn closing_routes() -> [fn(&Projects, &Path) -> Receiver<Result<(), String>>; 2] {
        [|h, p| h.close(p), |h, p| h.close_without_writing(p)]
    }

    /// A branch asked for before a close and reopen makes nothing, by either closing route; one
    /// asked for from the instance open now is made.
    #[test]
    fn a_branch_job_from_before_a_close_and_reopen_is_refused() {
        for closing in closing_routes() {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let (start, stale) = (lock(&host.inner.open)[0].head_of("main").unwrap(), instance_of(&host.inner, &path));
            assert_eq!(closing(host, &path).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            let (reply, refused) = channel();
            writer(&host.inner)
                .send(Job::CreateBranch(path.clone(), branch("stale"), Some(start.clone()), generation_of(&host.inner, &path), stale, reply))
                .unwrap();
            assert_eq!(refused.recv_timeout(Duration::from_secs(60)).unwrap(), Err(CreateBranchError::Reopened));
            assert!(!has_branch(&lock(&host.inner.open)[0], "stale"));
            assert!(!path.join(".creating-branch").exists(), "no record was left");
            let made = host.create_branch(&path, branch("fresh"), start).recv_timeout(Duration::from_secs(60)).unwrap();
            assert_eq!(made, Ok(BranchCreated::Listed));
            assert!(has_branch(&lock(&host.inner.open)[0], "fresh"));
        }
    }

    /// A branch from current work asked for before a close and reopen makes nothing and leaves the
    /// source's unsaved work alone, by either closing route; one asked for now is made.
    #[test]
    fn a_branch_job_from_current_work_from_before_a_reopen_is_refused() {
        for closing in closing_routes() {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let schema = schema(&host.inner).unwrap();
            host.write(&path, model("work"));
            assert_eq!(host.flush_for_test(&path), Ok(()));
            let stale = instance_of(&host.inner, &path);
            assert_eq!(closing(host, &path).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            let (reply, refused) = channel();
            writer(&host.inner)
                .send(Job::CreateBranch(path.clone(), branch("stale"), None, generation_of(&host.inner, &path), stale, reply))
                .unwrap();
            assert_eq!(refused.recv_timeout(Duration::from_secs(60)).unwrap(), Err(CreateBranchError::Reopened));
            {
                let open = lock(&host.inner.open);
                assert!(!has_branch(&open[0], "stale") && !path.join(".creating-branch").exists(), "nothing was made");
                assert_eq!(open[0].read_model(&schema).unwrap(), model("work"));
                assert!(open[0].has_unsaved().unwrap(), "the unsaved work is untouched");
            }
            assert_eq!(
                host.create_branch_from_work(&path, branch("fresh")).recv_timeout(Duration::from_secs(60)).unwrap(),
                Ok(BranchCreated::Listed)
            );
            assert_eq!(lock(&host.inner.open)[0].check_branch(&branch("fresh"), &schema).unwrap(), model("work"));
        }
    }

    /// A job from before a close and reopen is refused and writes nothing, by either closing route;
    /// one from the instance open now runs.
    #[test]
    fn a_restore_from_before_a_close_and_reopen_is_refused() {
        for closing in closing_routes() {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let (old, before) = (version(host, 1), instance_of(&host.inner, &path));
            assert_eq!(closing(host, &path).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            assert_eq!(instance_of(&host.inner, &path), before + 1);
            let (messages, unmoved) = (sandbox.messages(), work(host));
            assert!(matches!(restore_of_instance(host, &path, &old, before), Err(RestoreError::Reopened)));
            assert_eq!((sandbox.messages(), work(host)), (messages, unmoved));
            assert_eq!(restore_of(host, &path, &old).unwrap(), Restored::Version { kept_work: false }, "the instance open now runs");
        }
    }

    /// Another project's closing leaves this one's instance as it was.
    #[test]
    fn another_projects_close_does_not_refuse_a_restore() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let other = wait(&host.create(&sandbox.base.join("q"), "Other")).unwrap().path;
        let job = job_of(host, &path, &version(host, 1));
        assert_eq!(close(host, &other), Ok(()));
        assert!(matches!(ran(&(job.run)()), Ok(())));
        assert_eq!(work(host), model("a"));
    }

    /// Asks for a switch in two steps; gives its check's answer and the move.
    fn checked(host: &Projects, path: &Path, name: &str) -> (Result<CheckedBranch, SwitchError>, SwitchMove) {
        let SwitchSteps { check, moving } = host.start_switch(path, branch(name)).unwrap();
        (check.recv_timeout(Duration::from_secs(60)).expect("the check answers"), moving)
    }

    /// The answer of a move, once run.
    fn moved(moving: SwitchMove) -> Result<Switched, SwitchError> {
        moving.run().recv_timeout(Duration::from_secs(60)).expect("the move answers")
    }

    /// Asserts that the project is on `main`, at the generation `before`.
    #[track_caller]
    fn still_on_main(host: &Projects, path: &Path, before: u64) {
        assert_eq!((on(host).as_str(), host.generation_of(path)), ("main", before));
    }

    /// The check does everything but move: what waits is written, the conversations are committed
    /// and the target's model and unsaved state come back. The move then moves.
    #[test]
    fn a_switch_checks_first_and_moves_only_when_told() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        undone_since_the_save(host, &path);
        let before = host.generation_of(&path);
        host.write(&path, model("c"));
        let (check, moving) = checked(host, &path, "elsewhere");
        assert_eq!(moving.limit, Duration::from_secs(5 * 60));
        assert_eq!(check.unwrap(), CheckedBranch { rows: Arc::new(model("a")), unsaved: false });
        still_on_main(host, &path, before);
        assert_eq!(work(host), model("c"), "what waited was written, on the branch left");
        let chats = on_writer(&host.inner, &path, |p| p.commit_chats().map_err(|e| e.to_string()));
        assert_eq!(chats.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(false), "the conversations were committed");
        assert!(matches!(moved(moving), Ok(Switched::Moved(_))));
        assert_eq!((on(host).as_str(), host.generation_of(&path)), ("elsewhere", before + 1));
        // Unsaved work left on a branch is told when coming back to it.
        host.write(&path, model("d"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        let (back, moving) = checked(host, &path, "main");
        assert_eq!(back.unwrap(), CheckedBranch { rows: Arc::new(model("c")), unsaved: true });
        assert!(matches!(moved(moving), Ok(Switched::Moved(_))));
        let (there, _moving) = checked(host, &path, "elsewhere");
        assert_eq!(there.unwrap(), CheckedBranch { rows: Arc::new(model("d")), unsaved: true });
    }

    /// The branch the project is on, in any case, is refused before anything is written: what
    /// waits is still unwritten by the job right behind the check, ahead of any timer.
    #[test]
    fn a_switch_to_the_branch_in_use_is_refused_before_anything_is_written() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (before, was, shape) = (host.generation_of(&path), work(host), schema(&host.inner).unwrap());
        let release = hold_the_writer(host, &path);
        host.write(&path, model("c"));
        let SwitchSteps { check, moving: _moving } = host.start_switch(&path, branch("MAIN")).unwrap();
        let behind = on_writer(&host.inner, &path, move |p| p.read_model(&shape).map_err(|e| e.to_string()));
        drop(release);
        let check = check.recv_timeout(Duration::from_secs(60)).expect("the check answers");
        assert!(matches!(check, Err(SwitchError::AlreadyOnIt)), "{check:?}");
        assert_eq!(behind.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(was), "nothing was written yet");
        still_on_main(host, &path, before);
    }

    /// A failed check leaves the project where it was, and the switch can be asked for again.
    #[test]
    fn a_failed_check_leaves_the_project_on_its_branch() {
        use archforge_project::Error;
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let before = host.generation_of(&path);
        let (check, moving) = checked(host, &path, "scripted");
        assert!(matches!(check, Err(SwitchError::Checks(Error::Scripted { .. }))), "{check:?}");
        let why = check.unwrap_err();
        assert!(why.words().contains(&why.to_string()), "the words carry the reason: {why}");
        still_on_main(host, &path, before);
        drop(moving);
        let (check, _moving) = checked(host, &path, "elsewhere");
        assert!(check.is_ok(), "{check:?}");
    }

    /// A second request between the steps is refused for as long as the first holds, and goes
    /// through once its move has ended or been dropped. Another project's switch is no reason.
    #[test]
    fn a_second_switch_is_refused_while_the_first_is_under_way() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (_, first) = checked(host, &path, "elsewhere");
        assert!(matches!(host.start_switch(&path, branch("scripted")), Err(SwitchError::UnderWay)));
        assert!(host.start_switch(&sandbox.base.join("nowhere"), branch("x")).is_ok(), "another project's");
        let (running, started) = channel();
        let (go_on, hold) = channel::<()>();
        *lock(&host.inner.before_move) = Some(Box::new(move || {
            let _ = running.send(());
            let _ = hold.recv_timeout(Duration::from_secs(30));
        }));
        let reply = first.run();
        started.recv_timeout(Duration::from_secs(60)).expect("the move is under way");
        assert!(matches!(host.start_switch(&path, branch("scripted")), Err(SwitchError::UnderWay)), "during the move");
        drop(go_on);
        assert!(matches!(reply.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(Switched::Moved(_))));
        let (_, second) = checked(host, &path, "main");
        assert!(matches!(host.start_switch(&path, branch("elsewhere")), Err(SwitchError::UnderWay)));
        drop(second);
        let (check, _third) = checked(host, &path, "main");
        assert!(check.is_ok(), "{check:?}");
    }

    /// Between the steps a change of generation, or a close and reopen by either route, refuses the
    /// move by its reason, and nothing moves.
    #[test]
    fn a_move_from_before_a_change_between_the_steps_is_refused() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (_, moving) = checked(host, &path, "elsewhere");
        switched(&host.inner, &path);
        let before = host.generation_of(&path);
        assert!(matches!(moved(moving), Err(SwitchError::BranchChanged)));
        still_on_main(host, &path, before);
        for closing in closing_routes() {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let (_, moving) = checked(host, &path, "elsewhere");
            assert_eq!(closing(host, &path).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            let before = host.generation_of(&path);
            assert!(matches!(moved(moving), Err(SwitchError::Reopened)));
            still_on_main(host, &path, before);
        }
    }

    /// A check asked for before the project closed or its generation moved is refused too.
    #[test]
    fn a_check_from_before_a_change_is_refused_by_its_reason() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let release = hold_the_writer(host, &path);
        let SwitchSteps { check, .. } = host.start_switch(&path, branch("elsewhere")).unwrap();
        switched(&host.inner, &path);
        drop(release);
        assert!(matches!(check.recv_timeout(Duration::from_secs(60)).unwrap(), Err(SwitchError::BranchChanged)));
        let (generation, instance) = (host.generation_of(&path), instance_of(&host.inner, &path));
        let (mut waiting, mut failed) = (BTreeMap::new(), false);
        let stale = check_switch(&host.inner, &path, &branch("elsewhere"), (generation, instance.wrapping_sub(1)), &mut waiting, &mut failed);
        assert!(matches!(stale, Err(SwitchError::Reopened)), "{stale:?}");
        let both =
            check_switch(&host.inner, &path, &branch("elsewhere"), (generation.wrapping_sub(1), instance.wrapping_sub(1)), &mut waiting, &mut failed);
        assert!(matches!(both, Err(SwitchError::Reopened)), "a close is the reason that outlasts the rest: {both:?}");
    }

    /// A switch whose check has answered within the real limit, its move left with `limit`: the
    /// check does real work on the writer, which a short limit must not bound.
    fn checked_within(host: &Projects, path: &Path, limit: Duration) -> SwitchMove {
        let SwitchSteps { check, mut moving } = host.start_switch(path, branch("elsewhere")).unwrap();
        check.recv_timeout(Duration::from_secs(60)).expect("the check answers").expect("the check passes");
        moving.limit = limit;
        moving
    }

    /// A check that gets no answer in time says so by its reason, and the project stays.
    #[test]
    fn a_check_with_no_answer_in_time_says_so() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let before = host.generation_of(&path);
        let release = hold_the_writer(host, &path);
        let SwitchSteps { check, moving } = host.start_switch_within(&path, branch("elsewhere"), Duration::from_millis(20)).unwrap();
        let check = check.recv_timeout(Duration::from_secs(60)).expect("the wait answers");
        assert!(matches!(check, Err(SwitchError::NoAnswer)), "{check:?}");
        drop((moving, release));
        no_thread_left(host);
        still_on_main(host, &path, before);
    }

    /// A move not started when the limit passes never runs: the wait's claim stops the writer, the
    /// answer is a plain failure in words, and the switch can be asked for again.
    #[test]
    fn a_move_given_up_on_before_it_starts_never_runs() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let before = host.generation_of(&path);
        let moving = checked_within(host, &path, Duration::from_millis(20));
        let release = hold_the_writer(host, &path);
        let ReplacementJob { run, read_back } = moving.into_job();
        let answer = run().recv_timeout(Duration::from_secs(60)).expect("the wait answers");
        assert!(matches!(answer, Err(JobFailure::Failed(_))), "a plain failure");
        drop((release, read_back));
        assert_eq!(host.on_writer(&path, |_| Ok(())).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()), "the writer went past the job");
        still_on_main(host, &path, before);
        assert!(checked(host, &path, "elsewhere").0.is_ok(), "the claim was released");
    }

    /// A move that started and whose wait ended, as it does at the limit or an exit, cannot be told:
    /// its answer is unclear, the read-back says it cannot tell until the writer has finished, and
    /// then finds where it went. The wait is ended by the exit flag once the writer holds the move,
    /// as a short limit could pass before the writer reached it.
    #[test]
    fn a_move_past_its_wait_is_read_back_only_once_the_writer_has_finished() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let moving = checked_within(host, &path, RESTORE_LIMIT);
        let (running, started) = channel();
        let (go_on, hold) = channel::<()>();
        *lock(&host.inner.before_move) = Some(Box::new(move || {
            let _ = running.send(());
            let _ = hold.recv_timeout(Duration::from_secs(30));
        }));
        let ReplacementJob { run, read_back } = moving.into_job();
        let answer = run();
        started.recv_timeout(Duration::from_secs(60)).expect("the move is under way");
        host.inner.cancel.store(true, Ordering::SeqCst);
        let answer = answer.recv_timeout(Duration::from_secs(60)).expect("the wait answers");
        host.inner.cancel.store(false, Ordering::SeqCst);
        assert!(matches!(answer, Err(JobFailure::Unclear)), "unclear");
        let watch = lock(&host.inner.switches).get(&path).and_then(std::sync::Weak::upgrade).expect("still under way");
        let told = host.read_back_switch(&path, branch("elsewhere"), watch).recv_timeout(Duration::from_secs(60)).unwrap();
        assert!(told.is_err(), "not while the writer still runs it: {told:?}");
        drop(go_on);
        // Refused by the generation the move changed, which is the writer having got past it.
        host.on_writer(&path, |_| Ok(())).recv_timeout(Duration::from_secs(60)).expect("the writer finished the move").unwrap_err();
        assert_eq!(on(host), "elsewhere");
        assert_eq!(read_back().recv_timeout(Duration::from_secs(60)).unwrap(), Ok(true));
    }

    /// A finished move that stayed reads back as not moved; and the branch is read after the job is
    /// known finished, so one finishing during the read-back is not mistaken for one that stayed.
    #[test]
    fn a_switch_read_back_reads_whether_it_finished_first() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let target = branch("elsewhere");
        let finished = Arc::new(JobWatch::default());
        finished.finished.store(true, Ordering::SeqCst);
        assert_eq!(host.read_back_switch(&path, target.clone(), finished).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(false));
        let unfinished = Arc::new(JobWatch::default());
        let late = unfinished.clone();
        *lock(&host.inner.before_move) = Some(Box::new(move || late.finished.store(true, Ordering::SeqCst)));
        let told = host.read_back_switch(&path, target, unfinished).recv_timeout(Duration::from_secs(60)).unwrap();
        assert!(told.is_err(), "unfinished when first looked at: {told:?}");
    }

    /// Each reason is worded through the catalogue and only a switch that happened is no failure.
    #[test]
    fn a_switchs_reasons_are_worded() {
        use archforge_ui_egui::i18n::set_language;
        let reasons = || {
            let words = |e: SwitchError| e.words();
            [
                words(SwitchError::AlreadyOnIt),
                words(SwitchError::NoAnswer),
                words(SwitchError::BranchChanged),
                words(SwitchError::Reopened),
                words(SwitchError::UnderWay),
                words(SwitchError::NotOpen),
                words(SwitchError::Unwritten("w".to_string())),
                words(SwitchError::Failed("reason".to_string())),
            ]
        };
        let english = reasons();
        set_language("de");
        let german = reasons();
        set_language("en");
        assert!(english.iter().zip(&german).all(|(en, de)| en != de && !de.is_empty()), "{german:?}");
        assert!(german[7].contains("reason"), "the reason is told");
        assert_eq!(english[2], english[5], "one wording for changed or busy");
        assert_ne!(english[0], english[1]);
        assert_ne!(english[6], english[1]);
        assert!(matches!(switch_answer(Ok(Switched::AlreadyThere)), Ok(())));
        assert!(matches!(switch_answer(Err(SwitchError::NoAnswer)), Err(JobFailure::Failed(_))));
    }

    /// What the window starts, on the real writer: the check, the move, and refusals in words.
    #[test]
    fn the_windows_switch_forwards_the_check_and_moves() {
        let wait = Duration::from_secs(60);
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let there = |name: &str| host.begin_branch_switch(&path, name.to_string());
        host.write(&path, model("c"));
        let BranchSwitch { read, unsaved, job } = there("elsewhere").unwrap();
        assert_eq!(read.recv_timeout(wait).unwrap(), Ok(Arc::new(model("a"))));
        assert!(!unsaved.load(Ordering::SeqCst));
        let ReplacementJob { run, read_back } = job;
        assert!(matches!(run().recv_timeout(wait).unwrap(), Ok(())));
        drop(read_back);
        assert_eq!(on(host), "elsewhere");
        let same = there("ELSEWHERE").unwrap();
        assert_eq!(same.read.recv_timeout(wait).unwrap(), Err(SwitchError::AlreadyOnIt.words()));
        drop(same);
        let back = there("main").unwrap();
        assert!(back.read.recv_timeout(wait).unwrap().is_ok() && back.unsaved.load(Ordering::SeqCst), "main holds unsaved work");
        assert_eq!(there("scripted").err(), Some(SwitchError::UnderWay.words()));
    }

    /// A close asked for while either step runs on the writer waits for it: the step ends as it
    /// would have, and only then does the project close.
    #[test]
    fn a_close_during_either_step_waits_for_it() {
        for step in ["check", "move"] {
            let (sandbox, path) = branched();
            let host = sandbox.host.clone();
            let (running, started) = channel();
            let (go_on, hold) = channel::<()>();
            let held: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = running.send(());
                let _ = hold.recv_timeout(Duration::from_secs(30));
            });
            let (answer, closing) = if step == "check" {
                *lock(&host.inner.before_move) = Some(held);
                let SwitchSteps { check, moving } = host.start_switch(&path, branch("elsewhere")).unwrap();
                started.recv_timeout(Duration::from_secs(60)).expect("the check is under way");
                let closing = host.close(&path);
                assert!(closing.recv_timeout(Duration::from_millis(300)).is_err(), "the close waits for the check");
                drop(go_on);
                let answer = check.recv_timeout(Duration::from_secs(60)).unwrap().map(|_| ());
                drop(moving);
                (answer, closing)
            } else {
                let (check, moving) = checked(&host, &path, "elsewhere");
                assert!(check.is_ok());
                *lock(&host.inner.before_move) = Some(held);
                let reply = moving.run();
                started.recv_timeout(Duration::from_secs(60)).expect("the move is under way");
                let closing = host.close(&path);
                assert!(closing.recv_timeout(Duration::from_millis(300)).is_err(), "the close waits for the move");
                drop(go_on);
                (reply.recv_timeout(Duration::from_secs(60)).unwrap().map(|_| ()), closing)
            };
            assert!(answer.is_ok(), "{step}: the step finished with the project open: {answer:?}");
            assert_eq!(closing.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()), "{step}");
        }
    }

    /// A switch that stopped before it tried to write leaves the writer's timer running: rows queued
    /// just ahead of it, so no timer got to them first, are written with no further job.
    #[test]
    fn a_switch_that_wrote_nothing_leaves_the_rows_to_the_timer() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let nowhere = sandbox.base.join("nowhere");
        let timed_after = |name: &str, ask: &dyn Fn() -> Receiver<Result<Switched, SwitchError>>| {
            let release = hold_the_writer(host, &path);
            host.write(&path, model(name));
            let asked = ask();
            drop(release);
            let answer = asked.recv_timeout(Duration::from_secs(60)).expect("the switch answers");
            let deadline = Instant::now() + Duration::from_secs(30);
            while host.writing(&path).pending {
                assert!(Instant::now() < deadline, "{name}: the rows are still waiting after {answer:?}");
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(lock(&host.inner.open)[0].check_branch(&branch("main"), &schema(&host.inner).unwrap()).unwrap(), model(name));
            answer
        };
        assert!(matches!(timed_after("c", &|| host.switch_branch(&path, branch("MAIN"))), Ok(Switched::AlreadyThere)));
        assert!(matches!(timed_after("d", &|| host.switch_branch(&nowhere, branch("elsewhere"))), Err(SwitchError::NotOpen)));
        let stale = || {
            let asked = host.switch_branch(&nowhere, branch("elsewhere"));
            switched(&host.inner, &nowhere);
            asked
        };
        assert!(matches!(timed_after("e", &stale), Err(SwitchError::BranchChanged)));
        host.inner.fail_schema.store(true, Ordering::SeqCst);
        assert!(matches!(timed_after("f", &|| host.switch_branch(&path, branch("elsewhere"))), Err(SwitchError::Failed(_))));
        assert_eq!(on(host), "main");
    }

    /// A switch whose write failed leaves the writer waiting for the next change, as any failed
    /// write does (each retry would record its error again), and a later early stop leaves that.
    #[test]
    fn a_switch_whose_write_failed_stops_the_timer() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let mut broken = model("c");
        broken.tables.iter_mut().find(|t| !t.is_empty()).unwrap()[0].clear();
        let release = hold_the_writer(host, &path);
        host.write(&path, broken);
        let asked = host.switch_branch(&path, branch("elsewhere"));
        drop(release);
        assert!(matches!(asked.recv_timeout(Duration::from_secs(60)).unwrap(), Err(SwitchError::Unwritten(_))));
        assert_eq!(switch_to(host, &path, "main").unwrap(), Switched::AlreadyThere);
        lock(&host.inner.writing).get_mut(&path).unwrap().error = None;
        std::thread::sleep(SETTLE * 4);
        assert_eq!(host.writing(&path).error, None, "no timer after a failed write");
        host.write(&path, model("b"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
    }

    /// Leaves a creation record in the project folder, as a stop would.
    fn stopped_creation(path: &Path, branch: &str, source: &str, start: &str) {
        std::fs::write(path.join(".creating-branch"), format!("archforge branch creation 1\nbranch {branch}\nsource {source}\nstart {start}\n"))
            .unwrap();
    }

    /// Whether the project has a branch of exactly this name.
    fn has_branch(project: &Project, name: &str) -> bool {
        project.branches(50).unwrap().listed.iter().any(|b| b.name == name)
    }

    /// Opening cleans up after a creation that stopped, before the project is used.
    #[test]
    fn opening_cleans_up_a_creation_that_stopped() {
        let (sandbox, path) = branched();
        let start = lock(&sandbox.host.inner.open)[0].head_of("main").unwrap();
        lock(&sandbox.host.inner.open)[0].run_on_branch("main", "CALL DOLT_BRANCH('halfway')").unwrap();
        stopped_creation(&path, "halfway", "main", &start);
        close(&sandbox.host, &path).unwrap();
        wait(&sandbox.host.open(&path)).unwrap();
        assert!(!has_branch(&lock(&sandbox.host.inner.open)[0], "halfway"));
        assert!(!path.join(".creating-branch").exists());
    }

    /// On a real server, with the head and the delete real: only a branch still at its start goes.
    #[test]
    fn recovery_deletes_only_a_branch_still_at_its_start() {
        use archforge_project::CreationRecovery::{Deleted, Finished, NoBranch, NoRecord, Protected};
        let (sandbox, path) = branched();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        let start = project.head_of("main").unwrap();
        let on_branch = |name: &str, statement: &str| project.run_on_branch(name, statement).unwrap();
        let made = |name: &str| on_branch("main", &format!("CALL DOLT_BRANCH('{name}')"));
        let recover = |name: &str, source: &str| {
            stopped_creation(&path, name, source, &start);
            project.recover_creation()
        };
        // Stopped before the branch was made, and after it, with rows the delete does not warn of.
        assert_eq!(recover("notmade", "main"), NoBranch(branch("notmade")));
        made("halfway");
        on_branch("halfway", "CREATE TABLE unsaved (a INT PRIMARY KEY)");
        assert_eq!(recover("halfway", "main"), Deleted(branch("halfway")));
        assert!(!has_branch(project, "halfway"));
        assert_eq!(project.recover_creation(), NoRecord, "the record went");
        // Made from a branch that is not merged anywhere, which only a forced delete can remove.
        made("topic");
        on_branch("topic", "CALL DOLT_COMMIT('--allow-empty', '-m', 't', '--author', 'A Person <a@person>')");
        on_branch("topic", "CALL DOLT_BRANCH('fromtopic')");
        stopped_creation(&path, "fromtopic", "topic", &project.head_of("topic").unwrap());
        assert_eq!(project.recover_creation(), Deleted(branch("fromtopic")));
        // A record that came back: a later Save or the chat commit that finishes a creation.
        for (name, author) in [("saved", "A Person <a@person>"), ("chatted", archforge_project::CHAT_AUTHOR)] {
            made(name);
            on_branch(name, &format!("CALL DOLT_COMMIT('--allow-empty', '-m', 'm', '--author', '{author}')"));
            assert_eq!(recover(name, "main"), Finished(branch(name)));
            assert!(has_branch(project, name));
        }
        // A name that differs only in case reaches no branch; protected names stay in any case.
        made("cased");
        assert_eq!(recover("CASED", "main"), NoBranch(branch("CASED")));
        assert!(has_branch(project, "cased"));
        assert_eq!(recover("MAIN", "main"), Protected(branch("MAIN")));
        made("opening");
        project.set_current_branch(branch("OPENING"));
        assert_eq!(recover("opening", "main"), Protected(branch("opening")));
        project.set_current_branch(branch("main"));
        assert!(has_branch(project, "opening") && has_branch(project, "main"));
    }

    /// How many rows `table` holds on `branch`.
    fn rows_on(project: &Project, branch: &str, table: &str) -> u64 {
        project.first_on_branch(branch, &format!("SELECT CAST(COUNT(*) AS CHAR) FROM `{table}`")).unwrap().parse().unwrap()
    }

    /// A new branch has no conversations and the version's model, whatever was inherited, and its
    /// one chat commit is not a version; the branch it came from is untouched.
    #[test]
    fn a_new_branch_starts_at_the_version_with_no_conversations() {
        let (sandbox, path) = branched();
        undone_since_the_save(&sandbox.host, &path);
        assert_eq!(sandbox.host.flush_for_test(&path), Ok(()));
        let schema = schema(&sandbox.host.inner).unwrap();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        let (held, old) = (project.head_of("main").unwrap(), project.head_of("elsewhere").unwrap());
        let chats = rows_on(project, "main", "chat_turn");
        assert!(chats > 0, "there is something to inherit");
        project.create_branch(&branch("inherits"), &held, &schema).unwrap();
        for table in archforge_project::CHAT_TABLES {
            assert_eq!(rows_on(project, "inherits", table), 0, "{table}");
        }
        assert_eq!(project.check_branch(&branch("inherits"), &schema).unwrap(), project.check_branch(&branch("main"), &schema).unwrap());
        assert_eq!((project.head_of("main").unwrap(), rows_on(project, "main", "chat_turn")), (held.clone(), chats), "the source is untouched");
        // A version with nothing to inherit: the commit is an empty one, and made all the same.
        project.create_branch(&branch("empty"), &old, &schema).unwrap();
        assert_eq!(project.check_branch(&branch("empty"), &schema).unwrap(), model("a"));
        // The creation commit, which has no model tables: they are prepared on the new branch.
        let first = project.first_on_branch("main", "SELECT CAST(commit_hash AS CHAR) FROM dolt_log WHERE message = 'Create the project'").unwrap();
        project.create_branch(&branch("first"), &first, &schema).unwrap();
        assert!(project.first_on_branch("first", "SELECT CAST(COUNT(*) AS CHAR) FROM `model_tables`").is_ok(), "prepared");
        for (name, start) in [("inherits", &held), ("empty", &old), ("first", &first)] {
            assert_ne!(&project.head_of(name).unwrap(), start, "{name}: finished, so past its start");
            let parent = format!("SELECT CAST(parent_hash AS CHAR) FROM dolt_commit_ancestors WHERE commit_hash = HASHOF('{name}')");
            assert_eq!(&project.first_on_branch(name, &parent).unwrap(), start, "{name}: one commit on its start");
            let email = project.first_on_branch(name, "SELECT CAST(email AS CHAR) FROM dolt_log LIMIT 1").unwrap();
            assert!(!archforge_project::is_version(&email), "{name}: left out of History");
            // What was removed is in that commit; only the tables prepared on `first` are unsaved.
            let dirty = format!("SELECT CAST(dirty AS CHAR) FROM dolt_branches WHERE name = '{name}'");
            assert_eq!(project.first_on_branch("main", &dirty).unwrap() == "1", name == "first", "{name}");
        }
        assert!(!path.join(".creating-branch").exists());
    }

    /// A stop just before each step, then the real recovery on reopening: the branch goes up to
    /// the chat commit, and is kept once that is made.
    #[test]
    fn a_creation_that_stops_is_cleaned_up_only_before_its_chat_commit() {
        use archforge_project::CreationStep::{ChatCommit, MakeBranch, PrepareModel, RemoveConversations, RemoveRecord};
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let reopened = || {
            close(host, &path).unwrap();
            wait(&host.open(&path)).unwrap();
        };
        let name = |n: usize| format!("stop{n}");
        for (n, (step, kept)) in [(MakeBranch, false), (PrepareModel, false), (RemoveConversations, false), (ChatCommit, false), (RemoveRecord, true)]
            .into_iter()
            .enumerate()
        {
            {
                let open = lock(&host.inner.open);
                let start = open[0].head_of("main").unwrap();
                assert!(open[0].create_branch_halted(&branch(&name(n)), Some(&start), &schema(&host.inner).unwrap(), (step, false)).is_err());
                assert!(path.join(".creating-branch").exists(), "{step:?}: a stop leaves the record");
            }
            reopened();
            assert_eq!(has_branch(&lock(&host.inner.open)[0], &name(n)), kept, "{step:?}");
            assert!(!path.join(".creating-branch").exists(), "{step:?}");
        }
        // Finished, and then the record came back: only the record goes.
        let start = lock(&host.inner.open)[0].head_of("main").unwrap();
        lock(&host.inner.open)[0].create_branch(&branch("finished"), &start, &schema(&host.inner).unwrap()).unwrap();
        stopped_creation(&path, "finished", "main", &start);
        reopened();
        assert!(has_branch(&lock(&host.inner.open)[0], "finished"));
    }

    /// A version holding a script is refused by what it holds, before the record or the branch, so
    /// that nothing that could fire it is ever run. Each is committed at the version.
    #[test]
    fn a_version_with_a_script_is_refused_before_anything_is_written() {
        let (sandbox, path) = branched();
        undone_since_the_save(&sandbox.host, &path);
        let schema = schema(&sandbox.host.inner).unwrap();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        let rows = [
            ("trigger", "CREATE TRIGGER t BEFORE DELETE ON chat FOR EACH ROW BEGIN SIGNAL SQLSTATE '45000'; END", "triggers"),
            ("procedure", "CREATE PROCEDURE p() SELECT 1", "stored procedures"),
            ("event", "CREATE EVENT e ON SCHEDULE EVERY 1 YEAR DO SELECT 1", "events"),
        ];
        for (name, script, what) in rows {
            project.run_on_branch("main", &format!("CALL DOLT_BRANCH('{name}')")).unwrap();
            project.run_on_branch(name, script).unwrap();
            project.run_on_branch(name, "CALL DOLT_COMMIT('-A', '-m', 'x', '--author', 'A Person <a@person>')").unwrap();
            let made = format!("from{name}");
            let answer = project.create_branch(&branch(&made), &project.head_of(name).unwrap(), &schema);
            assert_eq!(answer, Err(archforge_project::BranchCreationError::Scripted { what }), "{name}");
            assert!(!has_branch(project, &made) && !path.join(".creating-branch").exists(), "{name}: nothing was made");
        }
    }

    /// After the chat commit the branch is finished: a record that stays hides it until the next
    /// opening, which removes the record and leaves the branch.
    #[test]
    fn a_record_that_cannot_be_removed_leaves_a_finished_branch() {
        use archforge_project::CreationStep::RemoveRecord;
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        {
            let open = lock(&host.inner.open);
            let start = open[0].head_of("main").unwrap();
            let answer = open[0].create_branch_halted(&branch("kept"), Some(&start), &schema(&host.inner).unwrap(), (RemoveRecord, true));
            assert!(matches!(answer, Ok(BranchCreated::RecordStays { .. })), "{answer:?}");
            assert_ne!(open[0].head_of("kept").unwrap(), start, "it was not undone");
            assert!(path.join(".creating-branch").exists() && !has_branch(&open[0], "kept"), "hidden while the record stays");
        }
        close(host, &path).unwrap();
        wait(&host.open(&path)).unwrap();
        assert!(has_branch(&lock(&host.inner.open)[0], "kept") && !path.join(".creating-branch").exists());
    }

    /// The list leaves out what a good record names, before the cap, so `cut` counts only what
    /// could be listed; a damaged record says nothing, and the branch in use always shows.
    #[test]
    fn the_list_leaves_out_a_recorded_branch_before_the_cap() {
        let (sandbox, path) = branched();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        let start = project.head_of("main").unwrap();
        let names = |max: usize| {
            let list = project.branches(max).unwrap();
            (list.listed.iter().map(|b| b.name.clone()).collect::<Vec<_>>(), list.cut)
        };
        let cut = |names: &[&str], cut: bool| (names.iter().map(ToString::to_string).collect::<Vec<_>>(), cut);
        assert_eq!(names(2), cut(&["elsewhere", "main"], true));
        stopped_creation(&path, "ELSEWHERE", "main", &start);
        assert_eq!(names(2), cut(&["main", "scripted"], false), "left out in any case, and not counted");
        stopped_creation(&path, "MAIN", "elsewhere", &start);
        assert_eq!(names(3), cut(&["elsewhere", "main", "scripted"], false), "the branch in use");
        std::fs::write(path.join(".creating-branch"), b"broken").unwrap();
        assert_eq!(names(3), cut(&["elsewhere", "main", "scripted"], false), "a damaged record names nothing");
        std::fs::remove_file(path.join(".creating-branch")).unwrap();
    }

    /// The writer makes the branch, and refuses a job from before a switch or for no open project.
    #[test]
    fn the_writer_makes_a_branch_and_refuses_a_stale_job() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let start = lock(&host.inner.open)[0].head_of("main").unwrap();
        let ask = |at: &Path, name: &str| host.create_branch(at, branch(name), start.clone());
        let answer = |asked: Receiver<_>| asked.recv_timeout(Duration::from_secs(60)).expect("the job answers");
        assert_eq!(answer(ask(&path, "fresh")), Ok(BranchCreated::Listed));
        assert!(has_branch(&lock(&host.inner.open)[0], "fresh"));
        assert_eq!(answer(ask(&path, "FRESH")), Err(CreateBranchError::Creation(BranchCreationError::NameTaken)));
        assert_eq!(answer(ask(&sandbox.base.join("nowhere"), "lost")), Err(CreateBranchError::NotOpen));
        let release = hold_the_writer(host, &path);
        let asked = ask(&path, "stale");
        switched(&host.inner, &path);
        drop(release);
        assert_eq!(answer(asked), Err(CreateBranchError::BranchChanged));
        assert!(!has_branch(&lock(&host.inner.open)[0], "stale"));
        assert!(!path.join(".creating-branch").exists());
    }

    /// A rename moves the branch with what it holds, and a branch in use stays current under its
    /// new name, with what was waiting and no change of generation.
    #[test]
    fn a_rename_takes_what_the_branch_holds_and_the_current_branch_follows() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let schema = schema(&host.inner).unwrap();
        {
            let open = lock(&host.inner.open);
            let head = open[0].head_of("elsewhere").unwrap();
            open[0].run_on_branch("elsewhere", "CREATE TABLE unsaved (a INT PRIMARY KEY)").unwrap();
            assert_eq!(open[0].rename_branch(&branch("elsewhere"), &branch("other")), Ok(()));
            assert!(has_branch(&open[0], "other") && !has_branch(&open[0], "elsewhere"));
            assert_eq!(open[0].head_of("other").unwrap(), head);
            assert_eq!(
                open[0].first_on_branch("main", "SELECT CAST(dirty AS CHAR) FROM dolt_branches WHERE name = 'other'").unwrap(),
                "1",
                "its work went with it"
            );
            assert_eq!(open[0].current_branch().stored(), "main", "another branch's rename moves nothing here");
        }
        switch_to(host, &path, "other").unwrap();
        let generation = host.generation_of(&path);
        let release = hold_the_writer(host, &path);
        host.write(&path, model("waited"));
        let asked = host.rename_branch(&path, branch("other"), branch("again"));
        drop(release);
        assert_eq!(asked.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
        assert_eq!((on(host).as_str(), host.generation_of(&path)), ("again", generation));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        assert_eq!(lock(&host.inner.open)[0].check_branch(&branch("again"), &schema).unwrap(), model("waited"));
        // The current branch may be held in another case, which `USE` accepts: it follows too.
        let open = lock(&host.inner.open);
        open[0].set_current_branch(branch("AGAIN"));
        assert_eq!(open[0].rename_branch(&branch("again"), &branch("third")), Ok(()));
        assert_eq!(open[0].current_branch().stored(), "third");
    }

    /// Each refusal by its own reason, with nothing renamed.
    #[test]
    fn a_rename_is_refused_by_its_own_reason() {
        use archforge_project::RenameBranchError::{Main, NameTaken, NoSuchBranch, RecordRemains};
        let (sandbox, path) = branched();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        let start = project.head_of("main").unwrap();
        let rename = |from: &str, to: &str| project.rename_branch(&branch(from), &branch(to));
        let rows = [
            ("main", "x", Main),
            ("MAIN", "x", Main),
            ("Elsewhere", "x", NoSuchBranch),
            ("nowhere", "x", NoSuchBranch),
            ("elsewhere", "scripted", NameTaken),
            ("elsewhere", "SCRIPTED", NameTaken),
            ("elsewhere", "MAIN", NameTaken),
            ("elsewhere", "ELSEWHERE", NameTaken),
            ("elsewhere", "elsewhere", NameTaken),
        ];
        for (from, to, why) in rows {
            assert_eq!(rename(from, to), Err(why), "{from} to {to}");
        }
        // A record's name is held, though the list leaves it out, in any case and from either side.
        stopped_creation(&path, "held", "main", &start);
        assert_eq!(rename("elsewhere", "HELD"), Err(RecordRemains));
        stopped_creation(&path, "elsewhere", "main", &start);
        assert_eq!(rename("elsewhere", "free"), Err(RecordRemains));
        std::fs::write(path.join(".creating-branch"), b"broken").unwrap();
        assert_eq!(rename("scripted", "renamed"), Ok(()), "a damaged record names nothing");
        std::fs::remove_file(path.join(".creating-branch")).unwrap();
        assert!(has_branch(project, "elsewhere") && has_branch(project, "renamed"), "only that one was renamed");
    }

    /// Dolt said yes and the check after it broke; the reply lost, landed; lost, not landed: the
    /// next write and the next switch carry on under the name that is true.
    #[test]
    fn a_rename_whose_outcome_is_known_carries_on_under_the_true_name() {
        for (lands, answered, readable, name) in [(true, true, false, "moved"), (true, false, true, "moved"), (false, false, true, "elsewhere")] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let schema = schema(&host.inner).unwrap();
            switch_to(host, &path, "elsewhere").unwrap();
            let answer = lock(&host.inner.open)[0].rename_branch_faulted(&branch("elsewhere"), &branch("moved"), lands, answered, readable);
            assert_eq!(answer.is_ok(), lands, "{answer:?}");
            assert!(lands || matches!(answer, Err(RenameBranchError::Failed(_))), "{answer:?}");
            assert_eq!(on(host), name);
            host.write(&path, model("next"));
            assert_eq!(host.flush_for_test(&path), Ok(()), "the next write goes through");
            assert_eq!(lock(&host.inner.open)[0].check_branch(&branch(name), &schema).unwrap(), model("next"));
            assert!(matches!(switch_to(host, &path, "main"), Ok(Switched::Moved(_))));
        }
    }

    /// Neither the reply nor where the branch is can be had: nothing is written to either name and
    /// a switch is refused, even with nothing waiting, until the same rename, asked again, settles
    /// it or the project closes. Another branch's unclear rename holds nothing of this one.
    #[test]
    fn a_rename_that_cannot_be_told_holds_the_project_until_it_is_settled() {
        let unclear = |answer: Result<(), RenameBranchError>| assert!(matches!(answer, Err(RenameBranchError::CannotTell(_))), "{answer:?}");
        let (from, to) = (branch("elsewhere"), branch("moved"));
        for (lands, settled) in [(true, "moved"), (false, "moved")] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let schema = schema(&host.inner).unwrap();
            switch_to(host, &path, "elsewhere").unwrap();
            unclear(lock(&host.inner.open)[0].rename_branch_faulted(&from, &to, lands, false, false));
            host.write(&path, model("held"));
            assert!(host.flush_for_test(&path).is_err() && host.writing(&path).pending, "not written to either name");
            assert!(matches!(switch_to(host, &path, "main"), Err(SwitchError::Unwritten(_))), "the next switch is refused");
            unclear(lock(&host.inner.open)[0].rename_branch_faulted(&from, &to, lands, false, false));
            unclear(lock(&host.inner.open)[0].rename_branch(&branch("scripted"), &branch("other")));
            assert_eq!(lock(&host.inner.open)[0].rename_branch(&from, &to), Ok(()), "asked again, it is settled");
            assert_eq!(on(host), settled);
            assert_eq!(host.flush_for_test(&path), Ok(()), "what waited now has somewhere to go");
            assert_eq!(lock(&host.inner.open)[0].check_branch(&to, &schema).unwrap(), model("held"));
        }
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        switch_to(host, &path, "elsewhere").unwrap();
        unclear(lock(&host.inner.open)[0].rename_branch_faulted(&from, &to, false, false, false));
        let answer = switch_to(host, &path, "main");
        assert!(matches!(answer, Err(SwitchError::Checks(archforge_project::Error::BranchUnclear { .. }))), "{answer:?}");
        close(host, &path).unwrap();
        wait(&host.open(&path)).unwrap();
        host.write(&path, model("after"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        let open = lock(&host.inner.open);
        unclear(open[0].rename_branch_faulted(&branch("scripted"), &to, true, false, false));
        assert!(open[0].has_unsaved().is_ok(), "the branch in use is not held");
        assert_eq!(open[0].rename_branch(&branch("scripted"), &to), Ok(()), "and a retry does not fail");
    }

    /// The writer's job is refused when it comes from before a switch or a reopen, or finds no project.
    #[test]
    fn a_rename_job_from_before_a_switch_or_a_reopen_is_refused() {
        for closing in closing_routes() {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let answer = |asked: Receiver<Result<(), RenameJobError>>| asked.recv_timeout(Duration::from_secs(60)).expect("the job answers");
            assert_eq!(answer(host.rename_branch(&sandbox.base.join("nowhere"), branch("a"), branch("b"))), Err(RenameJobError::NotOpen));
            let stale = instance_of(&host.inner, &path);
            assert_eq!(closing(host, &path).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            let (reply, refused) = channel();
            writer(&host.inner)
                .send(Job::RenameBranch(path.clone(), branch("elsewhere"), branch("x"), generation_of(&host.inner, &path), stale, reply))
                .unwrap();
            assert_eq!(answer(refused), Err(RenameJobError::Reopened));
            let release = hold_the_writer(host, &path);
            let asked = host.rename_branch(&path, branch("elsewhere"), branch("y"));
            switched(&host.inner, &path);
            drop(release);
            assert_eq!(answer(asked), Err(RenameJobError::BranchChanged));
            assert!(has_branch(&lock(&host.inner.open)[0], "elsewhere"), "nothing was renamed");
            assert_eq!(answer(host.rename_branch(&path, branch("elsewhere"), branch("z"))), Ok(()));
        }
    }

    /// The counts are the target's, read while another branch is current, and that branch differs
    /// from it in versions, conversations and unsaved work.
    #[test]
    fn the_counts_are_those_of_the_branch_asked_about() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let counts = |name: &str| host.branch_counts(&path, branch(name)).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        let on_it = |name: &str, statement: &str| lock(&host.inner.open)[0].run_on_branch(name, statement).unwrap();
        let commit = |name: &str, author: &str| on_it(name, &format!("CALL DOLT_COMMIT('--allow-empty', '-m', 'x', '--author', '{author}')"));
        let main = BranchCounts { versions_only_here: 1, conversations: 0, unsaved: false };
        assert_eq!(counts("main"), main, "its second save");
        assert_eq!(counts("elsewhere"), BranchCounts { versions_only_here: 0, conversations: 0, unsaved: false }, "its head is on main too");
        // Conversations made on the target, whose chat commits are no versions.
        switch_to(host, &path, "elsewhere").unwrap();
        undone_since_the_save(host, &path);
        assert_eq!(host.flush_for_test(&path), Ok(()));
        switch_to(host, &path, "main").unwrap();
        let base = counts("elsewhere");
        assert_eq!((base.conversations, counts("main")), (1, main.clone()));
        commit("elsewhere", "A Person <a@person>");
        let first = lock(&host.inner.open)[0].head_of("elsewhere").unwrap();
        commit("elsewhere", archforge_project::CHAT_AUTHOR);
        commit("elsewhere", "A Person <a@person>");
        assert_eq!(counts("elsewhere").versions_only_here, base.versions_only_here + 2, "a chat commit is no version");
        // Another branch holding the first of them, under a name outside the safe set.
        on_it("main", &format!("CALL DOLT_BRANCH('odd+name', '{first}')"));
        assert!(SafeBranch::found("odd+name").is_err());
        assert_eq!(counts("elsewhere").versions_only_here, 1, "only what no other branch has");
        // Only the model's changes are unsaved: a conversation's are not.
        on_it("elsewhere", "UPDATE chat SET title = 'not committed'");
        assert!(!counts("elsewhere").unsaved);
        on_it("elsewhere", "CREATE TABLE unsaved (a INT PRIMARY KEY)");
        assert!(counts("elsewhere").unsaved);
        assert_eq!(counts("main"), main, "nothing of that is the current branch's");
        // The other way round: asked from the branch that has the conversations.
        lock(&host.inner.open)[0].set_current_branch(branch("elsewhere"));
        assert_eq!(counts("main"), main);
    }

    /// With no other branch to leave out, its whole line counts, less the commits never shown.
    #[test]
    fn the_counts_of_the_only_branch_need_no_other() {
        let sandbox = Sandbox::new(None);
        let path = sandbox.project();
        sandbox.host.write(&path, model("a"));
        assert_eq!(saved(&sandbox.host, &path), Ok(true));
        let counts = sandbox.host.branch_counts(&path, branch("main")).recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
        assert_eq!(counts, BranchCounts { versions_only_here: 1, conversations: 0, unsaved: false });
    }

    /// A chat seeded on `name`, a branch of its own, with the project back on `main`.
    fn chat_on(host: &Projects, path: &Path, name: &str) -> Uuid {
        switch_to(host, path, name).unwrap();
        let (chat, _) = undone_since_the_save(host, path);
        assert_eq!(host.flush_for_test(path), Ok(()));
        switch_to(host, path, "main").unwrap();
        chat
    }

    /// The branch goes and its own conversations are answered; one shared with a branch made
    /// outside Archforge is not, until the last branch holding it goes.
    #[test]
    fn a_deleted_branch_answers_only_the_conversations_no_other_branch_holds() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let chat = chat_on(host, &path, "elsewhere");
        let on_main = |statement: &str| lock(&host.inner.open)[0].run_on_branch("main", statement).unwrap();
        on_main("CALL DOLT_BRANCH('outside', 'elsewhere')");
        let delete = |name: &str| lock(&host.inner.open)[0].delete_branch(&branch(name));
        assert_eq!(delete("elsewhere"), Ok(Vec::new()), "the branch made outside still holds it");
        assert!(!has_branch(&lock(&host.inner.open)[0], "elsewhere"));
        // A name outside the safe set holds it just as well, and the last holder answers it.
        on_main("CALL DOLT_BRANCH('odd+name', 'outside')");
        on_main("CALL DOLT_BRANCH('last', 'outside')");
        assert_eq!(delete("outside"), Ok(Vec::new()));
        on_main("CALL DOLT_BRANCH('-D', 'odd+name')");
        assert_eq!(delete("last"), Ok(vec![chat]));
    }

    /// Each refusal by its own reason, in each case, and no delete is asked for.
    #[test]
    fn a_delete_is_refused_by_its_own_reason_and_asks_nothing() {
        use archforge_project::DeleteBranchError::{Current, Held, Main, NoSuchBranch, RecordRemains};
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        switch_to(host, &path, "elsewhere").unwrap();
        let open = lock(&host.inner.open);
        let project = &open[0];
        let asked = std::cell::Cell::new(false);
        let delete = |name: &str| project.delete_branch_faulted(&branch(name), true, true, &asked);
        let start = project.head_of("main").unwrap();
        for (name, why) in [
            ("main", Main),
            ("MAIN", Main),
            ("Main", Main),
            ("elsewhere", Current),
            ("ELSEWHERE", Current),
            ("Scripted", NoSuchBranch),
            ("nowhere", NoSuchBranch),
        ] {
            assert_eq!(delete(name), Err(why), "{name}");
        }
        for recorded in ["scripted", "SCRIPTED"] {
            stopped_creation(&path, recorded, "main", &start);
            assert_eq!(delete("scripted"), Err(RecordRemains), "{recorded}");
        }
        std::fs::remove_file(path.join(".creating-branch")).unwrap();
        assert!(!asked.get() && has_branch(project, "scripted"), "no delete was asked for");
        assert!(project.rename_branch_faulted(&branch("elsewhere"), &branch("moved"), false, false, false).is_err());
        assert_eq!(delete("scripted"), Err(Held));
        assert!(!asked.get());
    }

    /// The read decides. Still there is a failure with no ids; gone answers them; and what cannot
    /// be told answers none, until the same delete asked again says, whichever way it went.
    #[test]
    fn a_delete_is_read_back_and_a_second_try_is_safe() {
        for (lands, then_there) in [(true, false), (false, true)] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let chat = chat_on(host, &path, "elsewhere");
            let open = lock(&host.inner.open);
            let project = &open[0];
            let (target, asked) = (branch("elsewhere"), std::cell::Cell::new(false));
            let answer = project.delete_branch_faulted(&target, false, true, &asked);
            assert!(matches!(answer, Err(DeleteBranchError::Failed(_))) && has_branch(project, "elsewhere"), "not landed: {answer:?}");
            let answer = project.delete_branch_faulted(&target, lands, false, &asked);
            assert!(matches!(answer, Err(DeleteBranchError::CannotTell(_))), "{answer:?}");
            assert_eq!(has_branch(project, "elsewhere"), then_there);
            assert!(
                matches!(project.delete_branch_faulted(&target, true, false, &asked), Err(DeleteBranchError::CannotTell(_))),
                "still cannot tell"
            );
            assert_eq!(project.delete_branch(&target), Ok(vec![chat]), "the ids, whichever way it went");
            assert!(!has_branch(project, "elsewhere"));
            assert_eq!(project.delete_branch(&target), Err(DeleteBranchError::NoSuchBranch));
        }
    }

    /// While a delete is unclear nothing may change what its name means, and nothing else is held;
    /// once it is settled all of that is allowed again. Asked in another case, it reads the branch
    /// it recorded, so a name that merely differs in case does not read as the branch being gone.
    #[test]
    fn an_unclear_delete_keeps_its_name_from_changing_meaning() {
        use archforge_project::BranchCreationError::NameTaken;
        use archforge_project::RenameBranchError::CannotTell as Unsettled;
        for lands in [false, true] {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let chat = chat_on(host, &path, "elsewhere");
            let schema = schema(&host.inner).unwrap();
            let open = lock(&host.inner.open);
            let project = &open[0];
            let (target, other, asked) = (branch("elsewhere"), branch("scripted"), std::cell::Cell::new(false));
            let start = project.head_of("main").unwrap();
            assert!(matches!(project.delete_branch_faulted(&target, lands, false, &asked), Err(DeleteBranchError::CannotTell(_))));
            for from in ["elsewhere", "ELSEWHERE"] {
                assert!(matches!(project.rename_branch(&branch(from), &branch("moved")), Err(Unsettled(_))), "{from}");
            }
            for to in ["elsewhere", "Elsewhere"] {
                assert!(matches!(project.rename_branch(&other, &branch(to)), Err(Unsettled(_))), "{to}");
                assert_eq!(project.create_branch(&branch(to), &start, &schema), Err(NameTaken), "{to}");
            }
            assert!(matches!(project.delete_branch(&other), Err(DeleteBranchError::CannotTell(_))), "not recorded over it");
            assert!(has_branch(project, "scripted"));
            assert!(project.write_model(&schema, &model("saved on")).is_ok(), "autosave is not held");
            // Settled, the first delete answers its own ids, and in another case reads what it recorded.
            let variant = project.delete_branch(&branch("ELSEWHERE"));
            if lands {
                assert_eq!(variant, Ok(vec![chat]));
            } else {
                assert_eq!(variant, Err(DeleteBranchError::NoSuchBranch), "still there, under its exact name");
                assert_eq!(project.delete_branch(&target), Ok(vec![chat]));
            }
            assert_eq!(project.create_branch(&target, &start, &schema), Ok(BranchCreated::Listed));
            assert_eq!(project.rename_branch(&target, &branch("moved")), Ok(()));
            assert_eq!(project.rename_branch(&other, &target), Ok(()));
            assert_eq!(project.delete_branch(&branch("moved")), Ok(Vec::new()));
        }
    }

    /// Asked again once its branch is the one in use, an unclear delete is settled all the same,
    /// so that the name is free to change again, and it still deletes nothing there.
    #[test]
    fn an_unclear_delete_is_settled_even_on_the_branch_now_in_use() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (target, asked) = (branch("elsewhere"), std::cell::Cell::new(false));
        let unclear = lock(&host.inner.open)[0].delete_branch_faulted(&target, false, false, &asked);
        assert!(matches!(unclear, Err(DeleteBranchError::CannotTell(_))), "{unclear:?}");
        asked.set(false);
        switch_to(host, &path, "elsewhere").unwrap();
        let open = lock(&host.inner.open);
        let still = open[0].delete_branch_faulted(&target, true, false, &asked);
        assert!(matches!(still, Err(DeleteBranchError::CannotTell(_))), "not settled yet: {still:?}");
        assert_eq!(open[0].delete_branch_faulted(&target, true, true, &asked), Err(DeleteBranchError::Current));
        assert!(!asked.get() && has_branch(&open[0], "elsewhere"), "no delete was asked for");
        assert_eq!(open[0].rename_branch(&target, &branch("moved")), Ok(()), "settled, so the name may change");
    }

    /// A read on the branch when it goes fails; it does not answer for anything else.
    #[test]
    fn a_read_in_flight_on_a_deleted_branch_only_fails() {
        let (sandbox, _path) = branched();
        let open = lock(&sandbox.host.inner.open);
        let target = branch("elsewhere");
        let read = open[0].version_reader().branch_counts_interrupted(&target, || assert_eq!(open[0].delete_branch(&target), Ok(Vec::new())));
        assert!(read.is_err(), "{read:?}");
    }

    /// Forgetting one project's rows, by Close without writing or by Don't save, leaves alone what
    /// the writer knows of another's. With nothing left waiting, new rows are written on the timer.
    #[test]
    fn forgetting_one_projects_rows_says_nothing_of_anothers() {
        let sandbox = Sandbox::new(None);
        let (host, path) = (&sandbox.host, sandbox.project());
        let other = wait(&host.create(&sandbox.base.join("p"), "Other")).unwrap().path;
        host.write(&path, model("a"));
        assert_eq!(saved(host, &path), Ok(true));
        let mut broken = model("c");
        broken.tables.iter_mut().find(|t| !t.is_empty()).unwrap()[0].clear();
        let send = |job: Job| writer(&host.inner).send(job).unwrap();
        let peek = || {
            let (reply, answer) = channel();
            send(Job::Peek(reply));
            answer.recv_timeout(Duration::from_secs(60)).unwrap()
        };
        type Done = Box<dyn FnOnce()>;
        let forget: [&dyn Fn() -> Done; 2] = [
            &|| {
                let (reply, answer) = channel();
                send(Job::Drop(path.clone(), reply));
                Box::new(move || answer.recv_timeout(Duration::from_secs(60)).unwrap())
            },
            &|| {
                let (reply, answer) = channel();
                send(Job::Discard(path.clone(), Arc::new(AtomicBool::new(false)), reply));
                Box::new(move || assert_eq!(answer.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(())))
            },
        ];
        for job in forget {
            // Queued behind a held writer, so that no timer runs between these jobs.
            let release = hold_the_writer(host, &path);
            host.write(&other, model("waits"));
            let done = job();
            let not_failed = peek_queued(host);
            drop(release);
            done();
            assert!(!not_failed.recv_timeout(Duration::from_secs(60)).unwrap(), "no write was tried, so none failed");
            assert_eq!(host.flush_for_test(&other), Ok(()));
            // Another project's rows that could not be written: they wait for the next change.
            let release = hold_the_writer(host, &path);
            host.write(&other, broken.clone());
            let (reply, flushed) = channel();
            send(Job::Flush(other.clone(), reply));
            let done = job();
            let still_failed = peek_queued(host);
            drop(release);
            done();
            assert!(flushed.recv_timeout(Duration::from_secs(60)).unwrap().is_err());
            assert!(still_failed.recv_timeout(Duration::from_secs(60)).unwrap(), "their write failed, and no other was tried");
            // Those rows forgotten as well, nothing waits; what comes next is written on the timer.
            let (reply, gone) = channel();
            send(Job::Drop(other.clone(), reply));
            gone.recv_timeout(Duration::from_secs(60)).unwrap();
            assert!(peek(), "left as it was, with nothing waiting");
            let release = hold_the_writer(host, &path);
            host.write(&path, model("next"));
            let next = peek_queued(host);
            drop(release);
            assert!(!next.recv_timeout(Duration::from_secs(60)).unwrap(), "the value left over does not hold back the next rows");
            assert_eq!(host.flush_for_test(&path), Ok(()));
        }
    }

    /// Asks the writer, behind whatever is queued, whether it takes the last write to have failed.
    fn peek_queued(host: &Projects) -> Receiver<bool> {
        let (reply, answer) = channel();
        writer(&host.inner).send(Job::Peek(reply)).unwrap();
        answer
    }

    /// The writer's job is refused from before a reopen, by either closing route, from before a
    /// switch, by either route that moves the project, and with no project; one asked for now runs.
    #[test]
    fn a_delete_job_from_before_a_switch_or_a_reopen_is_refused() {
        for closing in closing_routes() {
            let (sandbox, path) = branched();
            let host = &sandbox.host;
            let answer = |asked: Receiver<Result<Vec<Uuid>, DeleteJobError>>| asked.recv_timeout(Duration::from_secs(60)).expect("the job answers");
            assert_eq!(answer(host.delete_branch(&sandbox.base.join("nowhere"), branch("a"))), Err(DeleteJobError::NotOpen));
            let stale = instance_of(&host.inner, &path);
            assert_eq!(closing(host, &path).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(()));
            wait(&host.open(&path)).unwrap();
            let (reply, refused) = channel();
            writer(&host.inner).send(Job::DeleteBranch(path.clone(), branch("scripted"), generation_of(&host.inner, &path), stale, reply)).unwrap();
            assert_eq!(answer(refused), Err(DeleteJobError::Reopened));
            // The switch is queued first and the delete asked for before the writer reaches it.
            let release = hold_the_writer(host, &path);
            let switch = host.switch_branch(&path, branch("elsewhere"));
            let stale = host.delete_branch(&path, branch("scripted"));
            drop(release);
            assert!(matches!(switch.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(Switched::Moved(_))));
            assert_eq!(answer(stale), Err(DeleteJobError::BranchChanged), "one switch route");
            let (check, moving) = checked(host, &path, "main");
            assert!(check.is_ok());
            let release = hold_the_writer(host, &path);
            let switch = moving.run();
            let stale = host.delete_branch(&path, branch("scripted"));
            drop(release);
            assert!(matches!(switch.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(Switched::Moved(_))));
            assert_eq!(answer(stale), Err(DeleteJobError::BranchChanged), "the other, in two steps");
            assert!(has_branch(&lock(&host.inner.open)[0], "scripted"), "nothing was deleted");
            assert_eq!(answer(host.delete_branch(&path, branch("scripted"))), Ok(Vec::new()));
            assert_eq!(answer(host.delete_branch(&path, branch("main"))), Err(DeleteJobError::Delete(DeleteBranchError::Main)));
        }
    }

    /// The window is told of conversations only for a branch seen gone: not for a job's refusal,
    /// an unclear delete or a failure, each by its reason; asked again, the delete names them.
    #[test]
    fn the_window_hears_of_conversations_only_for_a_branch_seen_gone() {
        use BranchNotDeleted::{Failed, Moved, Unclear};
        use DeleteBranchError::{CannotTell, Current, Held, Main, NoSuchBranch, RecordRemains};
        use archforge_ui_egui::ai_agents::Provider;
        use archforge_ui_egui::chat_store::{Answer, ChatStore, Write};
        for (error, kept) in [
            (DeleteJobError::BranchChanged, Moved),
            (DeleteJobError::Reopened, Moved),
            (DeleteJobError::NotOpen, Moved),
            (DeleteJobError::Delete(CannotTell("why".into())), Unclear("why".into())),
            (DeleteJobError::Delete(DeleteBranchError::Failed("why".into())), Failed("why".into())),
            (DeleteJobError::Failed("why".into()), Failed("why".into())),
        ] {
            assert_eq!(not_deleted(error), kept);
        }
        for refused in [Main, Current, RecordRemains, NoSuchBranch, Held] {
            assert!(matches!(not_deleted(DeleteJobError::Delete(refused)), Failed(_)));
        }
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let delete = |at: &Path, name: &str| ProjectHost::delete_branch(&**host, at, name);
        let answer = |asked: Receiver<_>| asked.recv_timeout(Duration::from_secs(60)).expect("the delete answers");
        let moved = |to: &str| assert!(matches!(wait_for_switch(host, &path, to), Ok(Switched::Moved(_))), "{to}");
        assert_eq!(answer(delete(&sandbox.base.join("nowhere"), "a")), Err(Moved));
        assert!(matches!(answer(delete(&path, "a'b")), Err(Failed(_))));
        assert!(matches!(answer(delete(&path, "main")), Err(Failed(_))));
        let release = hold_the_writer(host, &path);
        let switch = host.switch_branch(&path, branch("elsewhere"));
        let stale = delete(&path, "scripted");
        drop(release);
        assert!(matches!(switch.recv_timeout(Duration::from_secs(60)).unwrap(), Ok(Switched::Moved(_))));
        assert_eq!(answer(stale), Err(Moved), "a switch got there first");
        // A conversation only on the branch: unclear, the delete names it to nobody; asked again, it does.
        let keys = archforge_secrets::MemoryStore::new();
        let agents = Arc::new(crate::agents::Agents::with(&sandbox.base.join("agents"), Box::new(keys), archforge_agents::HostEnv::default()));
        let chats = crate::chats::Chats::new(host.clone(), agents);
        let chat = Uuid::v7(1_762_400_000_000, [7; 10]);
        let create =
            Write::Create { chat, title: "On it".into(), provider: Provider::Claude, model: "sonnet".into(), skip: Vec::new(), now: 1_762_437_731 };
        assert!(matches!(chats.write(&path, create).recv_timeout(Duration::from_secs(60)).unwrap(), Ok(Answer::Created { .. })));
        moved("main");
        let asked = std::cell::Cell::new(false);
        let unclear = lock(&host.inner.open)[0].delete_branch_faulted(&branch("elsewhere"), false, false, &asked);
        assert!(matches!(unclear, Err(CannotTell(_))));
        assert_eq!(answer(delete(&path, "elsewhere")), Ok(vec![chat]));
        assert!(!has_branch(&lock(&host.inner.open)[0], "elsewhere"));
    }

    /// Switches to `to` and waits for the answer.
    fn wait_for_switch(host: &Projects, path: &Path, to: &str) -> Result<Switched, SwitchError> {
        host.switch_branch(path, branch(to)).recv_timeout(Duration::from_secs(60)).expect("the switch answers")
    }

    /// Has the project close and open again just before the next job picks it, as a closing does
    /// it: on the writer's thread the closing's own wait for the writer could not end.
    fn reopened_at_selection(host: &Arc<Projects>, path: &Path) {
        let (late, at) = (host.clone(), path.to_path_buf());
        let reopen = move || {
            let project = lock(&late.inner.open).remove(0);
            settle_reads(&late.inner, &at, Duration::from_secs(30));
            *lock(&late.inner.instances).entry(at.clone()).or_default() += 1;
            project.close().unwrap();
            wait(&late.open(&at)).unwrap();
        };
        lock(&host.inner.restore_faults).push(Fault::AtSelection(Box::new(reopen)));
    }

    /// A close and reopen just before a job picks its project answers `Reopened` and touches
    /// nothing in the new instance, for every job that names the instance it was asked for under.
    #[test]
    fn a_job_is_refused_when_the_project_reopens_just_before_it_picks_it() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let wait_for = Duration::from_secs(60);
        let start = lock(&host.inner.open)[0].head_of("main").unwrap();
        let old = version(host, 1);
        reopened_at_selection(host, &path);
        assert_eq!(host.delete_branch(&path, branch("scripted")).recv_timeout(wait_for).unwrap(), Err(DeleteJobError::Reopened));
        reopened_at_selection(host, &path);
        assert_eq!(host.rename_branch(&path, branch("scripted"), branch("x")).recv_timeout(wait_for).unwrap(), Err(RenameJobError::Reopened));
        reopened_at_selection(host, &path);
        assert_eq!(host.create_branch(&path, branch("made"), start).recv_timeout(wait_for).unwrap(), Err(CreateBranchError::Reopened));
        reopened_at_selection(host, &path);
        assert!(matches!(host.switch_branch(&path, branch("elsewhere")).recv_timeout(wait_for).unwrap(), Err(SwitchError::Reopened)));
        reopened_at_selection(host, &path);
        assert!(matches!(restore_of(host, &path, &old), Err(RestoreError::Reopened)));
        let open = lock(&host.inner.open);
        assert!(has_branch(&open[0], "scripted") && !has_branch(&open[0], "x") && !has_branch(&open[0], "made"));
        assert_eq!(open[0].current_branch().stored(), "main");
    }

    /// Closes the project and opens it again on `stored`, and says what the opening said.
    fn reopened_on(host: &Projects, path: &Path, stored: Option<&str>) -> Opened {
        close(host, path).unwrap();
        wait(&host.open_on(path, stored)).unwrap()
    }

    /// A project opens on the branch it was left on, a name in another case being that branch, and
    /// on `main` otherwise, by its typed reason: gone, unusable, or refused by an opening's checks.
    #[test]
    fn a_project_reopens_on_the_branch_it_was_left_on_or_says_why_not() {
        use BranchNotice::{CannotOpen, Gone};
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        for (stored, on_it) in [("elsewhere", "elsewhere"), ("ELSEWHERE", "elsewhere")] {
            let opened = reopened_on(host, &path, Some(stored));
            assert_eq!((opened.notice, on(host).as_str()), (None, on_it), "{stored}");
            assert_eq!(opened.model.as_deref(), Some(&model("a")), "{stored}: that branch's own model");
        }
        let opened = reopened_on(host, &path, None);
        assert_eq!((opened.notice, on(host).as_str()), (None, "main"), "a preference from before branches");
        assert_eq!(opened.model.as_deref(), Some(&model("b")));
        let opened = reopened_on(host, &path, Some("gone"));
        assert_eq!((opened.notice, on(host).as_str()), (Some(Gone { name: "gone".into() }), "main"));
        for stored in ["a'b", "scripted"] {
            let opened = reopened_on(host, &path, Some(stored));
            assert!(matches!(&opened.notice, Some(CannotOpen { name, .. }) if name == stored), "{stored}: {:?}", opened.notice);
            assert_eq!(on(host), "main", "{stored}");
            assert_eq!(opened.model.as_deref(), Some(&model("b")), "{stored}: main's own model");
        }
    }

    /// A creation record naming the branch the project reopens on leaves it standing.
    #[test]
    fn recovery_keeps_the_branch_the_project_reopens_on() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        for (recorded, stored) in [("elsewhere", "elsewhere"), ("elsewhere", "ELSEWHERE")] {
            let start = lock(&host.inner.open)[0].head_of("elsewhere").unwrap();
            stopped_creation(&path, recorded, "main", &start);
            let opened = reopened_on(host, &path, Some(stored));
            assert_eq!((opened.notice, on(host).as_str()), (None, "elsewhere"), "{stored}");
            assert!(has_branch(&lock(&host.inner.open)[0], "elsewhere") && !path.join(".creating-branch").exists(), "{stored}");
        }
        // Recovery does its work even when the branch it was told about has gone.
        let start = lock(&host.inner.open)[0].head_of("scripted").unwrap();
        stopped_creation(&path, "scripted", "main", &start);
        reopened_on(host, &path, Some("gone"));
        assert!(!has_branch(&lock(&host.inner.open)[0], "scripted") && !path.join(".creating-branch").exists());
    }

    /// A saved model and unsaved work on the source that differ in the rows and diagrams.
    fn saved_then_work(host: &Projects, path: &Path, saved_rows: Rows, work: Rows) {
        host.write(path, saved_rows);
        assert_eq!(saved(host, path), Ok(true));
        host.write(path, work);
        assert_eq!(host.flush_for_test(path), Ok(()));
    }

    /// The unsaved rows reach the new branch, those that were removed and the diagrams too, in
    /// both directions, and stay unsaved there; the source keeps its work and its head.
    #[test]
    fn a_branch_from_current_work_holds_what_is_unsaved() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let schema = schema(&host.inner).unwrap();
        let (with_diagram, _) = short_and_fitted_box();
        let without = model("without");
        let listed = |project: &Project, name: &str| project.branches(50).unwrap().listed.into_iter().find(|b| b.name == name).unwrap().unsaved;
        for (name, saved_rows, work) in [("removes", &with_diagram, &without), ("adds", &without, &with_diagram)] {
            saved_then_work(host, &path, saved_rows.clone(), work.clone());
            let open = lock(&host.inner.open);
            let project = &open[0];
            let start = project.head_of("main").unwrap();
            assert_eq!(project.create_branch_from_work(&branch(name), &schema), Ok(BranchCreated::Listed));
            assert_eq!(&project.check_branch(&branch(name), &schema).unwrap(), work, "{name}: the work was copied");
            assert_eq!(&project.read_model_at(&schema, &start).unwrap(), saved_rows, "{name}: the start is the save");
            assert_eq!(
                project
                    .first_on_branch(name, "SELECT CAST(parent_hash AS CHAR) FROM dolt_commit_ancestors WHERE commit_hash = HASHOF('HEAD')")
                    .unwrap(),
                start
            );
            assert_eq!(listed(project, name), Some(true), "{name}: unsaved there, as Save would commit it");
            assert_eq!((&project.read_model(&schema).unwrap(), project.head_of("main").unwrap()), (work, start), "{name}: the source");
            assert!(project.has_unsaved().unwrap(), "{name}: the source keeps its unsaved work");
        }
        // With nothing unsaved, the new branch has nothing unsaved either.
        assert_eq!(saved(host, &path), Ok(true));
        let open = lock(&host.inner.open);
        assert_eq!(open[0].create_branch_from_work(&branch("clean"), &schema), Ok(BranchCreated::Listed));
        assert_eq!(listed(&open[0], "clean"), Some(false));
        drop(open);
        // At the creation commit, which has no model tables to copy to until they are prepared.
        let fresh = Sandbox::new(None);
        let path = fresh.project();
        fresh.host.write(&path, model("first"));
        assert_eq!(fresh.host.flush_for_test(&path), Ok(()));
        let open = lock(&fresh.host.inner.open);
        assert_eq!(open[0].create_branch_from_work(&branch("early"), &schema), Ok(BranchCreated::Listed));
        assert_eq!(open[0].check_branch(&branch("early"), &schema).unwrap(), model("first"));
    }

    /// A stop before or after the copy leaves an unfinished branch for the real recovery, a failed
    /// step undoes it (by a test hook: the failing statement is not real), and the source's unsaved
    /// work is as it was each time.
    #[test]
    fn a_branch_from_current_work_that_stops_or_fails_leaves_the_source_as_it_was() {
        use archforge_project::CreationStep::{ChatCommit, CopyWork, RemoveConversations};
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let schema = schema(&host.inner).unwrap();
        saved_then_work(host, &path, model("saved"), model("work"));
        let source_kept = |what: String| {
            let open = lock(&host.inner.open);
            assert_eq!(open[0].read_model(&schema).unwrap(), model("work"), "{what}");
            assert!(open[0].has_unsaved().unwrap(), "{what}");
        };
        for (n, step) in [CopyWork, RemoveConversations, ChatCommit].into_iter().enumerate() {
            let (stopped, broke) = (format!("stop{n}"), format!("fail{n}"));
            let answer = lock(&host.inner.open)[0].create_branch_halted(&branch(&stopped), None, &schema, (step, false));
            assert!(answer.is_err() && path.join(".creating-branch").exists(), "{step:?}");
            source_kept(format!("stopped at {step:?}"));
            close(host, &path).unwrap();
            wait(&host.open(&path)).unwrap();
            assert!(!has_branch(&lock(&host.inner.open)[0], &stopped), "{step:?}: recovery removed it");
            source_kept(format!("reopened after {step:?}"));
            let answer = lock(&host.inner.open)[0].create_branch_halted(&branch(&broke), None, &schema, (step, true));
            assert!(matches!(answer, Err(BranchCreationError::Failed { step: s, .. }) if s == step), "{step:?}: {answer:?}");
            assert!(!has_branch(&lock(&host.inner.open)[0], &broke) && !path.join(".creating-branch").exists(), "{step:?}");
            source_kept(format!("failed at {step:?}"));
        }
    }

    /// No conversation reaches the new branch, and rows of one not yet committed stay on the source.
    #[test]
    fn a_branch_from_current_work_takes_no_conversations() {
        let (sandbox, path) = branched();
        undone_since_the_save(&sandbox.host, &path);
        let schema = schema(&sandbox.host.inner).unwrap();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        project.run_on_branch("main", "UPDATE chat SET title = 'not committed'").unwrap();
        assert_eq!(project.create_branch_from_work(&branch("clear"), &schema), Ok(BranchCreated::Listed));
        for table in archforge_project::CHAT_TABLES {
            assert_eq!(rows_on(project, "clear", table), 0, "{table}");
        }
        assert_eq!(project.first_on_branch("main", "SELECT CAST(title AS CHAR) FROM chat").unwrap(), "not committed");
        let email = project.first_on_branch("clear", "SELECT CAST(email AS CHAR) FROM dolt_log LIMIT 1").unwrap();
        assert!(!archforge_project::is_version(&email));
        // The head is checked for scripts as any version is.
        project.run_on_branch("main", "CALL DOLT_BRANCH('guarded')").unwrap();
        project.run_on_branch("guarded", "CREATE PROCEDURE p() SELECT 1").unwrap();
        project.run_on_branch("guarded", "CALL DOLT_COMMIT('-A', '-m', 'x', '--author', 'A Person <a@person>')").unwrap();
        project.set_current_branch(branch("guarded"));
        assert_eq!(project.create_branch_from_work(&branch("scriptless"), &schema), Err(BranchCreationError::Scripted { what: "stored procedures" }));
        project.set_current_branch(branch("main"));
    }

    /// The writer writes what waits first, and rows it cannot write refuse the branch and keep waiting.
    #[test]
    fn the_writer_writes_what_waits_before_a_branch_from_current_work() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let schema = schema(&host.inner).unwrap();
        let answer =
            |asked: Receiver<Result<BranchCreated, CreateBranchError>>| asked.recv_timeout(Duration::from_secs(60)).expect("the job answers");
        let release = hold_the_writer(host, &path);
        host.write(&path, model("waited"));
        let asked = host.create_branch_from_work(&path, branch("fromwork"));
        drop(release);
        assert_eq!(answer(asked), Ok(BranchCreated::Listed));
        assert_eq!(lock(&host.inner.open)[0].check_branch(&branch("fromwork"), &schema).unwrap(), model("waited"));
        let mut broken = model("c");
        broken.tables.iter_mut().find(|t| !t.is_empty()).unwrap()[0].clear();
        let release = hold_the_writer(host, &path);
        host.write(&path, broken);
        let asked = host.create_branch_from_work(&path, branch("refused"));
        drop(release);
        assert!(matches!(answer(asked), Err(CreateBranchError::Unwritten(_))));
        assert!(host.writing(&path).pending, "the rows are still waiting");
        assert!(!has_branch(&lock(&host.inner.open)[0], "refused"));
        // That write failed, so the timer stops, as after any failed write (a retry would say so).
        lock(&host.inner.writing).get_mut(&path).unwrap().error = None;
        std::thread::sleep(SETTLE * 4);
        assert_eq!(host.writing(&path).error, None, "no timer after a failed write");
        // A branch from a version does not depend on what waits, which does not reach it.
        let start = lock(&host.inner.open)[0].head_of("main").unwrap();
        assert_eq!(answer(host.create_branch(&path, branch("fromversion"), start)), Ok(BranchCreated::Listed));
        host.write(&path, model("b"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
    }

    /// A failure at any step deletes the branch and then the record, and refusals change nothing.
    #[test]
    fn a_creation_that_fails_leaves_the_project_as_it_was() {
        use archforge_project::BranchCreationError::{Failed, NameTaken, NotACommit, RecordRemains};
        use archforge_project::CreationStep::{ChatCommit, CheckVersion, MakeBranch, PrepareModel, RemoveConversations};
        let (sandbox, path) = branched();
        undone_since_the_save(&sandbox.host, &path);
        let schema = schema(&sandbox.host.inner).unwrap();
        let open = lock(&sandbox.host.inner.open);
        let project = &open[0];
        let start = project.head_of("main").unwrap();
        let as_it_was =
            |name: &str| !has_branch(project, name) && !path.join(".creating-branch").exists() && project.head_of("main").unwrap() == start;
        // By a test hook just before each step: the failure path is real, the failing statement is not.
        for (n, step) in [MakeBranch, PrepareModel, RemoveConversations, ChatCommit].into_iter().enumerate() {
            let name = format!("fail{n}");
            let answer = project.create_branch_halted(&branch(&name), Some(&start), &schema, (step, true));
            assert!(matches!(answer, Err(Failed { step: s, .. }) if s == step), "{step:?}: {answer:?}");
            assert!(as_it_was(&name), "{step:?}");
        }
        // A commit that does not exist, a start that is none, and names taken in any case.
        let unknown = "0123456789abcdefghijklmnopqrstuv";
        assert!(matches!(project.create_branch(&branch("nowhere"), unknown, &schema), Err(Failed { step: CheckVersion, .. })));
        assert!(as_it_was("nowhere"));
        assert_eq!(project.create_branch(&branch("bad"), "'; DROP DATABASE project; --", &schema), Err(NotACommit));
        assert_eq!(project.create_branch(&branch("ELSEWHERE"), &start, &schema), Err(NameTaken));
        assert_eq!(project.create_branch(&branch("MAIN"), &start, &schema), Err(NameTaken));
        // While a record remains, good or damaged, even a name taken says so.
        stopped_creation(&path, "held", "main", &start);
        assert_eq!(project.create_branch(&branch("elsewhere"), &start, &schema), Err(RecordRemains));
        std::fs::write(path.join(".creating-branch"), b"broken").unwrap();
        assert_eq!(project.create_branch(&branch("other"), &start, &schema), Err(RecordRemains));
        std::fs::remove_file(path.join(".creating-branch")).unwrap();
        assert!(as_it_was("other"));
    }

    /// The whole of a switch: what waits is written first, the target's rows come back, and the
    /// connections, ledger and generation move once.
    #[test]
    fn a_switch_writes_what_waits_then_moves_to_the_target() {
        use archforge_project::ChangeState::Undone;
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (chat, _) = undone_since_the_save(host, &path);
        let before = host.generation_of(&path);
        host.write(&path, model("c"));
        let moved = switch_to(host, &path, "elsewhere").unwrap();
        assert_eq!(moved, Switched::Moved(Arc::new(model("a"))));
        assert_eq!((on(host).as_str(), host.generation_of(&path)), ("elsewhere", before + 1));
        // The rows that waited went to the branch left.
        assert_eq!(lock(&host.inner.open)[0].check_branch(&branch("main"), &schema(&host.inner).unwrap()).unwrap(), model("c"));
        // A later switch back leaves the ledger empty: Don't save moves nothing back.
        switch_to(host, &path, "main").unwrap();
        assert_eq!(after_dont_save(host, &path, chat), Some(Undone), "the ledger started again");
    }

    /// Each way a switch can fail leaves the branch, the generation and the ledger as they were.
    #[test]
    fn a_failed_switch_moves_nothing() {
        use archforge_project::{ChangeState::Applied, Error};
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let (chat, _) = undone_since_the_save(host, &path);
        let before = host.generation_of(&path);
        let unmoved = || assert_eq!((on(host).as_str(), host.generation_of(&path)), ("main", before));
        assert!(matches!(switch_to(host, &path, "nosuchbranch"), Err(SwitchError::Checks(Error::Damaged { .. }))));
        unmoved();
        assert!(matches!(switch_to(host, &path, "scripted"), Err(SwitchError::Checks(Error::Scripted { what: "triggers", .. }))));
        unmoved();
        // Rows that cannot be written stop it, and stay waiting.
        let mut broken = model("c");
        broken.tables.iter_mut().find(|t| !t.is_empty()).unwrap()[0].clear();
        host.write(&path, broken);
        assert!(matches!(switch_to(host, &path, "elsewhere"), Err(SwitchError::Unwritten(_))));
        unmoved();
        assert!(host.writing(&path).pending, "the rows are still waiting");
        host.write(&path, model("b"));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        // The same case of the same branch is no switch at all.
        assert_eq!(switch_to(host, &path, "MAIN").unwrap(), Switched::AlreadyThere);
        unmoved();
        assert_eq!(after_dont_save(host, &path, chat), Some(Applied), "the ledger still knew the save");
    }

    /// A job asked for between the steps, before the generation moved, is refused; one after is not.
    #[test]
    fn a_job_asked_for_during_a_switch_is_refused_and_a_later_one_is_not() {
        let (sandbox, path) = branched();
        let host = sandbox.host.clone();
        let (late, late_path) = (host.clone(), path.clone());
        let (asked, answer) = channel();
        *lock(&host.inner.before_move) = Some(Box::new(move || {
            late.write(&late_path, model("late"));
            let _ = asked.send(late.save(&late_path, None, nobody()));
        }));
        let before = host.generation_of(&path);
        assert!(matches!(switch_to(&host, &path, "elsewhere"), Ok(Switched::Moved(_))));
        assert_eq!(host.generation_of(&path), before + 1, "moved exactly once");
        let queued = answer.recv_timeout(Duration::from_secs(60)).unwrap();
        assert_eq!(made(queued.recv_timeout(Duration::from_secs(60)).unwrap()), Err(SaveError::BranchChanged));
        assert_eq!(host.flush_for_test(&path), Ok(()));
        assert!(host.writing(&path).error.is_some(), "the write from before the move was recorded");
        host.write(&path, model("after"));
        assert_eq!(saved(&host, &path), Ok(true), "a job asked for after the move is accepted");
    }

    /// A switch asked for before another moved the project is refused and moves nothing.
    #[test]
    fn a_switch_from_before_another_is_refused() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let release = hold_the_writer(host, &path);
        let asked = host.switch_branch(&path, branch("elsewhere"));
        switched(&host.inner, &path);
        drop(release);
        assert!(matches!(asked.recv_timeout(Duration::from_secs(60)).unwrap(), Err(SwitchError::BranchChanged)));
        assert_eq!(on(host), "main");
    }

    /// A lost answer is read back from the branch the connections are on, in any case of its name.
    #[test]
    fn a_lost_switch_answer_is_read_back() {
        let (sandbox, path) = branched();
        let host = &sandbox.host;
        let read = |name: &str| host.read_back_branch(&path, branch(name)).recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(read("elsewhere"), Ok(false));
        // Conversations that cannot be committed do not stop a switch.
        host.inner.fail_chats.store(true, Ordering::SeqCst);
        assert!(matches!(switch_to(host, &path, "elsewhere"), Ok(Switched::Moved(_))));
        assert_eq!((read("elsewhere"), read("ELSEWHERE"), read("main")), (Ok(true), Ok(true), Ok(false)));
        close(host, &path).unwrap();
        assert!(read("elsewhere").is_err(), "a project not open cannot be told");
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
