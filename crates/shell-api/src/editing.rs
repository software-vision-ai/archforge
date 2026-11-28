//! What editing on the canvas keeps between frames, which names no shell: the name typed in a
//! node, a note's text typed in place, the pointer gesture under way with the corner a resize
//! holds, and the element menu; and what the pointer asked for in a frame (`Action`), which a
//! diagram kind's press gives back. The canvas's state holds them (`canvas::State`); the shell edits
//! with them (`archforge_ui_egui::editing` re-exports them under their old paths), and so will a
//! notation's screens, without the shell. Beside them, what lies under a point and what a diagram
//! shows, and the actions on a diagram through the narrow context: selecting, placing an element
//! with its lines, taking nodes off a diagram and asking before an element goes from the model.

use archforge_diagram::grid::bounds_of;
use archforge_diagram::{Grid, Hit, ProjectView, diagram, node};
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, pos2, vec2};
use serde_json::json;

use crate::cx::{Ctx, Request};

/// A name being typed in a node's header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRename {
    pub node: Uuid,
    /// The element the node shows, which the name is typed for.
    pub element: Uuid,
    pub text: String,
    /// True until the field has taken the keyboard once.
    pub opened: bool,
    pub refused: Option<String>,
}

/// A note's text being typed in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEdit {
    pub node: Uuid,
    pub text: String,
    pub opened: bool,
    pub refused: Option<String>,
}

/// A corner handle of a selected node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A pointer gesture in progress, previewed on the canvas and committed on release.
#[derive(Debug, Clone, PartialEq)]
pub enum Gesture {
    /// The nodes moved together, from the screen point the drag started at.
    Move { nodes: Vec<Uuid>, from: Pos2 },
    /// A node resized by a corner, from its rectangle in diagram units.
    Resize { node: Uuid, corner: Corner, base: Rect, from: Pos2 },
    /// A selection rectangle from a screen point.
    Marquee { from: Pos2 },
    /// A line of a kind's connect tool drawn from a node, from the screen point the drag started
    /// at.
    Connect { tool: &'static str, from_node: Uuid, from: Pos2 },
    /// A note link drawn from a note's handle to a card or a line.
    LinkNote { note: Uuid },
    /// A cardinality text dragged off its place: its line, its end, the screen point the drag
    /// started at, its centre then and the end of the line it belongs to.
    EndText { edge: Uuid, end: u8, from: Pos2, centre: Pos2, anchor: Pos2 },
    /// A segment of a right-angled line slid across: the line, the segment's index in the route,
    /// the route as it was in diagram units, and the screen point the drag started at.
    Segment { edge: Uuid, index: usize, route: Vec<(f64, f64)>, from: Pos2 },
    /// A selected line's end dragged towards another element: the line's
    /// element, whether it is the source end, the screen point of the end that stays, and the
    /// revision of the model the drag began on, which the drop changes only if it still holds (AR-697).
    Reconnect { element: Uuid, source: bool, fixed: Pos2, at: archforge_engine::Revision },
    /// A label of the figures of no one node or line dragged where no node holds it yet (the label
    /// of a figure drawn across several lines): the element it stands for, the screen point the
    /// drag started at and the label's rectangle then.
    ExtraLabel { element: Uuid, from: Pos2, rect: Rect },
}

/// The element menu, open at a screen point.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementMenu {
    pub node: Uuid,
    /// The element the node shows, which the menu is about.
    pub element: Uuid,
    pub at: Pos2,
}

