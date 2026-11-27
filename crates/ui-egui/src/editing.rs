//! Editing on the canvas, shared by every diagram kind (from the design's cards "Two entities
//! selected", "Entity menu" and "Delete from the model"): a kind's tool places or connects through
//! the kind's hooks ([`crate::kinds::DiagramKind`]; ER's tables: [`crate::er_place`]); Select drags
//! nodes, resizes them by their handles, draws a marquee, nudges with the arrows and arranges two or
//! more with the Arrange bar; a selected card shows its kind's adorners (connect, and its new row);
//! the element menu opens in Models, hands the element to an agent, removes it from the diagram or
//! deletes it from the model after the dialog, with the rows its kind adds. Every change is one
//! engine step: a drag is previewed on the canvas and committed as one command or one batch on
//! release.

use std::collections::BTreeMap;

use archforge_diagram::grid::bounds_of;
use archforge_diagram::{Grid, Hit, ProjectView, diagram, node};
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

pub(crate) use archforge_shell_api::editing::Action;
pub use archforge_shell_api::editing::{Corner, ElementMenu, Gesture, NodeRename, NoteEdit};

use crate::canvas::{CARD_FIRST_ROW, CARD_HEADER, CARD_ROW, Tool, View};
use crate::cx::{Ctx, Request};
use crate::home::control;
use crate::i18n::tr;
use crate::notation_ui::{Layer, Overlaid};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

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
/// The element menu's width and row height.
const MENU_WIDTH: f32 = 260.0;
const MENU_ROW: f32 = 30.0;

/// A note's size when placed, in diagram units.
const NOTE_SIZE: (f64, f64) = (200.0, 72.0);

/// What lies at diagram point `(x, y)`: the grid's topmost node or nearest line, looking through a
/// node its notation says holds a label (a UML generalization set's, AR-129). A label takes a press
/// only where the last frame drew it (`canvas.extra_labels`), so one that draws nothing, its diagram
/// showing none of its set's lines, is never hit.
pub(crate) fn hit(store: &Store, notations: &ProjectView, grid: &Grid, x: f64, y: f64) -> Option<Hit> {
    grid.hit_keeping(x, y, |n| !notations.node_is_label(store, n))
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

/// [`select_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn select(shell: &mut Shell, ids: Vec<Uuid>) {
    shell.cx(|cx| select_cx(cx, ids));
}

/// Selects `ids`, as a click does; a refusal is said at the canvas's top.
pub(crate) fn select_cx(cx: &mut Ctx, ids: Vec<Uuid>) {
    let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    if let Err(e) = crate::panels::run_cx(cx, "view.select", json!({ "ids": ids })) {
        cx.canvas.notice = Some(e);
    }
}

/// Selects what each box and line drawn on diagram `d` stands for, as a click on it would, boxes
/// nested in others included, and what the diagram's kind takes in beside them
/// ([`crate::kinds::DiagramKind::also_selects`]: UML's generalization sets of its lines); or the
/// diagram itself when it draws nothing.
pub(crate) fn select_all(shell: &mut Shell, d: Obj) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let mut ids: Vec<Uuid> = Vec::new();
    let boxes =
        nodes_of(store, d).into_iter().flat_map(|top| archforge_diagram::node_tree(store, top)).map(|n| node_target(store, session.view(), n));
    let lines: Vec<Obj> = Grid::build(store, d).lines().map(|(e, _)| line_target(store, session.view(), e)).collect();
    let labels = crate::kinds::kind_of(shell, store, d).map(|k| k.also_selects(store, &lines)).unwrap_or_default();
    for id in boxes.chain(lines).chain(labels).filter_map(|o| uuid(store, o)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        ids.extend(uuid(store, d));
    }
    select(shell, ids);
}

/// What a click on line `e` selects: what its notation says a click on the element it draws
/// selects (a leg of an n-ary association its association, AR-128), and a note link itself.
fn line_target(store: &Store, notations: &ProjectView, e: Obj) -> Obj {
    notations.selects_as(store, store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).unwrap_or(e))
}

/// What a click on node `n` selects: what its notation says a click on the element it shows
/// selects ([`ProjectView::selects_as`]), and a note, which shows none, itself.
fn node_target(store: &Store, notations: &ProjectView, n: Obj) -> Obj {
    element_of(store, n).map_or(n, |e| notations.selects_as(store, e))
}

/// The id of `o`, while the store has one for it.
pub(crate) fn uuid(store: &Store, o: Obj) -> Option<Uuid> {
    store.uuid(o).ok()
}

/// The element node `n` shows, while the store has it alive; none for a note.
pub(crate) fn element_of(store: &Store, n: Obj) -> Option<Obj> {
    store.get(n, node::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e))
}

/// The element of node `n` when it takes part in the gestures of `kind`.
fn part_of_node(kind: Option<&'static dyn crate::kinds::DiagramKind>, store: &Store, n: Obj) -> Option<Obj> {
    element_of(store, n).filter(|e| kind.is_some_and(|k| k.takes_part(store, *e)))
}

/// The boxes of diagram `d` the store has alive, those at its top only.
pub(crate) fn nodes_of(store: &Store, d: Obj) -> Vec<Obj> {
    store.list(d, diagram::NODE).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref).filter(|n| store.is_alive(*n)).collect()
}

/// A node's rectangle in diagram units.
pub(crate) fn unit_rect(store: &Store, n: Obj) -> Rect {
    let b = bounds_of(store, n);
    Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32))
}

