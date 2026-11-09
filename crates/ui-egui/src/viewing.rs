//! Viewing an earlier version, without a screen (`docs/design/versioning.md`, sections 3 and 6): the
//! version is read and built under the hold, the real session and its view are set aside untouched,
//! and the version shows read-only in their place until the person goes back. While a version shows
//! the model is not handed to the host and Save is refused. Leaving the project while one shows is
//! not handled yet (AR-316), see [`Shell::view_version`].

use archforge_notations::Session;

use crate::hold::HoldRefused;
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

    /// Starts viewing the version `commit`: the project is held while it is read and built, and it
    /// shows once it has been. Refused while a version shows or another hold runs, and without a
    /// model or a host. A read that fails ends the hold and says why, with the real work as it was.
    ///
    /// Nothing may call this until AR-316 lands: the ways of leaving a project do not return to
    /// current work yet, so parking or closing while a version shows would lose the real session.
    pub fn view_version(&mut self, commit: &str) -> Result<(), HoldRefused> {
        if self.viewing.is_some() {
            return Err(HoldRefused::Another);
        }
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        self.begin_hold()?;
        self.start_replacing(host.read_version(&path, commit.to_string()), None, Some(commit.to_string()));
        Ok(())
    }

    /// The version read takes the session's place, and the real session and its view are set aside
    /// as they are. The hold ends first, while the real session is in place, so that it gets back
    /// the read-only state it had before the hold.
    pub(crate) fn show_version(&mut self, mut preview: Session, commit: String) {
        if let Err(e) = preview.set_read_only(true) {
            self.give_up(Some(e.to_string()));
            return;
        }
        self.end_hold();
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
    }

    /// Back to current work: what was set aside returns as it was. False when no version shows,
    /// which includes one still being read (AR-316 handles going back from those).
    pub fn back_to_current_work(&mut self) -> bool {
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
}
