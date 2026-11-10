//! Viewing an earlier version, without a screen (`docs/design/versioning.md`, sections 3 and 6): the
//! version is read and built under the hold, the real session and its view are set aside untouched,
//! and the version shows read-only in their place until the person goes back. While a version shows
//! the model is not handed to the host and Save is refused. It opens on the diagram that was
//! showing if it has it, otherwise its first. Showing another tab or Home, opening a project,
//! closing the project shown and quitting all return to current work first, so the work set aside
//! goes back to its own project.

use archforge_mof::Uuid;
use archforge_notations::Session;

use crate::hold::HoldRefused;
use crate::{Shell, canvas, diagrams, explorer, panels};

/// Which diagram a version opens on, when it holds it.
#[derive(Clone, Copy)]
pub(crate) enum OpensOn {
    /// The one showing when the version arrives: entering from current work.
    Showing,
    /// The one in view in the version left for this one, if any: never current work's.
    Carried(Option<Uuid>),
}

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
    /// shows once it has been. Refused while a version shows or another hold runs, and without a
    /// model or a host. A read that fails ends the hold and says why, with the real work as it was.
    pub fn view_version(&mut self, commit: &str) -> Result<(), HoldRefused> {
        if self.viewing.is_some() {
            return Err(HoldRefused::Another);
        }
        self.read_to_view(commit, OpensOn::Showing)
    }

    /// Goes from the version shown, if one is, to the version `commit`, staying on the diagram in
    /// view there rather than the one current work has open. A refusal, as for
    /// [`Self::view_version`], comes once the version has been left, so current work then shows.
    pub(crate) fn move_to_version(&mut self, commit: &str) -> Result<(), HoldRefused> {
        let opens_on = if self.viewing.is_some() { OpensOn::Carried(self.canvas.active) } else { OpensOn::Showing };
        self.leave_version();
        self.read_to_view(commit, opens_on)
    }

    /// Holds the project and has `commit` read to be shown. The diagram to open goes with the read,
    /// so a read dropped or refused leaves nothing behind for the next.
    fn read_to_view(&mut self, commit: &str, opens_on: OpensOn) -> Result<(), HoldRefused> {
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.begin_hold()?;
        self.start_replacing(host.read_version(&path, commit.to_string()), None, Some((commit.to_string(), opens_on)));
        Ok(())
    }

    /// The version read takes the session's place, and the real session and its view are set aside
    /// as they are. The hold ends first, while the real session is in place, so that it gets back
    /// the read-only state it had before the hold. The version then opens as `opens_on` says.
    pub(crate) fn show_version(&mut self, mut preview: Session, commit: String, opens_on: OpensOn) {
        if let Err(e) = preview.set_read_only(true) {
            self.give_up(Some(e.to_string()));
            return;
        }
        self.end_hold();
        let showing = match opens_on {
            OpensOn::Showing => self.canvas.active,
            OpensOn::Carried(id) => id,
        };
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
    /// dropped instead, and with its channel its answer, which can then replace nothing. False when
    /// neither applies.
    pub fn back_to_current_work(&mut self) -> bool {
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