/// The nodes of the selected elements on diagram `d`, with the hit node first.
fn selected_nodes(store: &Store, notations: &ProjectView, d: Obj, selection: &[Obj], hit: Obj) -> Vec<Uuid> {
    let mut out: Vec<Uuid> = uuid(store, hit).into_iter().collect();
    for n in nodes_of(store, d) {
        if n != hit && is_selected(store, notations, selection, n) {
            out.extend(uuid(store, n));
        }
    }
    out
}

/// Whether node `n` counts as selected, as the canvas draws it and as the gestures take it: the
/// node itself, the element it shows, or what a click on it selects ([`ProjectView::selects_as`])
/// is in `selection`.
pub(crate) fn is_selected(store: &Store, notations: &ProjectView, selection: &[Obj], n: Obj) -> bool {
    selection.contains(&n) || element_of(store, n).is_some_and(|e| selection.contains(&e) || selection.contains(&notations.selects_as(store, e)))
}

/// The one selected node of `d`, when exactly one is.
fn single_selected(store: &Store, notations: &ProjectView, d: Obj, selection: &[Obj]) -> Option<Obj> {
    let mut found = nodes_of(store, d).into_iter().filter(|n| is_selected(store, notations, selection, *n));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// The corner handle of the selected node under `pos`, if any.
fn handle_at(store: &Store, notations: &ProjectView, d: Obj, selection: &[Obj], view: View, pos: Pos2) -> Option<(Obj, Corner)> {
    let n = single_selected(store, notations, d, selection).filter(|n| !notations.node_is_label(store, *n))?;
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
        | Gesture::ExtraLabel { .. } => {}
    }
    out
}

