//! Editing on the ER canvas (from the design's frame "ER · Empty
//! diagram · First entity named in place" and the cards "Two entities selected", "Entity menu"
//! and "Delete from the model"): the Entity tool places a table and opens its name for typing in
//! the node's header; Select drags nodes, resizes them by their handles, draws a marquee, nudges
//! with the arrows and arranges two or more with the Arrange bar; a selected entity shows its two
//! adorners (connect waits for E2, add column opens the column editor); the entity menu removes
//! from the diagram or deletes from the model after the dialog. Every change is one engine step:
//! a drag is previewed on the canvas and committed as one command or one batch on release.

use std::collections::BTreeMap;

use archforge_diagram::grid::bounds_of;
use archforge_diagram::{Grid, Hit, diagram, node};
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::canvas::{ENTITY_FIRST_ROW, ENTITY_HEADER, ENTITY_ROW, Tool, View};
use crate::home::control;
use crate::i18n::tr;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// A new entity's node: the design's width, and its height without columns.
pub const ENTITY_WIDTH: f64 = 240.0;
pub const ENTITY_EMPTY_HEIGHT: f64 = 72.0;
/// The least a node may be resized to: the diagram rule's.
const MIN_SIZE: f64 = 20.0;
/// How far a drag must go before it counts as a move.
const DRAG_MIN: f32 = 0.5;
/// How near a line's segment the pointer grabs it, in logical pixels.
const SEGMENT_REACH: f32 = 6.0;
/// A nudge by the arrows, and with Shift.
const NUDGE: f64 = 1.0;
const NUDGE_FAR: f64 = 10.0;
/// The distance from a handle's centre that still hits it.
const HANDLE_REACH: f32 = 7.0;
/// An adorner's box and its gap from the node.
const ADORNER: f32 = 26.0;
const ADORNER_GAP: f32 = 8.0;
/// The Arrange bar's size and its gap under the selection.
const ARRANGE: Vec2 = vec2(189.0, 42.0);
const ARRANGE_GAP: f32 = 16.0;
/// The gap the stack action leaves between nodes.
const STACK_GAP: f64 = 24.0;
/// The entity menu's width and row height.
const MENU_WIDTH: f32 = 260.0;
const MENU_ROW: f32 = 30.0;
/// The most characters a name typed in place may have: the ER rule's.
const MAX_NAME: usize = archforge_er::rules::MAX_NAME;

/// A name being typed in a node's header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRename {
    pub node: Uuid,
    pub entity: Uuid,
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

/// A note's size when placed, in diagram units.
const NOTE_SIZE: (f64, f64) = (200.0, 72.0);

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
    /// at (ER's relationship, slice E2).
    Connect { tool: &'static str, from_node: Uuid, from: Pos2 },
    /// A note link drawn from a note's handle to a table or a relationship's line.
    LinkNote { note: Uuid },
    /// A cardinality text dragged off its place: its line, its end, the screen point the drag
    /// started at, its centre then and the end of the line it belongs to.
    EndText { edge: Uuid, end: u8, from: Pos2, centre: Pos2, anchor: Pos2 },
    /// A segment of a right-angled line slid across: the line, the segment's index in the route,
    /// the route as it was in diagram units, and the screen point the drag started at.
    Segment { edge: Uuid, index: usize, route: Vec<(f64, f64)>, from: Pos2 },
    /// A selected line's end dragged towards another element: the line's
    /// element, whether it is the source end, and the screen point of the end that stays.
    Reconnect { element: Uuid, source: bool, fixed: Pos2 },
    /// A generalization set's label dragged where no node holds it yet (AR-129): the set, the
    /// screen point the drag started at and the label's rectangle then.
    SetLabel { set: Uuid, from: Pos2, rect: Rect },
}

/// The entity menu, open at a screen point.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityMenu {
    pub node: Uuid,
    pub entity: Uuid,
    pub at: Pos2,
}

/// What lies at diagram point `(x, y)`: the grid's topmost node or nearest line, looking through a
/// generalization set's label node (AR-129). A label takes a press only where the last frame drew it
/// (`canvas.set_labels`), so one that draws nothing, its diagram showing none of its set's lines,
/// is never hit.
pub(crate) fn hit(store: &Store, grid: &Grid, x: f64, y: f64) -> Option<Hit> {
    grid.hit_keeping(x, y, |n| !archforge_uml::generalization_sets::is_label(store, n))
}

/// What the pointer asked for this frame, decided over the store and applied after.
pub(crate) enum Action {
    Nothing,
    Select(Vec<Uuid>),
    Toggle(Uuid),
    Start(Gesture, Option<Uuid>),
    Commit,
    Place(&'static str, f64, f64),
    Menu(EntityMenu),
    /// The context menu of generalization lines at a screen point, the line pressed selected
    /// first when it was not (AR-129).
    LineMenu(Pos2, Option<Uuid>),
    Rename(Uuid, Uuid),
    Open(Uuid, Option<usize>),
    PlaceNote(f64, f64),
    EditNote(Uuid),
    /// A cardinality text put back in its place, and a line's route worked out anew.
    ResetEnd(Uuid, u8),
    ResetRoute(Uuid),
    /// Something refused before it began, said at the canvas's top: a line the tool starts nowhere.
    Notice(String),
    /// The armed N-ary association tool clicked on a card, double or not; or anywhere else, which
    /// cancels the drawing (AR-128).
    NaryEnd(Uuid, bool),
    NaryCancel,
    /// A card clicked while Add end waits: the association's new end at its class (AR-128).
    NaryAddEnd(Uuid),
    /// The context menu of an n-ary association, from its diamond or a leg (AR-128).
    NaryMenu(Pos2, Uuid),
}

/// Whether a canvas action changes nothing, so that it stays on while a version shows.
pub(crate) fn only_looks(action: &Action) -> bool {
    matches!(action, Action::Nothing | Action::Select(_) | Action::Toggle(_) | Action::Start(Gesture::Marquee { .. }, _) | Action::Commit)
}

/// Runs a canvas command on the open project's session; a refusal comes back in Properties' plain
/// words ([`crate::panels::refusal`]), never with the command's id.
pub(crate) fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    crate::panels::run(shell, id, params)
}

fn select(shell: &mut Shell, ids: Vec<Uuid>) {
    let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    if let Err(e) = run(shell, "view.select", json!({ "ids": ids })) {
        shell.canvas.notice = Some(e);
    }
}

/// Selects what each box and line drawn on diagram `d` stands for, as a click on it would, boxes
/// nested in others included, or the diagram itself when it draws nothing.
pub(crate) fn select_all(shell: &mut Shell, d: Obj) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let mut ids: Vec<Uuid> = Vec::new();
    let boxes = nodes_of(store, d).into_iter().flat_map(|top| archforge_diagram::node_tree(store, top)).map(|n| element_of(store, n).unwrap_or(n));
    let lines: Vec<Obj> = Grid::build(store, d).lines().map(|(e, _)| line_target(store, e)).collect();
    for id in boxes.chain(lines).filter_map(|o| uuid(store, o)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        ids.extend(uuid(store, d));
    }
    select(shell, ids);
}

/// What a click on line `e` selects: the element it draws, a leg of an n-ary association its
/// association (AR-128), and a note link itself.
fn line_target(store: &Store, e: Obj) -> Obj {
    crate::uml_nary::selectable(store, store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).unwrap_or(e))
}

fn uuid(store: &Store, o: Obj) -> Option<Uuid> {
    store.uuid(o).ok()
}

fn element_of(store: &Store, n: Obj) -> Option<Obj> {
    store.get(n, node::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e))
}

/// The element of node `n` when it takes part in the gestures of `kind`.
fn part_of_node(kind: Option<&'static dyn crate::kinds::DiagramKind>, store: &Store, n: Obj) -> Option<Obj> {
    element_of(store, n).filter(|e| kind.is_some_and(|k| k.takes_part(store, *e)))
}

fn entity_of_node(store: &Store, n: Obj) -> Option<Obj> {
    element_of(store, n).filter(|e| store.class_of(*e).ok() == Some(archforge_er::ENTITY))
}

fn nodes_of(store: &Store, d: Obj) -> Vec<Obj> {
    store.list(d, diagram::NODE).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref).filter(|n| store.is_alive(*n)).collect()
}

/// A node's rectangle in diagram units.
fn unit_rect(store: &Store, n: Obj) -> Rect {
    let b = bounds_of(store, n);
    Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32))
}

/// The columns an entity node shows, and the height that shows them all.
pub fn content_height(columns: usize) -> f64 {
    if columns == 0 { ENTITY_EMPTY_HEIGHT } else { (ENTITY_FIRST_ROW + 9.0) as f64 + columns as f64 * ENTITY_ROW as f64 }
}

/// The nodes of the selected elements on diagram `d`, with the hit node first.
fn selected_nodes(store: &Store, d: Obj, selection: &[Obj], hit: Obj) -> Vec<Uuid> {
    let mut out: Vec<Uuid> = uuid(store, hit).into_iter().collect();
    for n in nodes_of(store, d) {
        if n == hit {
            continue;
        }
        if selection.contains(&n) || element_of(store, n).is_some_and(|e| selection.contains(&e)) {
            out.extend(uuid(store, n));
        }
    }
    out
}

fn is_selected(store: &Store, selection: &[Obj], n: Obj) -> bool {
    selection.contains(&n) || element_of(store, n).is_some_and(|e| selection.contains(&e))
}

