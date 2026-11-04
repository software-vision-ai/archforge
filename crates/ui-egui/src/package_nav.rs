//! Package navigation (AR-95, from the design's "Canvas · A package on a class diagram, selected,
//! with Open and the path bar", "Canvas · Inside a package: the path bar leads back" and "Package
//! navigation · thresholds, keys and transitions" frames). Each package on a class diagram leads to
//! its own class diagram, made with the package when it is placed.
//!
//! - **In**: a selected package's Open, ⌘↓, or zooming in until one package covers
//!   [`ENTER_COVER`] of the canvas opens the package's diagram, fitted.
//! - **Out**: the path bar's ancestors, ⌘↑, or zooming out on a package's diagram until what it
//!   shows covers less than [`LEAVE_COVER`] of the canvas (or the zoom is at its least) opens the
//!   parent's diagram as it was left; after a zoom out the package shows at [`REVEAL`] of the
//!   canvas's width, below the threshold.
//! - Thresholds rest for [`REST`] seconds after a switch, and only a zoom made by the user counts,
//!   so one gesture never passes through two levels. A switch fades in over [`FADE`] seconds.

use std::collections::HashMap;

use archforge_diagram::{diagram, grid::bounds_of, node};
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::tr;
use crate::ux::{self, Palette, icons, types};

/// The share of the canvas one package covers once zooming in opens it.
pub const ENTER_COVER: f32 = 0.6;
/// The share of the canvas a package's diagram covers once zooming out leaves it.
pub const LEAVE_COVER: f32 = 0.2;
/// How long the thresholds rest after a switch, in seconds.
pub const REST: f64 = 0.6;
/// How long a switch fades in, in seconds.
pub const FADE: f64 = 0.2;
/// The share of the canvas's width the package left takes after a zoom out.
pub const REVEAL: f32 = 0.33;
/// The deepest package chain the path bar walks.
const MAX_DEPTH: usize = 256;

/// What package navigation keeps between frames.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nav {
    /// The zoom the user asked for this frame, as a factor; zero for none.
    pub(crate) intent: f32,
    /// The view each diagram had when it was left for another level, by diagram.
    pub(crate) views: HashMap<Uuid, (Vec2, f32)>,
    /// The diagram each package's diagram was entered from, by the diagram entered: going back
    /// returns there, though the parent package has other class diagrams too.
    pub(crate) came_from: HashMap<Uuid, Uuid>,
    /// When the last switch happened, and when it began to fade in.
    pub(crate) switched: Option<f64>,
    /// The package to show at [`REVEAL`] of the canvas on the next frame, after a zoom out.
    pub(crate) reveal: Option<Uuid>,
    /// The path bar's ancestors as last drawn, where a package dragged on the canvas may go.
    pub(crate) pills: Vec<(Rect, Uuid)>,
}

/// Notes a zoom the user made by `factor`.
pub(crate) fn intend(nav: &mut Nav, factor: f32) {
    nav.intent = if nav.intent == 0.0 { factor } else { nav.intent * factor };
}

fn is_package(store: &Store, o: Obj) -> bool {
    store.class_of(o).ok() == Some(archforge_uml::PACKAGE)
}

/// The package a class diagram is of.
pub(crate) fn package_of_diagram(store: &Store, d: Obj) -> Option<Obj> {
    let kind = store.get(d, diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string))?;
    if kind != archforge_uml::CLASS_DIAGRAM {
        return None;
    }
    store.get(d, diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|p| store.is_alive(*p) && is_package(store, *p))
}

/// The packages from the model's root down to `package`.
pub(crate) fn chain(store: &Store, package: Obj) -> Vec<Obj> {
    let mut out = vec![package];
    while out.len() < MAX_DEPTH {
        match out.last().and_then(|p| archforge_uml::commands::package_of(store, *p)) {
            Some(parent) => out.push(parent),
            None => break,
        }
    }
    out.reverse();
    out
}

fn nodes(store: &Store, d: Obj) -> Vec<Obj> {
    store.list(d, diagram::NODE).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|n| store.is_alive(*n)).collect()
}

fn element(store: &Store, n: Obj) -> Option<Obj> {
    store.get(n, node::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e))
}

fn screen(view: crate::canvas::View, store: &Store, n: Obj) -> Rect {
    view.rect(&bounds_of(store, n))
}

