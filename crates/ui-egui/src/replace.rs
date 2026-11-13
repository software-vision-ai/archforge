//! Replacing the session under a hold (`docs/design/versioning.md`, section 6, steps 5 to 8): a
//! save under way answers and the latest rows go over, the replacement is read and built before
//! anything is written, the operation's writes run as one job, and the replacement takes the
//! session's place. A job with no clear answer is read back; when that cannot be read either,
//! Archforge says so, writes nothing back, and the project stays held until it is closed.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::rows::Rows;
use archforge_notations::Session;

use crate::hold::{CHAT_SETTLE_LIMIT, ChatSettling, HoldRefused};
use crate::i18n::{fmt, tr};
use crate::leaving::{Cleared, Leaving};
use crate::{BranchSwitch, SaveState, SavedBy, Shell, VersionRead, chat_panel, editing};

/// The most seconds reading a job back may take before what happened counts as unknown.
pub const READ_BACK_LIMIT: u64 = 30;

/// The most seconds a replacement waits for [`compare_off_frame`]'s answer, from when it started;
/// past it the replacement counts as repaired: it reads as unsaved and its rows are written again,
/// which changes nothing stored.
pub const COMPARE_LIMIT: u64 = 30;

#[cfg(test)]
thread_local! {
    /// For tests: the answer the next comparison gives in place of a worker's.
    pub(crate) static COMPARED: std::cell::RefCell<Option<Receiver<bool>>> = const { std::cell::RefCell::new(None) };
}

/// Whether the session built from `rows` holds anything else, row order aside, answered later: the
/// frame only freezes the session's objects, and their rows are built and compared on another
/// thread. A comparison that cannot finish, or a thread that cannot start, counts as a difference.
fn compare_off_frame(session: &Session, rows: Arc<Rows>) -> Receiver<bool> {
    #[cfg(test)]
    if let Some(answer) = COMPARED.with(|c| c.borrow_mut().take()) {
        return answer;
    }
    let frozen = session.doc().freeze();
    let (tx, rx) = channel();
    let compare = move || {
        let (tables, junctions) = archforge_notations::storage();
        let found = frozen
            .rows(&tables, &junctions)
            .ok()
            .and_then(|built| archforge_mof::compare::compare(&rows, &built, &tables, &junctions, archforge_mof::compare::MAX_COMPARED).ok());
        let _ = tx.send(found.is_none_or(|found| !found.is_empty()));
    };
    // A thread that cannot start drops the sender with it, which reads as a difference.
    if let Err(e) = std::thread::Builder::new().name("archforge-replacement-repair".into()).spawn(compare) {
        log::warn!("the replacement could not be compared off the interface thread: {e}");
    }
    rx
}

/// A comparison of a replacement running on another thread, when it started, and its answer once
/// it has come.
struct Comparing {
    answer: Receiver<bool>,
    since: u64,
    got: Option<bool>,
}

impl Comparing {
    /// The answer, once it has come: a sender gone without one counts as repaired.
    fn answered(&mut self) -> Option<bool> {
        if self.got.is_none() {
            self.got = match self.answer.try_recv() {
                Ok(differs) => Some(differs),
                Err(TryRecvError::Disconnected) => Some(true),
                Err(TryRecvError::Empty) => None,
            };
        }
        self.got
    }

    /// Whether the replacement was repaired, once that is known at `now`: no answer within
    /// [`COMPARE_LIMIT`] counts as repaired too.
    fn repaired(&mut self, now: u64) -> Option<bool> {
        self.answered().or_else(|| (now.saturating_sub(self.since) >= COMPARE_LIMIT).then_some(true))
    }
}

#[cfg(test)]
impl Shell {
    /// Whether a replacement's comparison off the frame has not answered yet.
    pub(crate) fn comparing(&mut self) -> bool {
        self.replacing.as_mut().and_then(|r| r.repaired.as_mut()).is_some_and(|c| c.answered().is_none())
    }
}

/// How a job that did not plainly finish went.
pub enum JobFailure {
    /// It failed, and the project is as it was.
    Failed(String),
    /// It failed after saving the unsaved work as a version: the model is the work, now saved.
    WorkSaved(String),
    /// A restore made no version, as the version is the same as current work; `work_saved` says
    /// unsaved work was saved as a version first. Trying again can't change it.
    Same { work_saved: bool },
    /// It gave no clear answer: what happened has to be read back.
    Unclear,
}

