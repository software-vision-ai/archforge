//! Steps between versions (`docs/design/versioning.md`, section 3): at most one read is under way,
//! and the newest version asked for is the one that shows. An answer overtaken is dropped. The
//! version in view stays until the next has arrived, and one that cannot be read or built leaves it.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use archforge_mof::rows::Rows;

use crate::Shell;
use crate::hold::HoldRefused;
use crate::i18n::tr;

/// What a read answers: a version's rows, or why they could not be read.
type Answer = Result<Arc<Rows>, String>;

/// The one read under way, of the version `commit`. A commit's hash stands for its content, so an
/// answer read for one project is as true of another that holds the same hash.
struct Reading {
    commit: String,
    answer: Receiver<Answer>,
}

/// The newest version a step asked for, and what the chip then says of it.
struct Wanted {
    commit: String,
    label: Option<String>,
}

/// The read under way and the newest version asked for, which may differ when it was overtaken.
#[derive(Default)]
pub(crate) struct Loads {
    reading: Option<Reading>,
    wanted: Option<Wanted>,
}

impl Loads {
    /// Whether a read is under way or a version waits to be read: the window must look again for
    /// the answer, as no other event will bring it.
    pub(crate) fn waiting(&self) -> bool {
        self.reading.is_some() || self.wanted.is_some()
    }

    /// Calls off the version asked for. A read under way goes on, as it cannot be stopped, and its
    /// answer is dropped.
    pub(crate) fn call_off(&mut self) {
        self.wanted = None;
    }

    /// The version a step has asked for and that has not arrived.
    pub(crate) fn wanted_commit(&self) -> Option<&str> {
        self.wanted.as_ref().map(|w| w.commit.as_str())
    }
}

impl Shell {
    /// Asks for the version `commit` from the version in view, overtaking the one asked for before;
    /// the version in view itself calls that one off. Refused without a host.
    pub(crate) fn step_to(&mut self, commit: &str, label: Option<String>) -> Result<(), HoldRefused> {
        if self.viewing() == Some(commit) {
            self.loads.call_off();
            return Ok(());
        }
        if self.services.projects.is_none() {
            return Err(HoldRefused::NoProject);
        }
        self.loads.wanted = Some(Wanted { commit: commit.to_string(), label });
        self.read_wanted();
        Ok(())
    }

    /// Starts the read of the version asked for, when none is under way.
    fn read_wanted(&mut self) {
        let (None, Some(wanted), Some(host), Some(open)) = (&self.loads.reading, &self.loads.wanted, &self.services.projects, &self.project) else {
            return;
        };
        let answer = host.read_version(&open.path, wanted.commit.clone());
        self.loads.reading = Some(Reading { commit: wanted.commit.clone(), answer });
    }

    /// Once a frame: takes the answer of the read under way, and starts the read of the version
    /// asked for if that one was overtaken.
    pub(crate) fn drive_loads(&mut self) {
        let answer = self.loads.reading.as_ref().and_then(|r| match r.answer.try_recv() {
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(tr("The reading stopped without an answer, so nothing was changed."))),
            Ok(answer) => Some(answer),
        });
        if let Some(answer) = answer {
            self.take_answer(answer);
        }
        self.read_wanted();
    }

    /// The read under way answered: only the version still asked for goes on. A step that failed
    /// stays on the version in view and says why at the top of the centre.
    fn take_answer(&mut self, answer: Answer) {
        let Some(read) = self.loads.reading.take() else { return };
        if self.loads.wanted_commit() != Some(read.commit.as_str()) {
            return;
        }
        let label = self.loads.wanted.take().and_then(|w| w.label);
        if let Err(why) = answer.and_then(|rows| self.show_step(&rows, read.commit, label)) {
            self.canvas.notice = Some(why);
        }
    }
}