fn area(r: Rect) -> f32 {
    if r.is_positive() { r.width() * r.height() } else { 0.0 }
}

fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

/// The diagram `package` leads to, made when it has none.
fn diagram_of(shell: &mut Shell, package: Uuid) -> Option<Uuid> {
    match run(shell, "uml.package.diagram", json!({ "package": package.to_string() })) {
        Ok(v) => v.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse),
        Err(e) => {
            shell.canvas.notice = Some(e);
            None
        }
    }
}

/// Opens the diagram of `package`, fitted, keeping the view of the diagram left.
pub(crate) fn enter(shell: &mut Shell, package: Uuid, now: f64) {
    let Some(from) = shell.canvas.active else { return };
    let Some(d) = diagram_of(shell, package) else { return };
    shell.canvas.nav.views.insert(from, (shell.canvas.pan, shell.canvas.zoom));
    if d != from {
        shell.canvas.nav.came_from.insert(d, from);
    }
    crate::canvas::open(shell, d);
    shell.canvas.fit_pending = true;
    shell.canvas.nav.switched = Some(now);
}

/// Opens the diagram of `package`, an ancestor of the open one, as it was left; `reveal`, the
/// package just left, then shows at [`REVEAL`] of the canvas.
pub(crate) fn leave(shell: &mut Shell, package: Uuid, reveal: Option<Uuid>, now: f64) {
    let from = shell.canvas.active;
    // The diagram of `package` the way in came through, while it is still of it; else its first.
    let Some(d) = from.and_then(|f| entered_from(shell, f, package)).or_else(|| diagram_of(shell, package)) else { return };
    if let Some(from) = from {
        shell.canvas.nav.views.insert(from, (shell.canvas.pan, shell.canvas.zoom));
    }
    crate::canvas::open(shell, d);
    match shell.canvas.nav.views.get(&d).copied() {
        Some((pan, zoom)) => {
            shell.canvas.pan = pan;
            shell.canvas.zoom = zoom;
            shell.canvas.fit_pending = false;
        }
        None => shell.canvas.fit_pending = true,
    }
    shell.canvas.nav.reveal = reveal;
    shell.canvas.nav.switched = Some(now);
}

/// The diagram of `package` on the way that led to the diagram `from`, walking back the levels
/// entered.
fn entered_from(shell: &Shell, from: Uuid, package: Uuid) -> Option<Uuid> {
    let store = shell.session.as_ref()?.doc();
    let package = store.by_uuid(package)?;
    let mut at = from;
    for _ in 0..MAX_DEPTH {
        let up = *shell.canvas.nav.came_from.get(&at)?;
        if store.by_uuid(up).and_then(|d| package_of_diagram(store, d)) == Some(package) {
            return Some(up);
        }
        at = up;
    }
    None
}

