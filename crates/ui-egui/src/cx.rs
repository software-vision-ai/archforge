//! The narrow context: the parts of the shell a notation's screens and the shared controls reach,
//! borrowed from it apart, in place of the whole `Shell` (ADR 0002: the interfaces hand a notation a
//! context that offers only what its screens need). [`Ctx`] is the context to change things
//! through, [`CtxRef`] its read-only twin; [`Shell::cx`] splits the shell's borrows into one, and
//! [`Shell::cx_ref`] into the other.
//!
//! What a notation may ask of the rest of the shell, which the context does not hold, it asks as a
//! [`Request`]: the shell carries the requests out after the call that asked them returns, in the
//! order they were asked, each once.

use std::collections::BTreeMap;

use archforge_mof::{Obj, Uuid};
use archforge_notations::Session;

use crate::explorer::Delete;
use crate::notation_ui::{NotationStates, NotationUi};
use crate::state::{Explorer, Panel};
use crate::{Shell, canvas, menu_bar, panels, widgets};

/// The shell's parts a notation's screens and the shared controls change, each borrowed apart, so
/// two of them can be held at once. The fields carry the shell's own names.
pub(crate) struct Ctx<'a> {
    /// The open project's model, when one is open.
    pub session: &'a mut Option<Session>,
    /// The right panels' own state: the open menu and the keys in it, the notice, the name being
    /// typed, the editors to bring into view.
    pub panels: &'a mut panels::State,
    /// The centre's own state: the open tabs, the view, the tool, the notice and the toast.
    pub canvas: &'a mut canvas::State,
    /// The controls laid out this frame, for automation.
    pub registry: &'a mut widgets::Registry,
    /// The notations the shell knows.
    pub notation_uis: &'static [&'static dyn NotationUi],
    /// Each notation's own state: its editors, menus, drawings and tools.
    pub notation_states: &'a mut NotationStates,
    /// The problems the rules find, by element, read once this frame.
    pub problems: &'a BTreeMap<Obj, Vec<String>>,
    /// Frames run so far.
    pub frames: u64,
    /// What the in-window menu bar knows of the inline editor it holds open.
    pub menu_bar: menu_bar::Hold<'a>,
    /// Whether a version shows, so that looking at it is all that is on.
    pub looking_only: bool,
    /// What the shell is to carry out after the call returns, in this order.
    pub requests: Vec<Request>,
}

/// The read-only twin of [`Ctx`]: the parts read by what only looks.
pub(crate) struct CtxRef<'a> {
    /// The open project's model, when one is open.
    pub session: &'a Option<Session>,
    /// The centre's own state: the open tabs, the view, the tool, the notice and the toast.
    pub canvas: &'a canvas::State,
    /// The notations the shell knows.
    pub notation_uis: &'static [&'static dyn NotationUi],
    /// Each notation's own state: its editors, menus, drawings and tools.
    pub notation_states: &'a NotationStates,
    /// Whether a version shows, so that looking at it is all that is on.
    pub looking_only: bool,
}

/// What the shell carries out for the context once the call that asked it has returned: the parts
/// of the shell the context does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    /// The Models tree opens the rows of these elements, so a row under them shows.
    ExpandRows(Vec<Uuid>),
    /// The centre stops showing a version's details or a changed element, so what was opened shows.
    HideVersionDetails,
    /// The right side shows this panel, unfolded.
    ShowPanel(Panel),
    /// The left side shows this explorer, unfolded.
    ShowExplorer(Explorer),
    /// Models opens the name of this element, just made with the name given, for typing in place:
    /// its filter goes, so the row is listed, the name is selected whole, so typing replaces it, and
    /// the keyboard goes to Models.
    NameInModels(Uuid, String),
    /// The delete dialog asks about this removal, naming what goes with it.
    AskDelete(Delete),
}

impl Ctx<'_> {
    /// The read-only twin of this context, over the same parts.
    pub(crate) fn cx_ref(&self) -> CtxRef<'_> {
        CtxRef {
            session: self.session,
            canvas: self.canvas,
            notation_uis: self.notation_uis,
            notation_states: self.notation_states,
            looking_only: self.looking_only,
        }
    }
}

impl Shell {
    /// Runs `f` with the narrow context over this shell's parts, then carries out the requests `f`
    /// made, in the order made, each once, and gives what `f` gave.
    pub(crate) fn cx<R>(&mut self, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
        let looking_only = self.looking_only();
        let mut cx = Ctx {
            session: &mut self.session,
            panels: &mut self.panels,
            canvas: &mut self.canvas,
            registry: &mut self.registry,
            notation_uis: self.notation_uis,
            notation_states: &mut self.notation_states,
            problems: &self.problems,
            frames: self.frames,
            menu_bar: self.menu_bar.hold(),
            looking_only,
            requests: Vec::new(),
        };
        let out = f(&mut cx);
        let requests = cx.requests;
        self.carry_out(requests);
        out
    }

    /// The read-only twin of the narrow context over this shell's parts.
    pub(crate) fn cx_ref(&self) -> CtxRef<'_> {
        CtxRef {
            session: &self.session,
            canvas: &self.canvas,
            notation_uis: self.notation_uis,
            notation_states: &self.notation_states,
            looking_only: self.looking_only(),
        }
    }

    /// Carries out the context's `requests`, in their order.
    fn carry_out(&mut self, requests: Vec<Request>) {
        for request in requests {
            match request {
                Request::ExpandRows(rows) => self.explorer.expanded.extend(rows),
                Request::HideVersionDetails => (self.versions.details, self.versions.element) = (None, None),
                Request::ShowPanel(panel) => self.state.show_panel(panel),
                Request::ShowExplorer(explorer) => self.state.show_explorer(explorer),
                Request::NameInModels(id, name) => {
                    self.explorer.filter.clear();
                    self.explorer.rename = Some(crate::explorer::Rename::fresh(id, name));
                    self.explorer.focused = true;
                }
                Request::AskDelete(delete) => self.explorer.delete = Some(delete),
            }
        }
    }
}
