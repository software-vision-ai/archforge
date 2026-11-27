//! Package navigation (AR-95, AR-110, from the design's "Canvas · A package on a class diagram,
//! selected, with Open and the path bar", "Canvas · Inside a package: the path bar leads back" and
//! "Package navigation · thresholds, keys and transitions" frames). Each package on a class
//! diagram leads to its default class diagram, the one its binding names (Decision 10); entering
//! never makes one, and a package with none, or an unsound binding, opens nothing and says why
//! (AR-582).
//!
//! - **In**: a selected package's Open, ⌘↓, or zooming in until one package covers
//!   [`ENTER_COVER`] of the canvas opens the package's diagram at 100 %, its content centred.
//! - **Out by zoom**: a package's diagram stays open until the zoom reaches its least, 10 %
//!   ([`archforge_shell_api::canvas::ZOOM_RANGE`]); there the diagram it was entered from opens
//!   again, even when that diagram is in another branch (Decision 4): fitted, down to 10 %, when it
//!   no longer has a card of the package; else as it was left when that view is still sound
//!   ([`restorable`]); else at 100 %, the package left centred (AR-668). The return takes its step
//!   off the tab's route and makes none. With no way in left, the parent's default opens instead,
//!   landing as a return with no view kept does; at the root, 10 % stays put and says so.
//! - **Out by the path bar or ⌘↑**: the owning ancestor's default diagram as it was last left, by
//!   ownership and never the way in; its tab starts a new route.
//! - One switch per gesture: after a switch the zoom must rest [`QUIET`] seconds, and [`REST`]
//!   seconds must pass, before the next switch. A switch fades in over the shell's
//!   `canvas::SWITCH_FADE` seconds.
//!
//! It is UML's own: UML's diagram kind runs it before and after the canvas draws a diagram
//! ([`archforge_shell_api::kinds::DiagramKind::before_draw`],
//! [`archforge_shell_api::kinds::DiagramKind::after_draw`]), and what it keeps between frames is
//! UML's ([`crate::uml_state::UmlState::nav`]). The canvas keeps what any diagram's view needs: the
//! view each diagram was left with, the zoom made this frame, and when a switch was last marked,
//! from which the canvas fades in.

use std::collections::HashMap;

use archforge_diagram::{diagram, grid::bounds_of, node};
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::uml_nav_route::{Route, Step};
use crate::uml_state::{uml, uml_mut};
use crate::ux::{self, Palette, icons, types};
use archforge_shell_api::cx::Ctx;
use archforge_shell_api::i18n::tr;
use archforge_shell_api::panels::control_cx;

/// The share of the canvas one package covers once zooming in opens it.
pub const ENTER_COVER: f32 = 0.6;
/// How long the thresholds rest after a switch, in seconds.
pub const REST: f64 = 0.6;
/// How long the zoom must rest after a switch before the next one: the end of a gesture.
pub const QUIET: f64 = 0.3;
/// The zoom a diagram opens at when a switch by zoom or Open lands on it (AR-110).
pub const LANDING: f32 = 1.0;
/// The deepest package chain the path bar walks.
const MAX_DEPTH: usize = 256;

/// Where a switch on the way out lands, on the next frame of the diagram it opened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    /// The package just left, whose card the landing measures or centres.
    pub(crate) package: Uuid,
    /// The view the diagram was left with when the package was entered from it, which a return
    /// restores while [`restorable`]; none for a way out that is no return.
    pub(crate) view: Option<(Vec2, f32)>,
}

/// What package navigation keeps between frames, as UML's own ([`crate::uml_state::UmlState`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nav {
    /// The return route of each tab, by the tab's diagram: the package entries that led to it, so
    /// going back returns the way the user came in, though the parent package has other class
    /// diagrams too. Browsing state only: never written into the model, and no undo step.
    pub routes: HashMap<Uuid, Route>,
    /// Where the next frame lands after a zoom out: the package left and the view to restore.
    pub reveal: Option<Landing>,
    /// True while the diagram just entered is still to be centred at [`LANDING`].
    pub centre: bool,
    /// The canvas's count of tabs opened ([`archforge_shell_api::canvas::State::settled`]) when
    /// `reveal` or `centre` was set: they are for the tab the switch opened, and any tab opened
    /// since drops them.
    pub(crate) settled: u64,
    /// When the user last zoomed, and whether the gesture that made the last switch is still on:
    /// no switch happens until it has ended.
    pub last_zoom: Option<f64>,
    pub disarmed: bool,
}