/// What a job's starting gives: its answer, later.
pub type JobAnswer = Receiver<Result<(), JobFailure>>;
/// What reading a job back gives: whether it finished, later, or why that could not be read.
pub type ReadBack = Receiver<Result<bool, String>>;

/// An operation's writes, run as one job on the writer. Both callbacks are called on the frame
/// thread, so each only hands its work to the writer and returns its answer's channel at once, as
/// the host's Save does; one that blocked would stop the window, which no limit here can catch.
pub struct Job {
    pub run: Box<dyn FnOnce() -> JobAnswer>,
    /// Reads `HEAD` or the branch list to see whether the job finished.
    pub read_back: Box<dyn FnOnce() -> ReadBack>,
}

/// Where a replacement stands.
enum Stage {
    Chat,
    Save,
    Build,
    /// Built, and waiting for Chat to be still before the job or the replacement.
    Ready(Box<Session>, Option<u64>),
    Writing(Box<Session>, JobAnswer),
    Reading(Box<Session>, ReadBack, u64),
    /// What happened could not be told: held until the project is closed.
    Unknown,
}

/// A replacement under way.
pub(crate) struct Replacing {
    stage: Stage,
    read: Receiver<VersionRead>,
    job: Option<Job>,
    read_back: Option<Box<dyn FnOnce() -> ReadBack>>,
    /// Whether the job has written, after which the operation can no longer simply stop.
    wrote: bool,
    /// The version to show in place of the session, set aside rather than replaced.
    viewing: Option<String>,
    /// Whether the session built differs from the rows read, a repair made while building it, once
    /// its comparison on another thread answers; none when nothing is compared.
    repaired: Option<Comparing>,
    /// A branch switch still to be asked of the host, once the latest rows have gone over.
    switch: Option<Box<dyn FnOnce() -> Result<BranchSwitch, String>>>,
    /// Asked of the host: whether the branch arrived on holds unsaved work.
    arrives_unsaved: Option<Arc<AtomicBool>>,
    /// The project and branch a switch moves to, kept once it has arrived.
    switching_to: Option<(PathBuf, String)>,
    /// The ordinary Save this switch does first: its ticket once asked for, gone once it is done.
    save_first: Option<Option<u64>>,
}

impl Replacing {
    /// `otherwise` unless this is a switch whose check has been asked for: that may have upgraded
    /// the target's tables, so no more is claimed than that the project stayed.
    fn stopped(&self, otherwise: String) -> String {
        if self.arrives_unsaved.is_some() { tr("The switch did not finish, so the project stayed on its branch.") } else { otherwise }
    }

    /// `why` the version a restore wanted could not be read or built, which is before its job has
    /// written anything, said so; other replacements keep their own words.
    fn before_writing(&self, why: String) -> String {
        if self.job.is_some() && self.arrives_unsaved.is_none() { fmt(&tr("{why} Nothing was changed."), &[("why", &why)]) } else { why }
    }
}

impl Shell {
    /// Restores the version `commit`, saved at `saved_at`, as a new version: the version in view
    /// gives way to current work first, and the project is held before the version is read. Unsaved
    /// work is kept under a name dated now, as the question quotes it ([`crate::restore_ask::asked_at`]). Refused
    /// as [`Self::clear_to_leave`] says, or without a host, with nothing changed.
    pub fn restore_version(&mut self, commit: &str, saved_at: &str) -> Result<(), HoldRefused> {
        let cleared = self.clear_to_leave(Some(Leaving::Restore { commit: commit.to_string(), saved_at: saved_at.to_string() }))?;
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.leave_version();
        self.begin_hold()?;
        let by = SavedBy::Person(self.state.user_name.clone());
        let dates = (saved_at.to_string(), crate::restore_ask::asked_at(self));
        let job = host.restore_job(&path, commit.to_string(), dates, by, editing::fit_tables);
        self.start_replacing(host.read_version(&path, commit.to_string()), Some(job), None, cleared);
        Ok(())
    }