/// Before the canvas draws: shows the package a zoom out left, takes ⌘↓, ⌘↑ and the zoom keys,
/// and follows the zoom made this frame across a threshold.
pub(crate) fn before_draw(shell: &mut Shell, ui: &egui::Ui, r: Rect, d: Obj) {
    let now = ui.input(|i| i.time);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(here) = package_of_diagram(store, d) else {
        shell.canvas.nav.intent = 0.0;
        return;
    };
    // The package just left, at a third of the canvas, centred: below the threshold, in sight.
    if let Some(left) = shell.canvas.nav.reveal.take()
        && let Some(n) = store.by_uuid(left).and_then(|p| nodes(store, d).into_iter().find(|n| element(store, *n) == Some(p)))
    {
        let b = bounds_of(store, n);
        let (lo, hi) = crate::canvas::ZOOM_RANGE;
        let zoom = (REVEAL * r.width() / b.width.max(1.0) as f32).clamp(lo, hi);
        let centre = vec2((b.x + b.width / 2.0) as f32, (b.y + b.height / 2.0) as f32);
        let pan = r.size() / 2.0 - centre * zoom;
        if pan.is_finite() {
            shell.canvas.zoom = zoom;
            shell.canvas.pan = pan;
            shell.canvas.fit_pending = false;
        }
    }
    keys(shell, ui, r, d, here, now);
    let intent = std::mem::take(&mut shell.canvas.nav.intent);
    let resting = shell.canvas.nav.switched.is_some_and(|t| now - t < REST);
    if intent == 0.0 || (intent - 1.0).abs() < f32::EPSILON || resting || shell.canvas.fit_pending {
        return;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let view = crate::canvas::View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    let canvas_area = area(r).max(1.0);
    if intent > 1.0 {
        // In: the package covering the most of the canvas, once it covers enough.
        let best = nodes(store, d)
            .into_iter()
            .filter_map(|n| element(store, n).filter(|e| is_package(store, *e)).map(|e| (e, area(screen(view, store, n).intersect(r)) / canvas_area)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((package, cover)) = best
            && cover >= ENTER_COVER
            && let Ok(id) = store.uuid(package)
        {
            enter(shell, id, now);
        }
    } else if let Some(parent) = archforge_uml::commands::package_of(store, here) {
        // Out: what the diagram shows has become small, or the zoom is at its least.
        let shown = nodes(store, d).into_iter().map(|n| screen(view, store, n)).reduce(Rect::union);
        let small = shown.is_none_or(|s| area(s.intersect(r)) / canvas_area < LEAVE_COVER);
        let least = shell.canvas.zoom <= crate::canvas::ZOOM_RANGE.0 + f32::EPSILON;
        if (small || least)
            && let (Ok(parent), Ok(here)) = (store.uuid(parent), store.uuid(here))
        {
            leave(shell, parent, Some(here), now);
        }
    }
}

/// ⌘↓ opens the selected package, ⌘↑ the parent; +, = and − zoom about the canvas's centre. Only
/// while the canvas has the keyboard and nothing is typed.
fn keys(shell: &mut Shell, ui: &egui::Ui, r: Rect, d: Obj, here: Obj, now: f64) {
    if !shell.canvas.focused
        || shell.canvas.rename.is_some()
        || shell.canvas.inline.is_some()
        || shell.canvas.note_edit.is_some()
        || ui.memory(|m| m.focused().is_some())
    {
        return;
    }
    let (down, up, zoom_in, zoom_out) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Plus)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::Equals)
                || i.consume_key(egui::Modifiers::SHIFT, egui::Key::Equals),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Minus),
        )
    });
    if zoom_in || zoom_out {
        crate::canvas::zoom_by(&mut shell.canvas, if zoom_in { crate::canvas::ZOOM_STEP } else { 1.0 / crate::canvas::ZOOM_STEP }, r.size() / 2.0);
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    if down && let Some(package) = selected_package(shell, store, d) {
        enter(shell, package, now);
    } else if up && let Some(parent) = archforge_uml::commands::package_of(store, here).and_then(|p| store.uuid(p).ok()) {
        leave(shell, parent, None, now);
    }
}

/// The package selected alone on diagram `d`.
fn selected_package(shell: &Shell, store: &Store, d: Obj) -> Option<Uuid> {
    let selection = &shell.session.as_ref()?.view().selection;
    let [one] = selection.as_slice() else { return None };
    let package = if is_package(store, *one) { *one } else { element(store, *one).filter(|e| is_package(store, *e))? };
    nodes(store, d).into_iter().any(|n| element(store, n) == Some(package)).then(|| store.uuid(package).ok()).flatten()
}

/// After the canvas draws: the path bar at its top left and a selected package's Open, then the
/// fade of a switch.
pub(crate) fn after_draw(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj) -> f32 {
    let now = ui.input(|i| i.time);
    let mut below = 0.0;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return below };
    if let Some(here) = package_of_diagram(store, d) {
        below = path_bar(shell, ui, p, r, here, now);
        open_control(shell, ui, p, r, d, now);
        drop_feedback(shell, ui, p, r, d);
    } else {
        shell.canvas.nav.pills.clear();
    }
    // The switch fades in: the canvas's colour over it, thinning out.
    if let Some(t) = shell.canvas.nav.switched {
        let k = ((now - t) / FADE) as f32;
        if k < 1.0 {
            ui.painter().rect_filled(r, 0.0, p.canvas.gamma_multiply(1.0 - k.max(0.0)));
            ui.ctx().request_repaint();
        }
    }
    below
}