/// Package navigation's state, UML's own; none while UML is not one of the shell's notations.
fn nav<'a>(cx: &'a mut Ctx<'_>) -> Option<&'a mut Nav> {
    uml_mut(cx.notation_states).map(|uml| &mut uml.nav)
}

/// Marks a switch made `now`, or a zoom out at the top package that went no further: the canvas
/// fades in, and the thresholds rest until the gesture that made it has ended.
fn switched(cx: &mut Ctx, now: f64) {
    cx.canvas.switched = Some(now);
    if let Some(nav) = nav(cx) {
        nav.disarmed = true;
    }
}

fn is_package(store: &Store, o: Obj) -> bool {
    store.class_of(o).ok() == Some(archforge_uml::PACKAGE)
}

/// The package a class diagram is of.
pub fn package_of_diagram(store: &Store, d: Obj) -> Option<Obj> {
    let kind = store.get(d, diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string))?;
    if kind != archforge_uml::STRUCTURE_DIAGRAM {
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

fn screen(view: archforge_shell_api::canvas::View, store: &Store, n: Obj) -> Rect {
    view.rect(&bounds_of(store, n))
}

fn area(r: Rect) -> f32 {
    if r.is_positive() { r.width() * r.height() } else { 0.0 }
}

/// Runs a command on the open project's session; a refusal comes back in Properties' plain words
/// ([`archforge_shell_api::panels::refusal`]).
fn run(cx: &mut Ctx, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    archforge_shell_api::panels::run_cx(cx, id, params)
}

/// The default diagram of `package`, which entry opens, as the query `uml.package.diagram` gives it
/// to agents too; none for a package with no class diagram yet or an unsound binding, whose
/// refusal the notice under the path bar says: what is wrong, and that a diagram has to be made
/// or chosen first. Nothing is made (AR-582).
fn default_of(cx: &mut Ctx, package: Uuid) -> Option<Uuid> {
    match run(cx, "uml.package.diagram", json!({ "package": package.to_string() })) {
        Ok(v) => v.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse),
        Err(e) => {
            cx.canvas.notice = Some(e);
            None
        }
    }
}

/// Opens the diagram of `package`, fitted, keeping the view of the diagram left. The diagram's tab
/// takes the route of the tab left plus this entry; entering the diagram already open records
/// nothing.
pub fn enter(cx: &mut Ctx, package: Uuid, now: f64) {
    enter_by(cx, package, now, false);
}

/// [`enter`], `by_zoom` when zooming in made the entry. Such an entry keeps no view for the return:
/// the package covered [`ENTER_COVER`] there, so coming back to it would let the next zoom-in tick
/// enter again, and the canvas may have grown since, so the return can't tell by measuring alone
/// ([`restorable`]); it centres the package at [`LANDING`] instead (AR-668).
fn enter_by(cx: &mut Ctx, package: Uuid, now: f64, by_zoom: bool) {
    let Some(from) = cx.canvas.active else { return };
    let Some(d) = default_of(cx, package) else { return };
    let view = (!by_zoom).then_some((cx.canvas.pan, cx.canvas.zoom));
    if d != from
        && let Some(session) = cx.session.as_ref()
        && let Some(nav) = uml_mut(cx.notation_states).map(|uml| &mut uml.nav)
    {
        let store = session.doc();
        let source_name = store.by_uuid(from).map(|o| archforge_core::name_of(store, o)).unwrap_or_default();
        let mut route = nav.routes.get(&from).cloned().unwrap_or_default();
        route.push(Step { source: from, destination: d, source_name, view, history: session.history() });
        nav.routes.insert(d, route);
    }
    archforge_shell_api::canvas::open_cx(cx, d);
    // At 100 %, its content centred on the next frame (AR-110).
    cx.canvas.fit_pending = false;
    cx.canvas.zoom = LANDING;
    let settled = cx.canvas.settled;
    if let Some(nav) = nav(cx) {
        (nav.centre, nav.reveal, nav.settled) = (true, None, settled);
    }
    switched(cx, now);
}