/// The one selected node of `d`, when exactly one is.
fn single_selected(store: &Store, d: Obj, selection: &[Obj]) -> Option<Obj> {
    let mut found = nodes_of(store, d).into_iter().filter(|n| is_selected(store, selection, *n));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// The corner handle of the selected node under `pos`, if any.
fn handle_at(store: &Store, d: Obj, selection: &[Obj], view: View, pos: Pos2) -> Option<(Obj, Corner)> {
    let n = single_selected(store, d, selection).filter(|n| !archforge_uml::generalization_sets::is_label(store, *n))?;
    let ring = view.rect(&bounds_of(store, n)).expand(4.0);
    let corners = [
        (Corner::TopLeft, ring.left_top()),
        (Corner::TopRight, ring.right_top()),
        (Corner::BottomLeft, ring.left_bottom()),
        (Corner::BottomRight, ring.right_bottom()),
    ];
    corners.into_iter().find(|(_, c)| (*c - pos).length() <= HANDLE_REACH).map(|(corner, _)| (n, corner))
}

/// The rectangle a resize by `corner` gives, in diagram units, never smaller than the rule's least.
fn resized(base: Rect, corner: Corner, delta: Vec2) -> Rect {
    let (mut min, mut max) = (base.min, base.max);
    match corner {
        Corner::TopLeft => min += delta,
        Corner::TopRight => {
            min.y += delta.y;
            max.x += delta.x;
        }
        Corner::BottomLeft => {
            min.x += delta.x;
            max.y += delta.y;
        }
        Corner::BottomRight => max += delta,
    }
    let least = MIN_SIZE as f32;
    if max.x - min.x < least {
        if matches!(corner, Corner::TopLeft | Corner::BottomLeft) { min.x = max.x - least } else { max.x = min.x + least }
    }
    if max.y - min.y < least {
        if matches!(corner, Corner::TopLeft | Corner::TopRight) { min.y = max.y - least } else { max.y = min.y + least }
    }
    Rect::from_min_max(min, max)
}

/// The screen rectangles of the nodes a gesture moves or resizes, as the canvas previews them.
pub(crate) fn preview_rects(shell: &Shell, view: View, pointer: Option<Pos2>) -> BTreeMap<Uuid, Rect> {
    let mut out = BTreeMap::new();
    let (Some(store), Some(gesture), Some(pointer)) = (shell.session.as_ref().map(|s| s.doc()), shell.canvas.gesture.as_ref(), pointer) else {
        return out;
    };
    if !pointer.is_finite() {
        return out;
    }
    match gesture {
        Gesture::Move { nodes, from } => {
            let delta = pointer - *from;
            for id in nodes {
                if let Some(n) = store.by_uuid(*id) {
                    out.insert(*id, view.rect(&bounds_of(store, n)).translate(delta));
                }
            }
        }
        Gesture::Resize { node, corner, base, from } => {
            let r = resized(*base, *corner, (pointer - *from) / view.zoom);
            out.insert(*node, Rect::from_min_size(view.to_screen(r.left() as f64, r.top() as f64), r.size() * view.zoom));
        }
        Gesture::Marquee { .. }
        | Gesture::Connect { .. }
        | Gesture::LinkNote { .. }
        | Gesture::EndText { .. }
        | Gesture::Segment { .. }
        | Gesture::Reconnect { .. }
        | Gesture::SetLabel { .. } => {}
    }
    out
}

/// A set's label being dragged where no node holds it yet: its set and how far it moved.
pub(crate) fn set_label_preview(shell: &Shell, pointer: Option<Pos2>) -> Option<(Uuid, Vec2)> {
    match (shell.canvas.gesture.as_ref(), pointer.filter(|p| p.is_finite())) {
        (Some(Gesture::SetLabel { set, from, .. }), Some(at)) => Some((*set, at - *from)),
        _ => None,
    }
}

/// A cardinality text being dragged: its line, end and where its centre is now.
pub(crate) fn end_preview(shell: &Shell, pointer: Option<Pos2>) -> Option<(Uuid, u8, Pos2)> {
    match (shell.canvas.gesture.as_ref(), pointer.filter(|p| p.is_finite())) {
        (Some(Gesture::EndText { edge, end, from, centre, .. }), Some(at)) => Some((*edge, *end, *centre + (at - *from))),
        _ => None,
    }
}

/// A line whose segment is being slid: its route as it is now, in diagram units.
pub(crate) fn segment_preview(shell: &Shell, view: View, pointer: Option<Pos2>) -> Option<(Uuid, Vec<(f64, f64)>)> {
    match (shell.canvas.gesture.as_ref(), pointer.filter(|p| p.is_finite())) {
        (Some(Gesture::Segment { edge, index, route, from }), Some(at)) => Some((*edge, slid(route, *index, (at - *from) / view.zoom))),
        _ => None,
    }
}

/// The route with segment `k` slid across by `by`: a level segment up or down, an upright one left
/// or right, its two corners with it.
fn slid(route: &[(f64, f64)], k: usize, by: Vec2) -> Vec<(f64, f64)> {
    let mut out = route.to_vec();
    if k + 1 >= out.len() {
        return out;
    }
    let level = (out[k].1 - out[k + 1].1).abs() < 1e-6;
    for p in &mut out[k..=k + 1] {
        if level {
            p.1 += f64::from(by.y);
        } else {
            p.0 += f64::from(by.x);
        }
    }
    out
}

/// The segment of a right-angled line `e` at diagram point (x, y), within `reach` units: its index
/// and the route. A middle segment slides across; the first or last slides where the line meets
/// its table, along the table's side.
fn segment_at(store: &Store, grid: &Grid, e: Obj, x: f64, y: f64, reach: f64) -> Option<(usize, Vec<(f64, f64)>)> {
    use archforge_diagram::Way;
    if !matches!(archforge_diagram::route::way_of(store, e), Way::RightAngled | Way::Avoiding) {
        return None;
    }
    let route = grid.line(e)?.to_vec();
    let n = route.len();
    (0..n.saturating_sub(1)).find_map(|k| {
        let ((ax, ay), (bx, by)) = (route[k], route[k + 1]);
        let near = if (ay - by).abs() < 1e-6 {
            (y - ay).abs() <= reach && x >= ax.min(bx) - reach && x <= ax.max(bx) + reach
        } else if (ax - bx).abs() < 1e-6 {
            (x - ax).abs() <= reach && y >= ay.min(by) - reach && y <= ay.max(by) + reach
        } else {
            false
        };
        near.then(|| (k, route.clone()))
    })
}

/// Reads what the pointer asked for this frame.
#[allow(clippy::too_many_arguments, reason = "the decision reads the response, the view, the grid and the store apart")]
fn decide(
    shell: &Shell,
    resp: &egui::Response,
    view: View,
    grid: &Grid,
    store: &Store,
    d: Obj,
    selection: &[Obj],
    shift: bool,
    origin: Option<Pos2>,
    released: bool,
) -> Action {
    let pos = resp.interact_pointer_pos().filter(|p| p.is_finite());
    let kind = crate::kinds::kind_of(shell, store, d);
    let hit_node = |pos: Pos2| {
        let (x, y) = view.to_canvas(pos);
        let card = match hit(store, grid, x, y) {
            Some(Hit::Node(n)) => Some(n),
            _ => None,
        };
        on_top(shell, store, grid, card, pos)
    };
    // A generalization set's label as the last frame drew it, where no card is over it (AR-129).
    let label_at = |pos: Pos2| {
        if hit_node(pos).is_some() {
            return None;
        }
        shell.canvas.set_labels.iter().rev().find(|l| l.rect.contains(pos)).cloned()
    };
    match shell.canvas.tool {
        Tool::Pan => Action::Nothing,
        // An n-ary association is drawn class by class, by clicks (AR-128).
        Tool::Connect(tool) if crate::uml_nary::clicks(shell, tool) => match pos {
            Some(pos) if resp.clicked() => match hit_node(pos).and_then(|n| uuid(store, n)) {
                // A third click in a row reads as a double one too: egui counts it a triple.
                Some(node) => Action::NaryEnd(node, resp.double_clicked() || resp.triple_clicked()),
                None => Action::NaryCancel,
            },
            _ => Action::Nothing,
        },
        Tool::Connect(tool) => {
            if (resp.drag_stopped_by(egui::PointerButton::Primary) || released) && shell.canvas.gesture.is_some() {
                return Action::Commit;
            }
            let Some(pos) = pos else { return Action::Nothing };
            if resp.drag_started_by(egui::PointerButton::Primary) && shell.canvas.gesture.is_none() {
                let from = origin.filter(|o| o.is_finite()).unwrap_or(pos);
                let Some((n, element)) = hit_node(from).and_then(|n| Some((n, part_of_node(kind, store, n)?))) else { return Action::Nothing };
                if let Some(why) = kind.and_then(|k| k.line_start_problem(shell, store, tool, element)) {
                    return Action::Notice(why);
                }
                return match uuid(store, n) {
                    Some(from_node) => Action::Start(Gesture::Connect { tool, from_node, from }, None),
                    None => Action::Nothing,
                };
            }
            if resp.clicked() {
                return match hit_node(pos) {
                    Some(n) => Action::Select(uuid(store, element_of(store, n).unwrap_or(n)).into_iter().collect()),
                    None => Action::Nothing,
                };
            }
            Action::Nothing
        }
        Tool::Link => {
            // Pressed on a note and dragged onto a table or a relationship's line, as the note's
            // own link handle does.
            if (resp.drag_stopped_by(egui::PointerButton::Primary) || released) && shell.canvas.gesture.is_some() {
                return Action::Commit;
            }
            let Some(pos) = pos else { return Action::Nothing };
            if resp.drag_started_by(egui::PointerButton::Primary) && shell.canvas.gesture.is_none() {
                let from = origin.filter(|o| o.is_finite()).unwrap_or(pos);
                return match hit_node(from).filter(|n| element_of(store, *n).is_none()).and_then(|n| uuid(store, n)) {
                    Some(note) => Action::Start(Gesture::LinkNote { note }, None),
                    None => Action::Nothing,
                };
            }
            if resp.clicked() {
                return match hit_node(pos) {
                    Some(n) => Action::Select(uuid(store, element_of(store, n).unwrap_or(n)).into_iter().collect()),
                    None => Action::Nothing,
                };
            }
            Action::Nothing
        }
        Tool::Note => match (resp.clicked(), pos) {
            (true, Some(pos)) => match hit_node(pos) {
                Some(n) => Action::Select(uuid(store, element_of(store, n).unwrap_or(n)).into_iter().collect()),
                None => {
                    let (x, y) = view.to_canvas(pos);
                    Action::PlaceNote(x, y)
                }
            },
            _ => Action::Nothing,
        },
        Tool::Place(tool) => match (resp.clicked(), pos) {
            (true, Some(pos)) => match hit_node(pos) {
                Some(n) => Action::Select(uuid(store, element_of(store, n).unwrap_or(n)).into_iter().collect()),
                None => {
                    let (x, y) = view.to_canvas(pos);
                    Action::Place(tool, x, y)
                }
            },
            _ => Action::Nothing,
        },
        Tool::Select => {
            if (resp.drag_stopped_by(egui::PointerButton::Primary) || released) && shell.canvas.gesture.is_some() {
                return Action::Commit;
            }
            let Some(pos) = pos else { return Action::Nothing };
            // Add end waits for a class: a click on a card gives it, anywhere else cancels (AR-128).
            if crate::uml_nary::adding_end(shell).is_some() && resp.clicked() {
                return match hit_node(pos).and_then(|n| uuid(store, n)) {
                    Some(node) => Action::NaryAddEnd(node),
                    None => Action::NaryCancel,
                };
            }
            if resp.drag_started_by(egui::PointerButton::Primary) && shell.canvas.gesture.is_none() {
                // The gesture starts where the button went down, not where the pointer is now.
                let from = origin.filter(|o| o.is_finite()).unwrap_or(pos);
                // A cardinality text, grabbed where the last frame drew it, unless a table or its
                // handle is over it there.
                if handle_at(store, d, selection, view, from).is_none()
                    && hit_node(from).is_none()
                    && let Some(t) = shell.canvas.ends.iter().find(|t| t.rect.contains(from))
                {
                    return Action::Start(Gesture::EndText { edge: t.edge, end: t.end, from, centre: t.rect.center(), anchor: t.anchor }, None);
                }
                if let Some((n, corner)) = handle_at(store, d, selection, view, from)
                    && let Some(id) = uuid(store, n)
                {
                    return Action::Start(Gesture::Resize { node: id, corner, base: unit_rect(store, n), from }, None);
                }
                // A set's label moves on this diagram alone: its node when it has one, else one is
                // made where it is let go.
                if let Some(l) = label_at(from) {
                    return match l.node {
                        Some(node) => Action::Start(Gesture::Move { nodes: vec![node], from }, Some(l.set)),
                        None => Action::Start(Gesture::SetLabel { set: l.set, from, rect: l.rect }, Some(l.set)),
                    };
                }
                return match hit_node(from) {
                    Some(n) => {
                        let nodes = if is_selected(store, selection, n) {
                            selected_nodes(store, d, selection, n)
                        } else {
                            uuid(store, n).into_iter().collect()
                        };
                        let select_first = if is_selected(store, selection, n) { None } else { uuid(store, element_of(store, n).unwrap_or(n)) };
                        Action::Start(Gesture::Move { nodes, from }, select_first)
                    }
                    None => {
                        let (x, y) = view.to_canvas(from);
                        match hit(store, grid, x, y) {
                            // A right-angled line's middle segment slides; the rest of a line holds.
                            Some(Hit::Edge(e)) => match (segment_at(store, grid, e, x, y, f64::from(SEGMENT_REACH / view.zoom)), uuid(store, e)) {
                                (Some((index, route)), Some(edge)) => {
                                    let element = store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).unwrap_or(e);
                                    let element = crate::uml_nary::selectable(store, element);
                                    Action::Start(Gesture::Segment { edge, index, route, from }, uuid(store, element))
                                }
                                _ => Action::Nothing,
                            },
                            _ => Action::Start(Gesture::Marquee { from }, None),
                        }
                    }
                };
            }
            // A right click on an n-ary association's diamond or leg opens its menu (AR-128).
            if resp.secondary_clicked() {
                let (x, y) = view.to_canvas(pos);
                let shown = match hit_node(pos) {
                    Some(n) => element_of(store, n),
                    None => match hit(store, grid, x, y) {
                        Some(Hit::Edge(e)) => store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()),
                        _ => None,
                    },
                };
                if let Some(a) = shown.and_then(|e| crate::uml_nary::shown_by(store, e)).and_then(|a| uuid(store, a)) {
                    return Action::NaryMenu(pos, a);
                }
            }
            // A right click on a generalization's line opens the set actions (AR-129).
            if resp.secondary_clicked()
                && hit_node(pos).is_none()
                && let (x, y) = view.to_canvas(pos)
                && let Some(Hit::Edge(e)) = hit(store, grid, x, y)
                && let Some(g) = store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref())
                && archforge_uml::generalization_sets::is_generalization(store, g)
            {
                let chosen = crate::uml_sets::elements(store, selection).contains(&g);
                return Action::LineMenu(pos, if chosen { None } else { uuid(store, g) });
            }
            if resp.secondary_clicked()
                && kind.is_some_and(|k| k.menu())
                && let Some(n) = hit_node(pos)
                && let Some(entity) = part_of_node(kind, store, n)
                && let (Some(node), Some(entity)) = (uuid(store, n), uuid(store, entity))
            {
                return Action::Menu(EntityMenu { node, entity, at: pos });
            }
            // A double click puts a moved cardinality text back, or a line's dragged route.
            if resp.double_clicked() && hit_node(pos).is_none() {
                if let Some(t) = shell.canvas.ends.iter().find(|t| t.rect.contains(pos))
                    && let Some(e) = store.by_uuid(t.edge)
                    && end_label(store, e, t.end).is_some()
                {
                    return Action::ResetEnd(t.edge, t.end);
                }
                let (x, y) = view.to_canvas(pos);
                if let Some(Hit::Edge(e)) = hit(store, grid, x, y)
                    && !store.list(e, archforge_diagram::edge::WAYPOINT).unwrap_or_default().is_empty()
                    && let Some(id) = uuid(store, e)
                {
                    return Action::ResetRoute(id);
                }
            }
            // A third click in a row still opens; egui counts it as a triple click.
            if (resp.double_clicked() || resp.triple_clicked())
                && let Some(n) = hit_node(pos)
                && element_of(store, n).is_none()
                && let Some(node) = uuid(store, n)
            {
                return Action::EditNote(node);
            }
            // A double click on a card that renames in place alone (a package): it renames.
            if (resp.double_clicked() || resp.triple_clicked())
                && let Some(n) = hit_node(pos)
                && let Some(e) = element_of(store, n).filter(|e| kind.is_some_and(|k| k.renames_in_place(store, *e)))
                && let (Some(node), Some(e)) = (uuid(store, n), uuid(store, e))
            {
                return Action::Rename(node, e);
            }
            // A double click on a constraint's braces or note opens its editor (AR-133).
            if (resp.double_clicked() || resp.triple_clicked())
                && let Some(n) = hit_node(pos)
                && let Some(e) = element_of(store, n)
                && shell
                    .session
                    .as_ref()
                    .and_then(|s| s.view().of_element(store, e)?.card(store, e, d))
                    .is_some_and(|c| matches!(c.tone, archforge_diagram::figure::Tone::Constraint | archforge_diagram::figure::Tone::Braces))
                && let Some(id) = uuid(store, e)
            {
                return Action::Open(id, None);
            }
            // A double click on a card: its header renames it in place, a row opens its editor there.
            if (resp.double_clicked() || resp.triple_clicked())
                && let Some(n) = hit_node(pos)
                && let Some(entity) = part_of_node(kind, store, n)
                && let (Some(node), Some(entity_id)) = (uuid(store, n), uuid(store, entity))
            {
                let rect = view.rect(&bounds_of(store, n));
                let y_units = (pos.y - rect.top()) / view.zoom;
                if y_units < ENTITY_HEADER {
                    return Action::Rename(node, entity_id);
                }
                // The row under the pointer, counted across the card's compartments.
                let card = shell.session.as_ref().and_then(|s| s.view().of_element(store, entity)?.card(store, entity, d));
                let row = match card {
                    Some(card) => crate::canvas::row_at(&card, y_units),
                    None => {
                        let row = ((y_units - (ENTITY_FIRST_ROW - 4.0)) / ENTITY_ROW).floor();
                        (row >= 0.0).then_some(row as usize)
                    }
                };
                return Action::Open(entity_id, row);
            }
            if resp.clicked()
                && let Some(l) = label_at(pos)
            {
                return if shift { Action::Toggle(l.set) } else { Action::Select(vec![l.set]) };
            }
            if resp.clicked() {
                let (x, y) = view.to_canvas(pos);
                // A package's tab is its card's (AR-113), over any line that crosses it; a card and a
                // tab over each other give the one drawn on top.
                let hit = hit(store, grid, x, y);
                let card = match hit {
                    Some(Hit::Node(n)) => Some(n),
                    _ => None,
                };
                let target = match (on_top(shell, store, grid, card, pos), hit) {
                    (Some(n), _) => element_of(store, n).unwrap_or(n),
                    (None, Some(Hit::Edge(e))) => line_target(store, e),
                    (None, _) => d,
                };
                let Some(id) = uuid(store, target) else { return Action::Nothing };
                return if shift && target != d { Action::Toggle(id) } else { Action::Select(vec![id]) };
            }
            Action::Nothing
        }
    }
}

