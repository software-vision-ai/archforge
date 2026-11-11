//! Viewing an earlier version, without a screen (`docs/design/versioning.md`, sections 3 and 6): the
//! version is read and built under the hold, the real session and its view are set aside untouched,
//! and the version shows read-only in their place until the person goes back. While a version shows
//! the model is not handed to the host and Save is refused. It opens on the diagram that was
//! showing if it has it, otherwise its first. Showing another tab or Home, opening a project,
//! closing the project shown and quitting all return to current work first, so the work set aside
//! goes back to its own project. A step to another version takes no hold ([`crate::version_load`]).

use archforge_mof::Uuid;
use archforge_mof::rows::Rows;
use archforge_notations::Session;

use crate::hold::HoldRefused;
use crate::leaving::{Cleared, Leaving};
use crate::{Shell, canvas, diagrams, explorer, panels};

/// The version being viewed and everything set aside for going back.
pub(crate) struct Viewing {
    commit: String,
    session: Session,
    written: Option<u64>,
    refused: Option<u64>,
    save_error: Option<String>,
    explorer: explorer::State,
    diagrams: diagrams::State,
    canvas: canvas::State,
    panels: panels::State,
}

impl Viewing {
    /// Whether the real work holds changes not yet saved.
    pub(crate) fn unsaved(&self) -> bool {
        self.session.is_dirty() || panels::description_pending(&self.panels)
    }
}

impl Shell {
    /// The version being viewed, if one is.
    pub fn viewing(&self) -> Option<&str> {
        self.viewing.as_ref().map(|v| v.commit.as_str())
    }

    /// Whether a version shows, so that looking at it is all that is on.
    pub(crate) fn looking_only(&self) -> bool {
        self.viewing.is_some()
    }

    /// The version shown or being read to be shown: while there is one, what may change anything
    /// is refused on the control channel.
    pub(crate) fn version_in_view(&self) -> Option<&str> {
        self.viewing().or_else(|| self.version_being_read())
    }

    /// Starts viewing the version `commit`: the project is held while it is read and built, and it
    /// shows once it has been. Refused while a version shows and as [`Self::clear_to_leave`] says.
    /// A read that fails ends the hold and says why, with the real work as it was.
    pub fn view_version(&mut self, commit: &str) -> Result<(), HoldRefused> {
        if self.viewing.is_some() {
            return Err(HoldRefused::Another);
        }
        self.move_to_version(commit, None)
    }

    /// Goes to the version `commit`, the chip then saying `label` of it when it has one. From current
    /// work it enters under a hold, Chat's question first. From a version it is a step, with no hold
    /// or question as no current work is left, and it ends a step still waiting for a read, which a
    /// hold ends from current work. A refusal leaves what shows as it was.
    pub(crate) fn move_to_version(&mut self, commit: &str, label: Option<String>) -> Result<(), HoldRefused> {
        if self.viewing.is_some() {
            self.clear_to_leave(None)?;
            self.version_list.steps.waiting = None;
            return self.step_to(commit, label);
        }
        let cleared = self.clear_to_leave(Some(Leaving::View { commit: commit.to_string(), label: label.clone() }))?;
        self.read_to_view(commit, cleared)?;
        if let Some(label) = label {
            self.version_list.shown = Some((commit.to_string(), label));
        }
        Ok(())
    }

    /// The version a step goes from: the one a step has asked for, otherwise the one in view.
    pub(crate) fn stepping_from(&self) -> Option<&str> {
        self.loads.wanted_commit().or_else(|| self.viewing())
    }