    /// Moves the project to `branch` under the hold: the version in view gives way to current work
    /// first, the check is asked for once the latest rows have gone over, and the branch's model
    /// takes the session's place once the move has finished. Refused as
    /// [`Self::clear_to_leave`] says, or without a host, with nothing changed.
    pub fn switch_branch(&mut self, branch: String) -> Result<(), HoldRefused> {
        self.start_switch(branch, false)
    }

    /// [`Self::switch_branch`], saving first when `save_first` says so: an ordinary Save inside the
    /// hold with the automatic description, and one that fails or can't be told ends the switch there.
    pub(crate) fn start_switch(&mut self, branch: String, save_first: bool) -> Result<(), HoldRefused> {
        // Chat's question can't carry the Save, so with one the hold settles a running turn without asking.
        let cleared = self.clear_to_leave((!save_first).then(|| Leaving::Switch(branch.clone())))?;
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.leave_version();
        self.begin_hold()?;
        self.start_replacing(channel().1, None, None, cleared);
        if let Some(r) = self.replacing.as_mut() {
            r.switching_to = Some((path.clone(), branch.clone()));
            r.switch = Some(Box::new(move || host.begin_branch_switch(&path, branch)));
            r.save_first = save_first.then_some(None);
        }
        Ok(())
    }

    /// The branch `from` of the project at `path` is now `to`: the project reopens on it when it was
    /// the one in use, and only then. For the screen that asks for a rename, once it is seen done.
    pub fn branch_renamed(&mut self, path: &Path, from: &str, to: &str) {
        if self.state.branch_of(path).is_some_and(|b| b.eq_ignore_ascii_case(from)) {
            self.state.keep_branch(path, Some(to.to_string()));
        }
    }

    /// Holds the project and starts replacing its session with the model `read` answers, running
    /// `job` once it is built. What happens is driven frame by frame; a Chat turn running is settled
    /// by the hold without a question, as its receiver and job can't be replayed after one.
    pub fn begin_replacement(&mut self, read: Receiver<VersionRead>, job: Option<Job>) -> Result<(), HoldRefused> {
        let cleared = self.clear_to_leave(None)?;
        self.begin_hold()?;
        self.start_replacing(read, job, None, cleared);
        Ok(())
    }

    /// Starts the replacement under the hold already taken; with `viewing`, what is built shows
    /// as that version and the session is set aside. Without it the project's versions or branch
    /// may change, even if this later fails, so the steps between versions read them afresh and
    /// the versions kept for viewing and what was found of versions go, as they hold of a branch.
    /// `_cleared` is the proof that [`Self::clear_to_leave`] let it through.
    pub(crate) fn start_replacing(&mut self, read: Receiver<VersionRead>, job: Option<Job>, viewing: Option<String>, _cleared: Cleared) {
        if viewing.is_none() {
            self.version_list.steps = Default::default();
            self.loads.clear();
            self.versions.unavailable.clear();
            self.versions.load_failed = None;
        }
        self.replacing = Some(Replacing {
            stage: Stage::Chat,
            read,
            job,
            read_back: None,
            wrote: false,
            viewing,
            repaired: None,
            switch: None,
            arrives_unsaved: None,
            switching_to: None,
            save_first: None,
        });
    }

    /// The version being read to be shown.
    pub(crate) fn version_being_read(&self) -> Option<&str> {
        self.replacing.as_ref()?.viewing.as_deref()
    }

    /// One frame of the replacement under way.
    pub(crate) fn drive_replacement(&mut self) {
        if let Some(r) = self.replacing.take() {
            self.replacing = self.advance(r);
        }
        crate::switch_ask::keep(self);
        crate::restore_ask::keep(self);
    }