/// Reads and applies what the pointer asked for this frame.
pub(crate) fn pointer(shell: &mut Shell, ui: &egui::Ui, resp: &egui::Response, view: View, grid: &Grid, d: Obj) {
    let (shift, origin, released) = ui.input(|i| (i.modifiers.shift, i.pointer.press_origin(), i.pointer.primary_released()));
    let pointer = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos()));
    let kind = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d));
    // The press that closed a menu of kinds ends with the menu: its release places nothing.
    let dismissing = shell.canvas.dismissing;
    if dismissing && (released || !ui.input(|i| i.pointer.any_down())) {
        shell.canvas.dismissing = false;
    }
    let action = match shell.session.as_ref() {
        _ if dismissing => Action::Nothing,
        Some(s) => {
            let selection = s.view().selection.clone();
            decide(shell, resp, view, grid, s.doc(), d, &selection, shift, origin, released)
        }
        None => Action::Nothing,
    };
    if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
        shell.canvas.focused = true;
        shell.canvas.menu = None;
        shell.canvas.line_menu = None;
        shell.canvas.nary_menu = None;
    }
    match if shell.looking_only() && !only_looks(&action) { Action::Nothing } else { action } {
        Action::Nothing => {}
        Action::NaryEnd(node, double) => crate::uml_nary::click(shell, d, node, double),
        Action::NaryCancel => crate::uml_nary::cancel(shell),
        Action::NaryAddEnd(node) => crate::uml_nary::add_end(shell, node),
        Action::NaryMenu(at, a) => {
            select(shell, vec![a]);
            shell.canvas.nary_menu = Some((at, a));
        }
        Action::Select(ids) => select(shell, ids),
        Action::Toggle(id) => {
            let mut ids: Vec<Uuid> =
                shell.session.as_ref().map(|s| s.view().selection.iter().filter_map(|o| s.doc().uuid(*o).ok()).collect()).unwrap_or_default();
            if let Some(i) = ids.iter().position(|x| *x == id) {
                ids.remove(i);
            } else {
                ids.push(id);
            }
            select(shell, ids);
        }
        Action::Notice(why) => shell.canvas.notice = Some(why),
        Action::Start(gesture, select_first) => {
            if let Some(id) = select_first {
                select(shell, vec![id]);
            }
            shell.canvas.gesture = Some(gesture);
        }
        Action::Commit => commit(shell, view, d, pointer, shift, resp.rect),
        Action::Place(tool, x, y) => {
            if let Some(kind) = kind {
                kind.place(shell, tool, d, x, y);
            }
        }
        Action::Menu(m) => shell.canvas.menu = Some(m),
        Action::LineMenu(at, first) => {
            if let Some(id) = first {
                select(shell, vec![id]);
            }
            shell.canvas.line_menu = Some(at);
        }
        Action::Rename(node, entity) => open_rename(shell, node, entity),
        Action::PlaceNote(x, y) => place_note(shell, d, x, y),
        Action::EditNote(node) => {
            let text = shell.session.as_ref().and_then(|s| {
                s.doc().by_uuid(node).map(|n| s.doc().get(n, node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default())
            });
            if let Some(text) = text {
                shell.canvas.note_edit = Some(NoteEdit { node, text, opened: true, refused: None });
            }
        }
        Action::Open(element, row) => {
            if let Some(kind) = kind {
                kind.open(shell, element, row);
            }
        }
        Action::ResetEnd(edge, end) => {
            let label = shell.session.as_ref().and_then(|s| s.doc().by_uuid(edge).and_then(|e| end_label(s.doc(), e, end)));
            if let Some(label) = label
                && let Err(e) = run(shell, "diagram.unlabel", json!({ "label": label.to_string() }))
            {
                shell.canvas.notice = Some(e);
            }
        }
        Action::ResetRoute(edge) => {
            if let Err(e) = run(shell, "diagram.route", json!({ "edge": edge.to_string(), "points": [] })) {
                shell.canvas.notice = Some(e);
            }
        }
    }
    // What the pointer may grab: a cardinality text moves, a right-angled line's segment slides
    // across.
    if shell.canvas.gesture.is_none()
        && shell.canvas.tool == Tool::Select
        && let Some(at) = ui.input(|i| i.pointer.hover_pos()).filter(|at| resp.rect.contains(*at))
    {
        let over_node = shell.session.as_ref().is_some_and(|s| {
            let (x, y) = view.to_canvas(at);
            matches!(hit(s.doc(), grid, x, y), Some(Hit::Node(_)))
        });
        if !over_node && shell.canvas.ends.iter().any(|t| t.rect.contains(at)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        } else if let Some(store) = shell.session.as_ref().map(|s| s.doc()) {
            let (x, y) = view.to_canvas(at);
            if let Some(Hit::Edge(e)) = hit(store, grid, x, y)
                && let Some((k, route)) = segment_at(store, grid, e, x, y, f64::from(SEGMENT_REACH / view.zoom))
            {
                let level = (route[k].1 - route[k + 1].1).abs() < 1e-6;
                ui.ctx().set_cursor_icon(if level { egui::CursorIcon::ResizeVertical } else { egui::CursorIcon::ResizeHorizontal });
            }
        }
    }
}

/// The label that holds where the user put the cardinality text at `end` of line `e`.
fn end_label(store: &Store, e: Obj, end: u8) -> Option<Uuid> {
    let kind = if end == 0 { "from_end" } else { "to_end" };
    store
        .list(e, archforge_diagram::edge::LABEL)
        .unwrap_or_default()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .find(|l| store.get(*l, archforge_diagram::label::KIND).ok().and_then(|v| v.as_str().map(|k| k == kind)).unwrap_or(false))
        .and_then(|l| uuid(store, l))
}

/// Ends the gesture: a move or a resize as one engine step, a marquee as a selection.
fn commit(shell: &mut Shell, view: View, d: Obj, pointer: Option<Pos2>, shift: bool, canvas: Rect) {
    let Some(gesture) = shell.canvas.gesture.take() else { return };
    let Some(pointer) = pointer.filter(|p| p.is_finite()) else { return };
    match gesture {
        Gesture::Move { nodes, from } => {
            let delta = (pointer - from) / view.zoom;
            if delta.length() < DRAG_MIN {
                return;
            }
            // A package let go on another package, or on an ancestor in the path bar, goes into it.
            if let [node] = nodes.as_slice()
                && crate::package_nav::drop_moves(shell, view, canvas, d, *node, pointer)
            {
                return;
            }
            let ids: Vec<String> = nodes.iter().map(Uuid::to_string).collect();
            if let Err(e) = run(shell, "diagram.move", json!({ "nodes": ids, "dx": delta.x as f64, "dy": delta.y as f64 })) {
                shell.canvas.notice = Some(e);
            }
        }
        Gesture::Resize { node, corner, base, from } => {
            let r = resized(base, corner, (pointer - from) / view.zoom);
            if (r.min - base.min).length() < DRAG_MIN && (r.size() - base.size()).length() < DRAG_MIN {
                return;
            }
            let Some(session) = shell.session.as_mut() else { return };
            let node = node.to_string();
            let (dx, dy) = ((r.left() - base.left()) as f64, (r.top() - base.top()) as f64);
            let (w, h) = (r.width() as f64, r.height() as f64);
            let out = session.batch(&tr("Resize"), |b| {
                if dx.abs() >= DRAG_MIN as f64 || dy.abs() >= DRAG_MIN as f64 {
                    b.execute("diagram.move", &json!({ "nodes": [&node], "dx": dx, "dy": dy }))?;
                }
                b.execute("diagram.resize", &json!({ "node": &node, "width": w, "height": h }))?;
                Ok(())
            });
            if let Err(e) = out {
                shell.canvas.notice = Some(crate::panels::refusal(&e));
            }
        }
        Gesture::EndText { edge, end, from, centre, anchor } => {
            if (pointer - from).length() / view.zoom < DRAG_MIN {
                return;
            }
            // Kept as the text's centre from its line's end, in diagram units, so it moves with it.
            let at = (centre + (pointer - from) - anchor) / view.zoom;
            let old = shell.session.as_ref().and_then(|s| s.doc().by_uuid(edge).and_then(|e| end_label(s.doc(), e, end)));
            let Some(session) = shell.session.as_mut() else { return };
            let (kind, place) = if end == 0 { ("from_end", "source") } else { ("to_end", "target") };
            let out = session.batch(&tr("Move cardinality"), |b| {
                if let Some(old) = old {
                    b.execute("diagram.unlabel", &json!({ "label": old.to_string() }))?;
                }
                b.execute("diagram.label", &json!({ "edge": edge.to_string(), "kind": kind, "at": place, "dx": at.x as f64, "dy": at.y as f64 }))?;
                Ok(())
            });
            if let Err(e) = out {
                shell.canvas.notice = Some(crate::panels::refusal(&e));
            }
        }
        Gesture::Segment { edge, index, route, from } => {
            let by = (pointer - from) / view.zoom;
            if by.length() < DRAG_MIN || route.len() < 2 {
                return;
            }
            // The corners between the ends become the line's waypoints; a line straight across keeps
            // both its ends, which the route then meets its tables at.
            let moved = slid(&route, index, by);
            let kept = if moved.len() == 2 { &moved[..] } else { &moved[1..moved.len() - 1] };
            let points: Vec<serde_json::Value> = kept.iter().map(|(x, y)| json!({ "x": x, "y": y })).collect();
            if let Err(e) = run(shell, "diagram.route", json!({ "edge": edge.to_string(), "points": points })) {
                shell.canvas.notice = Some(e);
            }
        }
        Gesture::Connect { tool, from_node, from } => {
            let Some(kind) = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d)) else { return };
            if let Some(to_node) = node_at(shell, view, d, pointer) {
                kind.connect(shell, tool, d, from_node, to_node);
                // The line drawn, the tool hands back to Select, as the placing tools do.
                shell.canvas.tool = Tool::Select;
            } else if canvas.contains(pointer) && (pointer - from).length() / view.zoom >= DRAG_MIN {
                // Dropped on empty canvas: a new element there, joined to the one the drag began on.
                let (x, y) = view.to_canvas(pointer);
                kind.connect_to_new(shell, tool, d, from_node, x, y);
                shell.canvas.tool = Tool::Select;
            }
        }
        Gesture::SetLabel { set, from, rect } => {
            if ((pointer - from) / view.zoom).length() < DRAG_MIN {
                return;
            }
            let (x, y) = view.to_canvas(rect.min + (pointer - from));
            let Some(diagram) = shell.session.as_ref().and_then(|s| s.doc().uuid(d).ok()) else { return };
            let params = json!({ "diagram": diagram.to_string(), "set": set.to_string(), "x": x.round(), "y": y.round() });
            if let Err(e) = run(shell, "uml.generalization-set.place", params) {
                shell.canvas.notice = Some(e);
            }
        }
        Gesture::Reconnect { element, source, .. } => {
            let Some(kind) = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d)) else { return };
            if let Some(to_node) = node_at(shell, view, d, pointer) {
                kind.reconnect(shell, element, source, to_node);
            }
        }
        Gesture::LinkNote { note } => {
            if let Some(target) = link_target(shell, view, d, pointer) {
                let ids = shell.session.as_ref().and_then(|s| s.doc().uuid(d).ok());
                if let Some(diagram) = ids
                    && let Err(e) = run(
                        shell,
                        "diagram.link-note",
                        json!({ "diagram": diagram.to_string(), "note": note.to_string(), "target": target.to_string() }),
                    )
                {
                    shell.canvas.notice = Some(e);
                } else {
                    // The link drawn, the tool hands back to Select, as the placing tools do.
                    shell.canvas.tool = Tool::Select;
                }
            } else {
                shell.canvas.notice = Some(tr("Drop the link on a table or a relationship's line"));
            }
        }
        Gesture::Marquee { from } => {
            let area = Rect::from_two_pos(from, pointer);
            let Some(session) = shell.session.as_ref() else { return };
            let store = session.doc();
            let mut ids: Vec<Uuid> = if shift { session.view().selection.iter().filter_map(|o| uuid(store, *o)).collect() } else { Vec::new() };
            for n in nodes_of(store, d) {
                if view.rect(&bounds_of(store, n)).intersects(area)
                    && let Some(id) = uuid(store, element_of(store, n).unwrap_or(n))
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
            }
            if ids.is_empty() {
                ids.extend(uuid(store, d));
            }
            select(shell, ids);
        }
    }
}

