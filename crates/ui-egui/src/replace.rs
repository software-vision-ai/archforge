//! Replacing the session under a hold (`docs/design/versioning.md`, section 6, steps 5 to 8): a
//! save under way answers and the latest rows go over, the replacement is read and built before
//! anything is written, the operation's writes run as one job, and the replacement takes the
//! session's place. A job with no clear answer is never guessed at: Archforge says it could not
//! tell, writes nothing back, and the project stays held.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use archforge_mof::rows::Rows;
use archforge_notations::Session;

use crate::hold::{CHAT_SETTLE_LIMIT, ChatSettling, HoldRefused};
use crate::i18n::tr;
use crate::{Shell, chat_panel, editing};

/// How a job that did not plainly finish went.
pub enum JobFailure {
    /// It failed, and the project is as it was.
    Failed(String),
    /// It gave no clear answer.
    Unclear,
}

/// What a job's starting gives: its answer, later.
pub type JobAnswer = Receiver<Result<(), JobFailure>>;

/// An operation's writes, run as one job on the writer.
pub struct Job {
    pub run: Box<dyn FnOnce() -> JobAnswer>,
}

/// Where a replacement stands.
enum Stage {
    Chat,
    Save,
    Build,
    /// Built, and waiting for Chat to be still before the job or the replacement.
    Ready(Box<Session>, Option<u64>),
    Writing(Box<Session>, JobAnswer),
    /// What happened could not be told: the project stays held.
    Unknown,
}

/// A replacement under way.
pub(crate) struct Replacing {
    stage: Stage,
    read: Receiver<Result<Arc<Rows>, String>>,
    job: Option<Job>,
    /// Whether the job has written, after which the operation can no longer simply stop.
    wrote: bool,
}

impl Shell {
    /// Holds the project and starts replacing its session with the model `read` answers, running
    /// `job` once it is built. What happens is driven frame by frame.
    pub fn begin_replacement(&mut self, read: Receiver<Result<Arc<Rows>, String>>, job: Option<Job>) -> Result<(), HoldRefused> {
        self.begin_hold()?;
        self.replacing = Some(Replacing { stage: Stage::Chat, read, job, wrote: false });
        Ok(())
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
                Stage::Build
            }
            Stage::Build => match r.read.try_recv() {
                Err(TryRecvError::Empty) => Stage::Build,
                Err(TryRecvError::Disconnected) => return self.give_up(Some(tr("The reading stopped without an answer, so nothing was changed."))),
                Ok(Err(why)) => return self.give_up(Some(why)),
                Ok(Ok(rows)) => match self.build(&rows) {
                    Ok(session) => Stage::Ready(Box::new(session), None),
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
                        return self.give_up(Some(tr("Chat did not finish in time, so nothing was changed.")));
                    }
                } else if let Some(job) = r.job.take() {
                    Stage::Writing(session, (job.run)())
                } else {
                    self.install(*session);
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
                // A lost reply says nothing about what happened: it is never guessed at.
                Ok(Err(JobFailure::Unclear)) | Err(TryRecvError::Disconnected) => self.could_not_tell(),
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

    /// Ends the operation with the session as it was, saying `why` when given.
    fn give_up(&mut self, why: Option<String>) -> Option<Replacing> {
        if why.is_some() {
            self.save_error = why;
        }
        self.fail_hold();
        None
    }

    /// Says that what happened cannot be told. Nothing is written back, and the project stays held.
    fn could_not_tell(&mut self) -> Stage {
        let why = tr("Archforge could not tell whether the last change finished. Nothing more was written. Close the project and reopen it.");
        self.save_error = Some(why);
        Stage::Unknown
    }

    /// The replacement takes the session's place, and the hold ends. What was read is what the
    /// project holds, so nothing is handed over for it.
    fn install(&mut self, session: Session) {
        self.written = Some(session.revision());
        self.refused = None;
        self.session = Some(session);
        self.session_replaced();
        self.end_hold();
    }
}