    /// Takes `r` one stage on; none once it has ended.
    fn advance(&mut self, mut r: Replacing) -> Option<Replacing> {
        r.stage = match std::mem::replace(&mut r.stage, Stage::Unknown) {
            Stage::Chat => match self.settle_chat() {
                ChatSettling::Waiting => Stage::Chat,
                ChatSettling::Settled => Stage::Save,
                ChatSettling::Refused => {
                    self.save_error = Some(tr("Chat did not finish in time, so nothing was changed."));
                    crate::restore_ask::failed(self, true);
                    return None;
                }
            },
            Stage::Save if self.is_saving() => Stage::Save,
            Stage::Save => {
                if !self.hand_over_held() {
                    // Nothing could be handed over: a drag still runs, or a version is in view.
                    return self.give_up(None);
                }
                match r.save_first.take() {
                    Some(None) => {
                        r.save_first = Some(Some(self.save_held()));
                        r.stage = Stage::Save;
                        return Some(r);
                    }
                    // The save's own error, which says whether a version was made, is what is told.
                    Some(Some(ticket)) if !matches!(self.save_state(ticket), SaveState::Saved(_)) => return self.give_up(None),
                    _ => {}
                }
                if let Some(start) = r.switch.take() {
                    match start() {
                        Ok(BranchSwitch { read, unsaved, job }) => {
                            r.read = read;
                            r.job = Some(job);
                            r.arrives_unsaved = Some(unsaved);
                        }
                        Err(why) => return self.give_up(Some(why)),
                    }
                }
                Stage::Build
            }
            Stage::Build => match r.read.try_recv() {
                Err(TryRecvError::Empty) => Stage::Build,
                Err(TryRecvError::Disconnected) => {
                    return self.give_up(Some(r.stopped(tr("The reading stopped without an answer, so nothing was changed."))));
                }
                Ok(Err(why)) => return self.give_up(Some(r.before_writing(why.to_string()))),
                Ok(Ok(rows)) => match self.build(&rows) {
                    Ok(session) => {
                        let since = chat_panel::clock(self).now;
                        r.repaired = r.viewing.is_none().then(|| Comparing { answer: compare_off_frame(&session, rows), since, got: None });
                        Stage::Ready(Box::new(session), None)
                    }
                    Err(why) => {
                        if let Some(commit) = r.viewing.take() {
                            self.mark_unbuilt(commit, &why);
                        }
                        return self.give_up(Some(r.before_writing(why)));
                    }
                },
            },
            Stage::Ready(session, since) => {
                let now = chat_panel::clock(self).now;
                let since = since.unwrap_or(now);
                if !chat_panel::settled(self) {
                    if now.saturating_sub(since) < CHAT_SETTLE_LIMIT {
                        Stage::Ready(session, Some(since))
                    } else if r.wrote {
                        self.could_not_tell()
                    } else {
                        return self.give_up(Some(r.stopped(tr("Chat did not finish in time, so nothing was changed."))));
                    }
                } else if let Some(job) = r.job.take() {
                    r.read_back = Some(job.read_back);
                    Stage::Writing(session, (job.run)())
                } else {
                    let repaired = match r.repaired.as_mut().map(|c| c.repaired(now)) {
                        None => false,
                        Some(Some(repaired)) => repaired,
                        // Still being compared: the hold's wake-ups bring the next look. Chat is
                        // settled here, so a Chat that is busy later gets its whole settle time.
                        Some(None) => {
                            r.stage = Stage::Ready(session, None);
                            return Some(r);
                        }
                    };
                    match r.viewing.take() {
                        Some(commit) => self.show_version(*session, commit),
                        None => self.install(*session, repaired),
                    }
                    if let Some(unsaved) = r.arrives_unsaved {
                        // On another branch: the status bar's reasons about the branch left go, and
                        // the project reopens here.
                        self.save_error = None;
                        self.versions.switching = None;
                        if let Some(o) = self.project.as_mut() {
                            o.notice = None;
                        }
                        if let Some((path, branch)) = r.switching_to.take() {
                            self.state.keep_branch(&path, Some(branch));
                        }
                        if unsaved.load(Ordering::SeqCst)
                            && let Some(session) = self.session.as_mut()
                        {
                            session.mark_unsaved();
                        }
                        chat_panel::branch_changed(self);
                    }
                    return None;
                }
            }
            Stage::Writing(session, answer) => match answer.try_recv() {
                Err(TryRecvError::Empty) => Stage::Writing(session, answer),
                Ok(Ok(())) => {
                    r.wrote = true;
                    Stage::Ready(session, None)
                }
                Ok(Err(JobFailure::Failed(why))) => return self.give_up(Some(why)),
                Ok(Err(JobFailure::WorkSaved(why))) => {
                    self.mark_work_saved();
                    return self.give_up(Some(why));
                }
                Ok(Err(JobFailure::Same { work_saved })) => {
                    let mut why = tr("This version is the same as your current work.");
                    if work_saved {
                        self.mark_work_saved();
                        why = tr("This version is the same as your current work. Your unsaved work was kept as a version.");
                    }
                    return self.end_failed(Some(why), false);
                }
                // A lost reply says nothing about what happened: it is read back, never guessed.
                Ok(Err(JobFailure::Unclear)) | Err(TryRecvError::Disconnected) => match r.read_back.take() {
                    Some(read) => {
                        // Before the call, so that the time it takes to return counts.
                        let since = chat_panel::clock(self).now;
                        Stage::Reading(session, read(), since)
                    }
                    None => self.could_not_tell(),
                },
            },
            Stage::Reading(session, answer, since) => match answer.try_recv() {
                Err(TryRecvError::Empty) => {
                    let now = chat_panel::clock(self).now;
                    if now.saturating_sub(since) < READ_BACK_LIMIT { Stage::Reading(session, answer, since) } else { self.could_not_tell() }
                }
                Ok(Ok(true)) => {
                    r.wrote = true;
                    Stage::Ready(session, None)
                }
                Ok(Ok(false)) => return self.give_up(Some(r.stopped(tr("The change did not finish, so the project is as it was.")))),
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => self.could_not_tell(),
            },
            Stage::Unknown => Stage::Unknown,
        };
        Some(r)
    }

