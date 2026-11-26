//! The tools and gestures of each diagram kind: a kind brings its toolbar
//! tools, each placing an element or connecting two, and the hooks the canvas's gestures run on
//! its elements: place, connect, a line dropped on empty canvas, renaming in place, opening an
//! element's editor at a row of its card, a second adorner, the rows its element menu adds with
//! what each does, the live line while connecting, a press and a right click it reads before the
//! shared gestures, the keys a drawing under way takes, whether its popover's menu takes the keys,
//! what Select all takes in beside the boxes and lines, and where a label of its own figures is
//! let go; and the hooks the canvas runs as it
//! draws one of its diagrams: before and after the drawing, where a line starts, what goes with a
//! line, the figures of no one node or line, and the hint bar; and its rows of the Model menu.
//! Select, Pan, Note, Link note, Grid and Arrange stay shared; the status marks toggle shows where
//! a kind has marks (ER's key status).

use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke};

pub(crate) use archforge_shell_api::kinds::{KindTool, Press, ToolUse};

use crate::Shell;
use crate::canvas::{Drawing, ExtraLabel, LineDrawn, View};
use crate::editing::{Action, EntityMenu};
use crate::ux::{Icon, Palette};
use crate::widgets::Registry;

/// A row a diagram kind adds to its elements' menu ([`DiagramKind::menu`]), with what it does when
/// picked.
pub(crate) struct MenuRow {
    /// Its automation key (`workspace.canvas.menu.{key}`).
    pub(crate) key: &'static str,
    /// Its name, in the catalogue.
    pub(crate) label: &'static str,
    /// The keys it shows at its right, empty for none.
    pub(crate) keys: &'static str,
    /// Whether it acts: a row that does not is drawn muted and takes no click.
    pub(crate) live: bool,
    /// Whether it goes first, in a section of its own above Open in Models; otherwise it follows
    /// Open in Models, before Generate….
    pub(crate) first: bool,
    /// What it does when picked, on the menu's node and element.
    pub(crate) run: fn(&mut Shell, &EntityMenu),
}