    /// The version a step asked for has been read, as `rows`: it takes the place of the one in
    /// view, on the diagram in view when it arrives if it has it, otherwise its first, and what
    /// was set aside stays as it is. One that cannot be built changes nothing and gives why.
    pub(crate) fn show_step(&mut self, rows: &Rows, commit: String, label: Option<String>) -> Result<(), String> {
        let mut preview = self.build(rows)?;
        preview.set_read_only(true).map_err(|e| e.to_string())?;
        let Some(viewing) = self.viewing.as_mut() else { return Ok(()) };
        viewing.commit.clone_from(&commit);
        let showing = self.canvas.active;
        (self.explorer, self.diagrams, self.canvas, self.panels) = Default::default();
        self.session = Some(preview);
        self.session_replaced();
        self.open_in_version(showing);
        if let Some(label) = label {
            self.version_list.shown = Some((commit, label));
        }
        Ok(())
    }

    /// Holds the project and has `commit` read to be shown.
    fn read_to_view(&mut self, commit: &str, cleared: Cleared) -> Result<(), HoldRefused> {
        let (Some(_), Some(path)) = (&self.services.projects, self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.begin_hold()?;
        let read = self.read_for_entry(path, commit);
        self.start_replacing(read, None, Some(commit.to_string()), cleared);
        Ok(())
    }

    /// The version read takes the session's place, and the real session and its view are set aside
    /// as they are. The hold ends first, while the real session is in place, so that it gets back
    /// the read-only state it had before the hold. It opens on the diagram showing, if it has it.
    pub(crate) fn show_version(&mut self, mut preview: Session, commit: String) {
        if let Err(e) = preview.set_read_only(true) {
            self.give_up(Some(e.to_string()));
            return;
        }
        self.end_hold();
        let showing = self.canvas.active;
        let Some(session) = self.session.take() else { return };
        self.viewing = Some(Box::new(Viewing {
            commit,
            session,
            written: self.written.take(),
            refused: self.refused.take(),
            save_error: self.save_error.take(),
            explorer: std::mem::take(&mut self.explorer),
            diagrams: std::mem::take(&mut self.diagrams),
            canvas: std::mem::take(&mut self.canvas),
            panels: std::mem::take(&mut self.panels),
        }));
        self.session = Some(preview);
        self.session_replaced();
        self.open_in_version(showing);
    }

    /// Opens in the version just shown the diagram `showing` if it holds one by that id, otherwise
    /// the first of its Diagrams list; without any its canvas stays empty. Nothing of the set-aside
    /// view comes along, so a new tab fits the diagram as the version has it.
    fn open_in_version(&mut self, showing: Option<Uuid>) {
        let Some(store) = self.session.as_ref().map(Session::doc) else { return };
        let holds = |id: &Uuid| store.by_uuid(*id).is_some_and(|o| store.class_of(o).ok() == Some(archforge_diagram::DIAGRAM));
        let first = || diagrams::rows(store, &self.diagrams, self.kinds).rows.iter().find(|r| r.kind == diagrams::Kind::Diagram).map(|r| r.id);
        if let Some(id) = showing.filter(holds).or_else(first) {
            canvas::open(self, id);
        }
    }

    /// Back to current work: what was set aside returns as it was. A version still being read is
    /// dropped instead: its answer can then replace nothing and only goes into the cache. False
    /// when neither applies. Any step on its way is called off.
    pub fn back_to_current_work(&mut self) -> bool {
        self.loads.call_off();
        if self.version_in_view().is_some() {
            self.versions.load_failed = None;
        }
        if self.version_being_read().is_some() {
            self.replacing = None;
            self.end_hold();
            return true;
        }
        let Some(v) = self.viewing.take() else { return false };
        let v = *v;
        self.session = Some(v.session);
        self.written = v.written;
        self.refused = v.refused;
        self.save_error = v.save_error;
        self.explorer = v.explorer;
        self.diagrams = v.diagrams;
        self.canvas = v.canvas;
        self.panels = v.panels;
        self.session_replaced();
        true
    }

    /// Leaves a version that shows, before anything acts on the real project.
    pub(crate) fn leave_version(&mut self) {
        if self.viewing.is_some() {
            self.back_to_current_work();
        }
    }
}