/// A label of the figures of no one node or line being dragged where no node holds it yet: the
/// element it stands for and how far it moved.
pub(crate) fn label_preview(shell: &Shell, pointer: Option<Pos2>) -> Option<(Uuid, Vec2)> {
    match (shell.canvas.gesture.as_ref(), pointer.filter(|p| p.is_finite())) {
        (Some(Gesture::ExtraLabel { element, from, .. }), Some(at)) => Some((*element, at - *from)),
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
    notations: &ProjectView,
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
        let card = match hit(store, notations, grid, x, y) {
            Some(Hit::Node(n)) => Some(n),
            _ => None,
        };
        on_top(shell, store, grid, card, pos)
    };
    // A label of the figures of no one node or line as the last frame drew it, where no card is
    // over it (a UML generalization set's, AR-129).
    let label_at = |pos: Pos2| {
        if hit_node(pos).is_some() {
            return None;
        }
        shell.canvas.extra_labels.iter().rev().find(|l| l.rect.contains(pos)).cloned()
    };
    let line_at = |pos: Pos2| {
        let (x, y) = view.to_canvas(pos);
        match hit(store, notations, grid, x, y) {
            Some(Hit::Edge(e)) => Some(e),
            _ => None,
        }
    };
    // The press as the diagram's kind reads it.
    let press = crate::kinds::Press {
        at: pos,
        clicked: resp.clicked(),
        // A third click in a row reads as a double one too: egui counts it a triple.
        double: resp.double_clicked() || resp.triple_clicked(),
        store,
        notations,
        d,
        selection,
        node_at: &hit_node,
        line_at: &line_at,
    };
    let cx = shell.cx_ref();
    let own = |press: &crate::kinds::Press<'_>| kind.and_then(|k| k.pointer(&cx, press));
    match shell.canvas.tool {
        Tool::Pan => Action::Nothing,
        Tool::Connect(tool) => {
            // The kind's own clicks come first (UML's n-ary association, drawn class by class,
            // AR-128).
            if let Some(action) = own(&press) {
                return action;
            }
            if (resp.drag_stopped_by(egui::PointerButton::Primary) || released) && shell.canvas.gesture.is_some() {
                return Action::Commit;
            }
            let Some(pos) = pos else { return Action::Nothing };
            if resp.drag_started_by(egui::PointerButton::Primary) && shell.canvas.gesture.is_none() {
                let from = origin.filter(|o| o.is_finite()).unwrap_or(pos);
                let Some((n, element)) = hit_node(from).and_then(|n| Some((n, part_of_node(kind, store, n)?))) else { return Action::Nothing };
                if let Some(why) = kind.and_then(|k| k.line_start_problem(&cx, store, tool, element)) {
                    return Action::Notice(why);
                }
                return match uuid(store, n) {
                    Some(from_node) => Action::Start(Gesture::Connect { tool, from_node, from }, None),
                    None => Action::Nothing,
                };
            }
            if resp.clicked() {
                return match hit_node(pos) {
                    Some(n) => Action::Select(uuid(store, node_target(store, notations, n)).into_iter().collect()),
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
                    Some(n) => Action::Select(uuid(store, node_target(store, notations, n)).into_iter().collect()),
                    None => Action::Nothing,
                };
            }
            Action::Nothing
        }
        Tool::Note => match (resp.clicked(), pos) {
            (true, Some(pos)) => match hit_node(pos) {
                Some(n) => Action::Select(uuid(store, node_target(store, notations, n)).into_iter().collect()),
                None => {
                    let (x, y) = view.to_canvas(pos);
                    Action::PlaceNote(x, y)
                }
            },
            _ => Action::Nothing,
        },
        Tool::Place(tool) => match (resp.clicked(), pos) {
            (true, Some(pos)) => match hit_node(pos) {
                Some(n) => Action::Select(uuid(store, node_target(store, notations, n)).into_iter().collect()),
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
            // The kind's own clicks come before a drag starts (UML's Add end, waiting on a class,
            // AR-128).
            if let Some(action) = own(&press) {
                return action;
            }
            if resp.drag_started_by(egui::PointerButton::Primary) && shell.canvas.gesture.is_none() {
                // The gesture starts where the button went down, not where the pointer is now.
                let from = origin.filter(|o| o.is_finite()).unwrap_or(pos);
                // A cardinality text, grabbed where the last frame drew it, unless a table or its
                // handle is over it there.
                if handle_at(store, notations, d, selection, view, from).is_none()
                    && hit_node(from).is_none()
                    && let Some(t) = shell.canvas.ends.iter().find(|t| t.rect.contains(from))
                {
                    return Action::Start(Gesture::EndText { edge: t.edge, end: t.end, from, centre: t.rect.center(), anchor: t.anchor }, None);
                }
                if let Some((n, corner)) = handle_at(store, notations, d, selection, view, from)
                    && let Some(id) = uuid(store, n)
                {
                    return Action::Start(Gesture::Resize { node: id, corner, base: unit_rect(store, n), from }, None);
                }
                // Such a label moves on this diagram alone: its node when it has one, else the
                // diagram's kind places it where it is let go.
                if let Some(l) = label_at(from) {
                    return match l.node {
                        Some(node) => Action::Start(Gesture::Move { nodes: vec![node], from }, Some(l.element)),
                        None => Action::Start(Gesture::ExtraLabel { element: l.element, from, rect: l.rect }, Some(l.element)),
                    };
                }
                return match hit_node(from) {
                    Some(n) => {
                        let (nodes, select_first) = if is_selected(store, notations, selection, n) {
                            (selected_nodes(store, notations, d, selection, n), None)
                        } else {
                            // The press selects what a click on the node selects, and the drag
                            // moves every node that selection takes in, as the next drag would:
                            // the pressed node, and any other whose element is the target or whose
                            // click selects it too. Where nothing else is, the node alone.
                            let target = node_target(store, notations, n);
                            (selected_nodes(store, notations, d, &[target], n), uuid(store, target))
                        };
                        Action::Start(Gesture::Move { nodes, from }, select_first)
                    }
                    None => {
                        let (x, y) = view.to_canvas(from);
                        match hit(store, notations, grid, x, y) {
                            // A right-angled line's middle segment slides; the rest of a line holds.
                            Some(Hit::Edge(e)) => match (segment_at(store, grid, e, x, y, f64::from(SEGMENT_REACH / view.zoom)), uuid(store, e)) {
                                (Some((index, route)), Some(edge)) => {
                                    let element = store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).unwrap_or(e);
                                    let element = notations.selects_as(store, element);
                                    Action::Start(Gesture::Segment { edge, index, route, from }, uuid(store, element))
                                }
                                _ => Action::Nothing,
                            },
                            _ => Action::Start(Gesture::Marquee { from }, None),
                        }
                    }
                };
            }
            // A right click means what the diagram's kind says first (UML's menus of an n-ary
            // association and of generalization lines, AR-128, AR-129).
            if resp.secondary_clicked()
                && let Some(action) = kind.and_then(|k| k.secondary_click(&cx, &press))
            {
                return action;
            }
            // Its element menu, where the kind answers one for what the card shows.
            if resp.secondary_clicked()
                && let Some(n) = hit_node(pos)
                && let Some(element) = part_of_node(kind, store, n)
                && kind.is_some_and(|k| k.menu(store, element, resp.ctx.os() == egui::os::OperatingSystem::Mac).is_some())
                && let (Some(node), Some(element)) = (uuid(store, n), uuid(store, element))
            {
                return Action::Menu(ElementMenu { node, element, at: pos });
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
                if let Some(Hit::Edge(e)) = hit(store, notations, grid, x, y)
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
                if y_units < CARD_HEADER {
                    return Action::Rename(node, entity_id);
                }
                // The row under the pointer, counted across the card's compartments.
                let card = shell.session.as_ref().and_then(|s| s.view().of_element(store, entity)?.card(store, entity, d));
                let row = match card {
                    Some(card) => crate::canvas::row_at(&card, y_units),
                    None => {
                        let row = ((y_units - (CARD_FIRST_ROW - 4.0)) / CARD_ROW).floor();
                        (row >= 0.0).then_some(row as usize)
                    }
                };
                return Action::Open(entity_id, row);
            }
            if resp.clicked()
                && let Some(l) = label_at(pos)
            {
                return if shift { Action::Toggle(l.element) } else { Action::Select(vec![l.element]) };
            }
            if resp.clicked() {
                let (x, y) = view.to_canvas(pos);
                // A part of a card its kind drew outside it is the card's, over any line that crosses
                // it; a card and a part over each other give the one drawn on top.
                let hit = hit(store, notations, grid, x, y);
                let card = match hit {
                    Some(Hit::Node(n)) => Some(n),
                    _ => None,
                };
                let target = match (on_top(shell, store, grid, card, pos), hit) {
                    (Some(n), _) => node_target(store, notations, n),
                    (None, Some(Hit::Edge(e))) => line_target(store, notations, e),
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
            decide(shell, resp, view, grid, s.doc(), s.view(), d, &selection, shift, origin, released)
        }
        None => Action::Nothing,
    };
    if resp.clicked() || resp.drag_started() || resp.secondary_clicked() {
        shell.canvas.focused = true;
        shell.canvas.menu = None;
        shell.notation_states.canvas_pressed();
    }
    match if shell.looking_only() && !only_looks(&action) { Action::Nothing } else { action } {
        Action::Nothing => {}
        Action::Own(run) => shell.cx(|cx| run(cx)),
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
                shell.cx(|cx| kind.place(cx, tool, d, x, y));
            }
        }
        Action::Menu(m) => shell.canvas.menu = Some(m),
        Action::Rename(node, element) => open_rename(shell, node, element),
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
                shell.cx(|cx| kind.open(cx, element, row));
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
            matches!(hit(s.doc(), s.view(), grid, x, y), Some(Hit::Node(_)))
        });
        if !over_node && shell.canvas.ends.iter().any(|t| t.rect.contains(at)) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        } else if let Some(session) = shell.session.as_ref() {
            let (store, (x, y)) = (session.doc(), view.to_canvas(at));
            if let Some(Hit::Edge(e)) = hit(store, session.view(), grid, x, y)
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
            // Only the layout changes, wherever the nodes are let go: over a package or the path
            // bar too, ownership stays (AR-566).
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
                shell.canvas.notice = Some(crate::panels::refusal(shell.notation_uis, &e));
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
                shell.canvas.notice = Some(crate::panels::refusal(shell.notation_uis, &e));
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
                shell.cx(|cx| kind.connect(cx, tool, d, from_node, to_node));
                // The line drawn, the tool hands back to Select, as the placing tools do.
                shell.canvas.tool = Tool::Select;
            } else if canvas.contains(pointer) && (pointer - from).length() / view.zoom >= DRAG_MIN {
                // Dropped on empty canvas: a new element there, joined to the one the drag began on.
                let (x, y) = view.to_canvas(pointer);
                shell.cx(|cx| kind.connect_to_new(cx, tool, d, from_node, x, y));
                shell.canvas.tool = Tool::Select;
            }
        }
        Gesture::ExtraLabel { element, from, rect } => {
            if ((pointer - from) / view.zoom).length() < DRAG_MIN {
                return;
            }
            let (x, y) = view.to_canvas(rect.min + (pointer - from));
            let Some(kind) = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d)) else { return };
            shell.cx(|cx| kind.place_label(cx, d, element, x, y));
        }
        Gesture::Reconnect { element, source, at, .. } => {
            let Some(kind) = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d)) else { return };
            if let Some(to_node) = node_at(shell, view, d, pointer) {
                shell.cx(|cx| kind.reconnect(cx, element, source, to_node, at));
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
                    && let Some(id) = uuid(store, node_target(store, session.view(), n))
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

/// Removes the selected line's element of any notation (a class diagram's association,
/// generalization or dependency) with the lines that show it, as one undo step, and says so in the
/// toast with Undo; false when no such line is selected. An element its notation also draws as a
/// card (an association class) is never deleted from the model here, card or no card: Delete on
/// its line takes it from the diagram, and only ⇧⌫ and its dialog delete it.
pub fn remove_selected_line(shell: &mut Shell) -> bool {
    let Some(session) = shell.session.as_ref() else { return false };
    let store = session.doc();
    let element = session.view().selection.iter().copied().find_map(|o| match store.class_of(o).ok() {
        Some(c) if c == archforge_diagram::EDGE => store.get(o, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()),
        _ => Some(o),
    });
    let Some(element) = element.filter(|e| {
        let (_, edges, nodes) = archforge_diagram::views_of(store, *e);
        !edges.is_empty() && nodes.is_empty() && !session.view().drawn_as_card(store, *e)
    }) else {
        return false;
    };
    let Some(id) = store.uuid(element).ok() else { return false };
    let ui = crate::notation_ui::of(shell.notation_uis, store, element);
    let word = ui.and_then(|ui| ui.words(store, element)).map(|w| w.word).unwrap_or_default();
    // What its notation says of the removal is worked out while the element is still there: UML
    // names the generalization sets it alone made up, which go with it (AR-129). The one notation
    // that gives the toast answers, so the toast names only what that notation worked out.
    let removal = ui.map_or_else(crate::notation_ui::Removal::default, |ui| ui.before_removal(store, &[element]));
    let Some(session) = shell.session.as_mut() else { return false };
    let removed = session.execute("element.remove", &json!({ "ids": [id.to_string()] })).map_err(|e| crate::panels::refusal(shell.notation_uis, &e));
    match removed {
        Ok(_) => {
            let text = ui.map_or_else(|| crate::i18n::fmt(&tr("{what} removed"), &[("what", &word)]), |ui| ui.removed_text(&word, &removal));
            let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            shell.canvas.toast = Some(crate::canvas::Toast { text, until: shell.frames.saturating_add(crate::canvas::TOAST_FRAMES), undo_label });
            crate::notation_ui::say_after(shell, removal.after, "");
        }
        Err(e) => shell.canvas.notice = Some(e),
    }
    true
}

/// The topmost node of `d` under a screen point whose element takes part in the diagram kind's
/// gestures: on its card, or on a part of one its kind drew outside it, the one drawn on top where
/// a card covers a part (AR-570).
fn node_at(shell: &Shell, view: View, d: Obj, at: Pos2) -> Option<Uuid> {
    let session = shell.session.as_ref()?;
    let store = session.doc();
    let kind = crate::kinds::kind_of(shell, store, d);
    let card =
        nodes_of(store, d).into_iter().rev().find(|n| part_of_node(kind, store, *n).is_some() && view.rect(&bounds_of(store, *n)).contains(at));
    on_top(shell, store, &crate::canvas::grid(store, session.view(), d), card, at)
        .filter(|n| part_of_node(kind, store, *n).is_some())
        .and_then(|n| uuid(store, n))
}

/// Of the card under a point and the part of a card under it, the one drawn on top: the later in
/// the grid's drawing order, nested nodes included, as the canvas draws them (AR-113).
fn on_top(shell: &Shell, store: &Store, grid: &Grid, card: Option<Obj>, at: Pos2) -> Option<Obj> {
    match (card, part_at(shell, store, at)) {
        (Some(c), Some(t)) if c != t => Some(if grid.drawn_at(t) > grid.drawn_at(c) { t } else { c }),
        (c, t) => c.or(t),
    }
}

/// The node of the part of a card its kind drew outside it in the last frame under a screen point,
/// the topmost first: the part takes a press as its card does
/// (`archforge_shell_api::kinds::DiagramKind::draw_card`).
fn part_at(shell: &Shell, store: &Store, at: Pos2) -> Option<Obj> {
    shell.canvas.card_parts.iter().rev().find(|t| t.rect.contains(at)).and_then(|t| store.by_uuid(t.node)).filter(|n| store.is_alive(*n))
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

/// The note's text typed in place: a field over the note; Cmd/Ctrl+Enter, exactly, or the keyboard
/// leaving commits, though not for the in-window Edit menu, whose rows act on it; Escape closes, a
/// refusal stays under it.
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
        .text_color(p.text.into())
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
    let (enter, escape) =
        ui.input(|i| (crate::actions::chord_press(i, egui::Key::Enter, egui::Modifiers::COMMAND), i.key_pressed(egui::Key::Escape)));
    let nothing_focused = ui.memory(|m| m.focused().is_none());
    if escape && (has_focus || out.response.lost_focus() || nothing_focused) {
        shell.canvas.note_edit = None;
    } else if (has_focus && enter) || (!fresh && crate::menu_bar::ends(shell, ui.ctx(), id, out.response.lost_focus(), has_focus)) {
        commit_note(shell);
    }
}

/// Whether diagram `d` shows element `e`.
pub(crate) fn on_diagram(store: &Store, d: Obj, e: Obj) -> bool {
    nodes_of(store, d).into_iter().any(|n| element_of(store, n) == Some(e))
}

/// Places element `e` dropped on diagram `d`, centred on `at` at `size`, with those of `links`
/// that join it to an element the diagram shows, as one undo step named `label`; then selects it.
#[allow(clippy::too_many_arguments, reason = "a drop is placed with its size, its lines and its step's name")]
pub(crate) fn place_with_lines_cx(
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
        Some(Ok(_)) => select_cx(cx, vec![entity]),
        Some(Err(e)) => cx.canvas.notice = Some(crate::panels::refusal(cx.notation_uis, &e)),
        None => {}
    }
}