/// What a note link dropped at `at` points at: the table under it, else the relationship's line
/// nearest it.
fn link_target(shell: &Shell, view: View, d: Obj, at: Pos2) -> Option<Uuid> {
    if let Some(n) = node_at(shell, view, d, at) {
        return Some(n);
    }
    let store = shell.session.as_ref()?.doc();
    let (x, y) = view.to_canvas(at);
    let shows_element = |e: Obj| store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).is_some();
    archforge_diagram::Grid::build(store, d).hit_edge(x, y, shows_element).and_then(|e| uuid(store, e))
}

/// Whether the selection is one note link; it is removed (Delete), and true is given.
fn remove_selected_link(shell: &mut Shell) -> bool {
    let Some(session) = shell.session.as_ref() else { return false };
    let store = session.doc();
    let link = match session.view().selection.as_slice() {
        [e] if store.class_of(*e).ok() == Some(archforge_diagram::EDGE)
            && store.get(*e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).is_none() =>
        {
            uuid(store, *e)
        }
        _ => None,
    };
    let Some(link) = link else { return false };
    if let Err(e) = run(shell, "diagram.disconnect", json!({ "edge": link.to_string() })) {
        shell.canvas.notice = Some(e);
    }
    true
}

/// The topmost node of `d` under a screen point whose element takes part in the diagram kind's
/// gestures: on its card, or on its package's tab.
fn node_at(shell: &Shell, view: View, d: Obj, at: Pos2) -> Option<Uuid> {
    let store = shell.session.as_ref()?.doc();
    let kind = crate::kinds::kind_of(shell, store, d);
    let card =
        nodes_of(store, d).into_iter().rev().find(|n| part_of_node(kind, store, *n).is_some() && view.rect(&bounds_of(store, *n)).contains(at));
    on_top(shell, store, &Grid::build(store, d), card, at).filter(|n| part_of_node(kind, store, *n).is_some()).and_then(|n| uuid(store, n))
}

/// Of the card under a point and the package tab under it, the one drawn on top: the later in
/// the grid's drawing order, nested nodes included, as the canvas draws them (AR-113).
fn on_top(shell: &Shell, store: &Store, grid: &Grid, card: Option<Obj>, at: Pos2) -> Option<Obj> {
    match (card, tab_at(shell, store, at)) {
        (Some(c), Some(t)) if c != t => Some(if grid.drawn_at(t) > grid.drawn_at(c) { t } else { c }),
        (c, t) => c.or(t),
    }
}

/// The package whose tab the last frame drew under a screen point, the topmost first: the tab is
/// part of its card (AR-113).
fn tab_at(shell: &Shell, store: &Store, at: Pos2) -> Option<Obj> {
    shell.canvas.package_tabs.iter().rev().find(|t| t.rect.contains(at)).and_then(|t| store.by_uuid(t.node)).filter(|n| store.is_alive(*n))
}

/// Places a note at `(x, y)`, its centre under the click, and opens its text for typing.
fn place_note(shell: &mut Shell, d: Obj, x: f64, y: f64) {
    shell.canvas.notice = None;
    let Some(diagram_id) = shell.session.as_ref().and_then(|s| s.doc().uuid(d).ok()) else { return };
    let text = tr("Note");
    let params = json!({ "diagram": diagram_id.to_string(), "text": text, "x": (x - NOTE_SIZE.0 / 2.0).round(), "y": (y - NOTE_SIZE.1 / 2.0).round(), "width": NOTE_SIZE.0, "height": NOTE_SIZE.1 });
    match run(shell, "diagram.note", params) {
        Ok(v) => {
            if let Some(node) = v.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse) {
                shell.canvas.note_edit = Some(NoteEdit { node, text, opened: true, refused: None });
                shell.canvas.focused = true;
            }
            // A creating tool makes one thing, then hands back to Select.
            shell.canvas.tool = Tool::Select;
        }
        Err(e) => shell.canvas.notice = Some(e),
    }
}

/// Commits the note's text; an empty text is refused by the engine and stays open.
fn commit_note(shell: &mut Shell) {
    let Some(edit) = shell.canvas.note_edit.clone() else { return };
    let current = shell.session.as_ref().and_then(|s| {
        s.doc().by_uuid(edit.node).map(|n| s.doc().get(n, node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default())
    });
    if current.as_deref() == Some(edit.text.trim()) {
        shell.canvas.note_edit = None;
        return;
    }
    match run(shell, "diagram.set-text", json!({ "node": edit.node.to_string(), "text": edit.text.trim() })) {
        Ok(_) => shell.canvas.note_edit = None,
        Err(e) => {
            if let Some(ne) = shell.canvas.note_edit.as_mut() {
                ne.refused = Some(e);
                ne.opened = true;
            }
        }
    }
}

/// The note's text typed in place: a field over the note; Cmd/Ctrl+Enter or the keyboard
/// leaving commits, Escape closes, a refusal stays under it.
fn note_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, rect: Rect, z: f32) {
    let paint = ui.painter().clone();
    let field = rect.shrink(6.0 * z);
    paint.rect_filled(field, 2.0 * z, p.surface);
    widgets::record_visible(ui, &mut shell.registry, "workspace.canvas.note", "field", field);
    let Some(ne) = shell.canvas.note_edit.as_mut() else { return };
    let fresh = std::mem::take(&mut ne.opened);
    let id = widgets::egui_id("workspace.canvas.note");
    let edit = egui::TextEdit::multiline(&mut ne.text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(types::UI.scaled(z).font())
        .text_color(p.text)
        .char_limit(archforge_diagram::rules::MAX_TEXT)
        .desired_width(field.width().max(0.0));
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(field.shrink(4.0 * z))));
    let _ = (*out.response).clone().accessible_name(tr("Note"));
    if fresh {
        out.response.request_focus();
    }
    if let Some(refused) = ne.refused.clone() {
        ux::text(&paint, pos2(rect.left(), rect.bottom() + 4.0 * z), &refused, types::CAPTION, p.amber, Some(rect.width().max(0.0)));
    }
    let has_focus = out.response.has_focus();
    let (enter, escape) = ui.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    let nothing_focused = ui.memory(|m| m.focused().is_none());
    if escape && (has_focus || out.response.lost_focus() || nothing_focused) {
        shell.canvas.note_edit = None;
    } else if (has_focus && enter) || (!fresh && out.response.lost_focus()) {
        commit_note(shell);
    }
}

/// The name a new entity gets: "Entity", then "Entity 2" and on, never one its schema has.
fn entity_name(store: &Store, schema: Obj) -> Option<String> {
    let taken: Vec<String> = store
        .list(schema, archforge_er::schema::ENTITY)
        .unwrap_or_default()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .map(|e| archforge_core::name_of(store, e).to_lowercase())
        .collect();
    let stem = tr("Entity");
    if !taken.contains(&stem.to_lowercase()) {
        return Some(stem);
    }
    (2..=10_000u32).map(|n| format!("{stem} {n}")).find(|name| !taken.contains(&name.to_lowercase()))
}

/// Adds a table to the diagram's schema and places it at `(x, y)`, its centre under the click,
/// as one undo step; then its name is open for typing.
pub(crate) fn place_entity(shell: &mut Shell, d: Obj, x: f64, y: f64) {
    shell.canvas.notice = None;
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(schema) = crate::diagrams::schema_of(store, d) else {
        shell.canvas.notice = Some(tr("The diagram is of no schema"));
        return;
    };
    let (Some(schema_id), Some(diagram_id)) = (uuid(store, schema), uuid(store, d)) else { return };
    let Some(name) = entity_name(store, schema) else {
        shell.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (px, py) = ((x - ENTITY_WIDTH / 2.0).round(), (y - ENTITY_EMPTY_HEIGHT / 2.0).round());
    let name_for_batch = name.clone();
    let out = session.batch(&tr("Add table"), |b| {
        let e = b.execute("er.entity.add", &json!({ "schema": schema_id.to_string(), "name": name_for_batch }))?;
        let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute(
            "diagram.place",
            &json!({ "diagram": diagram_id.to_string(), "element": entity, "x": px, "y": py, "width": ENTITY_WIDTH, "height": ENTITY_EMPTY_HEIGHT }),
        )?;
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            if let (Some(entity), Some(node)) = (id_at(0), id_at(1)) {
                select(shell, vec![entity]);
                shell.canvas.rename = Some(NodeRename { node, entity, text: name, opened: true, refused: None });
                shell.canvas.focused = true;
            }
            shell.canvas.tool = Tool::Select;
        }
        Err(e) => shell.canvas.notice = Some(crate::panels::refusal(&e)),
    }
}

/// A connection dropped on empty canvas: a new table centred there, joined to the table of
/// `from_node` by the relationship the tool is set to, all one undo step; then the new table's name
/// is open for typing. The relationship's foreign key starts off (and so it is not identifying):
/// the new table is still to be shaped, and its keys come when the user turns the key on.
pub(crate) fn connect_to_new(shell: &mut Shell, d: Obj, from_node: Uuid, x: f64, y: f64) {
    use crate::connect::cardinality_word;
    shell.canvas.notice = None;
    let kind = shell.canvas.relationship;
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(schema) = crate::diagrams::schema_of(store, d) else {
        shell.canvas.notice = Some(tr("The diagram is of no schema"));
        return;
    };
    let Some(from) = store.by_uuid(from_node).and_then(|n| entity_of_node(store, n)) else {
        shell.canvas.notice = Some(tr("A relationship joins two tables"));
        return;
    };
    let (Some(schema_id), Some(diagram_id), Some(from_id)) = (uuid(store, schema), uuid(store, d), uuid(store, from)) else { return };
    let Some(name) = entity_name(store, schema) else {
        shell.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (px, py) = ((x - ENTITY_WIDTH / 2.0).round(), (y - ENTITY_EMPTY_HEIGHT / 2.0).round());
    let name_for_batch = name.clone();
    let out = session.batch(&tr("Add table"), |b| {
        let e = b.execute("er.entity.add", &json!({ "schema": schema_id.to_string(), "name": name_for_batch }))?;
        let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let n = b.execute(
            "diagram.place",
            &json!({ "diagram": diagram_id.to_string(), "element": &entity, "x": px, "y": py, "width": ENTITY_WIDTH, "height": ENTITY_EMPTY_HEIGHT }),
        )?;
        let node = n.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let r = b.execute(
            "er.relationship.add",
            &json!({
                "from": from_id.to_string(),
                "to": &entity,
                "from_end": cardinality_word(kind.from),
                "to_end": cardinality_word(kind.to),
                "identifying": false,
                "create_foreign_key": false,
            }),
        )?;
        let rel = r.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let edge = b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram_id.to_string(), "element": rel, "source": from_node.to_string(), "target": node }),
        )?;
        let edge = edge.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute("diagram.label", &json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            if let (Some(entity), Some(node)) = (id_at(0), id_at(1)) {
                select(shell, vec![entity]);
                shell.canvas.rename = Some(NodeRename { node, entity, text: name, opened: true, refused: None });
                shell.canvas.focused = true;
            }
        }
        Err(e) => shell.canvas.notice = Some(crate::panels::refusal(&e)),
    }
}