/// Opens the default diagram of `package`, an owning ancestor of the open one, by the path bar or
/// ⌘↑: as it was last left, or fitted when it never was. It follows ownership, never the way in,
/// and starts a new route in the tab it opens, with no step made up from the diagram left
/// (Decision 4). Browsing state only: no model change and no undo step.
pub fn leave(cx: &mut Ctx, package: Uuid, now: f64) {
    let Some(d) = default_of(cx, package) else { return };
    if let Some(nav) = nav(cx) {
        nav.routes.remove(&d);
    }
    switch_out(cx, d, None, now);
}

/// Opens the diagram `d` on the way out of the open one, keeping the view of the diagram left: after a zoom
/// out (`reveal`) as its next frame lands it ([`land`]); else as `d` was left, or fitted when it
/// never was.
fn switch_out(cx: &mut Ctx, d: Uuid, reveal: Option<Landing>, now: f64) {
    archforge_shell_api::canvas::open_cx(cx, d);
    match (reveal, cx.canvas.views.get(&d).copied()) {
        (Some(_), _) => {
            cx.canvas.zoom = LANDING;
            cx.canvas.fit_pending = false;
        }
        (None, Some((pan, zoom))) => {
            cx.canvas.pan = pan;
            cx.canvas.zoom = zoom;
            cx.canvas.fit_pending = false;
        }
        (None, None) => cx.canvas.fit_pending = true,
    }
    let settled = cx.canvas.settled;
    if let Some(nav) = nav(cx) {
        (nav.reveal, nav.centre, nav.settled) = (reveal, false, settled);
    }
    switched(cx, now);
}

/// The way back from the open diagram by zoom: the newest step on its tab's route made in this
/// session's history whose source is still a diagram, taken off the route with every newer one
/// passed over. Gives that source, which may be in another branch than the open diagram, and the
/// view it was left with; the tab of the source keeps what is left of the route, the one left
/// keeps none, and nothing is added, since a return is no entry. Gives none, the route spent,
/// when no step leads anywhere.
fn take_return(cx: &mut Ctx) -> Option<(Uuid, Option<(Vec2, f32)>)> {
    let from = cx.canvas.active?;
    let session = cx.session.as_ref()?;
    let store = session.doc();
    let nav = uml_mut(cx.notation_states).map(|uml| &mut uml.nav)?;
    let mut route = nav.routes.remove(&from)?;
    let shown = |s: Uuid| store.by_uuid(s).is_some_and(|d| store.is_alive(d) && store.class_of(d).ok() == Some(archforge_diagram::DIAGRAM));
    let step = route.take_return(session.history(), shown).reached?;
    nav.routes.insert(step.source, route);
    Some((step.source, step.view))
}

