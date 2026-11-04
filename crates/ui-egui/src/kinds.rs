//! The tools and gestures of each diagram kind: a kind brings its toolbar
//! tools, each placing an element or connecting two, and the hooks the canvas's gestures run on
//! its elements: place, connect, a line dropped on empty canvas, renaming in place, opening an
//! element's editor at a row of its card, a second adorner, and the live line while connecting.
//! Select, Pan, Note, Link note, Grid and Arrange stay shared; the status marks toggle shows where
//! a kind has marks (ER's key status).

use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke};

use crate::Shell;
use crate::canvas::View;
use crate::ux::{Icon, Palette};

/// What a kind's tool does on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolUse {
    /// A click on empty canvas places a new element there.
    Place,
    /// A drag from an element's node to another's connects them.
    Connect,
}

/// A tool a diagram kind brings to the toolbar, after Select and Pan.
pub(crate) struct KindTool {
    /// Its automation key (`workspace.tools.{key}`).
    pub(crate) key: &'static str,
    pub(crate) icon: &'static Icon,
    /// Its hover text, in the catalogue.
    pub(crate) hint: &'static str,
    pub(crate) use_: ToolUse,
    /// A chevron beside it that opens the kind's popover, with its key and hover text; the tool is
    /// twice as wide then, so what it draws reads.
    pub(crate) chevron: Option<(&'static str, &'static str)>,
}