    /// Asks the host for an ordinary Save inside the hold, once the latest rows are over; gives its ticket.
    fn save_held(&mut self) -> u64 {
        self.save_ticket += 1;
        if let (Some(host), Some(project), Some(revision)) = (&self.services.projects, &self.project, self.written) {
            let by = SavedBy::Person(self.state.user_name.clone());
            self.saving = Some((host.save(&project.path, None, by), revision, self.save_ticket));
            self.save_error = None;
        }
        self.save_ticket
    }

    /// A session of `rows`, repaired as an opening repairs it.
    pub(crate) fn build(&self, rows: &Rows) -> Result<Session, String> {
        let Some(source) = self.services.ids.clone() else { return Err(tr("The host gives no ids for the model's elements")) };
        archforge_notations::session_from_rows_repaired(rows, Box::new(move || source()), editing::fit_tables)
    }

    /// Ends the operation with the session as it was, saying `why` when given. The same session
    /// stays, so nothing else tells the Versions panel to look: a job that failed may have kept
    /// the work as a version first.
    pub(crate) fn give_up(&mut self, why: Option<String>) -> Option<Replacing> {
        self.end_failed(why, true)
    }

    /// [`Self::give_up`], saying whether trying again could change the outcome (`retry`).
    fn end_failed(&mut self, why: Option<String>, retry: bool) -> Option<Replacing> {
        if why.is_some() {
            self.save_error = why;
        }
        self.versions.stale = true;
        self.fail_hold();
        crate::restore_ask::failed(self, retry);
        None
    }

    /// Says that what happened cannot be told. Nothing is written back; the project stays held and
    /// its closing offers only to close without writing. Versions reads what the project holds.
    fn could_not_tell(&mut self) -> Stage {
        self.versions.stale = true;
        let why = tr("Archforge could not tell whether the last change finished. Nothing more was written. Close the project and reopen it.");
        if let Some(tab) = self.active.and_then(|i| self.tabs.get_mut(i)) {
            tab.close_error = Some(why.clone());
        }
        self.save_error = Some(why);
        self.holding.keep_until_closed();
        Stage::Unknown
    }

    /// The job saved the unsaved work as a version before it failed: what had been handed over is
    /// saved now, and anything since would stay unsaved.
    fn mark_work_saved(&mut self) {
        if let (Some(session), Some(written)) = (self.session.as_mut(), self.written)
            && let Err(e) = session.mark_saved_at(written)
        {
            log::warn!("the work kept as a version could not be marked saved: {e}");
        }
    }

    /// The replacement takes the session's place, and the hold ends. What was read is what the
    /// project holds, so nothing is handed over for it, unless a repair changed it while it was
    /// built: then the project reads as unsaved and the repair is written like any change.
    pub(crate) fn install(&mut self, mut session: Session, repaired: bool) {
        self.written = Some(session.revision()).filter(|_| !repaired);
        if repaired {
            session.mark_unsaved();
        }
        // One opened while held would write to the replacement's selection.
        self.panels.picker = None;
        self.session = Some(session);
        self.session_replaced();
        self.end_hold();
    }
}