/// The package whose card on diagram `d` covers the most of canvas `r` at `view`, with the share of
/// the canvas it covers; none when `d` shows no package. Zooming in enters it once that share
/// reaches [`ENTER_COVER`].
fn most_covered(store: &Store, d: Obj, view: archforge_shell_api::canvas::View, r: Rect) -> Option<(Obj, f32)> {
    let canvas_area = area(r).max(1.0);
    nodes(store, d)
        .into_iter()
        .filter_map(|n| element(store, n).filter(|e| is_package(store, *e)).map(|e| (e, area(screen(view, store, n).intersect(r)) / canvas_area)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Whether `view`, the pan and zoom diagram `d` was left with, can be shown again on canvas `r`
/// holding `card`, `d`'s card of the package left: both finite, the zoom within
/// [`archforge_shell_api::canvas::ZOOM_RANGE`], the card at least partly on the canvas there, and
/// no package card of `d` covering [`ENTER_COVER`] of it, the measure zooming in enters by
/// ([`most_covered`]). A view that hides the package left, as a window narrowed since can, is no
/// place to come back to, nor one some package covers more of, since the next zoom-in tick would
/// take that package (delegated UX decision, AR-668); a zoom entry keeps no view at all
/// ([`enter_by`]).
fn restorable(store: &Store, r: Rect, d: Obj, card: Obj, (pan, zoom): (Vec2, f32)) -> bool {
    let (least, most) = archforge_shell_api::canvas::ZOOM_RANGE;
    if !pan.is_finite() || !zoom.is_finite() || !(least..=most).contains(&zoom) {
        return false;
    }
    let view = archforge_shell_api::canvas::View { origin: r.min, pan, zoom };
    area(screen(view, store, card).intersect(r)) > 0.0 && most_covered(store, d, view, r).is_none_or(|(_, cover)| cover < ENTER_COVER)
}

/// Lands diagram `d` on canvas `r` after a zoom out. When `d` has no card of the package left, its
/// content fitted, down to the least zoom, its middle shown when it is too big for that
/// ([`archforge_shell_api::canvas::fit`]), as an emptied diagram is; else at the view it was left
/// with when that is [`restorable`]; else at [`LANDING`] with that card centred.
fn land(cx: &mut Ctx, r: Rect, d: Obj, landing: Landing) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let card = store.by_uuid(landing.package).and_then(|p| nodes(store, d).into_iter().find(|n| element(store, *n) == Some(p)));
    cx.canvas.fit_pending = false;
    match (card, landing.view) {
        (Some(card), Some(view)) if restorable(store, r, d, card, view) => (cx.canvas.pan, cx.canvas.zoom) = view,
        (Some(card), _) => {
            let b = bounds_of(store, card);
            let centre = vec2((b.x + b.width / 2.0) as f32, (b.y + b.height / 2.0) as f32);
            centre_at_landing(cx.canvas, r, Some(centre));
        }
        (None, _) => {
            let bounds = archforge_shell_api::canvas::diagram_bounds(store, d);
            archforge_shell_api::canvas::fit(cx.canvas, r.size(), bounds, archforge_shell_api::canvas::ZOOM_RANGE.0);
        }
    }
}

/// Shows canvas `r` at [`LANDING`] with `centre`, a point of the diagram, in its middle; with
/// none, from the diagram's top left, as a new diagram opens.
fn centre_at_landing(state: &mut archforge_shell_api::canvas::State, r: Rect, centre: Option<Vec2>) {
    state.zoom = LANDING;
    state.fit_pending = false;
    let pan = centre.map_or(Vec2::ZERO, |c| r.size() / 2.0 - c * LANDING);
    state.pan = if pan.is_finite() { pan } else { Vec2::ZERO };
}

/// Before the canvas draws diagram `d`, a package's: shows the package a zoom out left, takes ⌘↓,
/// ⌘↑ and the zoom keys, and follows the zoom made this frame across a threshold. Another
/// diagram's frame leaves it all be.
pub(crate) fn before_draw(cx: &mut Ctx, ui: &egui::Ui, r: Rect, d: Obj) {
    let now = ui.input(|i| i.time);
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let Some(here) = package_of_diagram(store, d) else { return };
    // A switch landed: a zoom out as `land` places it, an entry at 100 % with its content centred.
    // Only the tab the switch opened lands: opening another first drops it (`canvas::open`).
    let settled = cx.canvas.settled;
    let (reveal, centre) = match uml_mut(cx.notation_states).map(|uml| &mut uml.nav) {
        Some(nav) if nav.settled == settled => (nav.reveal.take(), std::mem::take(&mut nav.centre)),
        Some(nav) => {
            (nav.reveal, nav.centre) = (None, false);
            (None, false)
        }
        None => (None, false),
    };
    if let Some(landing) = reveal {
        land(cx, r, d, landing);
    } else if centre {
        let boxes: Vec<archforge_diagram::Rect> = nodes(store, d).into_iter().map(|n| bounds_of(store, n)).collect();
        let centre = if boxes.is_empty() {
            None
        } else {
            let (x0, y0) = boxes.iter().fold((f64::INFINITY, f64::INFINITY), |(x, y), b| (x.min(b.x), y.min(b.y)));
            let (x1, y1) = boxes.iter().fold((f64::NEG_INFINITY, f64::NEG_INFINITY), |(x, y), b| (x.max(b.x + b.width), y.max(b.y + b.height)));
            Some(vec2(((x0 + x1) / 2.0) as f32, ((y0 + y1) / 2.0) as f32))
        };
        centre_at_landing(cx.canvas, r, centre);
    }
    keys(cx, ui, r, d, here, now);
    let intent = std::mem::take(&mut cx.canvas.zoom_intent);
    let (last_switch, fit_pending) = (cx.canvas.switched, cx.canvas.fit_pending);
    let Some(nav) = nav(cx) else { return };
    // The gesture that made the last switch ends once the zoom has rested; only then may another.
    // The rest is measured before this frame's zoom is noted, so the first frame of a new gesture
    // after a pause re-arms, though no idle frame was drawn in between.
    let rested = nav.last_zoom.is_none_or(|t| now - t >= QUIET) && last_switch.is_none_or(|t| now - t >= REST);
    if nav.disarmed && rested {
        nav.disarmed = false;
    }
    if intent != 0.0 {
        nav.last_zoom = Some(now);
    }
    if intent == 0.0 || (intent - 1.0).abs() < f32::EPSILON || nav.disarmed || fit_pending {
        return;
    }
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let view = archforge_shell_api::canvas::View { origin: r.min, pan: cx.canvas.pan, zoom: cx.canvas.zoom };
    if intent > 1.0 {
        // In: the package covering the most of the canvas, once it covers enough.
        if let Some((package, cover)) = most_covered(store, d, view, r)
            && cover >= ENTER_COVER
            && let Ok(id) = store.uuid(package)
        {
            enter_by(cx, id, now, true);
        }
    } else if cx.canvas.zoom <= archforge_shell_api::canvas::ZOOM_RANGE.0 + f32::EPSILON {
        // Out: the zoom has reached its least, 10 %: back the way in came, even into another
        // branch; with no way in, one level up; at the root, no further.
        let Ok(left) = store.uuid(here) else { return };
        let parent = archforge_uml::commands::package_of(store, here).map(|p| store.uuid(p));
        match (take_return(cx), parent) {
            (Some((back, view)), _) => switch_out(cx, back, Some(Landing { package: left, view }), now),
            (None, Some(Ok(parent))) => {
                // The owning parent's default; its tab keeps its own route.
                if let Some(d) = default_of(cx, parent) {
                    switch_out(cx, d, Some(Landing { package: left, view: None }), now);
                }
            }
            (None, Some(Err(_))) => {}
            (None, None) => {
                // Said under the path bar until the next action.
                cx.canvas.notice = Some(tr("This is the top package: zooming out goes no further"));
                switched(cx, now);
            }
        }
    }
}

/// ⌘↓ opens the selected package, ⌘↑ the parent; +, = and − zoom about the canvas's centre. Only
/// while the canvas has the keyboard and nothing is typed. The keys are read by `pressed`.
fn keys(cx: &mut Ctx, ui: &egui::Ui, r: Rect, d: Obj, here: Obj, now: f64) {
    if !cx.canvas.focused
        || cx.canvas.rename.is_some()
        || uml(cx.notation_states).is_some_and(|uml| uml.inline.is_some())
        || cx.canvas.note_edit.is_some()
        || ui.memory(|m| m.focused().is_some())
    {
        return;
    }
    let (down, up, zoom_in, zoom_out) = ui.input_mut(pressed);
    if zoom_in || zoom_out {
        archforge_shell_api::canvas::zoom_by(
            cx.canvas,
            if zoom_in { archforge_shell_api::canvas::ZOOM_STEP } else { 1.0 / archforge_shell_api::canvas::ZOOM_STEP },
            r.size() / 2.0,
        );
    }
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    if down && let Some(package) = selected_package(cx, store, d) {
        enter(cx, package, now);
    } else if up && let Some(parent) = archforge_uml::commands::package_of(store, here).and_then(|p| store.uuid(p).ok()) {
        leave(cx, parent, now);
    }
}

/// ⌘↓, ⌘↑, zoom in and zoom out pressed in `i`, each press taken. ⌘↓ and ⌘↑ are exact, as the
/// actions' chords are (`actions::take`): with Shift or Alt more they stay in the input, and with a
/// Mac's Control they are taken and do nothing. The zoom keys are plain (`actions::take_plain`):
/// with a command key, Control or Alt they stay, but Shift is free, since + is Shift and = on many
/// layouts and some systems report that press as = with Shift.
fn pressed(i: &mut egui::InputState) -> (bool, bool, bool, bool) {
    use archforge_shell_api::actions::{Keys, take, take_plain};
    let chord = |key| Keys { modifiers: egui::Modifiers::COMMAND, key };
    (
        take(i, &chord(egui::Key::ArrowDown)),
        take(i, &chord(egui::Key::ArrowUp)),
        take_plain(i, egui::Key::Plus) | take_plain(i, egui::Key::Equals),
        take_plain(i, egui::Key::Minus),
    )
}

/// The package selected alone on diagram `d`.
fn selected_package(cx: &Ctx, store: &Store, d: Obj) -> Option<Uuid> {
    let selection = &cx.session.as_ref()?.view().selection;
    let [one] = selection.as_slice() else { return None };
    let package = if is_package(store, *one) { *one } else { element(store, *one).filter(|e| is_package(store, *e))? };
    nodes(store, d).into_iter().any(|n| element(store, n) == Some(package)).then(|| store.uuid(package).ok()).flatten()
}

/// After the canvas draws diagram `d`, a package's: the path bar at its top left and a selected
/// package's Open. Gives how much room the path bar takes at the canvas's top; none on another
/// diagram.
pub(crate) fn after_draw(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj) -> f32 {
    let now = ui.input(|i| i.time);
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return 0.0 };
    let Some(here) = package_of_diagram(store, d) else { return 0.0 };
    let below = path_bar(cx, ui, p, r, here, now);
    open_control(cx, ui, p, r, d, now);
    below
}

/// The path bar: the packages from the root down to `here`, each ancestor a button; gives how
/// much room it takes at the canvas's top.
fn path_bar(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, here: Obj, now: f64) -> f32 {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return 0.0 };
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
    widgets_record(cx, ui, bar);
    paint.rect_filled(bar, 8.0, p.surface);
    paint.rect_stroke(bar, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let clip = paint.with_clip_rect(bar);
    let mut x = bar.left() + 4.0;
    let mut go: Option<Uuid> = None;
    let chevron =
        |x: f32, y: f32| ux::icon_in(&clip, Rect::from_center_size(pos2(x + 9.0, y), Vec2::splat(12.0)), &icons::CHEVRON_RIGHT, p.text_muted);
    if start > 0 {
        // "…" stands for the ancestors folded away and leads to the nearest of them.
        let b = Rect::from_min_size(pos2(x, bar.top() + 4.0), vec2(MORE, 22.0));
        let hidden: Vec<&str> = path[..start].iter().map(|(_, n)| n.as_str()).collect();
        let resp = control_cx(cx, ui, "workspace.canvas.path.more", "button", b, true).on_hover_text(hidden.join(" › "));
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
            let resp = control_cx(cx, ui, &format!("workspace.canvas.path.{i}"), "button", b.intersect(bar), true)
                .on_hover_text(tr("Open this package's diagram"));
            clip.rect_filled(b, 5.0, if resp.hovered() { p.accent_tint } else { p.field });
            if resp.has_focus() {
                clip.rect_stroke(b, 5.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
            }
            if resp.clicked() {
                go = Some(*id);
            }
        } else {
            archforge_shell_api::widgets::record_visible(ui, cx.registry, &format!("workspace.canvas.path.{i}"), "text", b.intersect(bar));
        }
        let ink = if last { p.text } else { p.accent };
        clip.galley(pos2(b.left() + 10.0, b.center().y - g.size().y / 2.0), g, ink.into());
        x = b.right();
        if !last {
            chevron(x, b.center().y);
            x += CHEVRON;
        }
    }
    if let Some(id) = go {
        leave(cx, id, now);
    }
    bar.height() + 14.0
}

/// Records the path bar `bar` for automation, as a toolbar.
fn widgets_record(cx: &mut Ctx, ui: &egui::Ui, bar: Rect) {
    archforge_shell_api::widgets::record_visible(ui, cx.registry, "workspace.canvas.path", "toolbar", bar);
}

/// A selected package's Open, inside its card's top right while the card is big enough to hold it.
fn open_control(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, d: Obj, now: f64) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let Some(package) = selected_package(cx, store, d) else { return };
    let Some(n) = store.by_uuid(package).and_then(|o| nodes(store, d).into_iter().find(|n| element(store, *n) == Some(o))) else { return };
    let Ok(node_id) = store.uuid(n) else { return };
    let view = archforge_shell_api::canvas::View { origin: r.min, pan: cx.canvas.pan, zoom: cx.canvas.zoom };
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
    let resp = control_cx(cx, ui, &format!("workspace.canvas.node.{node_id}.enter"), "button", b, true)
        .on_hover_text(tr("Open this package's diagram (⌘↓)"));
    paint.rect_filled(b, 6.0, p.accent_fill);
    if resp.has_focus() {
        paint.rect_stroke(b.expand(2.0), 8.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
    }
    ux::icon_in(&paint, Rect::from_center_size(pos2(b.left() + 14.0, b.center().y), Vec2::splat(10.0)), &icons::OPEN_IN, p.on_accent);
    paint.galley(pos2(b.left() + 24.0, b.center().y - g.size().y / 2.0), g, p.on_accent.into());
    if resp.clicked() {
        enter(cx, package, now);
    }
}

/// Moves `package` into `to` with what it holds and its own diagram, as Properties' Parent package
/// and a drop in Models both do: its cards stay where they are, and none is added to the diagram of
/// `to` (Decision 26, AR-568). One undo step, "Move package", with a toast offering Undo; already in
/// `to`, nothing happens. A refusal (into itself or a package it holds, beside the package Types)
/// comes back in plain words ([`archforge_shell_api::panels::refusal`]) for the caller to say.
pub(crate) fn move_package(cx: &mut Ctx, package: Obj, to: Obj) -> Result<(), String> {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return Ok(()) };
    if archforge_uml::commands::package_of(store, package) == Some(to) {
        return Ok(());
    }
    let (Ok(package_id), Ok(to_id)) = (store.uuid(package), store.uuid(to)) else { return Ok(()) };
    let (what, into) = (archforge_core::name_of(store, package), archforge_core::name_of(store, to));
    let label = tr("Move package");
    let Some(session) = cx.session.as_mut() else { return Ok(()) };
    let moved =
        session.batch(&label, |b| b.execute("uml.package.move", &json!({ "package": package_id.to_string(), "to": to_id.to_string() })).map(|_| ()));
    moved.map_err(|e| archforge_shell_api::panels::refusal(cx.notation_uis, &e))?;
    let text = archforge_shell_api::i18n::fmt(&tr("{what} moved into {where}"), &[("what", what.as_str()), ("where", into.as_str())]);
    cx.canvas.toast = Some(archforge_shell_api::canvas::Toast { text, until: cx.frames.saturating_add(300), undo_label: Some(label) });
    Ok(())
}

