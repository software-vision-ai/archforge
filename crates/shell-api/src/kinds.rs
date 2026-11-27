//! The interface `DiagramKind`, what a diagram kind does with the canvas's tools and gestures and
//! what it draws, with the data types it hands between the shell's canvas and a kind: what a kind's
//! tool does, the tool itself, a press on the canvas as the kind reads it, a line as the canvas drew
//! it, a card the kind may draw itself, a label the kind drew of no one node or line, a row a kind
//! adds to its elements' menu and a row it gives the Model menu; and the kind of a diagram, of the
//! diagram the canvas shows and whether the open popover takes the keys, each asked of the kinds it
//! is handed. The shell keeps its list of kinds (`archforge_ui_egui::kinds::KINDS`, which names
//! ER's and UML's) and re-exports these under their old paths (`archforge_ui_egui::kinds` and
//! `canvas`).

use archforge_diagram::ProjectView;
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke};

use crate::canvas::{self, Drawing, Tool, View};
use crate::cx::{Ctx, CtxRef};
use crate::editing::{Action, EntityMenu};
use crate::ux::{Icon, Palette};
use crate::widgets::Registry;

/// What a kind's tool does on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolUse {
    /// A click on empty canvas places a new element there.
    Place,
    /// A drag from an element's node to another's connects them.
    Connect,
}

/// A tool a diagram kind brings to the toolbar, after Select and Pan.
pub struct KindTool {
    /// Its automation key (`workspace.tools.{key}`).
    pub key: &'static str,
    pub icon: &'static Icon,
    /// Its hover text, in the catalogue.
    pub hint: &'static str,
    pub use_: ToolUse,
    /// A chevron beside it that opens the kind's popover, with its key and hover text.
    pub chevron: Option<(&'static str, &'static str)>,
    /// Twice as wide, so a face that draws what the tool makes reads (a relationship's line); an
    /// icon's face is as wide as any tool's.
    pub wide: bool,
}

/// A press on the canvas as a diagram kind reads it (`DiagramKind::pointer`,
/// `DiagramKind::secondary_click`): the shared gestures' own view of it, read once.
pub struct Press<'a> {
    /// Where the pointer is on screen, when it is on the canvas.
    pub at: Option<Pos2>,
    /// Whether the primary button clicked, and whether as a double click (a third click in a row
    /// counts, as egui counts it a triple).
    pub clicked: bool,
    pub double: bool,
    pub store: &'a Store,
    pub notations: &'a ProjectView,
    /// The diagram the canvas shows, and what is selected.
    pub d: Obj,
    pub selection: &'a [Obj],
    /// The node under a screen point that takes the press: a card, or a part of one its kind drew
    /// outside it ([`DiagramKind::draw_card`]), the one drawn on top.
    pub node_at: &'a dyn Fn(Pos2) -> Option<Obj>,
    /// The line under a screen point, nodes aside.
    pub line_at: &'a dyn Fn(Pos2) -> Option<Obj>,
}

/// A label of the figures of no one node or line as its diagram's kind drew it
/// (`DiagramKind::draw_extras`; a UML generalization set's, AR-129): the element it
/// stands for, where it was on screen, the node that holds where it was moved to on this diagram,
/// when it was moved, and what it says while the pointer is on it, its whole text in its
/// notation's words.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtraLabel {
    pub element: Uuid,
    pub rect: Rect,
    pub node: Option<Uuid>,
    pub hover: String,
}

/// A line as drawn this frame, for its diagram's kind to draw what goes with it
/// (`DiagramKind::line_extras`): the line, its element, its points on screen from
/// its source to its target, and whether it is drawn selected.
pub struct LineDrawn<'a> {
    pub line: Obj,
    pub element: Obj,
    pub points: &'a [Pos2],
    pub selected: bool,
}

/// A node's card as the canvas is about to draw it this frame, for its diagram's kind to draw it
/// itself ([`DiagramKind::draw_card`]): the node, its element, the card its notation describes (a
/// row typed in place already in it), its rectangle on screen (grown to hold that row), and whether
/// it is drawn selected.
pub struct CardDrawn<'a> {
    pub node: Obj,
    pub element: Obj,
    pub card: &'a archforge_diagram::figure::Card,
    pub rect: Rect,
    pub selected: bool,
}