/// Opens the name field in the header of `node` for the element it shows, holding the element's
/// name as it is; nothing opens when no project is open or the element is gone.
fn open_rename(shell: &mut Shell, node: Uuid, element: Uuid) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(element) else { return };
    let text = archforge_core::name_of(store, e);
    shell.canvas.rename = Some(NodeRename { node, element, text, opened: true, refused: None });
}

/// Commits the name typed in place; a refusal keeps it open.
fn commit_rename(shell: &mut Shell) {
    let Some(r) = shell.canvas.rename.clone() else { return };
    let current = shell.session.as_ref().and_then(|s| s.doc().by_uuid(r.element).map(|e| archforge_core::name_of(s.doc(), e)));
    if current.as_deref() == Some(r.text.trim()) {
        shell.canvas.rename = None;
        return;
    }
    let kind = shell.session.as_ref().and_then(|s| {
        let d = s.doc().by_uuid(r.node).and_then(|n| archforge_diagram::diagram_of(s.doc(), n))?;
        crate::kinds::kind_of(shell, s.doc(), d)
    });
    let Some(kind) = kind else { return };
    match shell.cx(|cx| kind.rename(cx, r.element, r.text.trim())) {
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

/// [`unplace_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn unplace(shell: &mut Shell, nodes: Vec<Uuid>) {
    shell.cx(|cx| unplace_cx(cx, nodes));
}

/// Removes the nodes from the diagram, the model untouched; a refusal is said at the canvas's top.
pub(crate) fn unplace_cx(cx: &mut Ctx, nodes: Vec<Uuid>) {
    if nodes.is_empty() {
        return;
    }
    let ids: Vec<String> = nodes.iter().map(Uuid::to_string).collect();
    if let Err(e) = crate::panels::run_cx(cx, "diagram.unplace", json!({ "nodes": ids })) {
        cx.canvas.notice = Some(e);
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

/// [`ask_delete_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn ask_delete(shell: &mut Shell, node: Uuid, entity: Uuid) {
    shell.cx(|cx| ask_delete_cx(cx, node, entity));
}

/// Asks before an entity goes from the model, with what goes with it; what its notation keeps
/// (the package Types while it holds the built-in types) is said at the canvas's top instead.
pub(crate) fn ask_delete_cx(cx: &mut Ctx, node: Uuid, entity: Uuid) {
    ask_delete_of(cx, Some(node), entity);
}

/// [`ask_delete_cx`] for an entity selected on the diagram by its card `node`, or with none by a
/// line alone (an association class drawn without its card): the dialog then offers no removal
/// from the diagram, as the line's own Delete does that. The shell opens the dialog once the call
/// that asked returns ([`Request::AskDelete`]).
fn ask_delete_of(cx: &mut Ctx, node: Option<Uuid>, entity: Uuid) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(entity) else { return };
    // What its notation keeps (UML's package Types with the built-in types) is said, not asked.
    if let Some(why) = crate::notation_ui::of(cx.notation_uis, store, e).and_then(|ui| ui.remove_refusal(cx.notation_uis, store, e)) {
        cx.canvas.notice = Some(why);
        cx.canvas.menu = None;
        return;
    }
    let consequences = crate::notation_ui::of(cx.notation_uis, store, e).map(|ui| ui.consequences(cx.notation_uis, store, e)).unwrap_or_default();
    let diagrams = diagrams_showing(store, e);
    // A nested package hands what it holds up: the dialog says where, and so does the toast.
    let hands_to = crate::notation_ui::of(cx.notation_uis, store, e)
        .and_then(|ui| Some((archforge_core::name_of(store, ui.hands_up(store, e)?), ui.has_children(store, e))));
    cx.requests.push(Request::AskDelete(crate::explorer::Delete {
        ids: vec![entity],
        // Named as every path that asks names it, an unnamed relationship by its ends (AR-216).
        name: crate::notation_ui::label(cx.notation_uis, store, e),
        consequences,
        diagrams,
        node,
        hands_to,
    }));
    cx.canvas.menu = None;
}

/// Draws what sits over the diagram: the marquee, a selected node's adorners, the Arrange bar,
/// the name typed in place and the element menu, with the notations' drafts, rows and menus each at
/// its layer among them ([`crate::notation_ui::Layer`]); then reads the keys.
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
        kind.live_line(&shell.cx_ref(), &paint, p, view, from_node, pointer, target);
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
    // What the notations draw over the canvas, each at its place: a line drafted, not in the model
    // yet; a row typed in place on a card; their own menus.
    let on = Overlaid { p, r, view, d, overrides };
    crate::notation_ui::overlays(shell, ui, Layer::Drafts, &on);
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
                .filter(|n| is_selected(store, s.view(), selection, *n))
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
    if let Some(hint) = kind.and_then(|k| k.reconnects())
        && shell.canvas.gesture.is_none()
    {
        line_ends(shell, ui, p, r, view, d, &hint);
    }
    if let Some(rect) = rename_rect {
        rename_field(shell, ui, p, rect, view.zoom);
    }
    crate::notation_ui::overlays(shell, ui, Layer::Rows, &on);
    let note_rect = shell
        .session
        .as_ref()
        .and_then(|s| shell.canvas.note_edit.as_ref().and_then(|ne| s.doc().by_uuid(ne.node)).map(|n| view.rect(&bounds_of(s.doc(), n))));
    if let Some(rect) = note_rect {
        note_field(shell, ui, p, rect, view.zoom);
    }
    if let Some(menu) = shell.canvas.menu.clone() {
        element_menu(shell, ui, p, r, &menu);
    }
    crate::notation_ui::overlays(shell, ui, Layer::Menus, &on);
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
    // Its notation says whether its line has handles: an n-ary association's leg moves only its
    // class end, by its own handle (AR-128); a card's element has no line, but an association class
    // has both, and its line keeps its handles.
    if !view.line_handles(store, *element) {
        return None;
    }
    let (_, edges, _) = archforge_diagram::views_of(store, *element);
    let edge = edges.into_iter().find(|e| store.container(*e).ok().flatten().is_some_and(|(c, _)| c == d))?;
    let route = Grid::build(store, d).line(edge)?.to_vec();
    let line = view.of_element(store, *element).and_then(|n| n.line(store, *element)).unwrap_or_default();
    Some((uuid(store, *element)?, route, [line.ends[0].mark, line.ends[1].mark]))
}

