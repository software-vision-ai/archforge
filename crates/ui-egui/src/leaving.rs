//! The guards before leaving current work (`docs/design/versioning.md`, section 6). Whatever
//! replaces the session comes through [`Shell::clear_to_leave`], and `start_replacing` asks for
//! its proof, so no way in can skip it. An open editor, a drawing, a drag, a dialog or a refused
//! description stops it with nothing changed. A Chat turn running asks first, and Stop and go on
//! runs what was asked for through the same guards; the hold then ends the turn.

use crate::close_ask::Ask;
use crate::hold::HoldRefused;
use crate::{Shell, chat_panel, panels, skill_tab, version_chip};

/// What the person was leaving for, kept with its own target while Chat's question waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leaving {
    /// The version `commit`; `label` is what the chip then says of it, if it was picked there.
    View { commit: String, label: Option<String> },
    /// The version `commit`, saved at `saved_at`, restored as a new version.
    Restore { commit: String, saved_at: String },
    /// The branch named, the question about unsaved work answered or not needed.
    Switch(String),
    /// The branch named, the question about unsaved work still to come.
    AskSwitch(String),
}

/// The proof that the guards let an operation through; only [`Shell::clear_to_leave`] makes one.
pub(crate) struct Cleared(());

impl Shell {
    /// Whether an inline editor, a drawing, a drag or a dialog is open on what shows.
    fn editing(&self) -> bool {
        let (c, e) = (&self.canvas, &self.explorer);
        let canvas =
            c.rename.is_some() || c.inline.is_some() || c.note_edit.is_some() || c.gesture.is_some() || c.nary.is_some() || c.minimap_drag.is_some();
        canvas || e.rename.is_some() || e.dragging.is_some() || panels::editing(&self.panels) || version_chip::dialog_open(self)
    }

    /// The one gate before the work shown is left. Refused, in this order and with nothing changed:
    /// while a hold runs; without a model; while an editor, a drawing, a drag or a dialog is open.
    /// Past those, with `then` given, a Chat turn running opens the question for it, unless a text
    /// already refused stops it anyway or the question has just been answered; without `then`
    /// nothing is asked, as a hold settles the turn or a step leaves no current work. Only past the
    /// question are the description and skill texts being typed written, and one refused stops it,
    /// kept and said. Only then does a colour picker close, so a refusal leaves it open.
    pub(crate) fn clear_to_leave(&mut self, then: Option<Leaving>) -> Result<Cleared, HoldRefused> {
        let answered = std::mem::take(&mut self.chat_answered);
        if self.holding.pending() {
            return Err(HoldRefused::Another);
        }
        if self.session.is_none() || self.project.is_none() {
            return Err(HoldRefused::NoProject);
        }
        if self.editing() {
            return Err(HoldRefused::Editing);
        }
        let refused = self.viewing.is_none() && (panels::refused_description(&self.panels) || !skill_tab::unkept(&self.canvas).is_empty());
        if let Some(then) = then.filter(|_| !answered && !refused && chat_panel::turn_running(self)) {
            let path = self.project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
            self.ask = Some(Ask::StopChat { path, generation: self.session_generation(), then });
            return Err(HoldRefused::ChatRunning);
        }
        // A version shows read-only, and what was set aside with the work was written on entering.
        if self.viewing.is_none() {
            skill_tab::sync(self, 0.0, true, None);
            if panels::commit_description(self) || !skill_tab::unkept(&self.canvas).is_empty() {
                return Err(HoldRefused::Description);
            }
        }
        // As Cancel does: it holds nothing unwritten, and writes only on Apply.
        self.panels.picker = None;
        Ok(Cleared(()))
    }

    /// Stop and go on was answered: `then` runs through the guards again, past the question, and
    /// the hold it starts ends the turn. What stops it now is said, and the version details it came
    /// from close only once it goes ahead.
    pub(crate) fn go_on_leaving(&mut self, then: Leaving) {
        self.chat_answered = true;
        let done = match then {
            Leaving::View { commit, label } => self.move_to_version(&commit, label),
            Leaving::Restore { commit, saved_at } => self.restore_version(&commit, &saved_at),
            Leaving::Switch(branch) => self.switch_branch(branch),
            Leaving::AskSwitch(branch) => self.ask_switch_branch(branch),
        };
        let words = done.err().and_then(|why| version_chip::refused_words(self, why));
        self.canvas.say_refused_step(words);
        if done.is_ok() {
            self.versions.details = None;
        }
    }
}