/// What a diagram kind does with the canvas's tools and gestures.
pub(crate) trait DiagramKind: Sync {
    /// The diagram kind it serves, as the diagram metamodel keeps it.
    fn kind(&self) -> &'static str;
    /// Its tools, in toolbar order.
    fn tools(&self) -> &'static [KindTool];
    /// Its rows of the Model menu while one of its diagrams shows, by section, in the menu's order:
    /// one for each thing its tools make, each arming the tool as its toolbar button and popover
    /// do (made with [`crate::model_menu::menu_row`]). The menu adds Note and Link note after them.
    /// Every kind answers, as it answers its tools: a kind with tools and no rows would leave the
    /// menu without them.
    fn menu_rows(&self) -> &'static [&'static [crate::actions::Action]];
    /// Its name in the New diagram dialog.
    fn word(&self) -> String;
    /// The name of its group in the Diagrams explorer, which holds its diagrams.
    fn group_word(&self) -> String;
    /// What its diagrams show, in a sentence: the New diagram dialog's card for it.
    fn description(&self) -> String;
    /// The New diagram dialog's title once it is chosen ("New UML structure diagram").
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
    fn tool_face(&self, shell: &Shell, paint: &egui::Painter, p: &Palette, key: &str, b: Rect, ink: crate::ux::Colour, active: bool) -> bool {
        let _ = (shell, paint, p, key, b, ink, active);
        false
    }
    /// A tool's hover text when it names what the tool makes now; none takes its `hint`.
    fn tool_hint(&self, shell: &Shell, key: &str) -> Option<String> {
        let _ = (shell, key);
        None
    }
    /// The popover the chevron `key` opens, drawn while it is open.
    fn popover(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, chevron: Rect, key: &str) {
        let _ = (shell, ui, p, r, chevron, key);
    }
    /// Whether the element a node shows takes part in the kind's gestures: connecting from and to
    /// it, its adorners, renaming it in place, its menu and opening its editor.
    fn takes_part(&self, store: &Store, element: Obj) -> bool;
    /// Why a line of the tool `tool` starts nowhere at `element`, when it does not: the tool then
    /// draws nothing and the reason is said at the canvas's top. By default every element that
    /// takes part starts one.
    fn line_start_problem(&self, shell: &Shell, store: &Store, tool: &'static str, element: Obj) -> Option<String> {
        let _ = (shell, store, tool, element);
        None
    }
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
    /// click on its card (a package on a UML structure diagram).
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
    /// Moves the source or target end of the line element `element` to the element of `to_node`,
    /// only while the model is at `at`, the revision the drag began on, where the command checks one.
    fn reconnect(&self, shell: &mut Shell, element: Uuid, source: bool, to_node: Uuid, at: archforge_engine::Revision) {
        let _ = (shell, element, source, to_node, at);
    }
    /// Places an element dragged from the Models explorer on diagram `d`, centred on `at`.
    fn drop(&self, shell: &mut Shell, d: Obj, element: Uuid, at: (f64, f64)) {
        let _ = (shell, d, element, at);
    }
    /// The second adorner of the selected `element`, beside Connect: its key and hover text; it
    /// opens the editor's new row, as its menu's row and ⌘↩ do (a class's attribute, an
    /// enumeration's literal). None: the card takes no new row from the canvas.
    fn adorner(&self, store: &Store, element: Obj) -> Option<(&'static str, &'static str)> {
        let _ = (store, element);
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
    /// The rows a right click's menu on `element` adds to the shared ones (Open in Models, Generate…,
    /// Remove from this diagram, Delete from the model…), in order, each with what it does when
    /// picked; `mac` spells their keys as the Mac does. None: a right click opens no menu on it.
    /// Every kind answers: whether its elements take a menu and what that menu does is the kind's
    /// own, and a kind left without an answer would have no menu with no word of it.
    fn menu(&self, store: &Store, element: Obj, mac: bool) -> Option<Vec<MenuRow>>;
    /// What a press of the primary button on diagram `press.d` means to the kind before the shared
    /// gestures read it: with a Connect tool before any line starts, with Select once a gesture
    /// under way had its chance to end and before a drag starts one. Some takes the press, even as
    /// [`Action::Nothing`] (UML's n-ary association drawn by clicks, Add end waiting on a class);
    /// none by default, leaving it to the shared gestures.
    fn pointer(&self, shell: &Shell, press: &Press<'_>) -> Option<Action> {
        let _ = (shell, press);
        None
    }
    /// What a right click with Select at `press.at` means to the kind, before its element menu
    /// opens (UML's menu of an n-ary association, and of generalization lines); none by default,
    /// leaving the element menu to open where [`DiagramKind::menu`] says.
    fn secondary_click(&self, shell: &Shell, press: &Press<'_>) -> Option<Action> {
        let _ = (shell, press);
        None
    }
    /// Reads the keys a drawing under way on diagram `d` takes before the canvas's own (UML's n-ary
    /// association: Enter, Backspace and Escape); true when it took them this frame, false by
    /// default.
    fn keys(&self, shell: &mut Shell, ui: &egui::Ui, d: Obj) -> bool {
        let _ = (shell, ui, d);
        false
    }
    /// Whether the popover of its chevron `key` is a menu the arrow keys, Enter and Escape walk
    /// while the canvas keeps the focus (UML's classifier kinds); false for any other key, another
    /// kind's included, and by default.
    fn popover_takes_keys(&self, key: &str) -> bool {
        let _ = key;
        false
    }
    /// What Select all takes in beside each box and line it draws, from the elements the lines of
    /// the diagram stand for, `lines` (UML: the generalization sets those lines belong to, whose
    /// labels are drawn whether a node holds them or not, AR-129); none by default.
    fn also_selects(&self, store: &Store, lines: &[Obj]) -> Vec<Obj> {
        let _ = (store, lines);
        Vec::new()
    }
    /// Runs before the canvas `r` draws diagram `d`: may take keys, follow the zoom made since the
    /// last drawing ([`crate::canvas::State::zoom_intent`]) and set the view, or open another diagram (UML's
    /// package navigation); nothing by default.
    fn before_draw(&self, shell: &mut Shell, ui: &egui::Ui, r: Rect, d: Obj) {
        let _ = (shell, ui, r, d);
    }
    /// Draws over the canvas `r` once diagram `d` and its gestures are drawn, under the toast, the
    /// minimap and the toolbar (UML's path bar and a selected package's Open). Gives how much room
    /// it takes at the canvas's top, for the hint bar to go below; none by default, drawing nothing.
    fn after_draw(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj) -> f32 {
        let _ = (shell, ui, p, r, d);
        0.0
    }
    /// Where a line of the drawing `d` leaving the node `source` starts when not where its route
    /// meets the node's border, at `tip`, heading for `next` (a UML n-ary association's leg at its
    /// diamond's outline); none by default, the line starting at `tip`.
    fn line_start(&self, d: &Drawing<'_>, source: Obj, tip: Pos2, next: Pos2) -> Option<Pos2> {
        let _ = (d, source, tip, next);
        None
    }
    /// Draws what goes with a line beyond what its notation's figure for it says, over the line,
    /// recording each in `registry` (a UML association class's tie to its card, or its name while
    /// the card is not on the diagram). Gives where it drew the line's name, for the line's
    /// stereotypes to go above it; none by default, drawing nothing.
    fn line_extras(&self, paint: &egui::Painter, d: &Drawing<'_>, line: &LineDrawn<'_>, registry: &mut Registry, ui: &egui::Ui) -> Option<Rect> {
        let _ = (paint, d, line, registry, ui);
        None
    }
    /// Draws the figures of the diagram that belong to no one node or line, over the lines and
    /// under the cards, recording each in `registry` (UML's generalization sets across their
    /// members' lines). Gives their labels, which take a press on the next frame and say their
    /// hover text; none by default, drawing nothing.
    fn draw_extras(&self, paint: &egui::Painter, d: &Drawing<'_>, registry: &mut Registry, ui: &egui::Ui) -> Vec<ExtraLabel> {
        let _ = (paint, d, registry, ui);
        Vec::new()
    }
    /// Places a label of those figures that no node holds yet on diagram `d` alone, its top left
    /// corner at `(x, y)`, as a drag lets it go there: `element` is the element the label stands
    /// for (UML gives a generalization set's label a node there). A refusal is said at the
    /// canvas's top. Nothing by default.
    fn place_label(&self, shell: &mut Shell, d: Obj, element: Uuid, x: f64, y: f64) {
        let _ = (shell, d, element, x, y);
    }
    /// Draws the hint bar at the canvas `r`'s top, `below` the room the canvas's top already
    /// takes: what a drawing under way has so far and what the keys do (a UML n-ary association's,
    /// or what its Add end waits for); none by default.
    fn hint_bar(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, below: f32) {
        let _ = (shell, ui, p, r, below);
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

/// Whether the canvas's open popover is a menu the keys walk, as the kind whose chevron it is
/// answers ([`DiagramKind::popover_takes_keys`]): every kind is asked, so one left open from a
/// diagram of another kind still holds the keys until the toolbar closes it.
pub(crate) fn popover_takes_keys(shell: &Shell) -> bool {
    shell.canvas.popover.is_some_and(|key| shell.kinds.iter().any(|k| k.popover_takes_keys(key)))
}

/// The kind of the diagram the canvas shows.
pub(crate) fn active_kind(shell: &Shell) -> Option<&'static dyn DiagramKind> {
    let session = shell.session.as_ref()?;
    let d = session.doc().by_uuid(shell.canvas.active?)?;
    kind_of(shell, session.doc(), d)
}