/// A selected line's two end handles, each on the line clear of its end's mark so the mark shows
/// whole, on the end itself where it has none, the two kept apart on screen
/// ([`crate::canvas::end_handles_at`]): each drags onto another element to move that end there,
/// and says `hint` on hover, in the diagram kind's words.
fn line_ends(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, view: View, d: Obj, hint: &str) {
    if shell.looking_only() {
        return;
    }
    let Some((element, route, marks)) = shell.session.as_ref().and_then(|s| selected_line(s.doc(), s.view(), d, &s.view().selection)) else {
        return;
    };
    let mut screen: Vec<Pos2> = route.iter().map(|(x, y)| view.to_screen(*x, *y)).collect();
    // An end a kind moved off its node's border in the last frame starts where it moved to.
    let n = screen.len();
    for k in [0, n.saturating_sub(1)] {
        if let Some(at) = screen.get(k).copied()
            && let Some((_, outer)) = shell.canvas.moved_ends.iter().find(|(from, _)| (*from - at).length() < 0.5)
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
            .on_hover_text(hint);
        if resp.drag_started_by(egui::PointerButton::Primary)
            && let Some(at) = shell.session.as_ref().map(|s| s.token())
        {
            shell.canvas.gesture = Some(Gesture::Reconnect { element, source, fixed, at });
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
            shell.cx(|cx| kind.open(cx, entity, None));
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
                store.by_uuid(entity).and_then(|e| kind.line_start_problem(&shell.cx_ref(), store, tool, e))
            });
            if let Some(why) = problem {
                shell.canvas.notice = Some(why);
            } else {
                let from = ui.input(|i| i.pointer.press_origin()).filter(|o| o.is_finite()).unwrap_or(b.center());
                shell.canvas.gesture = Some(Gesture::Connect { tool, from_node: node, from });
            }
        }
    }
    shell.cx(|cx| kind.card_controls(cx, ui, p, r, rect, entity));
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
        shell.canvas.notice = Some(crate::panels::refusal(shell.notation_uis, &e));
    }
}