/// The path bar: the packages from the root down to `here`, each ancestor a button; gives how
/// much room it takes at the canvas's top.
fn path_bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, here: Obj, now: f64) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return 0.0 };
    let path: Vec<(Uuid, String)> =
        chain(store, here).into_iter().filter_map(|o| Some((store.uuid(o).ok()?, archforge_core::name_of(store, o)))).collect();
    let paint = ui.painter().clone();
    let room = (r.width() - 32.0).max(0.0);
    let galleys: Vec<_> = path
        .iter()
        .enumerate()
        .map(|(i, (_, name))| {
            let last = i + 1 == path.len();
            let ty = if last { types::UI_STRONG } else { types::UI };
            ux::galley(&paint, name, ty, if last { p.text } else { p.accent }, Some(180.0))
        })
        .collect();
    // Too long for the canvas: the ancestors nearest the root fold into "…", so the package open
    // and its nearest parents stay in sight and clickable.
    const MORE: f32 = 30.0;
    const CHEVRON: f32 = 18.0;
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 20.0).collect();
    let width_from = |start: usize| -> f32 {
        let shown: f32 = widths[start..].iter().sum::<f32>() + CHEVRON * (widths.len() - start).saturating_sub(1) as f32;
        shown + if start > 0 { MORE + CHEVRON } else { 0.0 } + 8.0
    };
    let mut start = 0;
    while start + 1 < widths.len() && width_from(start) > room {
        start += 1;
    }
    let bar = Rect::from_min_size(r.min + vec2(16.0, 14.0), vec2(width_from(start).min(room), 30.0));
    widgets_record(shell, ui, bar);
    paint.rect_filled(bar, 8.0, p.surface);
    paint.rect_stroke(bar, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let clip = paint.with_clip_rect(bar);
    let mut x = bar.left() + 4.0;
    let mut go: Option<Uuid> = None;
    let mut pills = Vec::new();
    let chevron =
        |x: f32, y: f32| ux::icon_in(&clip, Rect::from_center_size(pos2(x + 9.0, y), Vec2::splat(12.0)), &icons::CHEVRON_RIGHT, p.text_muted);
    if start > 0 {
        // "…" stands for the ancestors folded away and leads to the nearest of them.
        let b = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(MORE, 22.0));
        let hidden: Vec<&str> = path[..start].iter().map(|(_, n)| n.as_str()).collect();
        if let Some((id, _)) = path.get(start - 1) {
            pills.push((b, *id));
        }
        let resp = control(shell, ui, "workspace.canvas.path.more", "button", b, true).on_hover_text(hidden.join(" › "));
        clip.rect_filled(b, 5.0, if resp.hovered() { p.accent_tint } else { p.field });
        if resp.has_focus() {
            clip.rect_stroke(b, 5.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
        }
        ux::text_mid(&clip, b.left() + 9.0, b.center().y, "…", types::UI, p.accent, None);
        if resp.clicked() {
            go = path.get(start - 1).map(|(id, _)| *id);
        }
        x = b.right();
        chevron(x, b.center().y);
        x += CHEVRON;
    }
    for (i, ((id, _), g)) in path.iter().zip(galleys).enumerate().skip(start) {
        let last = i + 1 == path.len();
        let b = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(g.size().x + 20.0, 22.0));
        if !last {
            pills.push((b.intersect(bar), *id));
            let resp = control(shell, ui, &format!("workspace.canvas.path.{i}"), "button", b.intersect(bar), true)
                .on_hover_text(tr("Open this package's diagram"));
            clip.rect_filled(b, 5.0, if resp.hovered() { p.accent_tint } else { p.field });
            if resp.has_focus() {
                clip.rect_stroke(b, 5.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
            }
            if resp.clicked() {
                go = Some(*id);
            }
        } else {
            crate::widgets::record_visible(ui, &mut shell.registry, &format!("workspace.canvas.path.{i}"), "text", b.intersect(bar));
        }
        let ink = if last { p.text } else { p.accent };
        clip.galley(pos2(b.left() + 10.0, b.center().y - g.size().y / 2.0), g, ink);
        x = b.right();
        if !last {
            chevron(x, b.center().y);
            x += CHEVRON;
        }
    }
    shell.canvas.nav.pills = pills;
    if let Some(id) = go {
        leave(shell, id, None, now);
    }
    bar.height() + 14.0
}

fn widgets_record(shell: &mut Shell, ui: &egui::Ui, bar: Rect) {
    crate::widgets::record_visible(ui, &mut shell.registry, "workspace.canvas.path", "toolbar", bar);
}