/// Places a table dragged from the Models explorer on diagram `d`, centred on `at`; a table of
/// another schema is welcome (slice S7), one already on the diagram is said so and left.
pub(crate) fn place_dropped(shell: &mut Shell, d: Obj, entity: Uuid, at: (f64, f64)) {
    shell.canvas.notice = None;
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let Some(e) = store.by_uuid(entity).filter(|e| store.class_of(*e).ok() == Some(archforge_er::ENTITY)) else { return };
    if on_diagram(store, d, e) {
        shell.canvas.notice = Some(tr("Already on this diagram"));
        return;
    }
    let columns = store.list(e, archforge_er::entity::ATTRIBUTE).map(|l| l.len()).unwrap_or(0);
    // A very wide table lands as tall as a node may be; its rows past that say how many more.
    let height = content_height(columns).min(archforge_diagram::rules::MAX_SIZE);
    // Its relationships and subtype lines to the tables the diagram shows draw with it.
    let links: Vec<Obj> = store
        .referrers(e)
        .into_iter()
        .filter(|(_, prop)| *prop == archforge_er::relationship::FROM || *prop == archforge_er::relationship::TO)
        .map(|(r, _)| r)
        .chain(archforge_er::rules::supertype_links(store, e))
        .chain(archforge_er::rules::subtype_links(store, e))
        .collect();
    place_with_lines(shell, d, e, at, (ENTITY_WIDTH, height), &links, &archforge_er::ErNotation, &tr("Place table"));
}

/// Whether diagram `d` shows element `e`.
pub(crate) fn on_diagram(store: &Store, d: Obj, e: Obj) -> bool {
    nodes_of(store, d).into_iter().any(|n| element_of(store, n) == Some(e))
}

/// Places element `e` dropped on diagram `d`, centred on `at` at `size`, with those of `links`
/// that join it to an element the diagram shows, as one undo step named `label`; then selects it.
#[allow(clippy::too_many_arguments, reason = "a drop is placed with its size, its lines and its step's name")]
pub(crate) fn place_with_lines(
    shell: &mut Shell,
    d: Obj,
    e: Obj,
    at: (f64, f64),
    size: (f64, f64),
    links: &[Obj],
    notation: &dyn archforge_diagram::Notation,
    label: &str,
) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let (Some(diagram_id), Some(entity)) = (uuid(store, d), uuid(store, e)) else { return };
    let (x, y) = ((at.0 - size.0 / 2.0).round(), (at.1 - size.1 / 2.0).round());
    let place = json!({ "diagram": diagram_id.to_string(), "element": entity.to_string(), "x": x, "y": y, "width": size.0, "height": size.1 });
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
    let out = shell.session.as_mut().map(|session| {
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
        Some(Ok(_)) => select(shell, vec![entity]),
        Some(Err(e)) => shell.canvas.notice = Some(crate::panels::refusal(&e)),
        None => {}
    }
}

/// Where a new table lands on the open diagram when the diagram is of `schema`: to the right of
/// the rightmost node, or near the top left of an empty diagram; none when no such diagram is open.
fn free_place(shell: &Shell, schema: Obj) -> Option<(Uuid, f64, f64)> {
    let session = shell.session.as_ref()?;
    let store = session.doc();
    let d = shell.canvas.active.and_then(|id| store.by_uuid(id)).filter(|d| store.is_alive(*d))?;
    if crate::diagrams::schema_of(store, d) != Some(schema) {
        return None;
    }
    let nodes = nodes_of(store, d);
    let (x, y) = match nodes.iter().map(|n| unit_rect(store, *n)).filter(|r| r.is_finite()).max_by(|a, b| a.right().total_cmp(&b.right())) {
        Some(r) => ((r.right() + 40.0) as f64, r.top() as f64),
        None => (40.0, 40.0),
    };
    Some((uuid(store, d)?, x, y))
}

/// Adds a table to `schema` with `name`; on an open diagram of the schema it is placed as well,
/// in the same undo step. The table is selected either way. Gives the table's id.
pub fn add_entity(shell: &mut Shell, schema: Uuid, name: &str) -> Result<Uuid, String> {
    let placed = shell.session.as_ref().and_then(|s| s.doc().by_uuid(schema)).and_then(|s| free_place(shell, s));
    let session = shell.session.as_mut().ok_or_else(|| tr("The model could not be opened"))?;
    let add = json!({ "schema": schema.to_string(), "name": name });
    let entity = match placed {
        None => session.execute("er.entity.add", &add).map(|o| o.value).map_err(|e| crate::panels::refusal(&e))?,
        Some((diagram, x, y)) => {
            let out = session
                .batch(&tr("Add table"), |b| {
                    let e = b.execute("er.entity.add", &add)?;
                    let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                    b.execute(
                        "diagram.place",
                        &json!({ "diagram": diagram.to_string(), "element": entity, "x": x, "y": y, "width": ENTITY_WIDTH, "height": ENTITY_EMPTY_HEIGHT }),
                    )?;
                    Ok(())
                })
                .map_err(|e| crate::panels::refusal(&e))?;
            out.value.get(0).cloned().unwrap_or(serde_json::Value::Null)
        }
    };
    let id = entity.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse).ok_or_else(|| tr("The model could not be opened"))?;
    // Placing selected the node; the table is what the explorer and Properties follow.
    select(shell, vec![id]);
    Ok(id)
}

/// Runs a column command and, in the same undo step, grows every node of `entity` whose height
/// would hide the columns it then has.
pub fn run_growing(shell: &mut Shell, label: &str, cmd: &str, params: serde_json::Value, entity: Uuid) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    let store = session.doc();
    let Some(e) = store.by_uuid(entity) else { return Err(tr("The model could not be opened")) };
    let columns = store.list(e, archforge_er::entity::ATTRIBUTE).map(|l| l.len()).unwrap_or(0);
    let needed = content_height(columns.saturating_add(1));
    let (_, _, nodes) = archforge_diagram::views_of(store, e);
    let grow: Vec<(String, f64)> = nodes
        .iter()
        .filter_map(|n| {
            let b = bounds_of(store, *n);
            if b.height < needed { Some((uuid(store, *n)?.to_string(), b.width)) } else { None }
        })
        .collect();
    if grow.is_empty() {
        return session.execute(cmd, &params).map(|o| o.value).map_err(|e| crate::panels::refusal(&e));
    }
    session
        .batch(label, |b| {
            b.execute(cmd, &params)?;
            for (n, width) in &grow {
                b.execute("diagram.resize", &json!({ "node": n, "width": width, "height": needed }))?;
            }
            Ok(())
        })
        .map(|o| o.value.get(0).cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|e| crate::panels::refusal(&e))
}

/// Grows every table box of the model read that is too short for its columns, before the project's
/// session starts: a box an earlier version left short (its key columns hidden below its edge)
/// shows them all when the project opens, without an undo step.
pub fn fit_tables(store: &mut Store) {
    let mut grow: Vec<(Obj, f64)> = Vec::new();
    for e in store.alive().filter(|o| store.class_of(*o).ok() == Some(archforge_er::ENTITY)) {
        let columns = store
            .list(e, archforge_er::entity::ATTRIBUTE)
            .unwrap_or_default()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .filter(|a| store.is_alive(*a))
            .count();
        let needed = content_height(columns);
        let (_, _, nodes) = archforge_diagram::views_of(store, e);
        grow.extend(nodes.into_iter().filter(|n| bounds_of(store, *n).height < needed).map(|n| (n, needed)));
    }
    for (n, height) in grow {
        if let Err(e) = store.set(n, archforge_diagram::node::HEIGHT, archforge_mof::Value::Real(height)) {
            log::warn!("a table box could not be grown to show its columns: {e}");
        }
    }
}

/// Grows, inside batch `b`, every node of the tables `entities` whose height would hide the
/// columns the table has now: a command that adds key columns (a foreign key, a shared subtype
/// key) keeps them in sight in the same undo step.
pub(crate) fn grow_to_fit(
    b: &mut archforge_engine::Batch<'_, Store, archforge_diagram::ProjectView>,
    entities: &[Uuid],
) -> Result<(), archforge_engine::EngineError> {
    let mut grow: Vec<(String, f64, f64)> = Vec::new();
    {
        let store = b.doc();
        for id in entities {
            let Some(e) = store.by_uuid(*id) else { continue };
            let columns = store
                .list(e, archforge_er::entity::ATTRIBUTE)
                .unwrap_or_default()
                .iter()
                .filter_map(archforge_mof::Value::as_ref)
                .filter(|a| store.is_alive(*a))
                .count();
            let needed = content_height(columns);
            let (_, _, nodes) = archforge_diagram::views_of(store, e);
            for n in nodes {
                let bounds = bounds_of(store, n);
                if bounds.height < needed
                    && let Some(node) = uuid(store, n)
                    && !grow.iter().any(|(g, _, _)| *g == node.to_string())
                {
                    grow.push((node.to_string(), bounds.width, needed));
                }
            }
        }
    }
    for (node, width, height) in grow {
        b.execute("diagram.resize", &json!({ "node": node, "width": width, "height": height }))?;
    }
    Ok(())
}

/// Runs `cmd` and grows the nodes of `entities` to show their columns, as one undo step named
/// `label`; gives the command's result.
pub(crate) fn run_fitting(
    shell: &mut Shell,
    label: &str,
    cmd: &str,
    params: serde_json::Value,
    entities: &[Uuid],
) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session
        .batch(label, |b| {
            b.execute(cmd, &params)?;
            grow_to_fit(b, entities)
        })
        .map(|o| o.value.get(0).cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|e| crate::panels::refusal(&e))
}

fn open_rename(shell: &mut Shell, node: Uuid, entity: Uuid) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(entity) else { return };
    let text = archforge_core::name_of(store, e);
    shell.canvas.rename = Some(NodeRename { node, entity, text, opened: true, refused: None });
}

/// Commits the name typed in place; a refusal keeps it open.
fn commit_rename(shell: &mut Shell) {
    let Some(r) = shell.canvas.rename.clone() else { return };
    let current = shell.session.as_ref().and_then(|s| s.doc().by_uuid(r.entity).map(|e| archforge_core::name_of(s.doc(), e)));
    if current.as_deref() == Some(r.text.trim()) {
        shell.canvas.rename = None;
        return;
    }
    let kind = shell.session.as_ref().and_then(|s| {
        let d = s.doc().by_uuid(r.node).and_then(|n| archforge_diagram::diagram_of(s.doc(), n))?;
        crate::kinds::kind_of(shell, s.doc(), d)
    });
    let Some(kind) = kind else { return };
    match kind.rename(shell, r.entity, r.text.trim()) {
        Ok(()) => shell.canvas.rename = None,
        Err(e) => {
            // The field takes the keyboard again, the text selected, so the name can be retyped.
            if let Some(rename) = shell.canvas.rename.as_mut() {
                rename.refused = Some(e);
                rename.opened = true;
            }
        }
    }
}

/// Removes the nodes from the diagram, the model untouched.
pub(crate) fn unplace(shell: &mut Shell, nodes: Vec<Uuid>) {
    if nodes.is_empty() {
        return;
    }
    let ids: Vec<String> = nodes.iter().map(Uuid::to_string).collect();
    if let Err(e) = run(shell, "diagram.unplace", json!({ "nodes": ids })) {
        shell.canvas.notice = Some(e);
    }
}

/// How many diagrams show `element`, as a node or as a line (an association class drawn by its
/// line alone), each diagram once: the count the Delete from the model dialog gives, from the
/// canvas and from Models alike. A diagram that belongs to `element` (a package's own) counts only
/// when it shows `element` too.
pub fn diagrams_showing(store: &Store, element: Obj) -> usize {
    let (_, edges, nodes) = archforge_diagram::views_of(store, element);
    let diagrams: std::collections::BTreeSet<Obj> = nodes.iter().chain(&edges).filter_map(|v| archforge_diagram::diagram_of(store, *v)).collect();
    diagrams.len()
}