fn pos2_of((x, y): (f64, f64)) -> Pos2 {
    pos2(x as f32, y as f32)
}

/// The name typed in the node's header, or where a kind places it on a card the kind drew itself
/// in the last frame: the design's tinted field over the name, the text selected when
/// the entity is new; Enter commits, Escape closes, a refusal stays under it.
fn rename_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, rect: Rect, z: f32) {
    let paint = ui.painter().clone();
    let placed = shell.canvas.rename.as_ref().and_then(|rn| {
        let drew = shell.canvas.kind_cards.iter().find(|(node, _)| *node == rn.node)?.1;
        let kind = shell.kinds.iter().find(|k| k.kind() == drew)?;
        let store = shell.session.as_ref()?.doc();
        kind.name_field(&paint, store, store.by_uuid(rn.node)?, rect, &rn.text, z)
    });
    let (field, font) = placed.unwrap_or_else(|| {
        (Rect::from_min_size(rect.min + vec2(14.0, 28.0) * z, vec2((rect.width() - 28.0 * z).max(0.0), 30.0 * z)), types::SECTION.scaled(z))
    });
    paint.rect_filled(field, 2.0 * z, p.accent_tint);
    widgets::record_visible(ui, &mut shell.registry, "workspace.canvas.rename", "field", field);
    let Some(element) = shell.canvas.rename.as_ref().map(|rn| rn.element) else { return };
    let limit = crate::notation_ui::name_limit_of(shell, element);
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
        .text_color(p.text.into())
        .char_limit(limit)
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

/// The keys that open a new row on a selected card (⌘↩), as an element menu's row shows them; `mac`
/// spells them as the Mac does.
pub(crate) fn new_row_keys(mac: bool) -> &'static str {
    if mac { "⌘↩" } else { "Ctrl Enter" }
}