/// A row a diagram kind adds to its elements' menu ([`DiagramKind::menu`]), with what it does when
/// picked.
pub struct MenuRow {
    /// Its automation key (`workspace.canvas.menu.{key}`).
    pub key: &'static str,
    /// Its name, in the catalogue.
    pub label: &'static str,
    /// The keys it shows at its right, empty for none.
    pub keys: &'static str,
    /// Whether it acts: a row that does not is drawn muted and takes no click.
    pub live: bool,
    /// Whether it goes first, in a section of its own above Open in Models; otherwise it follows
    /// Open in Models, before Generate….
    pub first: bool,
    /// What it does when picked, on the menu's node and element.
    pub run: fn(&mut Ctx, &EntityMenu),
}

/// A row a diagram kind gives the Model menu ([`DiagramKind::menu_rows`]), as data: the shell makes
/// the menu's entry from it (`archforge_ui_egui::model_menu`), which arms the tool as the kind's
/// toolbar button and popover do.
pub struct ModelRow {
    /// Its id, in the menu and the table of actions (`model.class`).
    pub key: &'static str,
    /// Its name, in the language in use when asked.
    pub word: fn() -> String,
    /// The tool it arms.
    pub tool: Tool,
    /// The choice of the tool's popover it makes before arming the tool, in the notation's own state
    /// (UML's kind of classifier, or of relationship); none for a tool with no such choice.
    pub choice: Option<fn(&mut crate::notation_ui::NotationStates)>,
}