/// Asks before an entity goes from the model, with what goes with it; what its notation keeps
/// (the package Types while it holds the built-in types) is said at the canvas's top instead.
pub(crate) fn ask_delete(shell: &mut Shell, node: Uuid, entity: Uuid) {
    ask_delete_of(shell, Some(node), entity);
}

/// [`ask_delete`] for an entity selected on the diagram by its card `node`, or with none by a line
/// alone (an association class drawn without its card): the dialog then offers no removal from
/// the diagram, as the line's own Delete does that.
fn ask_delete_of(shell: &mut Shell, node: Option<Uuid>, entity: Uuid) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(entity) else { return };
    // What its notation keeps (UML's package Types with the built-in types) is said, not asked.
    if let Some(why) = crate::notation_ui::of(shell.notation_uis, store, e).and_then(|ui| ui.remove_refusal(store, e)) {
        shell.canvas.notice = Some(why);
        shell.canvas.menu = None;
        return;
    }
    let (relationships, keys, foreign_keys) = crate::notation_ui::of(shell.notation_uis, store, e).map_or((0, 0, 0), |ui| ui.consequences(store, e));
    let diagrams = diagrams_showing(store, e);
    let along = crate::notation_ui::of(shell.notation_uis, store, e).map(|ui| ui.goes_along(store, e)).unwrap_or_default();
    let applications = crate::notation_ui::of(shell.notation_uis, store, e).map(|ui| ui.applications_along(store, e)).unwrap_or_default();
    // A nested package hands what it holds up: the dialog says where, and so does the toast.
    let hands_to = crate::notation_ui::of(shell.notation_uis, store, e)
        .and_then(|ui| Some((archforge_core::name_of(store, ui.hands_up(store, e)?), ui.has_children(store, e))));
    shell.explorer.delete = Some(crate::explorer::Delete {
        along,
        applications,
        ids: vec![entity],
        // Named as every path that asks names it, an unnamed relationship by its ends (AR-216).
        name: crate::notation_ui::label(shell.notation_uis, store, e),
        relationships,
        keys,
        foreign_keys,
        diagrams,
        node,
        hands_to,
    });
    shell.canvas.menu = None;
}

/// Draws what sits over the diagram: the marquee, a selected node's adorners, the Arrange bar,
/// the name typed in place and the entity menu; then reads the keys.
pub(crate) fn overlays(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, view: View, d: Obj, overrides: &BTreeMap<Uuid, Rect>) {
    let paint = ui.painter().with_clip_rect(r);
    let kind = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d));
    if let Some(Gesture::Connect { from_node, .. }) = shell.canvas.gesture
        && let Some(kind) = kind
        && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
    {
        let target = node_at(shell, view, d, pointer).and_then(|id| shell.session.as_ref()?.doc().by_uuid(id)).map(|n| {
            let store = shell.session.as_ref().map(|s| s.doc());
            store.map(|s| view.rect(&bounds_of(s, n))).unwrap_or(Rect::NOTHING)
        });
        kind.live_line(shell, &paint, p, view, from_node, pointer, target);
    }
    // A note link being drawn: dashed from the note to the pointer.
    if let Some(Gesture::LinkNote { note }) = shell.canvas.gesture
        && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
        && let Some(rect) = shell.session.as_ref().and_then(|s| s.doc().by_uuid(note).map(|n| view.rect(&bounds_of(s.doc(), n))))
    {
        let start = crate::canvas::exit_point(rect, pointer);
        let z = view.zoom;
        let stroke = Stroke::new((1.2 * z).clamp(1.0, 2.5), p.accent);
        paint.extend(egui::Shape::dashed_line(&[start, pointer], stroke, 5.0 * z.max(0.5), 4.0 * z.max(0.5)));
    }
    // A line's end being moved: dashed from the end that stays to the pointer, the element under
    // the pointer ringed.
    if let Some(Gesture::Reconnect { fixed, .. }) = shell.canvas.gesture
        && let Some(pointer) = ui.input(|i| i.pointer.hover_pos())
    {
        let z = view.zoom;
        let stroke = Stroke::new((1.2 * z).clamp(1.0, 2.5), p.accent);
        paint.extend(egui::Shape::dashed_line(&[fixed, pointer], stroke, 5.0 * z.max(0.5), 4.0 * z.max(0.5)));
        let target = node_at(shell, view, d, pointer).and_then(|id| shell.session.as_ref()?.doc().by_uuid(id));
        if let Some(rect) = target.and_then(|n| shell.session.as_ref().map(|s| view.rect(&bounds_of(s.doc(), n)))) {
            paint.rect_stroke(rect.expand(3.0), 6.0, Stroke::new(2.0, p.accent), StrokeKind::Outside);
        }
    }
    // A template binding's draft (AR-131): dashed in the accent colour from the bound class to the
    // template until Apply, nothing in the model yet.
    if let Some((from, to)) = crate::uml_bindings::draft_line(shell, d)
        && let Some(store) = shell.session.as_ref().map(|s| s.doc())
    {
        let rect = |id: Uuid| overrides.get(&id).copied().or_else(|| store.by_uuid(id).map(|n| view.rect(&bounds_of(store, n))));
        if let (Some(a), Some(b)) = (rect(from), rect(to)) {
            crate::uml_bindings::paint_draft(&paint, p, a, b, view.zoom);
        }
    }
    if let Some(Gesture::Marquee { from }) = shell.canvas.gesture
        && let Some(to) = ui.input(|i| i.pointer.hover_pos())
    {
        let area = Rect::from_two_pos(from, to);
        paint.rect_filled(area, 2.0, p.accent_tint.gamma_multiply(0.6));
        paint.rect_stroke(area, 2.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    }
    // What is selected on this diagram, read once.
    let (single, many, rename_rect, note) = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let selection = &s.view().selection;
            let selected: Vec<(Obj, Uuid, Rect)> = nodes_of(store, d)
                .into_iter()
                .filter(|n| is_selected(store, selection, *n))
                .filter_map(|n| {
                    let id = uuid(store, n)?;
                    Some((n, id, overrides.get(&id).copied().unwrap_or_else(|| view.rect(&bounds_of(store, n)))))
                })
                .collect();
            let single = match selected.as_slice() {
                [(n, id, rect)] => part_of_node(kind, store, *n).and_then(|e| uuid(store, e)).map(|e| (*id, e, *rect)),
                _ => None,
            };
            // A selected note alone: its link handle.
            let note = match selected.as_slice() {
                [(n, id, rect)] if element_of(store, *n).is_none() => Some((*id, *rect)),
                _ => None,
            };
            let many: Vec<(Uuid, Rect)> = if selected.len() >= 2 { selected.iter().map(|(_, id, rect)| (*id, *rect)).collect() } else { Vec::new() };
            let rename_rect = shell.canvas.rename.as_ref().and_then(|rn| store.by_uuid(rn.node)).map(|n| view.rect(&bounds_of(store, n)));
            (single, many, rename_rect, note)
        }
        None => (None, Vec::new(), None, None),
    };
    if let Some((node, entity, rect)) = single
        && let Some(kind) = kind
        && shell.canvas.gesture.is_none()
    {
        adorners(shell, ui, p, r, kind, rect, node, entity);
    }
    if let Some((note, rect)) = note
        && shell.canvas.gesture.is_none()
        && shell.canvas.note_edit.is_none()
    {
        link_handle(shell, ui, p, r, rect, note);
    }
    if !many.is_empty() && shell.canvas.gesture.is_none() {
        arrange_bar(shell, ui, p, r, view, &many);
    }
    if kind.is_some_and(|k| k.reconnects()) && shell.canvas.gesture.is_none() {
        line_ends(shell, ui, p, r, view, d);
    }
    if let Some(rect) = rename_rect {
        rename_field(shell, ui, p, rect, view.zoom);
    }
    crate::uml_inline::row(shell, ui, p, view);
    let note_rect = shell
        .session
        .as_ref()
        .and_then(|s| shell.canvas.note_edit.as_ref().and_then(|ne| s.doc().by_uuid(ne.node)).map(|n| view.rect(&bounds_of(s.doc(), n))));
    if let Some(rect) = note_rect {
        note_field(shell, ui, p, rect, view.zoom);
    }
    if let Some(menu) = shell.canvas.menu.clone() {
        entity_menu(shell, ui, p, r, &menu);
    }
    if let Some(at) = shell.canvas.line_menu {
        crate::uml_sets::line_menu(shell, ui, p, r, at, d);
    }
    // An n-ary association's draft, its legs' end handles and its menu (AR-128).
    crate::uml_nary::overlay(shell, ui, p, r, view, d);
    keyboard(shell, ui, d);
}

/// The selected line element of `d`, the route of its line there and its ends' marks, when one
/// line is selected.
fn selected_line(
    store: &Store,
    view: &archforge_diagram::ProjectView,
    d: Obj,
    selection: &[Obj],
) -> Option<(Uuid, Vec<(f64, f64)>, [archforge_diagram::figure::EndMark; 2])> {
    let [element] = selection else { return None };
    // An n-ary association's leg moves only its class end, by its own handle (AR-128).
    if archforge_uml::nary::association_of_end(store, *element).is_some() {
        return None;
    }
    let (_, edges, nodes) = archforge_diagram::views_of(store, *element);
    // A card's element has no line, but an association class has both: its line keeps its handles.
    if !nodes.is_empty() && store.class_of(*element).ok() != Some(archforge_uml::ASSOCIATION_CLASS) {
        return None;
    }
    let edge = edges.into_iter().find(|e| store.container(*e).ok().flatten().is_some_and(|(c, _)| c == d))?;
    let route = Grid::build(store, d).line(edge)?.to_vec();
    let line = view.of_element(store, *element).and_then(|n| n.line(store, *element)).unwrap_or_default();
    Some((uuid(store, *element)?, route, [line.ends[0].mark, line.ends[1].mark]))
}

/// A selected line's two end handles, each on the line clear of its end's mark so the mark shows
/// whole, on the end itself where it has none, the two kept apart on screen
/// ([`crate::canvas::end_handles_at`]): each drags onto another element to move that end there.
fn line_ends(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, view: View, d: Obj) {
    if shell.looking_only() {
        return;
    }
    let Some((element, route, marks)) = shell.session.as_ref().and_then(|s| selected_line(s.doc(), s.view(), d, &s.view().selection)) else {
        return;
    };
    let mut screen: Vec<Pos2> = route.iter().map(|(x, y)| view.to_screen(*x, *y)).collect();
    // An end the last frame moved out to its qualifier box starts there (AR-127).
    let n = screen.len();
    for k in [0, n.saturating_sub(1)] {
        if let Some(at) = screen.get(k).copied()
            && let Some((_, outer)) = shell.canvas.qualified_ends.iter().find(|(from, _)| (*from - at).length() < 0.5)
        {
            screen[k] = *outer;
        }
    }
    let (Some(a), Some(b)) = (screen.first().copied(), screen.last().copied()) else { return };
    // Both handles placed together, so their hit boxes never meet on screen.
    let [at_source, at_target] = crate::canvas::end_handles_at(&screen, marks, view.zoom);
    let paint = ui.painter().with_clip_rect(r);
    for (source, at, fixed) in [(true, at_source.unwrap_or(a), b), (false, at_target.unwrap_or(b), a)] {
        let handle = Rect::from_center_size(at, Vec2::splat(12.0));
        if !r.contains(at) {
            continue;
        }
        let radius = crate::canvas::END_HANDLE;
        paint.circle_filled(at, radius, p.surface);
        paint.circle_stroke(at, radius, Stroke::new(1.5, p.accent));
        let id = format!("workspace.canvas.line-end.{}", if source { "source" } else { "target" });
        widgets::record_visible(ui, &mut shell.registry, &id, "button", handle);
        let resp = ui
            .interact(handle.expand(2.0), widgets::egui_id(&id), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Crosshair)
            .on_hover_text(tr("Drag onto another class to move this end"));
        if resp.drag_started_by(egui::PointerButton::Primary) {
            shell.canvas.gesture = Some(Gesture::Reconnect { element, source, fixed });
        }
    }
}