/// What a diagram kind does with the canvas's tools and gestures.
pub(crate) trait DiagramKind: Sync {
    /// The diagram kind it serves, as the diagram metamodel keeps it.
    fn kind(&self) -> &'static str;
    /// Its tools, in toolbar order.
    fn tools(&self) -> &'static [KindTool];
    /// Its name in the New diagram dialog.
    fn word(&self) -> String;
    /// The name of its group in the Diagrams explorer, which holds its diagrams.
    fn group_word(&self) -> String;
    /// What its diagrams show, in a sentence: the New diagram dialog's card for it.
    fn description(&self) -> String;
    /// The New diagram dialog's title once it is chosen ("New class diagram").
    fn new_word(&self) -> String;
    /// A small picture of one of its diagrams in `r`, for its card in the New diagram dialog.
    fn sketch(&self, paint: &egui::Painter, p: &Palette, r: Rect) {
        let _ = (paint, p, r);
    }
    /// The icon its diagrams show in their tabs, the Diagrams explorer and the search.
    fn icon(&self) -> &'static Icon {
        &crate::ux::icons::ER_DIAGRAM
    }
    /// What its diagrams are of: the word for it, and the choice that makes a new one.
    fn owner_word(&self) -> String;
    fn new_owner(&self) -> String;
    /// The words for "a new one" in the New diagram dialog, as `store` stands: by default
    /// `new_owner`; a kind with a default home names that home while it is still to be made.
    fn new_owner_in(&self, _store: &Store) -> String {
        self.new_owner()
    }
    /// The elements its diagrams may be of, in the model's order.
    fn owners(&self, store: &Store) -> Vec<Obj>;
    /// The element a diagram of it would be of, from the selection.
    fn owner_of(&self, store: &Store, selected: Obj) -> Option<Obj>;
    /// Makes a new element for a diagram to be of, named `name`; its id.
    fn add_owner(&self, shell: &mut Shell, name: &str) -> Result<String, String>;
    /// Whether the toolbar shows the status marks toggle.
    fn status_marks(&self) -> bool {
        false
    }
    /// Draws a tool's face itself; false draws its icon.
    #[allow(clippy::too_many_arguments, reason = "a face is drawn from the tool, its button, its ink and its state")]
    fn tool_face(&self, shell: &Shell, paint: &egui::Painter, p: &Palette, key: &str, b: Rect, ink: egui::Color32, active: bool) -> bool {
        let _ = (shell, paint, p, key, b, ink, active);
        false
    }
    /// The popover a tool's chevron opens, drawn while it is open.
    fn popover(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, chevron: Rect) {
        let _ = (shell, ui, p, r, chevron);
    }
    /// Whether the element a node shows takes part in the kind's gestures: connecting from and to
    /// it, its adorners, renaming it in place, its menu and opening its editor.
    fn takes_part(&self, store: &Store, element: Obj) -> bool;
    /// Places a new element at `(x, y)` on diagram `d` with the tool `tool`.
    fn place(&self, shell: &mut Shell, tool: &'static str, d: Obj, x: f64, y: f64);
    /// Connects the elements of two nodes with the tool `tool`.
    fn connect(&self, shell: &mut Shell, tool: &'static str, d: Obj, from_node: Uuid, to_node: Uuid);
    /// A line of the tool `tool` dropped on empty canvas at `(x, y)`: a new element there, joined
    /// to the node's.
    fn connect_to_new(&self, shell: &mut Shell, tool: &'static str, d: Obj, from_node: Uuid, x: f64, y: f64);
    /// Whether ⇧⌫ deletes the element of a selected node from the model: by default those that take
    /// part in the gestures.
    fn deletes(&self, store: &Store, element: Obj) -> bool {
        self.takes_part(store, element)
    }
    /// Whether an element that takes no part in the gestures still renames in place with a double
    /// click on its card (a package on a class diagram).
    fn renames_in_place(&self, store: &Store, element: Obj) -> bool {
        let _ = (store, element);
        false
    }
    /// Renames the element as typed in place.
    fn rename(&self, shell: &mut Shell, element: Uuid, name: &str) -> Result<(), String>;
    /// Opens the element's editor, at the row of its card under a double click (None: the editor's
    /// new row, as the second adorner asks).
    fn open(&self, shell: &mut Shell, element: Uuid, row: Option<usize>);
    /// Whether a selected line's ends drag onto another element to reconnect it.
    fn reconnects(&self) -> bool {
        false
    }
    /// Moves the source or target end of the line element `element` to the element of `to_node`.
    fn reconnect(&self, shell: &mut Shell, element: Uuid, source: bool, to_node: Uuid) {
        let _ = (shell, element, source, to_node);
    }
    /// Places an element dragged from the Models explorer on diagram `d`, centred on `at`.
    fn drop(&self, shell: &mut Shell, d: Obj, element: Uuid, at: (f64, f64)) {
        let _ = (shell, d, element, at);
    }
    /// The second adorner of a selected element, beside Connect: its key and hover text; it opens
    /// the editor's new row.
    fn adorner(&self) -> Option<(&'static str, &'static str)> {
        None
    }
    /// The other new row a selected element takes, with Shift added to the adorner's keys (a
    /// class's operation, beside its attribute); none by default.
    fn open_second(&self, shell: &mut Shell, element: Uuid) {
        let _ = (shell, element);
    }
    /// Controls a selected element's card holds on it (a class's + for each compartment), drawn
    /// with its adorners; none by default.
    fn card_controls(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, clip: Rect, rect: Rect, element: Uuid) {
        let _ = (shell, ui, p, clip, rect, element);
    }
    /// Whether a right click opens the kind's element menu.
    fn menu(&self) -> bool {
        false
    }
    /// The line drawn from a node to the pointer while connecting; a plain line by default.
    #[allow(clippy::too_many_arguments, reason = "a live line is drawn from its node to the pointer over the view")]
    fn live_line(&self, shell: &Shell, paint: &egui::Painter, p: &Palette, view: View, from_node: Uuid, pointer: Pos2, target: Option<Rect>) {
        let Some(rect) =
            shell.session.as_ref().and_then(|s| s.doc().by_uuid(from_node).map(|n| view.rect(&archforge_diagram::grid::bounds_of(s.doc(), n))))
        else {
            return;
        };
        let end = target.map_or(pointer, |t| crate::canvas::exit_point(t, rect.center()));
        let start = crate::canvas::exit_point(rect, end);
        paint.line_segment([start, end], Stroke::new(1.5, p.accent));
        if let Some(t) = target {
            paint.rect_stroke(t.expand(4.0), 4.0, Stroke::new(1.5, p.accent), egui::StrokeKind::Inside);
        }
    }
}

/// The kinds the shell knows.
pub(crate) static KINDS: &[&dyn DiagramKind] = &[&crate::er_kind::ErKind, &crate::uml_ui::UmlKind];

/// The kind of the diagram `d`, when the shell knows it.
pub(crate) fn kind_of(shell: &Shell, store: &Store, d: Obj) -> Option<&'static dyn DiagramKind> {
    let kind = store.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string))?;
    shell.kinds.iter().copied().find(|k| k.kind() == kind)
}

/// The icon of the diagram `d`: its kind's.
pub(crate) fn diagram_icon(kinds: &[&'static dyn DiagramKind], store: &Store, d: Obj) -> &'static Icon {
    let kind = store.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    kinds.iter().find(|k| k.kind() == kind).map_or(&crate::ux::icons::ER_DIAGRAM, |k| k.icon())
}

/// The kind of the diagram the canvas shows.
pub(crate) fn active_kind(shell: &Shell) -> Option<&'static dyn DiagramKind> {
    let session = shell.session.as_ref()?;
    let d = session.doc().by_uuid(shell.canvas.active?)?;
    kind_of(shell, session.doc(), d)
}
