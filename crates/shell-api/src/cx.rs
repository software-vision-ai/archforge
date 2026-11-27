//! The narrow context: the parts of the shell a notation's screens and the shared controls reach,
//! borrowed from it apart, in place of the whole `Shell` (ADR 0002: the interfaces hand a notation a
//! context that offers only what its screens need). [`Ctx`] is the context to change things
//! through, [`CtxRef`] its read-only twin; the shell splits its borrows into one
//! (`archforge_ui_egui::Shell::cx`) or the other (`Shell::cx_ref`).
//!
//! What a notation may ask of the rest of the shell, which the context does not hold, it asks as a
//! [`Request`]: the shell carries the requests out after the call that asked them returns, in the
//! order they were asked, each once.

use std::collections::BTreeMap;

use archforge_diagram::ProjectView;
use archforge_mof::{Obj, Store, Uuid};

use crate::explorer::Delete;
use crate::notation_ui::{NotationStates, NotationUi};
use crate::state::{Explorer, Panel};
use crate::{canvas, menu_bar, panels, widgets};

/// A project's session, as the notations' registry composes it (`archforge_notations::Session`),
/// named from the crates below it, so this crate takes no notation.
type Session = archforge_engine::Session<Store, ProjectView>;

/// The shell's parts a notation's screens and the shared controls change, each borrowed apart, so
/// two of them can be held at once. The fields carry the shell's own names.
pub struct Ctx<'a> {
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
pub struct CtxRef<'a> {
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
pub enum Request {
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
    pub fn cx_ref(&self) -> CtxRef<'_> {
        CtxRef {
            session: self.session,
            canvas: self.canvas,
            notation_uis: self.notation_uis,
            notation_states: self.notation_states,
            looking_only: self.looking_only,
        }
    }
}