#[cfg(test)]
mod tests {
    use egui::{Event, Key, Modifiers};

    /// What `pressed` reads from an input holding only `event`, and whether it took the press.
    fn read(event: &Event) -> ((bool, bool, bool, bool), bool) {
        let mut input = egui::InputState::default();
        input.events = vec![event.clone()];
        (super::pressed(&mut input), input.events.is_empty())
    }

    /// A press of `key` with `modifiers`.
    fn down(modifiers: Modifiers, key: Key) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    /// ⌘↓ and ⌘↑ count in both shapes and are taken; with Shift or Alt more they count for nothing
    /// and stay, with a Mac's Control they count for nothing and are taken. The zoom keys count
    /// plain or with Shift and are taken; with a command key, Control or Alt they stay.
    #[test]
    fn the_keys_are_exact_and_take_only_their_own_press() {
        let none = (false, false, false, false);
        let mac = Modifiers::MAC_CMD.plus(Modifiers::COMMAND);
        for shape in [mac, Modifiers::CTRL.plus(Modifiers::COMMAND)] {
            assert_eq!(read(&down(shape, Key::ArrowDown)), ((true, false, false, false), true), "{shape:?}");
            assert_eq!(read(&down(shape, Key::ArrowUp)), ((false, true, false, false), true), "{shape:?}");
            assert_eq!(read(&down(shape, Key::Minus)), (none, false), "{shape:?}: a chord of −");
            for key in [Key::ArrowDown, Key::ArrowUp, Key::Equals, Key::Minus] {
                for more in [Modifiers::SHIFT, Modifiers::ALT] {
                    assert_eq!(read(&down(shape | more, key)), (none, false), "{shape:?} {more:?} {key:?}");
                }
            }
        }
        for key in [Key::ArrowDown, Key::ArrowUp] {
            assert_eq!(read(&down(mac | Modifiers::CTRL, key)), (none, true), "a Mac's Control with {key:?}");
        }
        for held in [Modifiers::NONE, Modifiers::SHIFT] {
            for (key, read_as) in
                [(Key::Plus, (false, false, true, false)), (Key::Equals, (false, false, true, false)), (Key::Minus, (false, false, false, true))]
            {
                assert_eq!(read(&down(held, key)), (read_as, true), "{held:?} {key:?}");
                assert_eq!(read(&down(held | Modifiers::ALT, key)), (none, false), "{held:?} with Alt {key:?}");
            }
        }
        assert_eq!(read(&down(Modifiers::CTRL, Key::Minus)), (none, false), "Control alone");
    }
}