/// A selected package's Open, inside its card's top right while the card is big enough to hold it.
fn open_control(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj, now: f64) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(package) = selected_package(shell, store, d) else { return };
    let Some(n) = store.by_uuid(package).and_then(|o| nodes(store, d).into_iter().find(|n| element(store, *n) == Some(o))) else { return };
    let Ok(node_id) = store.uuid(n) else { return };
    let view = crate::canvas::View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    let card = screen(view, store, n);
    let paint = ui.painter().with_clip_rect(r);
    let label = tr("Open");
    let g = ux::galley(&paint, &label, types::CAPTION_STRONG, p.on_accent, None);
    let size = vec2(g.size().x + 34.0, 24.0);
    if card.width() < size.x + 24.0 || card.height() < size.y + 20.0 {
        return;
    }
    let b = Rect::from_min_size(pos2(card.right() - size.x - 12.0, card.top() + 12.0), size);
    if !r.contains_rect(b) {
        return;
    }
    let resp = control(shell, ui, &format!("workspace.canvas.node.{node_id}.enter"), "button", b, true)
        .on_hover_text(tr("Open this package's diagram (⌘↓)"));
    paint.rect_filled(b, 6.0, p.accent_fill);
    if resp.has_focus() {
        paint.rect_stroke(b.expand(2.0), 8.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
    }
    ux::icon_in(&paint, Rect::from_center_size(pos2(b.left() + 14.0, b.center().y), Vec2::splat(10.0)), &icons::OPEN_IN, p.on_accent);
    paint.galley(pos2(b.left() + 24.0, b.center().y - g.size().y / 2.0), g, p.on_accent);
    if resp.clicked() {
        enter(shell, package, now);
    }
}

/// Where a package node dragged on `d` would go if let go at `at`: another package's card under
/// it, or an ancestor in the path bar (AR-102).
fn drop_target(shell: &Shell, view: crate::canvas::View, canvas: Rect, d: Obj, node: Uuid, at: egui::Pos2) -> Option<(Obj, Obj)> {
    // Let go outside the canvas (over Properties or Models), nothing there takes it, not even a
    // card panned under the panel out of sight.
    if !canvas.contains(at) {
        return None;
    }
    let store = shell.session.as_ref()?.doc();
    let n = store.by_uuid(node)?;
    let package = element(store, n).filter(|e| is_package(store, *e))?;
    if let Some((_, to)) = shell.canvas.nav.pills.iter().find(|(r, _)| r.contains(at)) {
        return store.by_uuid(*to).map(|t| (package, t)).filter(|(p, t)| archforge_uml::commands::package_of(store, *p) != Some(*t));
    }
    // The topmost card under the pointer, the dragged one aside: a package's takes the drop; a
    // class or note over one keeps it a move on the diagram.
    let top = nodes(store, d).into_iter().rev().filter(|m| *m != n).find(|m| screen(view, store, *m).contains(at))?;
    let target = element(store, top).filter(|e| is_package(store, *e))?;
    Some((package, target))
}

/// A package's card held over a place it may go: that place ringed, and what the drop does said.
fn drop_feedback(shell: &Shell, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj) {
    let Some(crate::editing::Gesture::Move { nodes: moved, .. }) = shell.canvas.gesture.as_ref() else { return };
    let [node] = moved.as_slice() else { return };
    let Some(at) = ui.input(|i| i.pointer.hover_pos()) else { return };
    let view = crate::canvas::View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    let Some((package, to)) = drop_target(shell, view, r, d, *node, at) else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let paint = ui.painter().with_clip_rect(r);
    let ring = shell
        .canvas
        .nav
        .pills
        .iter()
        .find(|(_, id)| store.uuid(to).ok() == Some(*id))
        .map(|(pill, _)| *pill)
        .or_else(|| nodes(store, d).into_iter().find(|m| element(store, *m) == Some(to)).map(|m| screen(view, store, m)));
    let Some(ring) = ring else { return };
    paint.rect_stroke(ring.expand(3.0), 8.0, Stroke::new(2.0, p.accent), StrokeKind::Outside);
    let names = [("what", archforge_core::name_of(store, package)), ("where", archforge_core::name_of(store, to))];
    let says = crate::i18n::fmt(&tr("Move {what} into {where}"), &[("what", names[0].1.as_str()), ("where", names[1].1.as_str())]);
    let g = ux::galley(&paint, &says, types::CAPTION_STRONG, p.on_accent, Some(320.0));
    let tag = Rect::from_min_size(at + vec2(14.0, 14.0), g.size() + vec2(16.0, 8.0));
    paint.rect_filled(tag, 6.0, p.accent_fill);
    paint.galley(tag.min + vec2(8.0, 4.0), g, p.on_accent);
}