/// What the pointer asked for this frame, decided over the store and applied after.
pub enum Action {
    Nothing,
    Select(Vec<Uuid>),
    Toggle(Uuid),
    Start(Gesture, Option<Uuid>),
    Commit,
    Place(&'static str, f64, f64),
    Menu(ElementMenu),
    Rename(Uuid, Uuid),
    Open(Uuid, Option<usize>),
    PlaceNote(f64, f64),
    EditNote(Uuid),
    /// A cardinality text put back in its place, and a line's route worked out anew.
    ResetEnd(Uuid, u8),
    ResetRoute(Uuid),
    /// Something refused before it began, said at the canvas's top: a line the tool starts nowhere.
    Notice(String),
    /// What the diagram's kind makes of the press ([`crate::kinds::DiagramKind::pointer`],
    /// [`crate::kinds::DiagramKind::secondary_click`]), run through the narrow context once
    /// decided: the next click of a figure drawn click by click, or a menu of its own opened.
    Own(Box<dyn FnOnce(&mut Ctx)>),
}

/// What lies at diagram point `(x, y)`: the grid's topmost node or nearest line, looking through a
/// node its notation says holds a label (the label of a figure drawn across several lines). A label
/// takes a press only where the last frame drew it (`canvas.extra_labels`), so one that draws
/// nothing, its diagram showing none of its figure's lines, is never hit.
pub fn hit(store: &Store, notations: &ProjectView, grid: &Grid, x: f64, y: f64) -> Option<Hit> {
    grid.hit_keeping(x, y, |n| !notations.node_is_label(store, n))
}

/// Selects `ids`, as a click does; a refusal is said at the canvas's top.
pub fn select_cx(cx: &mut Ctx, ids: Vec<Uuid>) {
    let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    if let Err(e) = crate::panels::run_cx(cx, "view.select", json!({ "ids": ids })) {
        cx.canvas.notice = Some(e);
    }
}

/// The id of `o`, while the store has one for it.
pub fn uuid(store: &Store, o: Obj) -> Option<Uuid> {
    store.uuid(o).ok()
}

/// The element node `n` shows, while the store has it alive; none for a note.
pub fn element_of(store: &Store, n: Obj) -> Option<Obj> {
    store.get(n, node::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e))
}

/// The boxes of diagram `d` the store has alive, those at its top only.
pub fn nodes_of(store: &Store, d: Obj) -> Vec<Obj> {
    store.list(d, diagram::NODE).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref).filter(|n| store.is_alive(*n)).collect()
}

/// Whether diagram `d` shows element `e`.
pub fn on_diagram(store: &Store, d: Obj, e: Obj) -> bool {
    nodes_of(store, d).into_iter().any(|n| element_of(store, n) == Some(e))
}

/// A node's rectangle in diagram units.
pub fn unit_rect(store: &Store, n: Obj) -> Rect {
    let b = bounds_of(store, n);
    Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32))
}

/// The element a diagram is of, its owner, when it has one.
pub fn owner_of(store: &Store, d: Obj) -> Option<Obj> {
    store.get(d, diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o))
}

/// Places element `e` dropped on diagram `d`, centred on `at` at `size`, with those of `links`
/// that join it to an element the diagram shows, as one undo step named `label`; then selects it.
#[allow(clippy::too_many_arguments, reason = "a drop is placed with its size, its lines and its step's name")]
pub fn place_with_lines_cx(
    cx: &mut Ctx,
    d: Obj,
    e: Obj,
    at: (f64, f64),
    size: (f64, f64),
    links: &[Obj],
    notation: &dyn archforge_diagram::Notation,
    label: &str,
) {
    let Some(session) = cx.session.as_ref() else { return };
    let store = session.doc();
    let (Some(diagram_id), Some(element_id)) = (uuid(store, d), uuid(store, e)) else { return };
    let (x, y) = ((at.0 - size.0 / 2.0).round(), (at.1 - size.1 / 2.0).round());
    let place = json!({ "diagram": diagram_id.to_string(), "element": element_id.to_string(), "x": x, "y": y, "width": size.0, "height": size.1 });
    // (line's element, source, target), None for the element being placed.
    let shown = |t: Obj| nodes_of(store, d).into_iter().find(|n| element_of(store, *n) == Some(t)).and_then(|n| uuid(store, n));
    let mut lines: Vec<(Uuid, Option<Uuid>, Option<Uuid>)> = Vec::new();
    for &l in links {
        let Some((from, to)) = notation.ends(store, l) else { continue };
        let end = |t: Obj| if t == e { Some(None) } else { shown(t).map(Some) };
        if let (Some(a), Some(b), Some(id)) = (end(from), end(to), uuid(store, l))
            && !lines.iter().any(|(x, _, _)| *x == id)
        {
            lines.push((id, a, b));
        }
    }
    let out = cx.session.as_mut().map(|session| {
        session.batch(label, |b| {
            let placed = b.execute("diagram.place", &place)?;
            let node = placed.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
            for (element, source, target) in &lines {
                let end = |x: &Option<Uuid>| x.map_or(node.clone(), |u| u.to_string());
                b.execute(
                    "diagram.connect",
                    &json!({ "diagram": diagram_id.to_string(), "element": element.to_string(), "source": end(source), "target": end(target) }),
                )?;
            }
            Ok(())
        })
    });
    match out {
        Some(Ok(_)) => select_cx(cx, vec![element_id]),
        Some(Err(e)) => cx.canvas.notice = Some(crate::panels::refusal(cx.notation_uis, &e)),
        None => {}
    }
}