/// The element menu: the design's rows with their shortcuts. Open in Models, Generate…, Remove from
/// this diagram and Delete from the model… are every element's; the diagram's kind adds its own
/// ([`crate::kinds::DiagramKind::menu`]: a table's related tables, new column and collapse, a
/// class's new attribute), each with what it does.
fn element_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, menu: &ElementMenu) {
    let paint = ui.painter().clone();
    let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
    let (delete_chord, model_chord) = if mac { ("⌫", "⇧⌫") } else { ("Del", "Shift Del") };
    let kind = crate::kinds::active_kind(shell);
    let own: Vec<crate::kinds::MenuRow> =
        shell.session.as_ref().and_then(|s| s.doc().by_uuid(menu.element).zip(kind).and_then(|(e, k)| k.menu(s.doc(), e, mac))).unwrap_or_default();
    let (first, after): (Vec<&crate::kinds::MenuRow>, Vec<&crate::kinds::MenuRow>) = own.iter().partition(|row| row.first);
    let row_of = |row: &&crate::kinds::MenuRow| (row.key, row.label, row.keys, row.live, false);
    let mut rows: Vec<(&str, &str, &str, bool, bool)> = first.iter().map(row_of).collect();
    rows.push(("open", "Open in Models", "", true, false));
    rows.extend(after.iter().map(row_of));
    // The handoff to an agent, on every platform.
    let generate_chord = crate::actions::keys(crate::actions::GENERATE).map(crate::menu_bar::chord).unwrap_or_default();
    rows.push(("generate", "Generate…", if mac { "⇧⌘G" } else { generate_chord.as_str() }, true, false));
    rows.push(("remove", "Remove from this diagram", delete_chord, true, false));
    rows.push(("delete", "Delete from the model…", model_chord, true, true));
    let separators: Vec<usize> = if first.is_empty() { vec![rows.len() - 2] } else { vec![first.len(), rows.len() - 2] };
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
            paint.galley(pos2(row.right() - 10.0 - g.size().x, row.top() + 8.0), g, p.text_muted.into());
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
            select(shell, vec![menu.element]);
        }
        "generate" => {
            select(shell, vec![menu.element]);
            shell.open_generate();
        }
        "remove" => unplace(shell, vec![menu.node]),
        "delete" => ask_delete(shell, menu.node, menu.element),
        key => {
            if let Some(row) = own.iter().find(|row| row.key == key) {
                shell.cx(|cx| (row.run)(cx, menu));
            }
        }
    }
}

/// Whether something open on the canvas holds its keys: an editor, a menu, something a notation
/// opened there ([`crate::notation_ui::NotationState::holds_keys`]), or a notation's menu that takes
/// typing, Backspace, the arrows and Enter, as UML's Stereotypes picker does (AR-135,
/// [`crate::notation_ui::NotationState::menu_takes_keys`]).
pub(crate) fn canvas_busy(shell: &Shell) -> bool {
    let c = &shell.canvas;
    c.rename.is_some()
        || c.note_edit.is_some()
        || c.menu.is_some()
        || shell.notation_states.holds_keys()
        || shell.notation_states.menu_takes_keys(shell.panels.menu)
}

