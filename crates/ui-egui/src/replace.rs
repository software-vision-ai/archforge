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
use crate::i18n::tr;
use crate::{BranchSwitch, SavedBy, Shell, chat_panel, editing};

/// The most seconds reading a job back may take before what happened counts as unknown.
pub const READ_BACK_LIMIT: u64 = 30;

/// Whether the session built from `rows` holds anything else, row order aside: a comparison that
/// cannot finish counts as a difference.
fn differs_from(session: &Session, rows: &Rows) -> bool {
    let (tables, junctions) = archforge_notations::storage();
    let Ok(built) = archforge_mof::rows::snapshot(session.doc(), &tables, &junctions) else { return true };
    archforge_mof::compare::compare(rows, &built, &tables, &junctions, archforge_mof::compare::MAX_COMPARED).map_or(true, |found| !found.is_empty())
}

/// How a job that did not plainly finish went.
pub enum JobFailure {
    /// It failed, and the project is as it was.
    Failed(String),
    /// It failed after saving the unsaved work as a version: the model is the work, now saved.
    WorkSaved(String),
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
    read: Receiver<Result<Arc<Rows>, String>>,
    job: Option<Job>,
    read_back: Option<Box<dyn FnOnce() -> ReadBack>>,
    /// Whether the job has written, after which the operation can no longer simply stop.
    wrote: bool,
    /// The version to show in place of the session, set aside rather than replaced.
    viewing: Option<String>,
    /// The session built differs from the rows read: a repair made while building it.
    repaired: bool,
    /// A branch switch still to be asked of the host, once the latest rows have gone over.
    switch: Option<Box<dyn FnOnce() -> Result<BranchSwitch, String>>>,
    /// Asked of the host: whether the branch arrived on holds unsaved work.
    arrives_unsaved: Option<Arc<AtomicBool>>,
    /// The project and branch a switch moves to, kept once it has arrived.
    switching_to: Option<(PathBuf, String)>,
}

impl Replacing {
    /// `otherwise` unless this is a switch whose check has been asked for: that may have upgraded
    /// the target's tables, so no more is claimed than that the project stayed.
    fn stopped(&self, otherwise: String) -> String {
        if self.arrives_unsaved.is_some() { tr("The switch did not finish, so the project stayed on its branch.") } else { otherwise }
    }
}

impl Shell {
    /// Restores the version `commit`, saved at `saved_at`, as a new version: the version in view
    /// gives way to current work first, and the project is held before the version is read. Refused
    /// while another hold runs, or without a model or a host, with nothing changed.
    pub fn restore_version(&mut self, commit: &str, saved_at: &str) -> Result<(), HoldRefused> {
        if self.holding.pending() {
            return Err(HoldRefused::Another);
        }
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.leave_version();
        self.begin_hold()?;
        let by = SavedBy::Person(self.state.user_name.clone());
        let job = host.restore_job(&path, commit.to_string(), saved_at.to_string(), by, editing::fit_tables);
        self.start_replacing(host.read_version(&path, commit.to_string()), Some(job), None);
        Ok(())
    }

    /// Moves the project to `branch` under the hold: the version in view gives way to current work
    /// first, the check is asked for once the latest rows have gone over, and the branch's model
    /// takes the session's place once the move has finished. Refused while another hold runs, or
    /// without a model or a host, with nothing changed.
    pub fn switch_branch(&mut self, branch: String) -> Result<(), HoldRefused> {
        if self.holding.pending() {
            return Err(HoldRefused::Another);
        }
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.leave_version();
        self.begin_hold()?;
        self.start_replacing(channel().1, None, None);
        if let Some(r) = self.replacing.as_mut() {
            r.switching_to = Some((path.clone(), branch.clone()));
            r.switch = Some(Box::new(move || host.begin_branch_switch(&path, branch)));
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
    /// `job` once it is built. What happens is driven frame by frame.
    pub fn begin_replacement(&mut self, read: Receiver<Result<Arc<Rows>, String>>, job: Option<Job>) -> Result<(), HoldRefused> {
        self.begin_hold()?;
        self.start_replacing(read, job, None);
        Ok(())
    }

    /// Starts the replacement under the hold already taken; with `viewing`, what is built shows
    /// as that version and the session is set aside.
    pub(crate) fn start_replacing(&mut self, read: Receiver<Result<Arc<Rows>, String>>, job: Option<Job>, viewing: Option<String>) {
        self.replacing = Some(Replacing {
            stage: Stage::Chat,
            read,
            job,
            read_back: None,
            wrote: false,
            viewing,
            repaired: false,
            switch: None,
            arrives_unsaved: None,
            switching_to: None,
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
    }

    /// Takes `r` one stage on; none once it has ended.
    fn advance(&mut self, mut r: Replacing) -> Option<Replacing> {
        r.stage = match std::mem::replace(&mut r.stage, Stage::Unknown) {
            Stage::Chat => match self.settle_chat() {
                ChatSettling::Waiting => Stage::Chat,
                ChatSettling::Settled => Stage::Save,
                ChatSettling::Refused => {
                    self.save_error = Some(tr("Chat did not finish in time, so nothing was changed."));
                    return None;
                }
            },
            Stage::Save if self.is_saving() => Stage::Save,
            Stage::Save => {
                if !self.hand_over_held() {
                    // The rows could not be made, and the status bar already says why.
                    return self.give_up(None);
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
                Ok(Err(why)) => return self.give_up(Some(why)),
                Ok(Ok(rows)) => match self.build(&rows) {
                    Ok(session) => {
                        r.repaired = r.viewing.is_none() && differs_from(&session, &rows);
                        Stage::Ready(Box::new(session), None)
                    }
                    Err(why) => return self.give_up(Some(why)),
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
                    match r.viewing.take() {
                        Some(commit) => self.show_version(*session, commit),
                        None => self.install(*session, r.repaired),
                    }
                    if let Some(unsaved) = r.arrives_unsaved {
                        // On another branch: the status bar's reasons about the branch left go, and
                        // the project reopens here.
                        self.save_error = None;
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

    /// A session of `rows`, repaired as an opening repairs it.
    fn build(&self, rows: &Rows) -> Result<Session, String> {
        let Some(source) = self.services.ids.clone() else { return Err(tr("The host gives no ids for the model's elements")) };
        archforge_notations::session_from_rows_repaired(rows, Box::new(move || source()), editing::fit_tables)
    }

    /// Ends the operation with the session as it was, saying `why` when given. The same session
    /// stays, so nothing else tells the Versions panel to look: a job that failed may have kept
    /// the work as a version first.
    pub(crate) fn give_up(&mut self, why: Option<String>) -> Option<Replacing> {
        if why.is_some() {
            self.save_error = why;
        }
        self.versions.stale = true;
        self.fail_hold();
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
        self.refused = None;
        self.session = Some(session);
        self.session_replaced();
        self.end_hold();
    }
}