/// What a diagram kind does with the canvas's tools and gestures.
pub trait DiagramKind: Sync {
    /// The diagram kind it serves, as the diagram metamodel keeps it.
    fn kind(&self) -> &'static str;
    /// Its tools, in toolbar order.
    fn tools(&self) -> &'static [KindTool];
    /// Its rows of the Model menu while one of its diagrams shows, by section, in the menu's order:
    /// one for each thing its tools make, as data ([`ModelRow`]): the shell makes each entry, which
    /// arms the tool as its toolbar button and popover do. The menu adds Note and Link note after
    /// them. Every kind answers, as it answers its tools: a kind with tools and no rows would leave
    /// the menu without them.
    fn menu_rows(&self) -> &'static [&'static [ModelRow]];
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
    /// What its diagrams are of: the word for it, as the New diagram dialog labels its choice, and
    /// as Properties labels what one of its diagrams is of when that element's notation gives it
    /// no word of its own.
    fn owner_word(&self) -> String;
    /// The choice that makes a new one of what its diagrams are of, in the New diagram dialog.
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
    fn add_owner(&self, cx: &mut Ctx, name: &str) -> Result<String, String>;
    /// Whether the toolbar shows the status marks toggle.
    fn status_marks(&self) -> bool {
        false
    }
    /// Draws a tool's face itself; false draws its icon.
    #[allow(clippy::too_many_arguments, reason = "a face is drawn from the tool, its button, its ink and its state")]
    fn tool_face(&self, cx: &CtxRef, paint: &egui::Painter, p: &Palette, key: &str, b: Rect, ink: crate::ux::Colour, active: bool) -> bool {
        let _ = (cx, paint, p, key, b, ink, active);
        false
    }
    /// A tool's hover text when it names what the tool makes now; none takes its `hint`.
    fn tool_hint(&self, cx: &CtxRef, key: &str) -> Option<String> {
        let _ = (cx, key);
        None
    }
    /// The popover the chevron `key` opens, drawn while it is open.
    fn popover(&self, cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, chevron: Rect, key: &str) {
        let _ = (cx, ui, p, r, chevron, key);
    }
    /// Whether the element a node shows takes part in the kind's gestures: connecting from and to
    /// it, its adorners, renaming it in place, its menu and opening its editor.
    fn takes_part(&self, store: &Store, element: Obj) -> bool;
    /// Why a line of the tool `tool` starts nowhere at `element`, when it does not: the tool then
    /// draws nothing and the reason is said at the canvas's top. By default every element that
    /// takes part starts one.
    fn line_start_problem(&self, cx: &CtxRef, store: &Store, tool: &'static str, element: Obj) -> Option<String> {
        let _ = (cx, store, tool, element);
        None
    }
    /// Places a new element at `(x, y)` on diagram `d` with the tool `tool`.
    fn place(&self, cx: &mut Ctx, tool: &'static str, d: Obj, x: f64, y: f64);
    /// Connects the elements of two nodes with the tool `tool`.
    fn connect(&self, cx: &mut Ctx, tool: &'static str, d: Obj, from_node: Uuid, to_node: Uuid);
    /// A line of the tool `tool` dropped on empty canvas at `(x, y)`: a new element there, joined
    /// to the node's.
    fn connect_to_new(&self, cx: &mut Ctx, tool: &'static str, d: Obj, from_node: Uuid, x: f64, y: f64);
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
    fn rename(&self, cx: &mut Ctx, element: Uuid, name: &str) -> Result<(), String>;
    /// Where the name typed in place on the node `node`, whose card the kind drew itself in the last
    /// frame ([`DiagramKind::draw_card`]), goes on its card at `rect` at zoom `z` while it reads
    /// `text`, and in which text style, laid out with `paint` (a UML package's name in its tab, which
    /// grows with it). Asked as the field is drawn, so it follows a rename opened after the card was
    /// drawn. None, and by default: in the card's header, where the canvas's own cards hold it.
    fn name_field(&self, paint: &egui::Painter, store: &Store, node: Obj, rect: Rect, text: &str, z: f32) -> Option<(Rect, crate::ux::Type)> {
        let _ = (paint, store, node, rect, text, z);
        None
    }
    /// Opens the element's editor, at the row of its card under a double click (None: the editor's
    /// new row, as the second adorner asks).
    fn open(&self, cx: &mut Ctx, element: Uuid, row: Option<usize>);
    /// Whether a selected line's ends drag onto another element to reconnect it.
    fn reconnects(&self) -> bool {
        false
    }
    /// Moves the source or target end of the line element `element` to the element of `to_node`,
    /// only while the model is at `at`, the revision the drag began on, where the command checks one.
    fn reconnect(&self, cx: &mut Ctx, element: Uuid, source: bool, to_node: Uuid, at: archforge_engine::Revision) {
        let _ = (cx, element, source, to_node, at);
    }
    /// Places an element dragged from the Models explorer on diagram `d`, centred on `at`.
    fn drop(&self, cx: &mut Ctx, d: Obj, element: Uuid, at: (f64, f64)) {
        let _ = (cx, d, element, at);
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
    fn open_second(&self, cx: &mut Ctx, element: Uuid) {
        let _ = (cx, element);
    }
    /// Controls a selected element's card holds on it (a class's + for each compartment), drawn
    /// with its adorners; none by default.
    fn card_controls(&self, cx: &mut Ctx, ui: &egui::Ui, p: &Palette, clip: Rect, rect: Rect, element: Uuid) {
        let _ = (cx, ui, p, clip, rect, element);
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
    fn pointer(&self, cx: &CtxRef, press: &Press<'_>) -> Option<Action> {
        let _ = (cx, press);
        None
    }
    /// What a right click with Select at `press.at` means to the kind, before its element menu
    /// opens (UML's menu of an n-ary association, and of generalization lines); none by default,
    /// leaving the element menu to open where [`DiagramKind::menu`] says.
    fn secondary_click(&self, cx: &CtxRef, press: &Press<'_>) -> Option<Action> {
        let _ = (cx, press);
        None
    }
    /// Reads the keys a drawing under way on diagram `d` takes before the canvas's own (UML's n-ary
    /// association: Enter, Backspace and Escape); true when it took them this frame, false by
    /// default.
    fn keys(&self, cx: &mut Ctx, ui: &egui::Ui, d: Obj) -> bool {
        let _ = (cx, ui, d);
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
    fn before_draw(&self, cx: &mut Ctx, ui: &egui::Ui, r: Rect, d: Obj) {
        let _ = (cx, ui, r, d);
    }
    /// Draws over the canvas `r` once diagram `d` and its gestures are drawn, under the toast, the
    /// minimap and the toolbar (UML's path bar and a selected package's Open). Gives how much room
    /// it takes at the canvas's top, for the hint bar to go below; none by default, drawing nothing.
    fn after_draw(&self, cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj) -> f32 {
        let _ = (cx, ui, p, r, d);
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
    /// Draws the card `card` itself in place of the canvas's own figure for it, over the lines and
    /// the cards drawn before it, recording each part in `registry`, and in `d` the parts outside
    /// the card that take a press as it does (`Drawing::parts`) and the labels a screen reader and
    /// the pointer read (`Drawing::labels`) (UML's package: its tab, the path of its owners under
    /// it and its own diagram's thumbnail). The canvas asks the diagram's own kind first, then every
    /// other kind it knows in turn, so a card is drawn by the kind that draws its element whatever
    /// diagram shows it. True when it drew the card; the canvas then draws only what goes over any
    /// card: the selection ring, a template's signature and the problem badge. False, and by
    /// default: the next kind is asked, and when none draws it, the canvas does.
    fn draw_card(&self, paint: &egui::Painter, d: &Drawing<'_>, card: &CardDrawn<'_>, registry: &mut Registry, ui: &egui::Ui) -> bool {
        let _ = (paint, d, card, registry, ui);
        false
    }
    /// Places a label of those figures that no node holds yet on diagram `d` alone, its top left
    /// corner at `(x, y)`, as a drag lets it go there: `element` is the element the label stands
    /// for (UML gives a generalization set's label a node there). A refusal is said at the
    /// canvas's top. Nothing by default.
    fn place_label(&self, cx: &mut Ctx, d: Obj, element: Uuid, x: f64, y: f64) {
        let _ = (cx, d, element, x, y);
    }
    /// Draws the hint bar at the canvas `r`'s top, `below` the room the canvas's top already
    /// takes: what a drawing under way has so far and what the keys do (a UML n-ary association's,
    /// or what its Add end waits for); none by default.
    fn hint_bar(&self, cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, below: f32) {
        let _ = (cx, ui, p, r, below);
    }
    /// The line drawn from a node to the pointer while connecting; a plain line by default.
    #[allow(clippy::too_many_arguments, reason = "a live line is drawn from its node to the pointer over the view")]
    fn live_line(&self, cx: &CtxRef, paint: &egui::Painter, p: &Palette, view: View, from_node: Uuid, pointer: Pos2, target: Option<Rect>) {
        let Some(rect) =
            cx.session.as_ref().and_then(|s| s.doc().by_uuid(from_node).map(|n| view.rect(&archforge_diagram::grid::bounds_of(s.doc(), n))))
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

/// The kind of the diagram `d`, when it is one of `kinds` (the shell's, `Shell::kinds`).
pub fn kind_of(kinds: &[&'static dyn DiagramKind], store: &Store, d: Obj) -> Option<&'static dyn DiagramKind> {
    let kind = store.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string))?;
    kinds.iter().copied().find(|k| k.kind() == kind)
}

/// The icon of the diagram `d`: its kind's.
pub fn diagram_icon(kinds: &[&'static dyn DiagramKind], store: &Store, d: Obj) -> &'static Icon {
    let kind = store.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    kinds.iter().find(|k| k.kind() == kind).map_or(&crate::ux::icons::ER_DIAGRAM, |k| k.icon())
}

/// Whether the open popover of `canvas` is a menu the keys walk, as the kind whose chevron it is
/// answers ([`DiagramKind::popover_takes_keys`]): every kind of `kinds` is asked, so one left open
/// from a diagram of another kind still holds the keys until the toolbar closes it.
pub fn popover_takes_keys(kinds: &[&'static dyn DiagramKind], canvas: &canvas::State) -> bool {
    canvas.popover.is_some_and(|key| kinds.iter().any(|k| k.popover_takes_keys(key)))
}

/// The kind of the diagram `canvas` shows in the open project's `session`, among `kinds`.
pub fn active_kind(
    kinds: &[&'static dyn DiagramKind],
    canvas: &canvas::State,
    session: Option<&archforge_engine::Session<Store, ProjectView>>,
) -> Option<&'static dyn DiagramKind> {
    let session = session?;
    let d = session.doc().by_uuid(canvas.active?)?;
    kind_of(kinds, session.doc(), d)
}