/// The adorners of a selected element: connect (E2), with the kind's first connect tool, and the
/// kind's own (ER's add column) with its card's controls, when its card takes new rows.
#[allow(clippy::too_many_arguments, reason = "the adorners sit beside a node over the canvas, for its kind")]
fn adorners(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    r: Rect,
    kind: &'static dyn crate::kinds::DiagramKind,
    rect: Rect,
    node: Uuid,
    entity: Uuid,
) {
    if shell.looking_only() {
        return;
    }
    let paint = ui.painter().with_clip_rect(r);
    let connect = kind.tools().iter().find(|t| t.use_ == crate::kinds::ToolUse::Connect).map(|t| t.key);
    // The second adorner is the element's own: a class's attribute, an enumeration's literal.
    let second = shell.session.as_ref().and_then(|s| s.doc().by_uuid(entity).and_then(|e| kind.adorner(s.doc(), e)));
    let mut boxes = Vec::new();
    if connect.is_some() {
        boxes.push(("connect", &icons::ADORN_CONNECT, "Connect", pos2(rect.right() + ADORNER_GAP, rect.top() - ADORNER - 6.0), true));
    }
    if let Some((key, hint)) = second {
        boxes.push((key, &icons::ADORN_ADD, hint, pos2(rect.right() + ADORNER_GAP, rect.bottom() - ADORNER + 6.0), true));
    }
    for (key, icon, hint, at, live) in boxes {
        let b = Rect::from_min_size(at, Vec2::splat(ADORNER));
        if !r.intersects(b) {
            continue;
        }
        paint.rect_filled(b, 4.0, p.surface);
        paint.rect_stroke(b, 4.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon_in(&paint, b, icon, if live { p.text_secondary } else { p.text_muted });
        // The connect adorner is dragged, so it senses drags where the other takes clicks.
        let id = format!("workspace.canvas.adorn.{key}");
        let resp = if key == "connect" {
            widgets::record_visible(ui, &mut shell.registry, &id, "button", b);
            ui.interact(b, widgets::egui_id(&id), egui::Sense::click_and_drag()).on_hover_cursor(egui::CursorIcon::Crosshair).on_hover_text(tr(hint))
        } else {
            control(shell, ui, &id, "button", b, live).on_hover_text(tr(hint))
        };
        if resp.clicked() && key != "connect" {
            kind.open(shell, entity, None);
        }
        // Dragging the connect adorner draws a line from this element.
        if let Some(tool) = connect
            && key == "connect"
            && resp.drag_started_by(egui::PointerButton::Primary)
            && shell.canvas.gesture.is_none()
        {
            // A line the tool starts nowhere at this element is said, and not begun.
            let problem = shell.session.as_ref().and_then(|s| {
                let store = s.doc();
                store.by_uuid(entity).and_then(|e| kind.line_start_problem(shell, store, tool, e))
            });
            if let Some(why) = problem {
                shell.canvas.notice = Some(why);
            } else {
                let from = ui.input(|i| i.pointer.press_origin()).filter(|o| o.is_finite()).unwrap_or(b.center());
                shell.canvas.gesture = Some(Gesture::Connect { tool, from_node: node, from });
            }
        }
    }
    kind.card_controls(shell, ui, p, r, rect, entity);
}

/// A selected note's link handle at its top right: dragged onto a table or a relationship's line,
/// it links the note to it (owner decision).
fn link_handle(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, rect: Rect, note: Uuid) {
    if shell.looking_only() {
        return;
    }
    let paint = ui.painter().with_clip_rect(r);
    let b = Rect::from_min_size(pos2(rect.right() + ADORNER_GAP, rect.top() - ADORNER - 6.0), Vec2::splat(ADORNER));
    if !r.intersects(b) {
        return;
    }
    paint.rect_filled(b, 4.0, p.surface);
    paint.rect_stroke(b, 4.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon_in(&paint, b, &icons::TOOL_LINK_NOTE, p.text_secondary);
    let id = "workspace.canvas.adorn.link";
    widgets::record_visible(ui, &mut shell.registry, id, "button", b);
    let resp = ui
        .interact(b, widgets::egui_id(id), egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::Crosshair)
        .on_hover_text(tr("Link to a table or a relationship"));
    if resp.drag_started_by(egui::PointerButton::Primary) {
        shell.canvas.gesture = Some(Gesture::LinkNote { note });
    }
}

/// The Arrange bar under two or more selected nodes: align left, centre, right; distribute;
/// stack. Each is one undo step over the selection.
fn arrange_bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, view: View, nodes: &[(Uuid, Rect)]) {
    let paint = ui.painter().with_clip_rect(r);
    let bounds = nodes.iter().fold(Rect::NOTHING, |b, (_, rect)| b.union(*rect));
    let x = (bounds.center().x - ARRANGE.x / 2.0).clamp(r.left() + 8.0, (r.right() - ARRANGE.x - 8.0).max(r.left() + 8.0));
    let y = (bounds.bottom() + ARRANGE_GAP).min(r.bottom() - ARRANGE.y - 8.0);
    let bar = Rect::from_min_size(pos2(x, y), ARRANGE);
    ux::shadow(&paint, bar, 10.0, p.shadow);
    paint.rect_filled(bar, 10.0, p.surface);
    paint.rect_stroke(bar, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let buttons: [(&str, &ux::Icon, &str, f32); 5] = [
        ("left", &icons::ARRANGE_LEFT, "Align left", 7.0),
        ("centre", &icons::ARRANGE_CENTRE, "Align centres", 41.0),
        ("right", &icons::ARRANGE_RIGHT, "Align right", 75.0),
        ("distribute", &icons::ARRANGE_DISTRIBUTE, "Distribute horizontally", 120.0),
        ("stack", &icons::ARRANGE_STACK, "Stack vertically", 154.0),
    ];
    paint.rect_filled(Rect::from_min_size(pos2(bar.left() + 112.0, bar.top() + 11.0), vec2(1.0, 20.0)), 0.0, p.border);
    let mut picked = None;
    for (key, icon, hint, bx) in buttons {
        let b = Rect::from_min_size(pos2(bar.left() + bx, bar.top() + 5.0), Vec2::splat(32.0));
        ux::icon_in(&paint, b, icon, p.text_secondary);
        if control(shell, ui, &format!("workspace.canvas.arrange.{key}"), "button", b, true).on_hover_text(tr(hint)).clicked() {
            picked = Some(key);
        }
    }
    let Some(key) = picked else { return };
    // The moves in diagram units, from the rectangles as stored.
    let units: Vec<(Uuid, Rect)> =
        nodes.iter().map(|(id, rect)| (*id, Rect::from_min_size(pos2_of(view.to_canvas(rect.min)), rect.size() / view.zoom))).collect();
    let all = units.iter().fold(Rect::NOTHING, |b, (_, rect)| b.union(*rect));
    let mut moves: Vec<(Uuid, f64, f64)> = Vec::new();
    match key {
        "left" => moves.extend(units.iter().map(|(id, rc)| (*id, (all.left() - rc.left()) as f64, 0.0))),
        "centre" => moves.extend(units.iter().map(|(id, rc)| (*id, (all.center().x - rc.center().x) as f64, 0.0))),
        "right" => moves.extend(units.iter().map(|(id, rc)| (*id, (all.right() - rc.right()) as f64, 0.0))),
        "distribute" => {
            let mut sorted = units.clone();
            sorted.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
            let widths: f32 = sorted.iter().map(|(_, rc)| rc.width()).sum();
            let gap = ((all.width() - widths) / (sorted.len().saturating_sub(1)).max(1) as f32).max(0.0);
            let mut x = all.left();
            for (id, rc) in &sorted {
                moves.push((*id, (x - rc.left()) as f64, 0.0));
                x += rc.width() + gap;
            }
        }
        _ => {
            let mut sorted = units.clone();
            sorted.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
            let mut y = all.top();
            for (id, rc) in &sorted {
                moves.push((*id, (all.left() - rc.left()) as f64, (y - rc.top()) as f64));
                y += rc.height() + STACK_GAP as f32;
            }
        }
    }
    moves.retain(|(_, dx, dy)| dx.abs() >= DRAG_MIN as f64 || dy.abs() >= DRAG_MIN as f64);
    if moves.is_empty() {
        return;
    }
    let Some(session) = shell.session.as_mut() else { return };
    let out = session.batch(&tr("Arrange"), |b| {
        for (id, dx, dy) in &moves {
            b.execute("diagram.move", &json!({ "nodes": [id.to_string()], "dx": dx, "dy": dy }))?;
        }
        Ok(())
    });
    if let Err(e) = out {
        shell.canvas.notice = Some(crate::panels::refusal(&e));
    }
}

fn pos2_of((x, y): (f64, f64)) -> Pos2 {
    pos2(x as f32, y as f32)
}

/// The name typed in the node's header: the design's tinted field over the name, the text
/// selected when the entity is new; Enter commits, Escape closes, a refusal stays under it.
fn rename_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, rect: Rect, z: f32) {
    let paint = ui.painter().clone();
    // A package's name is typed in its tab, which grows with it (AR-113).
    let in_tab = shell.canvas.rename.as_ref().is_some_and(|rn| shell.canvas.package_tabs.iter().any(|t| t.node == rn.node));
    let field = if in_tab {
        let text = shell.canvas.rename.as_ref().map(|rn| rn.text.clone()).unwrap_or_default();
        let (tab, _, _) = crate::canvas::package_tab(&paint, rect, &text, z, p.text);
        tab.shrink2(vec2(4.0, 3.0) * z)
    } else {
        Rect::from_min_size(rect.min + vec2(14.0, 28.0) * z, vec2((rect.width() - 28.0 * z).max(0.0), 30.0 * z))
    };
    let font = if in_tab { types::SMALL_STRONG.scaled(z) } else { types::SECTION.scaled(z) };
    paint.rect_filled(field, 2.0 * z, p.accent_tint);
    widgets::record_visible(ui, &mut shell.registry, "workspace.canvas.rename", "field", field);
    let Some(rn) = shell.canvas.rename.as_mut() else { return };
    let fresh = std::mem::take(&mut rn.opened);
    let inner = Rect::from_min_max(
        pos2(field.left() + 4.0 * z, field.top() + 2.0 * z),
        pos2((field.right() - 4.0 * z).max(field.left() + 4.0 * z), field.bottom() - 2.0 * z),
    );
    let id = widgets::egui_id("workspace.canvas.rename");
    let edit = egui::TextEdit::singleline(&mut rn.text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(font.font())
        .text_color(p.text)
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(tr("Name"));
    if fresh {
        out.response.request_focus();
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(rn.text.chars().count()))));
            state.store(ui.ctx(), id);
        }
    }
    if let Some(refused) = rn.refused.clone() {
        ux::text(&paint, pos2(field.left(), field.bottom() + 4.0 * z), &refused, types::CAPTION, p.amber, Some(field.width().max(0.0)));
    }
    // Escape makes the field give up the keyboard before this reads it, so it is read first.
    let has_focus = out.response.has_focus();
    let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    let nothing_focused = ui.memory(|m| m.focused().is_none());
    if escape && (has_focus || out.response.lost_focus() || nothing_focused) {
        shell.canvas.rename = None;
    } else if (has_focus && enter) || (!fresh && crate::menu_bar::ends(shell, ui.ctx(), id, out.response.lost_focus(), has_focus)) {
        commit_rename(shell);
    }
}