/// The keys while the canvas has them: a drawing under way on the diagram's kind takes its own
/// first, then a notation's chord on the selection (UML's ⌥⌘G); Escape back to Select, Delete
/// removes a notation's selected element its own way or the selected nodes from the diagram (with
/// Shift, asks before deleting from the model), the arrows nudge (plain or with Shift, never as
/// part of a chord), ⌘↩ opens a new row on a card that takes one and ⇧⌘↩ its second kind of row;
/// the chords are matched exactly.
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
        // So does a diagram row's menu (AR-644).
        || shell.diagrams.menu.is_some()
    {
        return;
    }
    if ui.memory(|m| m.focused().is_some()) {
        return;
    }
    // A drawing under way on the diagram's kind takes its keys first (UML's n-ary association, or
    // Add end: Enter, Backspace and Escape, AR-128).
    let kind = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d));
    if kind.is_some_and(|k| shell.cx(|cx| k.keys(cx, ui, d))) {
        return;
    }
    // An open menu the keys walk (UML's classifier kinds) takes the arrows and Enter itself; the
    // other popovers leave them to the canvas.
    let menu = crate::kinds::popover_takes_keys(shell);
    let (escape, delete, shift, add_row, add_second, left, right, up, down) = ui.input(|i| {
        let command = egui::Modifiers::COMMAND;
        (
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            i.modifiers.shift,
            !menu && crate::actions::chord_press(i, egui::Key::Enter, command),
            !menu && crate::actions::chord_press(i, egui::Key::Enter, command | egui::Modifiers::SHIFT),
            !menu && crate::actions::plain_press(i, egui::Key::ArrowLeft),
            !menu && crate::actions::plain_press(i, egui::Key::ArrowRight),
            !menu && crate::actions::plain_press(i, egui::Key::ArrowUp),
            !menu && crate::actions::plain_press(i, egui::Key::ArrowDown),
        )
    });
    if shell.looking_only() && !escape {
        return;
    }
    // Escape in a menu the keys walk closes the menu alone; the armed tool stays.
    if escape && menu {
        shell.canvas.popover = None;
        shell.canvas.menu_focus = None;
        return;
    }
    // A notation's chord on the selection (UML's ⌥⌘G makes a generalization set, AR-129).
    let uis = shell.notation_uis;
    if uis.iter().any(|u| shell.cx(|cx| u.canvas_keys(cx, ui))) {
        return;
    }
    if escape {
        shell.canvas.tool = Tool::Select;
        shell.canvas.gesture = None;
        shell.canvas.popover = None;
        shell.notation_states.canvas_escape();
        return;
    }
    let selected: Vec<(Uuid, Option<Uuid>)> = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let selection = &s.view().selection;
            nodes_of(store, d)
                .into_iter()
                .filter(|n| is_selected(store, s.view(), selection, *n))
                .filter_map(|n| Some((uuid(store, n)?, part_of_node(kind, store, n).and_then(|e| uuid(store, e)))))
                .collect()
        }
        None => Vec::new(),
    };
    // A selection that holds boxes, such as Select all's, takes the boxes off below: a set or a
    // line selected beside them is never deleted from the model on its own.
    // A notation removes one of its own selected elements its own way: UML a generalization set,
    // its generalizations kept, with Shift too (AR-129); ER a relationship with its foreign key,
    // without. A set's label's node never counts as a box here: `selected` holds the diagram's own
    // nodes alone, and a label's node is always nested in a classifier's, so a set selected by its
    // label, moved or not, leaves `selected` empty
    // (`delete_on_a_set_label_removes_the_set_and_keeps_its_lines`).
    if delete && selected.is_empty() && uis.iter().any(|u| shell.cx(|cx| u.canvas_delete(cx, shift))) {
        return;
    }
    if delete && !shift && selected.is_empty() && (remove_selected_link(shell) || remove_selected_line(shell)) {
        return;
    }
    if selected.is_empty() {
        // A line whose element has its card on another diagram only, or on none (an association
        // class's, drawn here with its classes): Delete takes the line from this diagram, and
        // Shift+Delete asks to delete the element from the model.
        if delete && !shift {
            remove_lines_here(shell, d);
        } else if delete && let Some(entity) = line_element_here(shell, d, kind) {
            shell.cx(|cx| ask_delete_of(cx, None, entity));
        }
        return;
    }
    // The kind's new row, as its adorner opens it (ER's add column).
    if add_row || add_second {
        let takes_rows = |k: &&'static dyn crate::kinds::DiagramKind, entity: Uuid| {
            shell.session.as_ref().is_some_and(|s| s.doc().by_uuid(entity).is_some_and(|e| k.adorner(s.doc(), e).is_some()))
        };
        if let [(_, Some(entity))] = selected.as_slice()
            && let Some(kind) = kind.filter(|k| takes_rows(k, *entity))
        {
            if add_second { shell.cx(|cx| kind.open_second(cx, *entity)) } else { shell.cx(|cx| kind.open(cx, *entity, None)) }
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
        shell.canvas.notice = Some(crate::panels::refusal(shell.notation_uis, &e));
    }
}

/// Drops what the store no longer has.
pub fn retain_valid(state: &mut crate::canvas::State, store: &Store) {
    let alive = |id: &Uuid| store.by_uuid(*id).is_some_and(|o| store.is_alive(o));
    if state.rename.as_ref().is_some_and(|r| !alive(&r.node) || !alive(&r.element)) {
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
        "Align left",
        "Align centres",
        "Align right",
        "Distribute horizontally",
        "Stack vertically",
        "Open in Models",
        "Generate…",
        "Remove from this diagram",
        "Delete from the model…",
    ]
    .into_iter()
}