/// A package node let go on `d` at `at`: when it is over another package or an ancestor in the
/// path bar, it goes there ([`move_and_place`]); true when it went (the drag moved nothing on `d`
/// then).
pub(crate) fn drop_moves(shell: &mut Shell, view: crate::canvas::View, canvas: Rect, d: Obj, node: Uuid, at: egui::Pos2) -> bool {
    let Some((package, to)) = drop_target(shell, view, canvas, d, node, at) else { return false };
    if let Err(e) = move_and_place(shell, package, to) {
        shell.canvas.notice = Some(e);
    }
    true
}

/// Moves `package` into `to` with what it holds and its own diagram, and places it on the diagram
/// of `to` (made when it has none), right of what that shows: one undo step, with a toast offering
/// Undo, as a canvas drop, Properties' Parent package and a drop in Models all do (AR-102); the
/// engine's refusal (into itself or a package it holds) is given back for the caller to say.
pub(crate) fn move_and_place(shell: &mut Shell, package: Obj, to: Obj) -> Result<(), String> {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return Ok(()) };
    let (Ok(package_id), Ok(to_id)) = (store.uuid(package), store.uuid(to)) else { return Ok(()) };
    let (what, into) = (archforge_core::name_of(store, package), archforge_core::name_of(store, to));
    // On the new home's diagram, right of what it shows.
    let shown: Vec<archforge_diagram::Rect> = archforge_uml::commands::diagram_of(store, to)
        .map(|td| nodes(store, td).into_iter().map(|n| bounds_of(store, n)).collect())
        .unwrap_or_default();
    let right = shown.iter().map(|b| b.x + b.width).fold(f64::NEG_INFINITY, f64::max);
    let top = shown.iter().map(|b| b.y).fold(f64::INFINITY, f64::min);
    let (x, y) = if shown.is_empty() { (80.0, 80.0) } else { ((right + 60.0).round(), top.round()) };
    let (w, h) = archforge_uml::PACKAGE_SIZE;
    let label = tr("Move package");
    let Some(session) = shell.session.as_mut() else { return Ok(()) };
    let out = session.batch(&label, |b| {
        b.execute("uml.package.move", &json!({ "package": package_id.to_string(), "to": to_id.to_string() }))?;
        let td = b.execute("uml.package.diagram", &json!({ "package": to_id.to_string() }))?;
        let td = td.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute("diagram.place", &json!({ "diagram": td, "element": package_id.to_string(), "x": x, "y": y, "width": w, "height": h }))?;
        Ok(())
    });
    match out {
        Ok(_) => {
            let text = crate::i18n::fmt(&tr("{what} moved into {where}"), &[("what", what.as_str()), ("where", into.as_str())]);
            shell.canvas.toast = Some(crate::connect::Toast { text, until: shell.frames.saturating_add(300), undo_label: Some(label) });
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// A package moved by Properties or Models into `to`: shown on a diagram, it goes onto its new
/// home's diagram too ([`move_and_place`]); shown nowhere, it only moves. The engine's refusal is
/// given back.
pub(crate) fn move_package(shell: &mut Shell, package: Obj, to: Obj) -> Result<(), String> {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return Ok(()) };
    if archforge_uml::commands::package_of(store, package) == Some(to) {
        return Ok(());
    }
    if !archforge_diagram::views_of(store, package).2.is_empty() {
        return move_and_place(shell, package, to);
    }
    let (Ok(package_id), Ok(to_id)) = (store.uuid(package), store.uuid(to)) else { return Ok(()) };
    let Some(session) = shell.session.as_mut() else { return Ok(()) };
    session.execute("uml.package.move", &json!({ "package": package_id.to_string(), "to": to_id.to_string() })).map(|_| ()).map_err(|e| e.to_string())
}