/// The element menu: the design's rows with their shortcuts. A table's adds its related tables,
/// its new column and its collapse; any other element's adds the new row its kind's adorner opens
/// for it (a class's attribute, an enumeration's literal), when it has one.
fn entity_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, menu: &EntityMenu) {
    let paint = ui.painter().clone();
    let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
    let (add_chord, delete_chord, model_chord) = if mac { ("⌘↩", "⌫", "⇧⌫") } else { ("Ctrl Enter", "Del", "Shift Del") };
    let store = shell.session.as_ref().map(|s| s.doc());
    let element = store.and_then(|st| st.by_uuid(menu.entity));
    let table = store.zip(element).is_some_and(|(st, e)| st.class_of(e).ok() == Some(archforge_er::ENTITY));
    let collapsible = table && store.zip(element).is_some_and(|(st, e)| archforge_er::rules::collapse_problem(st, e).is_none());
    let kind = crate::kinds::active_kind(shell);
    let mut rows: Vec<(&str, &str, &str, bool, bool)> = Vec::new();
    if table {
        rows.push(("related", "Show related tables", "R", false, false));
    }
    rows.push(("open", "Open in Models", "", true, false));
    // Its kind's new row for it, as its adorner names it (a class's attribute, an enumeration's literal).
    if table {
        rows.push(("add-column", "Add column", add_chord, true, false));
    } else if let Some((_, label)) = store.zip(element).zip(kind).and_then(|((st, e), k)| k.adorner(st, e)) {
        rows.push(("add-row", label, add_chord, true, false));
    }
    if collapsible {
        rows.push(("collapse", "Collapse into a relationship", "", true, false));
    }
    // The handoff to an agent, on every platform.
    let generate_chord = crate::actions::keys(crate::actions::GENERATE).map(crate::menu_bar::chord).unwrap_or_default();
    rows.push(("generate", "Generate…", if mac { "⇧⌘G" } else { generate_chord.as_str() }, true, false));
    rows.push(("remove", "Remove from this diagram", delete_chord, true, false));
    rows.push(("delete", "Delete from the model…", model_chord, true, true));
    let separators: Vec<usize> = if table { vec![1, rows.len() - 2] } else { vec![rows.len() - 2] };
    let height = 8.0 + rows.len() as f32 * MENU_ROW + 2.0;
    let x = menu.at.x.min(r.right() - MENU_WIDTH - 8.0).max(r.left() + 8.0);
    let y = menu.at.y.min(r.bottom() - height - 8.0).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), vec2(MENU_WIDTH, height));
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    let mut ry = m.top() + 4.0;
    for (i, (key, label, shortcut, live, danger)) in rows.iter().enumerate() {
        if separators.contains(&i) {
            paint.rect_filled(Rect::from_min_size(pos2(m.left() + 8.0, ry), vec2(MENU_WIDTH - 16.0, 1.0)), 0.0, p.border);
            ry += 1.0;
        }
        let row = Rect::from_min_size(pos2(m.left() + 4.0, ry), vec2(MENU_WIDTH - 8.0, MENU_ROW));
        let resp = control(shell, ui, &format!("workspace.canvas.menu.{key}"), "menu-item", row, *live);
        if resp.hovered() && *live {
            paint.rect_filled(row, 5.0, p.page);
        }
        let ink = if *danger {
            p.danger
        } else if *live {
            p.text
        } else {
            p.text_muted
        };
        ux::text(&paint, row.min + vec2(10.0, 7.5), &tr(label), types::SMALL, ink, Some(MENU_WIDTH - 70.0));
        if !shortcut.is_empty() {
            let g = ux::galley(&paint, shortcut, types::CAPTION, p.text_muted, None);
            paint.galley(pos2(row.right() - 10.0 - g.size().x, row.top() + 8.0), g, p.text_muted);
        }
        if resp.clicked() && *live {
            picked = Some(*key);
        }
        ry += MENU_ROW;
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos)));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.canvas.menu = None;
    }
    let Some(key) = picked else { return };
    shell.canvas.menu = None;
    match key {
        "open" => {
            shell.state.show_explorer(crate::state::Explorer::Models);
            select(shell, vec![menu.entity]);
        }
        "add-column" => {
            crate::panels::open_column(shell, menu.entity, None);
            shell.state.show_panel(crate::state::Panel::Properties);
        }
        "add-row" => {
            if let Some(kind) = kind {
                kind.open(shell, menu.entity, None);
            }
        }
        "collapse" => {
            // The command selects the relationship it makes.
            if let Err(e) = run(shell, "er.relationship.collapse", json!({ "entity": menu.entity.to_string() })) {
                shell.canvas.notice = Some(e);
            }
        }
        "generate" => {
            select(shell, vec![menu.entity]);
            shell.open_generate();
        }
        "remove" => unplace(shell, vec![menu.node]),
        "delete" => ask_delete(shell, menu.node, menu.entity),
        _ => {}
    }
}

/// Whether something open on the canvas holds its keys: an editor, a menu or the Stereotypes
/// picker, which takes typing, Backspace, the arrows and Enter (AR-135).
pub(crate) fn canvas_busy(shell: &Shell) -> bool {
    let c = &shell.canvas;
    c.rename.is_some()
        || c.inline.is_some()
        || c.note_edit.is_some()
        || c.menu.is_some()
        || c.line_menu.is_some()
        || c.nary_menu.is_some()
        || shell.panels.menu == Some(crate::panels::Menu::StereotypePick)
}

/// The keys while the canvas has them: Escape back to Select, Delete removes the selected nodes
/// from the diagram (with Shift, asks before deleting from the model), the arrows nudge, ⌘↩ opens
/// a new row on a card that takes one.
fn keyboard(shell: &mut Shell, ui: &egui::Ui, d: Obj) {
    if !shell.canvas.focused
        || shell.generate.is_some()
        || shell.about.is_some()
        || canvas_busy(shell)
        || shell.explorer.delete.is_some()
        || shell.diagrams.new.is_some()
        // A Chat history row's menu or the confirmation of a deletion takes the keys (AR-217).
        || shell.chat_panel.row_menu.is_some()
        || shell.chat_panel.ask.is_some()
    {
        return;
    }
    if ui.memory(|m| m.focused().is_some()) {
        return;
    }
    // A drawing of an n-ary association, or Add end, takes Enter, Backspace and Escape (AR-128).
    if crate::uml_nary::keys(shell, ui, d) {
        return;
    }
    // An open menu the keys walk (the classifier's kinds) takes the arrows and Enter itself; the
    // other popovers leave them to the canvas.
    let menu = shell.canvas.popover.is_some_and(crate::uml_classifier::takes_keys);
    let (escape, delete, shift, add_column, left, right, up, down) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            i.modifiers.shift,
            !menu && i.modifiers.command && i.key_pressed(egui::Key::Enter),
            !menu && i.key_pressed(egui::Key::ArrowLeft),
            !menu && i.key_pressed(egui::Key::ArrowRight),
            !menu && i.key_pressed(egui::Key::ArrowUp),
            !menu && i.key_pressed(egui::Key::ArrowDown),
        )
    });
    if shell.looking_only() && !escape {
        return;
    }
    // Escape in the classifier's menu of kinds closes the menu alone; the armed tool stays.
    if escape && shell.canvas.popover.is_some_and(crate::uml_classifier::takes_keys) {
        shell.canvas.popover = None;
        shell.canvas.menu_focus = None;
        return;
    }
    // ⌥⌘G (Ctrl Alt G): the selected generalizations into a new set (AR-129).
    if ui.input(|i| i.modifiers.command && i.modifiers.alt && !i.modifiers.shift && i.key_pressed(egui::Key::G)) && crate::uml_sets::shortcut(shell) {
        return;
    }
    if escape {
        shell.canvas.tool = Tool::Select;
        shell.canvas.gesture = None;
        shell.canvas.popover = None;
        shell.canvas.mapping = None;
        return;
    }
    let kind = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d));
    let selected: Vec<(Uuid, Option<Uuid>)> = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let selection = &s.view().selection;
            nodes_of(store, d)
                .into_iter()
                .filter(|n| is_selected(store, selection, *n))
                .filter_map(|n| Some((uuid(store, n)?, part_of_node(kind, store, n).and_then(|e| uuid(store, e)))))
                .collect()
        }
        None => Vec::new(),
    };
    // A selection that holds boxes, such as Select all's, takes the boxes off below: a set or a
    // line selected beside them is never deleted from the model on its own.
    // A selected generalization set goes from the model, its generalizations kept (AR-129).
    if delete && selected.is_empty() && crate::uml_sets::remove_selected_set(shell) {
        return;
    }
    if delete
        && !shift
        && selected.is_empty()
        && (remove_selected_link(shell) || crate::connect::remove_selected_relationship(shell) || crate::connect::remove_selected_line(shell))
    {
        return;
    }
    if selected.is_empty() {
        // A line whose element has its card on another diagram only, or on none (an association
        // class's, drawn here with its classes): Delete takes the line from this diagram, and
        // Shift+Delete asks to delete the element from the model.
        if delete && !shift {
            remove_lines_here(shell, d);
        } else if delete && let Some(entity) = line_element_here(shell, d, kind) {
            ask_delete_of(shell, None, entity);
        }
        return;
    }
    // The kind's new row, as its adorner opens it (ER's add column).
    if add_column {
        let takes_rows = |k: &&'static dyn crate::kinds::DiagramKind, entity: Uuid| {
            shell.session.as_ref().is_some_and(|s| s.doc().by_uuid(entity).is_some_and(|e| k.adorner(s.doc(), e).is_some()))
        };
        if let [(_, Some(entity))] = selected.as_slice()
            && let Some(kind) = kind.filter(|k| takes_rows(k, *entity))
        {
            if shift { kind.open_second(shell, *entity) } else { kind.open(shell, *entity, None) }
        }
        return;
    }
    if delete {
        if shift {
            // Deleting from the model takes any element the kind deletes, a package's card too,
            // not only those taking part in its gestures.
            let deletable = |node: Uuid| {
                let store = shell.session.as_ref()?.doc();
                let e = element_of(store, store.by_uuid(node)?)?;
                kind.filter(|k| k.deletes(store, e)).and_then(|_| uuid(store, e))
            };
            if let Some((node, entity)) = selected.first().copied()
                && let Some(entity) = entity.or_else(|| deletable(node))
            {
                ask_delete(shell, node, entity);
            }
        } else {
            unplace(shell, selected.iter().map(|(n, _)| *n).collect());
        }
        return;
    }
    let step = if shift { NUDGE_FAR } else { NUDGE };
    let (dx, dy) = match (left, right, up, down) {
        (true, _, _, _) => (-step, 0.0),
        (_, true, _, _) => (step, 0.0),
        (_, _, true, _) => (0.0, -step),
        (_, _, _, true) => (0.0, step),
        _ => return,
    };
    let ids: Vec<String> = selected.iter().map(|(n, _)| n.to_string()).collect();
    if let Err(e) = run(shell, "diagram.move", json!({ "nodes": ids, "dx": dx, "dy": dy, "coalesce": "nudge" })) {
        shell.canvas.notice = Some(e);
    }
}

/// The elements of `selection` that diagram `d` is asked about: an element as it is, and a line
/// selected as a line (`view.select` takes any id, an agent's an edge's too) as the element it
/// shows, only when that line is on `d`: a line of another diagram never stands for this one's. A
/// note link, which shows none, is left out. Each once, in the selection's order.
fn selected_elements(store: &Store, d: Obj, selection: &[Obj]) -> Vec<Obj> {
    let mut out: Vec<Obj> = Vec::new();
    for o in selection {
        let e = if store.class_of(*o).ok() == Some(archforge_diagram::EDGE) {
            if archforge_diagram::diagram_of(store, *o) != Some(d) {
                continue;
            }
            store.get(*o, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref())
        } else {
            Some(*o)
        };
        if let Some(e) = e.filter(|e| !out.contains(e)) {
            out.push(e);
        }
    }
    out
}

/// The selected element a line of diagram `d` shows that the diagram's kind deletes from the model
/// (an association class whose card is not on `d`), when there is one.
fn line_element_here(shell: &Shell, d: Obj, kind: Option<&'static dyn crate::kinds::DiagramKind>) -> Option<Uuid> {
    let session = shell.session.as_ref()?;
    let store = session.doc();
    let element = selected_elements(store, d, &session.view().selection).into_iter().find(|e| {
        archforge_diagram::views_of(store, *e).1.iter().any(|edge| archforge_diagram::diagram_of(store, *edge) == Some(d))
            && kind.is_some_and(|k| k.deletes(store, *e))
    })?;
    uuid(store, element)
}

/// Removes from diagram `d` the lines that show the selected elements, as one undo step, the
/// model untouched: what Delete does to a line whose element's card is not on this diagram.
fn remove_lines_here(shell: &mut Shell, d: Obj) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let edges: Vec<String> = selected_elements(store, d, &session.view().selection)
        .into_iter()
        .flat_map(|e| archforge_diagram::views_of(store, e).1)
        .filter(|e| archforge_diagram::diagram_of(store, *e) == Some(d))
        .filter_map(|e| uuid(store, e).map(|u| u.to_string()))
        .collect();
    if edges.is_empty() {
        return;
    }
    let Some(session) = shell.session.as_mut() else { return };
    let out = session.batch(&tr("Remove from this diagram"), |b| {
        for edge in &edges {
            b.execute("diagram.disconnect", &json!({ "edge": edge }))?;
        }
        Ok(())
    });
    if let Err(e) = out {
        shell.canvas.notice = Some(crate::panels::refusal(&e));
    }
}

/// Drops what the store no longer has.
pub fn retain_valid(state: &mut crate::canvas::State, store: &Store) {
    let alive = |id: &Uuid| store.by_uuid(*id).is_some_and(|o| store.is_alive(o));
    if state.rename.as_ref().is_some_and(|r| !alive(&r.node) || !alive(&r.entity)) {
        state.rename = None;
    }
    if state.note_edit.as_ref().is_some_and(|ne| !alive(&ne.node)) {
        state.note_edit = None;
    }
    if state.menu.as_ref().is_some_and(|m| !alive(&m.node)) {
        state.menu = None;
    }
    let gone = match &state.gesture {
        Some(Gesture::Move { nodes, .. }) => nodes.iter().any(|n| !alive(n)),
        Some(Gesture::Resize { node, .. }) => !alive(node),
        Some(Gesture::Connect { from_node, .. }) => !alive(from_node),
        Some(Gesture::LinkNote { note }) => !alive(note),
        Some(Gesture::Reconnect { element, .. }) => !alive(element),
        _ => false,
    };
    if gone {
        state.gesture = None;
    }
}

/// Every hover text and menu row to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    [
        "Connect",
        "Collapse into a relationship",
        "Add column",
        "Align left",
        "Align centres",
        "Align right",
        "Distribute horizontally",
        "Stack vertically",
        "Show related tables",
        "Open in Models",
        "Generate…",
        "Remove from this diagram",
        "Delete from the model…",
    ]
    .into_iter()
}
