//! The guards before leaving current work (`docs/design/versioning.md`, section 6). Whatever
//! replaces the session comes through [`Shell::clear_to_leave`], and `start_replacing` asks for
//! its proof, so no way in can skip it. An open editor, a drawing, a drag, a dialog or a refused
//! description stops it with nothing changed. A Chat turn running is settled by the hold; AR-358
//! asks about it first.

use crate::hold::HoldRefused;
use crate::state::Panel;
use crate::{Shell, panels, skill_tab, version_chip};

/// The proof that the guards let an operation through; only [`Shell::clear_to_leave`] makes one.
pub(crate) struct Cleared(());

impl Shell {
    /// Whether an inline editor, a drawing, a drag or a dialog is open on what shows.
    fn editing(&self) -> bool {
        let (c, e) = (&self.canvas, &self.explorer);
        let canvas =
            c.rename.is_some() || c.inline.is_some() || c.note_edit.is_some() || c.gesture.is_some() || c.nary.is_some() || c.minimap_drag.is_some();
        // Only while it shows: hidden, it has nothing to lose, as the picker writes only on Apply.
        let picker = self.panels.picker.is_some() && self.state.panel == Panel::Appearance && !self.state.panel_collapsed;
        canvas || picker || e.rename.is_some() || e.dragging.is_some() || panels::editing(&self.panels) || version_chip::dialog_open(self)
    }

    /// The one gate before the work shown is left. Refused, in this order and with nothing changed:
    /// while a hold runs; without a model; while an editor, a drawing, a drag or a dialog is open.
    /// Past those the description and skill texts being typed are written, and one refused stops it,
    /// kept and said.
    pub(crate) fn clear_to_leave(&mut self) -> Result<Cleared, HoldRefused> {
        if self.holding.pending() {
            return Err(HoldRefused::Another);
        }
        if self.session.is_none() || self.project.is_none() {
            return Err(HoldRefused::NoProject);
        }
        if self.editing() {
            return Err(HoldRefused::Editing);
        }
        // A version shows read-only, and what was set aside with the work was written on entering.
        if self.viewing.is_none() {
            skill_tab::sync(self, 0.0, true, None);
            if panels::commit_description(self) || !skill_tab::unkept(&self.canvas).is_empty() {
                return Err(HoldRefused::Description);
            }
        }
        Ok(Cleared(()))
    }
}