/// Removes the nodes from the diagram, the model untouched; a refusal is said at the canvas's top.
pub fn unplace_cx(cx: &mut Ctx, nodes: Vec<Uuid>) {
    if nodes.is_empty() {
        return;
    }
    let ids: Vec<String> = nodes.iter().map(Uuid::to_string).collect();
    if let Err(e) = crate::panels::run_cx(cx, "diagram.unplace", json!({ "nodes": ids })) {
        cx.canvas.notice = Some(e);
    }
}

/// How many diagrams show `element`, as a node or as a line (an element drawn by its line alone),
/// each diagram once: the count the Delete from the model dialog gives, from the canvas and from
/// Models alike. A diagram that belongs to `element` (its own diagram) counts only
/// when it shows `element` too.
pub fn diagrams_showing(store: &Store, element: Obj) -> usize {
    let (_, edges, nodes) = archforge_diagram::views_of(store, element);
    let diagrams: std::collections::BTreeSet<Obj> = nodes.iter().chain(&edges).filter_map(|v| archforge_diagram::diagram_of(store, *v)).collect();
    diagrams.len()
}

/// Asks before an element goes from the model, with what goes with it; what its notation keeps
/// (an element that holds what the notation itself needs) is said at the canvas's top instead.
pub fn ask_delete_cx(cx: &mut Ctx, node: Uuid, element: Uuid) {
    ask_delete_of(cx, Some(node), element);
}

/// [`ask_delete_cx`] for an element selected on the diagram by its card `node`, or with none by a
/// line alone (an element whose card is not on the diagram): the dialog then offers no removal
/// from the diagram, as the line's own Delete does that. The shell opens the dialog once the call
/// that asked returns ([`Request::AskDelete`]).
pub fn ask_delete_of(cx: &mut Ctx, node: Option<Uuid>, element: Uuid) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(element) else { return };
    // What its notation keeps (an element that holds what the notation itself needs) is said, not
    // asked.
    if let Some(why) = crate::notation_ui::of(cx.notation_uis, store, e).and_then(|ui| ui.remove_refusal(cx.notation_uis, store, e)) {
        cx.canvas.notice = Some(why);
        cx.canvas.menu = None;
        return;
    }
    let consequences = crate::notation_ui::of(cx.notation_uis, store, e).map(|ui| ui.consequences(cx.notation_uis, store, e)).unwrap_or_default();
    let diagrams = diagrams_showing(store, e);
    // An element nested in another may hand what it holds up: the dialog says where, and so does
    // the toast.
    let hands_to = crate::notation_ui::of(cx.notation_uis, store, e)
        .and_then(|ui| Some((archforge_core::name_of(store, ui.hands_up(store, e)?), ui.has_children(store, e))));
    cx.requests.push(Request::AskDelete(crate::explorer::Delete {
        ids: vec![element],
        // Named as every path that asks names it, an unnamed line's element by its ends (AR-216).
        name: crate::notation_ui::label(cx.notation_uis, store, e),
        consequences,
        diagrams,
        node,
        hands_to,
    }));
    cx.canvas.menu = None;
}

/// The keys that open a new row on a selected card (⌘↩), as an element menu's row shows them; `mac`
/// spells them as the Mac does.
pub fn new_row_keys(mac: bool) -> &'static str {
    if mac { "⌘↩" } else { "Ctrl Enter" }
}
