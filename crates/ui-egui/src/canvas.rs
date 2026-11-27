//! The centre of the workspace (slice W4 of the workspace chrome, from the design's frames
//! "Workspace · First diagram open" and "ER · Properties · Entity selected"): the diagram tab
//! strip with the new-diagram `+`, the canvas that draws the open diagram's nodes and edges from
//! the diagram metamodel in the design's style, the minimap with the zoom control at the canvas's
//! bottom right, and the floating canvas toolbar at its bottom centre. Select and Pan work; the
//! drawing tools wait for their slice and say so on hover. A click selects what it hits through
//! `view.select`, so Properties follows the canvas; nothing on the canvas is moved or made yet.
//!
//! The geometry is the design's: the tab strip 36 tall with tabs 32 tall from x 44, an entity
//! node's header 66 units with the name at 20 pt, its rows 20 units apart from 77, the minimap
//! 200 by 156 at 16 px from the right and 14 px above the status bar, the toolbar 342 by 50 at
//! the bottom centre. The grid dots sit 32 units apart and thin out as the zoom falls.

use archforge_diagram::figure::{EndMark, Tone};
use archforge_diagram::grid::bounds_of;
use archforge_diagram::{DIAGRAM, Grid, LabelPlace, diagram, edge, label, node, point, style};
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

pub(crate) use archforge_shell_api::canvas::{Drawing, END_HANDLE, View, along, end_handles_at, exit_point, nesting_route};
pub use archforge_shell_api::canvas::{EndText, MinimapDrag, PackageTab, PathBadge, State, Toast, Tool, self_loop};
use archforge_shell_api::canvas::{beside, beside_pair, mark_outline, mark_shape, qualifier_place, signature_place};
pub use archforge_shell_api::kinds::ExtraLabel;
pub(crate) use archforge_shell_api::kinds::LineDrawn;

use crate::cx::{Ctx, Request};
use crate::home::control;
use crate::i18n::{fmt, tr, trn};
use crate::notation_ui::{Layer, Overlaid};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The tab strip's height, a tab's height and the first tab's x.
pub(crate) const TABS: f32 = 36.0;
const TAB_HEIGHT: f32 = 32.0;
const FIRST_TAB: f32 = 44.0;
const TAB_TEXT_MAX: f32 = 160.0;
/// The grid's pitch in diagram units, the least pitch on screen and the most dots drawn.
const GRID_STEP: f32 = 32.0;
const GRID_MIN: f32 = 12.0;
const GRID_MAX_DOTS: f32 = 6000.0;
/// The zoom's range and a step of the zoom buttons.
pub(crate) const ZOOM_RANGE: (f32, f32) = (0.1, 4.0);
pub(crate) const ZOOM_STEP: f32 = 1.25;
/// The margin "Whole diagram" leaves around the nodes.
const FIT_MARGIN: f32 = 48.0;
/// The minimap's box and the toolbar's pill.
const MINIMAP: Vec2 = vec2(200.0, 156.0);
const MINIMAP_GAP: Vec2 = vec2(16.0, 14.0);
/// The toolbar's height (its width comes from the open diagram kind's tools).
const TOOLBAR_HEIGHT: f32 = 50.0;
const TOOLBAR_GAP: f32 = 14.0;
/// The refused step's toast in frame V2.4: its width and least height, how far its foot is above
/// the canvas's foot, and where its words start after the dot.
const REFUSED_TOAST: Vec2 = vec2(440.0, 40.0);
const REFUSED_TOAST_FOOT: f32 = 80.0;
const REFUSED_TOAST_TEXT: f32 = 30.0;
/// How much higher it stands over a toast with Undo at that place, three rows at the most.
const OVER_UNDO_TOAST: f32 = 3.0 * types::SMALL.line + 22.0 + 8.0;
/// A toast's size and how many frames it stays.
const TOAST: Vec2 = vec2(440.0, 40.0);
/// The narrowest toast beside the minimap; with less room it moves above the minimap.
const TOAST_MIN: f32 = 300.0;
/// The most lines a toast's text wraps to.
const TOAST_ROWS: usize = 3;
pub(crate) const TOAST_FRAMES: u64 = 300;
/// An entity node: its header's height, its first row's baseline, the pitch between rows, the
/// text inset and the badge column.
pub(crate) const ENTITY_HEADER: f32 = archforge_diagram::figure::CARD_HEADER;
pub(crate) const ENTITY_FIRST_ROW: f32 = archforge_diagram::figure::CARD_FIRST_ROW;
pub(crate) const ENTITY_ROW: f32 = archforge_diagram::figure::CARD_ROW;
/// The pitches between the last row of a card's compartment and the first of the next: the
/// compartment's foot, as far as a card's last row is from its foot, and the next one's head.
pub(crate) const COMPARTMENT_GAP: f32 = archforge_diagram::figure::COMPARTMENT_GAP;
/// How far a compartment's first row is under the line over it: as far as a card's first row is
/// under its header's line.
const COMPARTMENT_TOP: f32 = ENTITY_FIRST_ROW - ENTITY_HEADER;
const ENTITY_INSET: f32 = 18.0;
pub(crate) const ENTITY_TEXT: f32 = 42.0;
/// How strongly a table of another schema shows: the design's 60 %.
const FOREIGN_ALPHA: f32 = 0.6;
/// Below this zoom a node shows its name only.
pub(crate) const DETAIL_ZOOM: f32 = 0.5;

/// Runs a command for the canvas; a refusal comes back in plain words ([`crate::panels::refusal`]).
fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    crate::panels::run(shell, id, params)
}

/// Selects `ids` for the canvas; a refusal is the canvas's notice.
fn select(cx: &mut Ctx, ids: Vec<Uuid>) {
    let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    if let Err(e) = crate::panels::run_cx(cx, "view.select", json!({ "ids": ids })) {
        cx.canvas.notice = Some(e);
    }
}

/// Keeps the active tab's pan and zoom as the view it was left with, for the tab and a diagram's
/// kind to restore ([`State::views`]). Every way of leaving a tab comes here first, so the view
/// kept is always the last (AR-666). Only a view drawn for that tab counts: one still to be fitted
/// or centred belongs to the tab before, so the tab's entry stays.
fn keep_left_view(state: &mut State) {
    if let Some(from) = state.active
        && state.shown == Some(from)
    {
        state.views.insert(from, (state.pan, state.zoom));
    }
}

/// The view `next` opens with when it takes the active tab's place; the view is not drawn for it
/// yet. A landing or a fit still pending was for the tab left, which navigation sets after opening,
/// so it goes: the count of tabs opened moves on ([`State::settled`]). A view the canvas never
/// drew for the tab left is half made, so `next` then shows as last left; after a drawn view an
/// open tab keeps it, as a new one is fitted; a tab with no view kept, never drawn, is fitted
/// (AR-758).
fn settle_view(state: &mut State, next: Option<Uuid>) {
    let drawn = state.active.is_some() && state.shown == state.active;
    state.shown = None;
    if state.active == next {
        return;
    }
    let new = next.is_some_and(|n| !state.tabs.contains(&n));
    state.settled = state.settled.wrapping_add(1);
    match next.and_then(|n| state.views.get(&n).copied()) {
        Some((pan, zoom)) if !drawn => (state.pan, state.zoom, state.fit_pending) = (pan, zoom, false),
        Some(_) if !new => state.fit_pending = false,
        _ => state.fit_pending = true,
    }
}

/// Opens a diagram in the centre: its tab, added when it has none, becomes the active one, the
/// view's open diagram follows, and the diagram is selected so Properties shows it. A skill opens
/// in its own tab the same way, with its text in place of the canvas. The view of the tab left is
/// kept first; a landing or fit still pending for it never places the diagram opened.
pub fn open(shell: &mut Shell, id: Uuid) {
    shell.cx(|cx| open_cx(cx, id));
}

/// [`open`] through the narrow context: the centre's half runs at once; the centre stops showing a
/// version's details once the call returns ([`Request::HideVersionDetails`]).
pub(crate) fn open_cx(cx: &mut Ctx, id: Uuid) {
    keep_left_view(cx.canvas);
    // What is opened shows in the centre, where a version's details or a changed element would hide it.
    cx.requests.push(Request::HideVersionDetails);
    if cx.session.as_ref().is_some_and(|s| crate::skill_tab::is_skill(s.doc(), id)) {
        crate::skill_tab::open_cx(cx, id);
        return;
    }
    cx.canvas.notice = None;
    if let Err(e) = crate::panels::run_cx(cx, "view.open-diagram", json!({ "diagram": id.to_string() })) {
        cx.canvas.notice = Some(e);
        return;
    }
    select(cx, vec![id]);
    settle_view(cx.canvas, Some(id));
    if !cx.canvas.tabs.contains(&id) {
        cx.canvas.tabs.push(id);
    }
    cx.canvas.active = Some(id);
}

/// Closes a tab; when it was the active one, the tab before it (else after it) takes over and
/// what was said about it ends. A skill's tab whose text the store refused stays open on it,
/// showing why, so nothing typed is lost.
pub fn close_tab(shell: &mut Shell, id: Uuid) {
    let Some(i) = shell.canvas.tabs.iter().position(|t| *t == id) else { return };
    keep_left_view(&mut shell.canvas);
    crate::skill_tab::sync(shell, 0.0, true, Some(id));
    if shell.canvas.skills.iter().any(|d| d.skill == id && d.text != d.saved) {
        crate::skill_tab::open(shell, id);
        return;
    }
    shell.canvas.tabs.remove(i);
    if shell.canvas.active == Some(id) {
        let next = shell.canvas.tabs.get(i.saturating_sub(1)).or_else(|| shell.canvas.tabs.get(i)).copied();
        shell.canvas.active = None;
        shell.canvas.notice = None;
        if let Some(n) = next {
            open(shell, n);
        }
    }
}

/// Drops the tabs of diagrams the store no longer has; what was said about the one showing goes
/// with it.
pub fn retain_valid(state: &mut State, store: &Store) {
    state.tabs.retain(|id| store.by_uuid(*id).is_some_and(|o| store.is_alive(o)));
    if state.active.is_some_and(|a| !state.tabs.contains(&a)) {
        settle_view(state, state.tabs.last().copied());
        state.active = state.tabs.last().copied();
        state.reopen_pending = state.active.is_some();
        state.notice = None;
    }
    crate::editing::retain_valid(state, store);
}

/// The active diagram, when the store still has it.
pub(crate) fn active_diagram(shell: &Shell) -> Option<Obj> {
    let store = shell.session.as_ref()?.doc();
    let d = store.by_uuid(shell.canvas.active?)?;
    (store.is_alive(d) && store.class_of(d).ok() == Some(DIAGRAM)).then_some(d)
}

/// The number `o` holds in `prop`, or 0 when it holds none or one that isn't finite.
pub(crate) fn real(store: &Store, o: Obj, prop: archforge_mof::PropId) -> f64 {
    match store.get(o, prop) {
        Ok(Value::Real(v)) if v.is_finite() => v,
        _ => 0.0,
    }
}

fn string(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The live objects the list `prop` of `o` refers to.
pub(crate) fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|r| store.is_alive(*r)).collect()
}

/// A node's or an edge's own look on this diagram (the Appearance panel): its colours and width,
/// each none when the diagram's look holds.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Look {
    pub fill: Option<ux::Colour>,
    pub stroke: Option<ux::Colour>,
    pub text: Option<ux::Colour>,
    /// The border's or the line's width; none for the diagram's.
    pub width: Option<f32>,
}

/// A colour `#rrggbb` as drawn, or none for anything else.
pub(crate) fn colour(hex: &str) -> Option<ux::Colour> {
    let h = hex.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii())?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(crate::ux::data_colour(byte(0)?, byte(2)?, byte(4)?))
}

/// The palette whose text reads on `fill`: Light's dark text on a light fill, Dark's light text on
/// a dark one, so a node's own fill never hides its words.
pub(crate) fn ink_for(fill: ux::Colour) -> &'static Palette {
    let luma = 0.2126 * fill.r() as f32 + 0.7152 * fill.g() as f32 + 0.0722 * fill.b() as f32;
    ux::palette(luma < 140.0)
}

/// The look a node (`node::STYLE`) or an edge (`edge::STYLE`) carries.
pub(crate) fn look_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Look {
    let Some(s) = element_of(store, o, prop) else { return Look::default() };
    let c = |p| colour(&string(store, s, p));
    let width = real(store, s, style::LINE) as f32;
    Look { fill: c(style::FILL), stroke: c(style::STROKE), text: c(style::TEXT), width: (width > 0.0 && width.is_finite()).then_some(width) }
}

/// The element the node or line `o` shows, through its property `prop`, when it has one.
pub(crate) fn element_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e))
}

/// The rectangle around every node of the diagram, in diagram units, or none when it has none.
pub(crate) fn diagram_bounds(store: &Store, d: Obj) -> Option<Rect> {
    let mut out: Option<Rect> = None;
    for n in refs(store, d, diagram::NODE) {
        let b = bounds_of(store, n);
        let r = Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32));
        if !r.is_finite() {
            continue;
        }
        out = Some(out.map_or(r, |o| o.union(r)));
    }
    out
}

/// Zooms by `factor` about `at`, a point on the screen, so what is under it stays there.
fn zoom_about(state: &mut State, factor: f32, at: Vec2) {
    if !factor.is_finite() || factor <= 0.0 {
        return;
    }
    let new = (state.zoom * factor).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
    let ratio = new / state.zoom;
    state.pan = at - (at - state.pan) * ratio;
    state.zoom = new;
}

/// A zoom the user makes by `factor` about `at`: the canvas zooms, and the diagram's kind hears of
/// it before the canvas next draws ([`State::zoom_intent`]).
pub(crate) fn zoom_by(state: &mut State, factor: f32, at: Vec2) {
    zoom_about(state, factor, at);
    state.zoom_intent = if state.zoom_intent == 0.0 { factor } else { state.zoom_intent * factor };
}

/// Fits the whole diagram into the canvas, zoomed out no further than `least`, or shows an empty
/// one at 100 % from its top left.
pub(crate) fn fit(state: &mut State, size: Vec2, bounds: Option<Rect>, least: f32) {
    match bounds {
        Some(b) if b.width() > 0.0 && b.height() > 0.0 => {
            let room = (size - Vec2::splat(2.0 * FIT_MARGIN)).max(Vec2::splat(1.0));
            let z = (room.x / b.width()).min(room.y / b.height()).min(1.0).max(least);
            state.zoom = z;
            state.pan = (size - b.size() * z) / 2.0 - b.min.to_vec2() * z;
        }
        _ => {
            state.zoom = 1.0;
            state.pan = Vec2::ZERO;
        }
    }
    if !state.pan.is_finite() {
        state.pan = Vec2::ZERO;
    }
}

/// Draws the centre: the tab strip over the canvas or a skill, and over either the last refusal,
/// a refused step of versions as the toast and anything else as the caption.
pub fn centre(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    if std::mem::take(&mut shell.canvas.reopen_pending)
        && let Some(id) = shell.canvas.active
    {
        open(shell, id);
    }
    let tabs = Rect::from_min_size(r.min, vec2(r.width(), TABS));
    let canvas_rect = Rect::from_min_max(pos2(r.left(), tabs.bottom()), r.max);
    tab_strip(shell, ui, p, tabs);
    let below_path = match shell.canvas.active.filter(|id| shell.session.as_ref().is_some_and(|s| crate::skill_tab::is_skill(s.doc(), *id))) {
        Some(skill) => {
            ui.scope(|ui| {
                ui.add_enabled_ui(!shell.looking_only(), |ui| crate::skill_tab::draw(shell, ui, p, canvas_rect, skill));
            });
            0.0
        }
        None => canvas(shell, ui, p, canvas_rect),
    };
    // With no diagram showing, as after the last open one was deleted, the toast and its Undo
    // show over the empty centre all the same (AR-644).
    if active_diagram(shell).is_none() {
        toast(shell, ui, p, canvas_rect);
    }
    if canvas_rect.width() > 0.0 && canvas_rect.height() > 0.0 {
        if shell.canvas.notice_is_refused_step() {
            refused_toast(shell, ui, p, canvas_rect);
        } else {
            notice(shell, ui, p, canvas_rect.min + vec2(16.0, 12.0 + below_path), (canvas_rect.width() - 32.0).max(0.0));
        }
    }
}

/// Says `canvas.notice`, if there is one, at `at` within `width`: wrapped to eight rows at most,
/// and whole to a screen reader as a status. Gives the height it took.
pub(crate) fn notice(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, width: f32) -> f32 {
    let Some(notice) = shell.canvas.notice.clone() else { return 0.0 };
    let g = ux::galley_rows(ui.painter(), &notice, types::CAPTION, p.amber, Some(width), 8);
    let area = Rect::from_min_size(at, g.size());
    ui.painter().galley(at, g, p.amber.into());
    widgets::announce(ui, &mut shell.registry, "workspace.canvas.notice", area, &notice);
    area.height()
}

/// Says the refused step in `canvas.notice` as frame V2.4 draws it: a raised card centred over the
/// toolbar of the canvas `r`, clear of the minimap and of a toast with Undo, with an amber dot.
/// Long words wrap to eight rows at most and the card grows upwards. It takes no clicks, as
/// nothing but the next action ends it, and is read whole as a status.
fn refused_toast(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let Some(words) = shell.canvas.notice.clone() else { return };
    let edge = 16.0;
    let minimap = crate::kinds::active_kind(shell).and_then(|_| minimap_box(shell, r));
    let right = minimap.map_or(r.right() - edge, |m| m.left() - edge);
    let width = REFUSED_TOAST.x.min(right - r.left() - edge).max(0.0);
    let paint = ui.painter();
    let text = ux::galley_rows(paint, &words, types::SMALL, p.text, Some((width - REFUSED_TOAST_TEXT - 8.0).max(0.0)), 8);
    let height = (text.size().y + 22.0).max(REFUSED_TOAST.y);
    let other_toast = shell.canvas.toast.is_some() || crate::chat_delete::toast_up(shell);
    let foot = r.bottom() - REFUSED_TOAST_FOOT - if other_toast { OVER_UNDO_TOAST } else { 0.0 };
    let left = (r.center().x - width / 2.0).min(right - width).max(r.left() + edge);
    let card = Rect::from_min_size(pos2(left, (foot - height).max(r.top() + 8.0)), vec2(width, height));
    ux::shadow(paint, card, 8.0, p.shadow);
    paint.rect_filled(card, 8.0, p.surface);
    let at = pos2(card.left() + REFUSED_TOAST_TEXT, card.center().y - text.size().y / 2.0);
    paint.circle_filled(pos2(card.left() + 17.0, at.y + types::SMALL.line / 2.0), 3.0, p.amber);
    paint.galley(at, text, p.text.into());
    widgets::announce(ui, &mut shell.registry, "workspace.canvas.notice", card, &words);
}

/// The toast at the canvas's foot: the text and Undo, gone after its frames or on Undo.
pub(crate) fn toast(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let Some(t) = shell.canvas.toast.clone() else { return };
    let top = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
    if shell.frames > t.until || top != t.undo_label {
        shell.canvas.toast = None;
        return;
    }
    let paint = ui.painter().clone();
    // Clear of the minimap: at the canvas's foot, left of the minimap while there is room
    // beside it, else above it; the text wraps rather than running under Undo.
    let edge = 16.0;
    let minimap = minimap_box(shell, r);
    let right = minimap.map_or(r.right() - edge, |m| m.left() - edge);
    let beside = right - (r.left() + edge) >= TOAST_MIN;
    let width = if beside { TOAST.x.min(right - r.left() - edge) } else { TOAST.x.min(r.width() - 2.0 * edge) }.max(0.0);
    let undo = ux::galley(&paint, &tr("Undo"), types::SMALL, p.accent_tint, None);
    let undo_w = undo.size().x + 16.0;
    let text = ux::galley_rows(&paint, &t.text, types::SMALL, p.surface, Some((width - 16.0 - 12.0 - undo_w - 8.0).max(0.0)), TOAST_ROWS);
    let height = (text.size().y + 22.0).max(TOAST.y);
    let bottom = match (beside, minimap) {
        (false, Some(mm)) => mm.top() - 12.0,
        _ => r.bottom() - 78.0,
    };
    let left = if beside { (r.center().x - width / 2.0).min(right - width) } else { r.center().x - width / 2.0 };
    let left = left.min(r.right() - edge - width).max(r.left() + edge);
    let m = Rect::from_min_size(pos2(left, bottom - height), vec2(width, height));
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.text);
    let b = Rect::from_min_size(pos2(m.right() - 8.0 - undo_w, m.center().y - 14.0), vec2(undo_w, 28.0));
    paint.galley(pos2(m.left() + 16.0, m.center().y - text.size().y / 2.0), text, p.surface.into());
    paint.galley(b.min + vec2(8.0, (b.height() - undo.size().y) / 2.0), undo, p.accent_tint.into());
    widgets::record_visible(ui, &mut shell.registry, "workspace.toast", "toast", m);
    if control(shell, ui, "workspace.toast.undo", "button", b, true).clicked() {
        shell.canvas.toast = None;
        if let Err(e) = run(shell, "edit.undo", serde_json::Value::Null) {
            shell.canvas.notice = Some(e);
        }
    }
}

/// The tab strip: the new-diagram `+` at the left, then a tab per open diagram or skill.
fn tab_strip(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.page);
    paint.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 1.0), r.max), 0.0, p.border);
    let add = Rect::from_min_size(pos2(r.left() + 8.0, r.top() + 4.0), Vec2::splat(28.0));
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
    let live = shell.session.is_some() && !shell.looking_only();
    if control(shell, ui, "workspace.tabs.add", "button", add, live).on_hover_text(tr("New diagram")).clicked() {
        crate::diagrams::open_new(shell);
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let tabs: Vec<(Uuid, String, &ux::Icon)> = shell
        .canvas
        .tabs
        .iter()
        .filter_map(|id| {
            let d = store.by_uuid(*id)?;
            if crate::skill_tab::is_skill(store, *id) {
                return Some((*id, archforge_core::skills::name_of(store, d), &icons::SKILL));
            }
            let name = crate::diagrams::diagram_name(store, d);
            Some((*id, if name.is_empty() { tr("Unnamed diagram") } else { name }, crate::kinds::diagram_icon(shell.kinds, store, d)))
        })
        .collect();
    let active = shell.canvas.active;
    let strip = Rect::from_min_max(pos2(r.left() + FIRST_TAB, r.top()), r.max);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(strip));
    clip.set_clip_rect(strip);
    let mut x = strip.left();
    let mut action: Option<(Uuid, bool)> = None;
    for (id, name, icon) in &tabs {
        let g = ux::galley(&paint, name, types::UI, p.text, Some(TAB_TEXT_MAX));
        let width = 34.0 + g.size().x.min(TAB_TEXT_MAX) + 29.0;
        let tab = Rect::from_min_size(pos2(x, r.top() + 3.0), vec2(width, TAB_HEIGHT));
        if tab.left() > strip.right() {
            break;
        }
        let is_active = active == Some(*id);
        let tab_id = format!("workspace.tabs.{id}");
        let resp = control(shell, &clip, &tab_id, "tab", tab, true);
        if is_active {
            paint.rect_filled(tab, 6.0, p.surface);
            paint.rect_stroke(tab, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        } else if resp.hovered() {
            paint.rect_filled(tab, 6.0, p.surface);
        }
        let ink = if is_active { p.text } else { p.text_secondary };
        ux::icon_in(&paint, Rect::from_min_size(pos2(tab.left() + 11.0, tab.top() + 9.0), Vec2::splat(15.0)), icon, p.text_secondary);
        paint.galley(pos2(tab.left() + 34.0, tab.top() + (TAB_HEIGHT - g.size().y) / 2.0), g, ink.into());
        let close = Rect::from_center_size(pos2(tab.right() - 15.0, tab.center().y), Vec2::splat(16.0));
        let close_resp = control(shell, &clip, &format!("{tab_id}.close"), "button", close, true).on_hover_text(tr("Close"));
        ux::icon_in(
            &paint,
            Rect::from_center_size(close.center(), Vec2::splat(8.0)),
            &icons::CLOSE,
            if close_resp.hovered() { p.text } else { p.text_muted },
        );
        if close_resp.clicked() {
            action = Some((*id, true));
        } else if resp.clicked() {
            action = Some((*id, false));
        }
        x += width + 2.0;
    }
    match action {
        Some((id, true)) => close_tab(shell, id),
        Some((id, false)) => open(shell, id),
        None => {}
    }
}

/// The canvas: plain when no diagram is open; else the grid, the diagram, the minimap and the
/// toolbar, with the pointer panning, zooming and selecting. Gives the room the path bar takes at
/// its top.
fn canvas(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> f32 {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.canvas);
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return 0.0;
    }
    let Some(d) = active_diagram(shell) else {
        return 0.0;
    };
    shell.canvas.size = r.size();
    widgets::record_visible(ui, &mut shell.registry, "workspace.canvas", "canvas", r);
    let bounds = shell.session.as_ref().map(|s| diagram_bounds(s.doc(), d)).unwrap_or_default();
    if shell.canvas.fit_pending {
        fit(&mut shell.canvas, r.size(), bounds, ZOOM_RANGE.0);
        shell.canvas.fit_pending = false;
    }
    // A node the search found comes to the centre, readable.
    if let Some(n) = shell.canvas.centre_on.take()
        && let Some(n) = shell.session.as_ref().and_then(|s| s.doc().by_uuid(n).map(|o| bounds_of(s.doc(), o)))
    {
        shell.canvas.zoom = shell.canvas.zoom.max(0.75);
        let centre = vec2((n.x + n.width / 2.0) as f32, (n.y + n.height / 2.0) as f32);
        let pan = r.size() / 2.0 - centre * shell.canvas.zoom;
        if pan.is_finite() {
            shell.canvas.pan = pan;
        }
    }
    // The pointer: the pan tool or the middle button drag the view, the wheel scrolls it, a pinch
    // or the wheel with the command key zoom it about the pointer, a click selects what it hits.
    let resp = ui.interact(r, widgets::egui_id("workspace.canvas"), Sense::click_and_drag());
    match shell.canvas.tool {
        Tool::Pan => {
            resp.clone().on_hover_cursor(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
        }
        Tool::Place(_) | Tool::Connect(_) | Tool::Note | Tool::Link => {
            resp.clone().on_hover_cursor(egui::CursorIcon::Crosshair);
        }
        Tool::Select => {}
    }
    if (resp.dragged_by(egui::PointerButton::Primary) && shell.canvas.tool == Tool::Pan) || resp.dragged_by(egui::PointerButton::Middle) {
        shell.canvas.pan += resp.drag_delta();
    }
    if resp.hovered() {
        let (scroll, pinch, pointer) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
        let at = pointer.map_or(r.center(), |pos| pos - r.min.to_vec2()).to_vec2();
        if (pinch - 1.0).abs() > f32::EPSILON {
            zoom_by(&mut shell.canvas, pinch, at);
        } else if scroll != Vec2::ZERO {
            shell.canvas.pan += scroll;
        }
    }
    if !shell.canvas.pan.is_finite() {
        shell.canvas.pan = Vec2::ZERO;
    }
    // The view is this diagram's once the diagram's kind has run before it draws: UML's places a
    // pending landing first, and a zoom across a package's threshold, ⌘↓ or ⌘↑ then move to another
    // level (AR-95). The zoom made since the last frame was the kind's to follow; it ends here.
    shell.canvas.shown = shell.canvas.active;
    let kind = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d));
    if let Some(kind) = kind {
        shell.cx(|cx| kind.before_draw(cx, ui, r, d));
    }
    shell.canvas.zoom_intent = 0.0;
    let view = View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    // An element dragged from the Models explorer lands where it was let go (slice S7), as the
    // diagram's kind places it.
    if let Some((element, at)) = shell.explorer.dropped.take()
        && r.contains(at)
        && let Some(kind) = kind
    {
        shell.cx(|cx| kind.drop(cx, d, element, view.to_canvas(at)));
    }
    let clip = paint.with_clip_rect(r);
    if shell.canvas.grid {
        grid_dots(&clip, p, r, &view);
    }
    let selection: Vec<Obj> = shell.session.as_ref().map(|s| s.view().selection.clone()).unwrap_or_default();
    // A gesture under way is previewed: its nodes draw where the pointer has them.
    let pointer = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos()));
    let overrides = crate::editing::preview_rects(shell, view, pointer);
    let renaming = shell.canvas.rename.as_ref().map(|rn| rn.node).or_else(|| shell.canvas.note_edit.as_ref().map(|ne| ne.node));
    let grid = shell.session.as_ref().map(|s| grid(s.doc(), s.view(), d));
    let mut pending_badges: Vec<(Pos2, Uuid, Uuid, Obj, bool)> = Vec::new();
    if let (Some(session), Some(grid)) = (shell.session.as_ref(), grid.as_ref()) {
        let store = session.doc();
        let live = Live {
            selection: &selection,
            overrides: &overrides,
            key_status: shell.canvas.key_status,
            looking_only: shell.looking_only(),
            renaming,
            problems: &shell.problems,
            // A line under the pointer, while no gesture runs and the pointer is on the canvas.
            hovered: ui
                .input(|i| i.pointer.hover_pos())
                .filter(|at| r.contains(*at) && shell.canvas.gesture.is_none() && ui.rect_contains_pointer(r))
                .and_then(|at| {
                    let (x, y) = view.to_canvas(at);
                    match crate::editing::hit(store, session.view(), grid, x, y) {
                        Some(archforge_diagram::Hit::Edge(e)) => Some(e),
                        _ => None,
                    }
                }),
            end_drag: crate::editing::end_preview(shell, pointer),
            segment_drag: crate::editing::segment_preview(shell, view, pointer),
            draft: shell.notation_states.card_draft(store),
            rename_text: shell.canvas.rename.as_ref().map(|rn| rn.text.as_str()),
            label_drag: crate::editing::label_preview(shell, pointer),
        };
        let scene = Scene { p, store, notations: session.view(), kind, diagram: d, view: &view, grid, area: r };
        let drawn = draw_scene(ui, &clip, &scene, live, &mut shell.registry);
        shell.canvas.ends = drawn.ends;
        shell.canvas.qualified_ends = drawn.qualified_ends;
        shell.canvas.extra_labels = drawn.extra_labels;
        shell.canvas.package_tabs = drawn.package_tabs;
        shell.canvas.path_badges = drawn.path_badges;
        // Such a label says its hover text while the pointer is on it.
        if let Some(at) = ui.input(|i| i.pointer.hover_pos())
            && shell.canvas.gesture.is_none()
            && let Some(l) = shell.canvas.extra_labels.iter().rev().find(|l| l.rect.contains(at))
        {
            let _ = resp.clone().on_hover_text_at_pointer(l.hover.as_str());
        }
        // A name its tab shows cut is shown whole while the pointer is on the tab.
        if let Some(at) = ui.input(|i| i.pointer.hover_pos())
            && let Some(name) = shell.canvas.package_tabs.iter().rev().find(|t| t.rect.contains(at)).and_then(|t| t.cut.clone())
        {
            let _ = resp.clone().on_hover_text_at_pointer(name);
        }
        // A package's owning path says its whole sentence to a screen reader, as a label's value,
        // and on hover.
        for b in shell.canvas.path_badges.iter().filter(|b| b.rect.intersects(r)) {
            ui.ctx().accesskit_node_builder(widgets::egui_id(&format!("workspace.canvas.node.{}.path", b.node)), |node| {
                node.set_role(egui::accesskit::Role::Label);
                node.set_value(b.whole.as_str());
            });
        }
        if let Some(at) = ui.input(|i| i.pointer.hover_pos())
            && shell.canvas.gesture.is_none()
            && let Some(b) = shell.canvas.path_badges.iter().rev().find(|b| b.rect.contains(at))
        {
            let _ = resp.clone().on_hover_text_at_pointer(b.whole.as_str());
        }
        pending_badges = drawn.badges;
    }
    for (at, control_id, element, element_obj, many) in pending_badges {
        let texts = shell.problems.get(&element_obj).cloned().unwrap_or_default();
        badge(shell, ui, p, at, control_id, element, &texts, many);
    }
    if let Some(grid) = grid.as_ref() {
        crate::editing::pointer(shell, ui, &resp, view, grid, d);
    }
    crate::editing::overlays(shell, ui, p, r, view, d, &overrides);
    // What the diagram's kind draws over the canvas (UML's path bar and a package's Open), and the
    // fade of a switch over that.
    let below_path = kind.map_or(0.0, |k| shell.cx(|cx| k.after_draw(cx, ui, p, r, d)));
    fade(shell, ui, p, r);
    // What the notations offer over the canvas (ER's mapping table offer), under the toast.
    crate::notation_ui::overlays(shell, ui, Layer::Offers, &Overlaid { p, r, view, d, overrides: &overrides });
    toast(shell, ui, p, r);
    if minimap_shown(shell, r) {
        minimap(shell, ui, p, r, bounds, &view);
    } else if r.width() >= 16.0 + layout(crate::kinds::active_kind(shell)).width + 16.0 + 28.0 + MINIMAP_GAP.x {
        // Its button, where the toolbar leaves room for it: live when the user folded the minimap,
        // not available when the canvas is too small for it.
        let b = Rect::from_min_size(pos2(r.right() - MINIMAP_GAP.x - 28.0, r.bottom() - MINIMAP_GAP.y - 26.0), vec2(28.0, 26.0));
        let folded = !shell.canvas.minimap_open;
        paint.rect_filled(b, 7.0, p.surface);
        paint.rect_stroke(b, 7.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), &icons::MAP, if folded { p.text_secondary } else { p.text_muted });
        let resp = control(shell, ui, "workspace.minimap.toggle", "button", b, true);
        let hint = if folded { tr("Show minimap") } else { tr("Make the canvas larger to show the minimap") };
        if resp.on_hover_text(hint).clicked() && folded {
            shell.canvas.minimap_open = true;
        }
    }
    toolbar(shell, ui, p, r);
    // What a drawing under way has so far, as the diagram's kind says (a UML n-ary association's).
    if let Some(kind) = kind {
        shell.cx(|cx| kind.hint_bar(cx, ui, p, r, below_path));
    }
    below_path
}

/// How long the canvas fades in after a diagram's kind marked a switch, in seconds.
pub(crate) const SWITCH_FADE: f64 = 0.2;

/// The fade of a switch ([`State::switched`]): the canvas's colour over `r`, thinning out over
/// [`SWITCH_FADE`].
fn fade(shell: &Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let Some(t) = shell.canvas.switched else { return };
    let k = ((ui.input(|i| i.time) - t) / SWITCH_FADE) as f32;
    if k < 1.0 {
        ui.painter().rect_filled(r, 0.0, p.canvas.gamma_multiply(1.0 - k.max(0.0)));
        ui.ctx().request_repaint();
    }
}

/// A problem badge on the canvas: the problems on hover; one whose figure offers a way out of the
/// problem (`resolvable`) shows a plus, says the way out and takes a click, as the element's
/// notation answers ([`crate::notation_ui::NotationUi::resolve_hint`]: ER's many-to-many
/// relationship opens the mapping table offer). Where the notation offers none, the badge stays a
/// plain one: its problems on hover, no click.
#[allow(clippy::too_many_arguments, reason = "a badge is drawn from its place, its control id, its element and its texts")]
pub(crate) fn badge(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, control_id: Uuid, element: Uuid, texts: &[String], resolvable: bool) {
    let paint = ui.painter().clone();
    let b = Rect::from_center_size(at, Vec2::splat(18.0));
    paint.circle_filled(b.center(), 9.0, p.surface);
    paint.circle_stroke(b.center(), 9.0, Stroke::new(1.2, p.key_foreign));
    let way_out = shell
        .session
        .as_ref()
        .filter(|_| resolvable)
        .and_then(|s| crate::notation_ui::of(shell.notation_uis, s.doc(), s.doc().by_uuid(element)?))
        .and_then(|notation| Some((notation, notation.resolve_hint()?)));
    if way_out.is_some() {
        ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(10.0)), &icons::PLUS, p.key_foreign);
    } else {
        let g = ux::galley(&paint, "!", types::CAPTION, p.key_foreign, None);
        paint.galley(b.center() - g.size() / 2.0, g, p.key_foreign.into());
    }
    let (resolver, hint) = match way_out {
        Some((notation, hint)) => (Some(notation), hint),
        None => (None, texts.join("\n")),
    };
    // Live for its hover; only a badge that offers a way out acts on a click.
    if control(shell, ui, &format!("workspace.canvas.problem.{control_id}"), "badge", b, true).on_hover_text(hint).clicked()
        && let Some(notation) = resolver
    {
        shell.cx(|cx| notation.resolve(cx, element));
    }
}

/// The index of diagram `d` as the canvas draws and hits it: the nodes their notations draw behind
/// (a package's card) under the other figures, each group in stored order, so a class or a note
/// placed over a package's card stays in sight and takes the press (AR-570). The store's order is
/// untouched.
pub(crate) fn grid(store: &Store, notations: &archforge_diagram::ProjectView, d: Obj) -> Grid {
    Grid::build_with(store, d, |n| notations.node_drawn_behind(store, n))
}

/// A diagram to draw: its store, the notations that describe it, its kind, which draws its own
/// figures beyond them, and where it goes on screen.
#[derive(Clone, Copy)]
pub(crate) struct Scene<'a> {
    pub(crate) p: &'a Palette,
    pub(crate) store: &'a Store,
    pub(crate) notations: &'a archforge_diagram::ProjectView,
    pub(crate) kind: Option<&'static dyn crate::kinds::DiagramKind>,
    pub(crate) diagram: Obj,
    pub(crate) view: &'a View,
    pub(crate) grid: &'a Grid,
    pub(crate) area: Rect,
}

/// What a diagram shows beyond its store: the selection, a gesture under way, what is being typed
/// and what the rules found. Everything is handed in, none of it read from elsewhere.
pub(crate) struct Live<'a> {
    pub(crate) selection: &'a [Obj],
    pub(crate) overrides: &'a std::collections::BTreeMap<Uuid, Rect>,
    pub(crate) key_status: bool,
    /// A version shows: the corner handles, which resize, are not drawn.
    pub(crate) looking_only: bool,
    pub(crate) renaming: Option<Uuid>,
    pub(crate) problems: &'a std::collections::BTreeMap<Obj, Vec<String>>,
    pub(crate) hovered: Option<Obj>,
    pub(crate) end_drag: Option<(Uuid, u8, Pos2)>,
    pub(crate) segment_drag: Option<(Uuid, Vec<(f64, f64)>)>,
    pub(crate) draft: Option<(Obj, usize, Option<usize>)>,
    pub(crate) rename_text: Option<&'a str>,
    pub(crate) label_drag: Option<(Uuid, Vec2)>,
}

/// What drawing a diagram leaves behind: the places the next frame's press looks at, and the
/// problem badges to draw over everything (their place, ids, element and whether there are many).
pub(crate) struct Drawn {
    pub(crate) ends: Vec<EndText>,
    pub(crate) qualified_ends: Vec<(Pos2, Pos2)>,
    pub(crate) extra_labels: Vec<ExtraLabel>,
    pub(crate) package_tabs: Vec<PackageTab>,
    pub(crate) path_badges: Vec<PathBadge>,
    pub(crate) badges: Vec<(Pos2, Uuid, Uuid, Obj, bool)>,
}

/// Draws the part of the diagram in view, lines under cards, from what it is handed: no window
/// state is read, so any store can be drawn, in a context that has the window's fonts. Each thing
/// drawn is recorded in `registry`.
pub(crate) fn draw_scene(ui: &egui::Ui, clip: &egui::Painter, scene: &Scene<'_>, live: Live<'_>, registry: &mut widgets::Registry) -> Drawn {
    let Scene { p, store, notations, kind, diagram: d, view, grid, area: r } = *scene;
    let overrides = live.overrides;
    let (x0, y0) = view.to_canvas(r.min);
    let (x1, y1) = view.to_canvas(r.max);
    let mut visible = grid.visible(archforge_diagram::Rect::new(x0, y0, x1 - x0, y1 - y0));
    // Nodes dragged into view draw too, in the drawing order: a package's card dragged in stays
    // under the other figures (AR-570).
    let shown = visible.nodes.len();
    for n in refs(store, d, diagram::NODE) {
        if !visible.nodes.contains(&n) && store.uuid(n).ok().is_some_and(|id| overrides.contains_key(&id)) {
            visible.nodes.push(n);
        }
    }
    if visible.nodes.len() > shown {
        grid.sort_drawn(&mut visible.nodes);
    }
    let draw = Drawing {
        p,
        store,
        view,
        selection: live.selection,
        overrides,
        key_status: live.key_status,
        looking_only: live.looking_only,
        renaming: live.renaming,
        problems: live.problems,
        diagram: d,
        notations,
        kind,
        boxes: refs(store, d, diagram::NODE).into_iter().map(|n| (n, model_rect(store, view, overrides, n))).collect(),
        spreads: archforge_diagram::route::spreads(store, d, |n| model_rect(store, view, overrides, n)),
        hovered: live.hovered,
        end_drag: live.end_drag,
        segment_drag: live.segment_drag,
        ends: std::cell::RefCell::new(Vec::new()),
        qualified_ends: std::cell::RefCell::new(Vec::new()),
        draft: live.draft,
        rename_text: live.rename_text,
        tabs: std::cell::RefCell::new(Vec::new()),
        paths: std::cell::RefCell::new(Vec::new()),
        led: std::cell::OnceCell::new(),
        label_drag: live.label_drag,
    };
    let mut badges = Vec::new();
    for e in &visible.edges {
        badges.extend(draw_edge(clip, &draw, *e, registry, ui));
    }
    draw_constraint_links(clip, &draw);
    draw_nesting_lines(clip, &draw);
    let extra_labels = draw.kind.map(|k| k.draw_extras(clip, &draw, registry, ui)).unwrap_or_default();
    for n in &visible.nodes {
        badges.extend(draw_node(clip, &draw, *n, registry, ui));
    }
    Drawn {
        ends: draw.ends.take(),
        qualified_ends: draw.qualified_ends.take(),
        extra_labels,
        package_tabs: draw.tabs.take(),
        path_badges: draw.paths.take(),
        badges,
    }
}

/// The grid: a dot every `GRID_STEP` units, doubled until the dots are at least `GRID_MIN` px
/// apart and few enough to draw.
fn grid_dots(paint: &egui::Painter, p: &Palette, r: Rect, view: &View) {
    let mut step = GRID_STEP * view.zoom;
    if !step.is_finite() || step <= 0.0 {
        return;
    }
    while step < GRID_MIN || (r.width() / step) * (r.height() / step) > GRID_MAX_DOTS {
        step *= 2.0;
        if step > r.width().max(r.height()) {
            return;
        }
    }
    let start = |origin: f32, offset: f32| origin + offset.rem_euclid(step);
    let mut x = start(r.left(), view.pan.x);
    while x < r.right() {
        let mut y = start(r.top(), view.pan.y);
        while y < r.bottom() {
            paint.circle_filled(pos2(x, y), 1.0, p.border);
            y += step;
        }
        x += step;
    }
}

/// A node's rectangle in diagram coordinates: as stored, or a gesture's preview turned back from
/// the screen. Lines are routed from these, so panning and zooming route nothing anew.
fn model_rect(store: &Store, view: &View, overrides: &std::collections::BTreeMap<Uuid, Rect>, n: Obj) -> archforge_diagram::Rect {
    match store.uuid(n).ok().and_then(|id| overrides.get(&id)) {
        Some(r) => {
            let (x, y) = view.to_canvas(r.min);
            archforge_diagram::Rect::new(x, y, f64::from(r.width() / view.zoom), f64::from(r.height() / view.zoom))
        }
        None => bounds_of(store, n),
    }
}

/// One node: an entity card with its eyebrow, name and columns, a note, or a plain card.
/// Gives the problem badge to draw at the node's corner, when its element has problems.
fn draw_node(
    paint: &egui::Painter,
    d: &Drawing<'_>,
    n: Obj,
    registry: &mut widgets::Registry,
    ui: &egui::Ui,
) -> Option<(Pos2, Uuid, Uuid, Obj, bool)> {
    let (p, store, view, selection, key_status) = (d.p, d.store, d.view, d.selection, d.key_status);
    let rect = d.node_rect(n);
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let z = view.zoom;
    // A label (a generalization set's) is drawn with its lines (AR-129), not as a card.
    if d.notations.node_is_label(store, n) {
        return None;
    }
    let id = store.uuid(n).ok();
    if let Some(id) = id {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}"), "node", rect);
    }
    let renaming = id.is_some() && id == d.renaming;
    let element = element_of(store, n, node::ELEMENT);
    // As the gestures take it: a card draws selected with what a click on it selects, as a line does.
    let selected = crate::editing::is_selected(store, d.notations, selection, n);
    // The card the element's notation describes, when it describes one.
    let mut card = element.and_then(|e| d.notations.of_element(store, e)?.card(store, e, d.diagram));
    // A row typed in place: its place is kept blank under the field, and a new one's card grows to
    // hold it while it is typed.
    let mut rect = rect;
    if let (Some(c), Some((of, compartment, row))) = (card.as_mut(), d.draft)
        && element == Some(of)
    {
        with_draft(c, compartment, row);
        if row.is_none() {
            rect.max.y = rect.max.y.max(rect.top() + card_height(c) * z);
        }
    }
    // Clipped to the card as drawn, so a draft's growing keeps the rows it moved down in sight.
    let inner = paint.with_clip_rect(rect.intersect(paint.clip_rect()));
    let annotation = card.as_ref().is_some_and(|c| matches!(c.tone, Tone::Constraint | Tone::Braces));
    // A template's signature rows, drawn over the card and its selection (AR-130).
    let signature = card.as_ref().filter(|c| !annotation && !c.template.is_empty() && z >= DETAIL_ZOOM).map(|c| c.template.clone());
    let diamond = card.as_ref().is_some_and(|c| c.tone == Tone::Diamond);
    match element {
        Some(_) if annotation && let Some(card) = card => draw_constraint(paint, &inner, d, n, &card, rect),
        // An n-ary association's diamond, its name above it (AR-128).
        Some(_) if diamond && let Some(card) = card => {
            let keyword = match &card.eyebrow {
                archforge_diagram::figure::Text::Plain(k) => Some(k.as_str()),
                archforge_diagram::figure::Text::Said(..) => None,
            };
            draw_diamond(paint, p, rect, &card.name, keyword, z, selected, &look_of(store, n, node::STYLE));
        }
        Some(_) if let Some(card) = card => {
            // An element of another scope: dimmed, dashed, its scope named in the eyebrow.
            let dim = |c: ux::Colour| if card.elsewhere { c.gamma_multiply(FOREIGN_ALPHA) } else { c };
            // The node's own look on this diagram, where it has one.
            let look = look_of(store, n, node::STYLE);
            // Its own fill brings the text that reads on it, unless it has its own text colour too.
            let ink: &Palette = match look.fill {
                Some(f) => ink_for(f),
                None => p,
            };
            // A plain card (a class) takes the surface, the border and the muted eyebrow.
            let tabbed = card.tone == archforge_diagram::figure::Tone::Tabbed;
            let plain = tabbed || card.tone == archforge_diagram::figure::Tone::Plain;
            let (fill, edge, eyebrow, line) =
                if plain { (p.surface, p.border, ink.text_secondary, p.border) } else { (p.entity, p.entity_edge, ink.entity_ink, p.entity_line) };
            let p = &Palette {
                entity: dim(look.fill.unwrap_or(fill)),
                entity_edge: dim(look.stroke.unwrap_or(edge)),
                entity_ink: dim(eyebrow),
                entity_line: dim(look.stroke.map_or(line, |c| c.gamma_multiply(0.5))),
                text: dim(look.text.unwrap_or(ink.text)),
                text_muted: dim(ink.text_muted),
                text_secondary: dim(ink.text_secondary),
                ..*p
            };
            let border = look.width.map_or(1.0, |w| (w * z).max(1.0));
            // A package's tab, as a folder's, over its top left, holding its name: the name as it
            // is typed while it is renamed (AR-113).
            let tab = tabbed.then(|| {
                let name = if renaming { d.rename_text.unwrap_or(&card.name) } else { card.name.as_str() };
                package_tab(paint, rect, name, z, p.text)
            });
            if let Some((tab, _, _)) = &tab {
                paint.rect_filled(*tab, 5.0 * z, p.entity);
                paint.rect_stroke(*tab, 5.0 * z, Stroke::new(border, if selected { p.accent } else { p.entity_edge }), StrokeKind::Inside);
            }
            paint.rect_filled(rect, 6.0 * z, p.entity);
            if card.elsewhere {
                dashed_rect(paint, rect, Stroke::new(border, p.entity_edge), 4.0 * z, 3.0 * z);
            } else {
                paint.rect_stroke(rect, 6.0 * z, Stroke::new(border, p.entity_edge), StrokeKind::Inside);
            }
            if let Some((tab, name, cut)) = tab {
                // The tab joins the card: no border between them.
                paint.rect_filled(
                    Rect::from_min_max(pos2(tab.left() + border, rect.top() - z), pos2(tab.right() - border, rect.top() + border)),
                    0.0,
                    p.entity,
                );
                if z >= DETAIL_ZOOM && !renaming {
                    paint.galley(tab.min + vec2(PACKAGE_TAB_PAD * z, (tab.height() - name.size().y) / 2.0), name, p.text.into());
                }
                if let Some(id) = id {
                    widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.tab"), "tab", tab);
                    d.tabs.borrow_mut().push(PackageTab { rect: tab, node: id, cut: cut.then(|| card.name.clone()) });
                }
            }
            // Where the package lives, under its card; the root, which no package owns, has none.
            if tabbed
                && z >= DETAIL_ZOOM
                && !card.owner_path.is_empty()
                && let Some(id) = id
            {
                let badge = path_badge(paint, rect, &card.owner_path, z, d.p.text_secondary, id);
                widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.path"), "label", badge.rect);
                d.paths.borrow_mut().push(badge);
            }
            let room = Some((rect.width() - 2.0 * ENTITY_INSET * z).max(0.0));
            if z >= DETAIL_ZOOM && tabbed {
                // The package's own diagram, scaled into its card.
                if let Some(e) = element {
                    let area = draw_preview(&inner, d, e, rect, z);
                    if let Some(id) = id {
                        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.preview"), "image", area);
                    }
                }
            } else if z >= DETAIL_ZOOM {
                let eyebrow = eyebrow_case(&say(&card.eyebrow));
                ux::text(&inner, rect.min + vec2(ENTITY_INSET, 12.0) * z, &eyebrow, types::EYEBROW.scaled(z), p.entity_ink, room);
                if !renaming {
                    let name = if card.slanted { ux::text_slanted } else { ux::text };
                    // Braces after the name (`{final}`) in the smaller keyword style, on its
                    // baseline; the name gives up room so they always show.
                    let braces = card.braces.as_deref().map(|b| ux::galley(&inner, b, types::SMALL.scaled(z), p.text_secondary, None));
                    let gap = 5.0 * z;
                    let room = room.map(|w| (w - braces.as_ref().map_or(0.0, |b| b.size().x + gap)).max(0.0));
                    let at = name(&inner, rect.min + vec2(ENTITY_INSET, 30.0) * z, &card.name, types::SECTION.scaled(z), p.text, room);
                    if let Some(b) = braces {
                        inner.galley(pos2(at.right() + gap, at.top() + 9.0 * z), b, p.text_secondary.into());
                    }
                }
                let line_y = rect.top() + ENTITY_HEADER * z;
                if line_y < rect.bottom() {
                    inner.rect_filled(Rect::from_min_size(pos2(rect.left(), line_y), vec2(rect.width(), 1.0)), 0.0, p.entity_line);
                }
                draw_rows(&inner, p, &card, rect, z, key_status, room);
            } else {
                let g = ux::galley(&inner, &card.name, types::SECTION.scaled(z.max(DETAIL_ZOOM)), p.text, room);
                inner.galley(rect.center() - g.size() / 2.0, g, p.text.into());
            }
        }
        Some(e) => {
            let look = look_of(store, n, node::STYLE);
            let border = look.width.map_or(1.0, |w| (w * z).max(1.0));
            paint.rect_filled(rect, 6.0 * z, look.fill.unwrap_or(p.surface));
            paint.rect_stroke(rect, 6.0 * z, Stroke::new(border, look.stroke.unwrap_or(p.border)), StrokeKind::Inside);
            let name = archforge_core::name_of(store, e);
            ux::text(
                &inner,
                rect.min + vec2(ENTITY_INSET, 12.0) * z,
                &name,
                types::UI.scaled(z),
                look.text.unwrap_or_else(|| look.fill.map_or(p.text, |f| ink_for(f).text)),
                Some((rect.width() - 2.0 * ENTITY_INSET * z).max(0.0)),
            );
        }
        None => {
            // A note: its own text on the amber tint, or in its own look.
            let look = look_of(store, n, node::STYLE);
            let border = look.width.map_or(1.0, |w| (w * z).max(1.0));
            paint.rect_filled(rect, 4.0 * z, look.fill.unwrap_or(p.amber_tint));
            paint.rect_stroke(rect, 4.0 * z, Stroke::new(border, look.stroke.unwrap_or(p.amber)), StrokeKind::Inside);
            if !renaming {
                let ink = look.text.unwrap_or_else(|| look.fill.map_or(p.text, |f| ink_for(f).text));
                let text = string(store, n, node::TEXT);
                let g = ux::galley_rows(&inner, &text, types::UI.scaled(z), ink, Some((rect.width() - 24.0 * z).max(0.0)), 8);
                inner.galley(rect.min + vec2(12.0, 10.0) * z, g, ink.into());
            }
        }
    }
    if selected {
        selection_ring(paint, p, rect, d.looking_only);
    }
    // A template's signature over the card's top-right corner (AR-130).
    if let Some(rows) = signature
        && let Some(sig) = signature_box(paint, p, &rows, rect, z)
        && let Some(id) = id
    {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.signature"), "label", sig);
    }
    let e = element.filter(|e| d.problems.contains_key(e))?;
    Some((rect.right_top() + vec2(-2.0, 2.0), id?, store.uuid(e).ok()?, e, false))
}

/// A hollow diamond filling `rect`, over `fill`.
pub(crate) fn diamond_shape(paint: &egui::Painter, rect: Rect, fill: ux::Colour, stroke: Stroke) {
    let points = vec![rect.left_center(), rect.center_top(), rect.right_center(), rect.center_bottom()];
    paint.add(egui::Shape::convex_polygon(points, fill, stroke));
}

/// Draws an n-ary association's diamond node at `rect` (AR-128): a hollow diamond in the line's
/// ink, the accent while it is selected, its name above it.
#[allow(clippy::too_many_arguments, reason = "a diamond is drawn with its name, its stereotypes and its look")]
pub(crate) fn draw_diamond(paint: &egui::Painter, p: &Palette, rect: Rect, name: &str, keyword: Option<&str>, z: f32, selected: bool, look: &Look) {
    let ink = if selected { p.accent } else { look.stroke.unwrap_or(p.text_secondary) };
    let width = look.width.map_or((1.5 * z).clamp(1.0, 3.0), |w| (w * z).clamp(1.0, 6.0));
    diamond_shape(paint, rect, look.fill.unwrap_or(p.canvas), Stroke::new(width, ink));
    let text_ink = look.text.unwrap_or(p.text_secondary);
    let mut top = rect.top() - 8.0 * z;
    if !name.is_empty() {
        let g = ux::galley(paint, name, types::SMALL.scaled(z), text_ink, None);
        top -= g.size().y;
        paint.galley(pos2(rect.center().x - g.size().x / 2.0, top), g, text_ink.into());
    }
    // Its stereotypes above its name, `«Reference»`, as a line's are (AR-135).
    if let Some(k) = keyword {
        let g = ux::galley(paint, k, types::CAPTION.scaled(z), text_ink, None);
        paint.galley(pos2(rect.center().x - g.size().x / 2.0, top - g.size().y - 1.0 * z), g, text_ink.into());
    }
}

/// A constraint as its diagram shows it (AR-133): braces, `{expression}` in purple beside its
/// element with no card around it; or a constraint note, the surface with a purple border and a
/// folded corner, `CONSTRAINT · NOT EVALUATED` in its purple eyebrow, its name, then its expression
/// between braces, as written. Its own look on the diagram wins, as a note's does.
fn draw_constraint(paint: &egui::Painter, inner: &egui::Painter, d: &Drawing<'_>, n: Obj, card: &archforge_diagram::figure::Card, rect: Rect) {
    let (p, z) = (d.p, d.view.zoom);
    let look = look_of(d.store, n, node::STYLE);
    let text_ink = look.text.unwrap_or_else(|| look.fill.map_or(p.text, |f| ink_for(f).text));
    let rows: Vec<&str> = card.compartments.iter().flatten().map(|r| r.text.as_str()).collect();
    if card.tone == Tone::Braces {
        let text = rows.first().copied().unwrap_or_default();
        let ink = look.text.unwrap_or(p.purple);
        ux::text(paint, rect.min + vec2(2.0, 2.0) * z, text, types::CAPTION.scaled(z), ink, None);
        return;
    }
    let edge = look.stroke.unwrap_or(p.purple);
    let border = look.width.map_or(1.25 * z, |w| w * z).max(1.0);
    paint.rect_filled(rect, 0.0, look.fill.unwrap_or(p.surface));
    paint.rect_stroke(rect, 0.0, Stroke::new(border, edge), StrokeKind::Inside);
    // The folded corner at the top right.
    let fold = (14.0 * z).min(rect.width() / 2.0).min(rect.height() / 2.0);
    let (tr_, fl, fb) = (rect.right_top(), rect.right_top() - vec2(fold, 0.0), rect.right_top() + vec2(0.0, fold));
    paint.add(egui::Shape::convex_polygon(vec![fl, pos2(fl.x, fb.y), fb], p.purple_tint, Stroke::new(border, edge)));
    paint.line_segment([fl, tr_], Stroke::new(border, look.fill.unwrap_or(p.canvas)));
    let room = Some((rect.width() - 24.0 * z - fold).max(0.0));
    if z < DETAIL_ZOOM {
        let g = ux::galley(inner, &card.name, types::SECTION.scaled(z.max(DETAIL_ZOOM)), text_ink, room);
        inner.galley(rect.center() - g.size() / 2.0, g, text_ink.into());
        return;
    }
    let eyebrow = eyebrow_case(&say(&card.eyebrow));
    ux::text(inner, rect.min + vec2(12.0, 12.0) * z, &eyebrow, types::EYEBROW.scaled(z), look.stroke.unwrap_or(p.purple), room);
    let mut y = 30.0;
    if !card.name.is_empty() {
        ux::text(inner, rect.min + vec2(12.0, y) * z, &card.name, types::SMALL_STRONG.scaled(z), text_ink, room);
        y += 21.0;
    }
    for row in rows {
        if rect.top() + (y + 17.0) * z > rect.bottom() {
            break;
        }
        ux::text(inner, rect.min + vec2(12.0, y) * z, row, types::SMALL.scaled(z), text_ink, Some((rect.width() - 24.0 * z).max(0.0)));
        y += 18.0;
    }
}

/// The dashed links of the diagram's constraint notes to the nodes of the elements they constrain
/// (AR-133), worked out from the model each frame: purple, the accent while the note is selected.
fn draw_constraint_links(paint: &egui::Painter, d: &Drawing<'_>) {
    let (p, store, z) = (d.p, d.store, d.view.zoom);
    for (n, _) in &d.boxes {
        let Some(e) = element_of(store, *n, node::ELEMENT) else { continue };
        let Some(notation) = d.notations.of_element(store, e) else { continue };
        let targets = notation.links(store, e, d.diagram);
        if targets.is_empty() {
            continue;
        }
        let from_rect = d.node_rect(*n);
        let selected = d.selection.contains(n) || d.selection.contains(&e);
        let stroke = Stroke::new((1.25 * z).clamp(1.0, 2.5), if selected { p.accent } else { p.purple });
        for t in targets {
            let to_rect = d.node_rect(t);
            let (from, to) = (exit_point(from_rect, to_rect.center()), exit_point(to_rect, from_rect.center()));
            if from.is_finite() && to.is_finite() {
                paint.extend(egui::Shape::dashed_line(&[from, to], stroke, (3.0 * z).max(1.5), (3.0 * z).max(1.5)));
            }
        }
    }
}

/// The radius of the ⊕ at an owner's end of a nesting line, in diagram units (AR-132).
const CONTAINMENT_RADIUS: f32 = 7.0;

/// The lines that tie each nested classifier's card to its owner's card where the diagram shows
/// both (AR-132): solid in the muted ink, the accent while either is selected, ending in ⊕ at the
/// owner. Worked out from the model each frame, never stored; overlapping cards draw none.
fn draw_nesting_lines(paint: &egui::Painter, d: &Drawing<'_>) {
    let (p, store, z) = (d.p, d.store, d.view.zoom);
    let mut node_of: std::collections::HashMap<Obj, Obj> = std::collections::HashMap::new();
    for (n, _) in &d.boxes {
        if let Some(e) = element_of(store, *n, node::ELEMENT) {
            node_of.entry(e).or_insert(*n);
        }
    }
    for (n, _) in &d.boxes {
        let Some(e) = element_of(store, *n, node::ELEMENT) else { continue };
        let Some(notation) = d.notations.of_element(store, e) else { continue };
        let Some(owner) = notation.nested_in(store, e) else { continue };
        let Some(on) = node_of.get(&owner).copied() else { continue };
        let others: Vec<Rect> = d.boxes.iter().filter(|(b, _)| *b != on && *b != *n).map(|(b, _)| d.node_rect(*b)).collect();
        let Some((points, centre)) = nesting_route(d.node_rect(on), d.node_rect(*n), CONTAINMENT_RADIUS * z, &others) else { continue };
        let selected = [*n, e, on, owner].iter().any(|o| d.selection.contains(o));
        let ink = if selected { p.accent } else { p.text_secondary };
        let stroke = Stroke::new((1.2 * z).clamp(1.0, 2.5), ink);
        paint.add(egui::Shape::line(points, stroke));
        let r = CONTAINMENT_RADIUS * z;
        paint.circle(centre, r, p.surface, stroke);
        let arm = r * 0.62;
        paint.line_segment([centre - vec2(arm, 0.0), centre + vec2(arm, 0.0)], stroke);
        paint.line_segment([centre - vec2(0.0, arm), centre + vec2(0.0, arm)], stroke);
    }
}

/// A package's tab: its height, the room either side of its name and its least width (AR-113).
pub(crate) const PACKAGE_TAB_HEIGHT: f32 = 22.0;
const PACKAGE_TAB_PAD: f32 = 12.0;
const PACKAGE_TAB_MIN: f32 = 56.0;
/// The margin between a package's card and the thumbnail of its diagram, and the room inside the
/// thumbnail around what it shows.
const PREVIEW_MARGIN: f32 = 10.0;
const PREVIEW_INSET: f32 = 8.0;
/// The most a thumbnail enlarges its diagram, so a few small cards still read as a thumbnail.
const PREVIEW_MOST: f32 = 0.5;

/// The tab over the top left of the package card at `rect` (AR-113): as wide as `name` and its
/// padding, never narrower than [`PACKAGE_TAB_MIN`] nor wider than the card, with the name laid out to
/// fit it (cut with an ellipsis where the card is narrower than the name), and whether it was cut.
pub(crate) fn package_tab(paint: &egui::Painter, rect: Rect, name: &str, z: f32, ink: ux::Colour) -> (Rect, std::sync::Arc<egui::Galley>, bool) {
    let ty = types::SMALL_STRONG.scaled(z);
    let pad = PACKAGE_TAB_PAD * z;
    let whole = ux::galley(paint, name, ty, ink, None);
    let room = (rect.width() - 2.0 * pad).max(0.0);
    let cut = whole.size().x > room;
    let shown = if cut { ux::galley(paint, name, ty, ink, Some(room)) } else { whole };
    let width = (shown.size().x + 2.0 * pad).max(PACKAGE_TAB_MIN * z).min(rect.width().max(0.0));
    let tab = Rect::from_min_size(pos2(rect.left(), rect.top() - (PACKAGE_TAB_HEIGHT - 1.0) * z), vec2(width, PACKAGE_TAB_HEIGHT * z));
    (tab, shown, cut)
}

/// How far a package's owning path is under its card.
const PATH_BADGE_GAP: f32 = 6.0;

/// Draws a package's owning path under its card, `Owner: Commerce platform › Operations`, in the
/// caption style and the canvas's secondary ink: its words say what it is, never its colour alone.
/// It is as wide as the card at most: its first owners fold into `…` so the nearest stay, and the
/// nearest alone is cut at its end ([`ux::cut_front`]).
fn path_badge(paint: &egui::Painter, card: Rect, owners: &[String], z: f32, ink: ux::Colour, node: Uuid) -> PathBadge {
    let ty = types::CAPTION.scaled(z);
    let template = tr("Owner: {path}");
    let say = |path: &str| fmt(&template, &[("path", path)]);
    let width = card.width().max(0.0);
    let (shown, _) = ux::cut_front(owners, crate::panels::PATH_SEP, |t| ux::galley(paint, &say(t), ty, ink, None).size().x <= width);
    let shown = say(&shown);
    let rect = ux::text(paint, pos2(card.left(), card.bottom() + PATH_BADGE_GAP * z), &shown, ty, ink, Some(width));
    PathBadge { rect, node, shown, whole: say(&owners.join(crate::panels::PATH_SEP)) }
}

/// How a card of a thumbnail is drawn: as the card it stands for is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MiniLook {
    Plain,
    Tinted,
    Tabbed,
    Note,
    /// A constraint note and a constraint's braces (AR-133).
    Constraint,
    Braces,
    /// An n-ary association's diamond (AR-128).
    Diamond,
}

/// One card of a thumbnail, on screen: its look and how many rows each of its compartments has.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MiniCard {
    pub rect: Rect,
    pub look: MiniLook,
    pub rows: Vec<usize>,
}

/// A diagram scaled into a thumbnail (AR-113): its cards and its lines as they are laid out, and
/// the scale it was drawn at.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Preview {
    pub cards: Vec<MiniCard>,
    pub lines: Vec<Vec<Pos2>>,
    pub scale: f32,
}

/// The diagram each element leads to, by the element: a package's own class diagram.
fn leads(store: &Store) -> std::collections::HashMap<Obj, Obj> {
    let mut out = std::collections::HashMap::new();
    for d in store.alive().filter(|o| store.class_of(*o).ok() == Some(DIAGRAM)) {
        if let Some(e) = element_of(store, d, diagram::ELEMENT) {
            out.entry(e).or_insert(d);
        }
    }
    out
}

/// `d` scaled to fit `area`, centred, enlarged at most [`PREVIEW_MOST`]: none when it shows
/// nothing.
pub(crate) fn preview(store: &Store, notations: &archforge_diagram::ProjectView, d: Obj, area: Rect) -> Option<Preview> {
    let grid = grid(store, notations, d);
    let all = grid.visible(archforge_diagram::Rect::new(-1.0e9, -1.0e9, 2.0e9, 2.0e9));
    let mut boxes: Vec<(Rect, MiniLook, Vec<usize>)> = Vec::new();
    for n in &all.nodes {
        let b = bounds_of(store, *n);
        let r = Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32));
        if !r.is_finite() || r.width() <= 0.0 || r.height() <= 0.0 {
            continue;
        }
        let (look, rows) = match element_of(store, *n, node::ELEMENT) {
            None => (MiniLook::Note, Vec::new()),
            Some(e) => match notations.of_element(store, e).and_then(|nt| nt.card(store, e, d)) {
                Some(card) => {
                    let look = match card.tone {
                        archforge_diagram::figure::Tone::Tabbed => MiniLook::Tabbed,
                        archforge_diagram::figure::Tone::Plain => MiniLook::Plain,
                        archforge_diagram::figure::Tone::Tinted => MiniLook::Tinted,
                        archforge_diagram::figure::Tone::Constraint => MiniLook::Constraint,
                        archforge_diagram::figure::Tone::Braces => MiniLook::Braces,
                        archforge_diagram::figure::Tone::Diamond => MiniLook::Diamond,
                    };
                    (look, card.compartments.iter().map(Vec::len).collect())
                }
                None => (MiniLook::Plain, Vec::new()),
            },
        };
        boxes.push((r, look, rows));
    }
    // Every line's route in one pass over the index, not a lookup per line.
    let lines: Vec<Vec<Pos2>> = grid
        .lines()
        .map(|(_, points)| points.iter().map(|(x, y)| pos2(*x as f32, *y as f32)).collect::<Vec<_>>())
        .filter(|points| points.len() >= 2 && points.iter().all(|q| q.is_finite()))
        .collect();
    // A package's tab stands above its card, so it is in the bounds too.
    let mut bounds = boxes
        .iter()
        .map(|(r, look, _)| if *look == MiniLook::Tabbed { r.with_min_y(r.top() - PACKAGE_TAB_HEIGHT) } else { *r })
        .reduce(|a, b| a.union(b))?;
    for q in lines.iter().flatten() {
        bounds.extend_with(*q);
    }
    let scale = (area.width() / bounds.width().max(1.0)).min(area.height() / bounds.height().max(1.0)).min(PREVIEW_MOST);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let origin = area.center() - bounds.size() * scale / 2.0;
    let map = |q: Pos2| origin + (q - bounds.min) * scale;
    let cards = boxes.into_iter().map(|(r, look, rows)| MiniCard { rect: Rect::from_min_max(map(r.min), map(r.max)), look, rows }).collect();
    let lines = lines.into_iter().map(|l| l.into_iter().map(map).collect()).collect();
    Some(Preview { cards, lines, scale })
}

/// The thumbnail of the package `package`'s own diagram on its card at `rect` (AR-113), on the
/// page tint inside the card's margin; Empty diagram when the diagram shows nothing or there is
/// none. Gives the thumbnail's area.
fn draw_preview(paint: &egui::Painter, d: &Drawing<'_>, package: Obj, rect: Rect, z: f32) -> Rect {
    let p = d.p;
    let area = rect.shrink(PREVIEW_MARGIN * z);
    if area.width() <= 0.0 || area.height() <= 0.0 {
        return area;
    }
    paint.rect_filled(area, 4.0 * z, p.page);
    let led = d.led.get_or_init(|| leads(d.store));
    let shown = led.get(&package).and_then(|g| preview(d.store, d.notations, *g, area.shrink(PREVIEW_INSET * z)));
    let Some(shown) = shown else {
        let g = ux::galley(paint, &tr("Empty diagram"), types::CAPTION.scaled(z), p.text_muted, Some(area.width()));
        paint.galley(area.center() - g.size() / 2.0, g, p.text_muted.into());
        return area;
    };
    let clip = paint.with_clip_rect(area.intersect(paint.clip_rect()));
    for line in &shown.lines {
        clip.add(egui::Shape::line(line.clone(), Stroke::new(1.0, p.text_secondary)));
    }
    for c in &shown.cards {
        // The preview's scale is already the screen's: the area it was fitted to is zoomed.
        mini_card(&clip, p, c, shown.scale);
    }
    area
}

/// One card of a thumbnail drawn at `s` (its diagram's scale on screen): its outline, a bar for
/// its name, a line under its header, and a bar for each row that fits.
fn mini_card(paint: &egui::Painter, p: &Palette, c: &MiniCard, s: f32) {
    let r = c.rect;
    if c.look == MiniLook::Braces {
        paint.rect_filled(Rect::from_min_size(r.left_top(), vec2(r.width(), (5.0 * s).max(1.0))), 1.0, p.purple);
        return;
    }
    // An n-ary association's diamond, hollow in the lines' ink (AR-128).
    if c.look == MiniLook::Diamond {
        let points = vec![r.left_center(), r.center_top(), r.right_center(), r.center_bottom()];
        paint.add(egui::Shape::convex_polygon(points, p.surface, Stroke::new(1.0, p.text_secondary)));
        return;
    }
    let (fill, edge) = match c.look {
        MiniLook::Tinted => (p.entity, p.entity_edge),
        MiniLook::Note => (p.amber_tint, p.amber),
        MiniLook::Constraint => (p.surface, p.purple),
        MiniLook::Plain | MiniLook::Tabbed | MiniLook::Braces | MiniLook::Diamond => (p.surface, p.border),
    };
    if c.look == MiniLook::Tabbed {
        let tab =
            Rect::from_min_size(pos2(r.left(), r.top() - (PACKAGE_TAB_HEIGHT - 1.0) * s), vec2((r.width() * 0.4).max(2.0), PACKAGE_TAB_HEIGHT * s));
        paint.rect_filled(tab, 2.0, fill);
        paint.rect_stroke(tab, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    paint.rect_filled(r, 2.0, fill);
    paint.rect_stroke(r, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    if matches!(c.look, MiniLook::Note | MiniLook::Tabbed | MiniLook::Constraint) {
        return;
    }
    let bar = |y: f32, w: f32, h: f32, colour: ux::Colour| {
        if y + h <= r.bottom() - 1.0 {
            paint.rect_filled(Rect::from_min_size(pos2(r.left() + ENTITY_INSET * s, y), vec2((r.width() * w).max(1.0), h.max(1.0))), 1.0, colour);
        }
    };
    bar(r.top() + 30.0 * s, 0.5, 8.0 * s, p.text_muted);
    let header = r.top() + ENTITY_HEADER * s;
    if header < r.bottom() {
        paint.rect_filled(Rect::from_min_size(pos2(r.left(), header), vec2(r.width(), 1.0)), 0.0, edge);
    }
    let mut y = r.top() + ENTITY_FIRST_ROW * s;
    for (i, rows) in c.rows.iter().enumerate() {
        if i > 0 {
            y += COMPARTMENT_GAP * ENTITY_ROW * s;
        }
        for _ in 0..*rows {
            bar(y + 4.0 * s, 0.45, 5.0 * s, p.border);
            y += ENTITY_ROW * s;
        }
    }
}

/// The row of `card` at `y` units below its top, counted across its compartments as
/// [`draw_rows`] lays them out: a row takes its pitch from a little over its top; none on a
/// compartment's padding or past the last row.
pub(crate) fn row_at(card: &archforge_diagram::figure::Card, y: f32) -> Option<usize> {
    slots(card).into_iter().filter(|(_, s, _)| matches!(s, Slot::Row(_))).position(|(top, _, _)| (top - 4.0..top - 4.0 + ENTITY_ROW).contains(&y))
}

/// A card's places under its header, each with its top in units below the card's and the
/// compartment it is in: a row a pitch apart, an empty compartment of several still a row. Each
/// compartment is padded as the card's first and last are: its rows start as far under its
/// dividing line as the first row under the header's, and end as far over the next line as the
/// last row over the card's foot, so a pitch and a dividing line part two compartments.
fn slots(card: &archforge_diagram::figure::Card) -> Vec<(f32, Slot<'_>, usize)> {
    let mut out = Vec::new();
    // Where the next row sits, in pitches below the first.
    let mut at = 0.0_f32;
    for (c, rows) in card.compartments.iter().enumerate() {
        if c > 0 {
            at += COMPARTMENT_GAP;
            out.push((ENTITY_FIRST_ROW + at * ENTITY_ROW - COMPARTMENT_TOP, Slot::Line, c));
        }
        if rows.is_empty() && card.compartments.len() > 1 {
            out.push((ENTITY_FIRST_ROW + at * ENTITY_ROW, Slot::Empty, c));
            at += 1.0;
        }
        for row in rows {
            out.push((ENTITY_FIRST_ROW + at * ENTITY_ROW, Slot::Row(row), c));
            at += 1.0;
        }
    }
    out
}

/// The top of row `index` of `card`, counted across its compartments, in units below the card's.
pub(crate) fn row_top(card: &archforge_diagram::figure::Card, index: usize) -> Option<f32> {
    slots(card).into_iter().filter(|(_, s, _)| matches!(s, Slot::Row(_))).nth(index).map(|(top, _, _)| top)
}

/// The top of compartment `c`'s first place (its first row, or the row an empty one takes).
pub(crate) fn compartment_top(card: &archforge_diagram::figure::Card, c: usize) -> Option<f32> {
    slots(card).into_iter().find(|(_, s, at)| *at == c && !matches!(s, Slot::Line)).map(|(top, _, _)| top)
}

/// How tall `card` is to show every place it has: [`archforge_diagram::figure::Card::height`],
/// the height a command grows a node to, which lays the places out as [`slots`] does.
pub(crate) fn card_height(card: &archforge_diagram::figure::Card) -> f32 {
    card.height()
}

/// `card` with a row being typed in place: a new, blank one at the end of compartment
/// `compartment`, or row `row` (counted across the compartments) blanked under its field.
pub(crate) fn with_draft(card: &mut archforge_diagram::figure::Card, compartment: usize, row: Option<usize>) {
    match row {
        None => {
            if let Some(rows) = card.compartments.get_mut(compartment) {
                rows.push(archforge_diagram::figure::Row { mark: None, text: String::new(), linked: false, slanted: false, underlined: false });
            }
        }
        Some(index) => {
            if let Some(r) = card.compartments.iter_mut().flatten().nth(index) {
                r.text.clear();
            }
        }
    }
}

/// One place in a card's compartments, at its top in units below the card's.
enum Slot<'a> {
    Row(&'a archforge_diagram::figure::Row),
    /// The row an empty compartment of several still takes, so the line between them shows.
    Empty,
    Line,
}

/// A card's compartments under its header: a row a pitch apart, half a pitch and a dividing line
/// between compartments. When a row does not fit the card's height, the rows from the last place
/// that fits are said in one muted row there, whichever compartment they are in, or at the card's
/// foot or beside its eyebrow when no row's place fits, so a card made short never looks empty.
fn draw_rows(inner: &egui::Painter, p: &Palette, card: &archforge_diagram::figure::Card, rect: Rect, z: f32, key_status: bool, room: Option<f32>) {
    let slots: Vec<(f32, Slot<'_>)> = slots(card).into_iter().map(|(top, slot, _)| (top, slot)).collect();
    // A row fits when it ends within the card, a unit of slack for rounding.
    let height = rect.height() / z;
    let fits = |top: f32| top + ENTITY_ROW <= height + 1.0;
    // Where the rows stop: the first that does not fit, and the place before it the rest is said in.
    // Rows the notation left out count as not fitting: the last place that fits says them.
    let first = slots.iter().position(|(top, s)| matches!(s, Slot::Row(_)) && !fits(*top)).or((card.hidden > 0).then_some(slots.len()));
    let cue = first.and_then(|f| slots.iter().take(f).rposition(|(top, s)| !matches!(s, Slot::Line) && fits(*top)));
    // Without such a place (not even a row fits under the header) only what is above the first is drawn.
    let end = cue.or(first).unwrap_or(slots.len());
    let mut shown = 0;
    for (top, slot) in slots.iter().take(end) {
        let y = rect.top() + top * z;
        match slot {
            Slot::Line if y < rect.bottom() => {
                inner.rect_filled(Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), 1.0)), 0.0, p.entity_line);
            }
            Slot::Row(row) => {
                if key_status && let Some(b) = row.mark {
                    ux::text(inner, pos2(rect.left() + ENTITY_INSET * z, y + z), b, types::CAPTION.scaled(z), p.entity_ink, None);
                }
                let text_x = rect.left() + ENTITY_TEXT * z;
                let room_x = (rect.right() - ENTITY_INSET * z - text_x - if row.linked { 18.0 * z } else { 0.0 }).max(0.0);
                // An abstract operation slanted, a static member underlined.
                let marks = ux::Marks { slanted: row.slanted, underlined: row.underlined };
                let drawn = ux::text_marked(inner, pos2(text_x, y), &row.text, types::UI.scaled(z), p.text, Some(room_x), marks);
                if row.linked {
                    ux::icon(inner, pos2(drawn.right() + 6.0 * z, y + 6.0 * z), &icons::LINK, 12.0 * z, p.text_secondary);
                }
                shown += 1;
            }
            _ => {}
        }
    }
    if first.is_none() {
        return;
    }
    let total: usize = card.compartments.iter().map(Vec::len).sum::<usize>() + card.hidden;
    let text = trn(card.more.0, card.more.1, total.saturating_sub(shown) as u64);
    // In the last place that fits; with none, a row's height above the card's foot while that is
    // still under the header; on a card shorter still, at the right of its eyebrow's line.
    let place = cue.and_then(|i| slots.get(i)).map(|(top, _)| *top).or(Some(height - ENTITY_ROW).filter(|top| *top >= ENTITY_HEADER - 4.0));
    match place {
        Some(top) => {
            ux::text(inner, pos2(rect.left() + ENTITY_TEXT * z, rect.top() + top * z), &text, types::UI.scaled(z), p.text_muted, room);
        }
        None => {
            let style = types::CAPTION.scaled(z);
            let width = ux::galley(inner, &text, style, p.text_muted, room).size().x;
            let x = (rect.right() - ENTITY_INSET * z - width).max(rect.left() + ENTITY_INSET * z);
            ux::text(inner, pos2(x, rect.top() + 11.0 * z), &text, style, p.text_muted, room);
        }
    }
}

/// An eyebrow as drawn: in capitals, but for a UML keyword between guillemets, which keeps its
/// case (`«interface» · BILLING`).
pub(crate) fn eyebrow_case(text: &str) -> String {
    let mut keyword = false;
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '«' => keyword = true,
            '»' => keyword = false,
            _ => {}
        }
        if keyword || c == '»' { out.push(c) } else { out.extend(c.to_uppercase()) }
    }
    out
}

/// A text a notation describes, in the user's language.
fn say(t: &archforge_diagram::figure::Text) -> String {
    match t {
        archforge_diagram::figure::Text::Said(template, args) => {
            let args: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
            fmt(&tr(template), &args)
        }
        archforge_diagram::figure::Text::Plain(s) => s.clone(),
    }
}

/// A rectangle's outline in dashes of `dash` with gaps of `gap`, as the design draws a table of
/// another schema.
fn dashed_rect(paint: &egui::Painter, rect: Rect, stroke: Stroke, dash: f32, gap: f32) {
    let r = rect.shrink(stroke.width / 2.0);
    let path = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    paint.extend(egui::Shape::dashed_line(&path, stroke, dash.max(1.0), gap.max(1.0)));
}

/// The accent ring of a selected node and its four corner handles, none while only `looking`.
fn selection_ring(paint: &egui::Painter, p: &Palette, rect: Rect, looking: bool) {
    let ring = rect.expand(4.0);
    paint.rect_stroke(ring, 4.0, Stroke::new(1.5, p.accent), StrokeKind::Inside);
    if looking {
        return;
    }
    for corner in [ring.left_top(), ring.right_top(), ring.left_bottom(), ring.right_bottom()] {
        let h = Rect::from_center_size(corner, Vec2::splat(6.0));
        paint.rect_filled(h, 1.0, p.surface);
        paint.rect_stroke(h, 1.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    }
}

/// Draws a line's triangle, diamond or open arrow at `tip`, coming from `back`, a hollow one filled
/// with `bg`, at its [`mark_outline`].
#[allow(clippy::too_many_arguments, reason = "a mark is drawn at its end, in its stroke and over what it is drawn on")]
pub(crate) fn line_mark(paint: &egui::Painter, mark: EndMark, tip: Pos2, back: Pos2, stroke: Stroke, z: f32, bg: ux::Colour) {
    let outline = mark_outline(mark, tip, back, z);
    match (mark, outline.as_slice()) {
        (EndMark::Triangle | EndMark::HollowDiamond, [_, ..]) => {
            paint.add(egui::Shape::convex_polygon(outline, bg, stroke));
        }
        (EndMark::FilledDiamond | EndMark::FilledTriangle, [_, ..]) => {
            paint.add(egui::Shape::convex_polygon(outline, stroke.color, stroke));
        }
        (EndMark::OpenArrow, [tip, left, right]) => {
            paint.line_segment([*tip, *left], stroke);
            paint.line_segment([*tip, *right], stroke);
        }
        _ => {}
    }
}

/// One edge: the line between its nodes' borders through its waypoints, as its element's notation
/// describes it (its ends' marks and texts, its roles, its dashes, its status), and its labels.
/// Gives the point and element of a problem badge to draw, when the line shows one.
fn draw_edge(
    paint: &egui::Painter,
    d: &Drawing<'_>,
    e: Obj,
    registry: &mut widgets::Registry,
    ui: &egui::Ui,
) -> Option<(Pos2, Uuid, Uuid, Obj, bool)> {
    let (p, store, view, selection) = (d.p, d.store, d.view, d.selection);
    let z = view.zoom;
    let Some(element) = element_of(store, e, edge::ELEMENT) else {
        draw_link(paint, d, e, registry, ui);
        return None;
    };
    let notation = d.notations.of_element(store, element);
    let line = notation.and_then(|nt| nt.line(store, element)).unwrap_or_default();
    let mut points = edge_points(d, e)?;
    let n = points.len();
    // A selected element marks the line its notation draws selected with it (a qualifier its
    // association's, as a selected member marks its card), and a line draws selected with what a
    // click on it selects (an n-ary association's leg with its association, AR-128).
    let selected = selection.contains(&e)
        || selection.contains(&element)
        || selection.iter().any(|o| d.notations.line_selected_with(store, *o) == Some(element))
        || selection.contains(&d.notations.selects_as(store, element));
    // A line starts where the diagram's kind says, when not at its source node's border (a leg of
    // a UML n-ary association at its diamond's outline).
    if let Some(kind) = d.kind
        && let Some(source) = element_of(store, e, edge::SOURCE)
        && let (Some(tip), Some(next)) = (points.first().copied(), points.get(1).copied())
        && let Some(on) = kind.line_start(d, source, tip, next)
    {
        points[0] = on;
    }
    // A qualified end's box sits against its node, the line starting from its outer side (AR-127).
    let boxes = qualifier_boxes(paint, d, e, &line, &mut points);
    let hovered = d.hovered == Some(e);
    let stroke = if line.styled {
        // The line's colour says its status while the marks are on: green when complete, orange
        // when not. A line's own colour shows while they are off; its own width shows always.
        let look = look_of(store, e, edge::STYLE);
        let own = look.stroke.unwrap_or(p.text_secondary);
        let ink = match (selected, d.key_status, line.status) {
            (true, _, _) => p.accent,
            (false, false, _) | (false, true, None) => own,
            (false, true, Some(true)) => p.green,
            (false, true, Some(false)) => p.key_foreign,
        };
        let width = look.width.map_or((1.5 * z).clamp(1.0, 3.0), |w| (w * z).clamp(1.0, 6.0));
        // The line under the pointer turns to the accent.
        Stroke::new(width, if hovered && !selected { p.accent } else { ink })
    } else {
        Stroke::new((1.3 * z).clamp(1.0, 3.0), if selected || hovered { p.accent } else { p.text_secondary })
    };
    // The line under the pointer: a soft halo under it.
    if hovered {
        halo(paint, &points, stroke, p);
    }
    let shape = |mark: EndMark, tip: Pos2, back: Pos2| mark_shape(mark, tip, back, z);
    let mut drawn = points.clone();
    if line.ends[0].mark.set_back() > 0.0 {
        drawn[0] = shape(line.ends[0].mark, points[0], points[1]).1;
    }
    if line.ends[1].mark.set_back() > 0.0 {
        drawn[n - 1] = shape(line.ends[1].mark, points[n - 1], points[n - 2]).1;
    }
    self::line(paint, &drawn, stroke, !line.dashed, z);
    for (i, rect, rows) in boxes {
        draw_qualifier_box(paint, p, rect, rows, Stroke::new((1.0 * z).clamp(1.0, 2.0), stroke.color), z);
        if let Ok(id) = store.uuid(e) {
            widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}.qualifiers.{i}"), "label", rect);
        }
    }
    // What the diagram's kind draws with the line beyond its figure (a UML association class's
    // tie to its card, or its name while the card is not here).
    let named = d.kind.and_then(|k| k.line_extras(paint, d, &LineDrawn { line: e, element, points: &points, selected }, registry, ui));
    // Where the line's name label sits, when it shows one: its stereotypes go above it.
    let mut name_at: Option<Rect> = named;
    for (end, tip, back) in [(&line.ends[0], points[0], points[1]), (&line.ends[1], points[n - 1], points[n - 2])] {
        let (dir, base, _, _) = shape(end.mark, tip, back);
        line_mark(paint, end.mark, tip, back, stroke, z, p.canvas);
        // A note beside the target's triangle, such as a generalization set's facts.
        if end.mark == EndMark::Triangle
            && let Some(note) = line.note.as_ref().filter(|_| std::ptr::eq(end, &line.ends[1]))
        {
            let text = note.iter().map(say).collect::<Vec<_>>().join(" · ");
            let gal = ux::galley(paint, &text, types::SMALL.scaled(z), p.text_secondary, None);
            paint.galley(base + vec2(10.0 * z, -gal.size().y / 2.0) - dir * 14.0 * z, gal, p.text_secondary.into());
        }
    }
    let mut badge = None;
    if line.badge
        && d.problems.contains_key(&element)
        && let (Ok(id), Ok(edge_id)) = (store.uuid(element), store.uuid(e))
    {
        badge = Some((along(&points, 0.5) + vec2(0.0, -14.0 * z.max(0.5)), edge_id, id, element, line.resolvable));
    }
    if let Ok(id) = store.uuid(e) {
        let bounds = Rect::from_points(&points).expand(6.0);
        widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}"), "edge", bounds);
    }
    // The ends' marks.
    end_glyph(paint, p, points[0], points[1], line.ends[0].mark, stroke, z, false);
    end_glyph(paint, p, points[n - 1], points[n - 2], line.ends[1].mark, stroke, z, false);
    // The ends' texts: beside the line near its end, or where the user put them; two placed by
    // default keep clear of each other.
    let sides = [(&line.ends[0], points[0], points[1]), (&line.ends[1], points[n - 1], points[n - 2])];
    let galleys = sides.map(|(end, _, _)| end.text.as_deref().map(|t| ux::galley(paint, t, types::SMALL.scaled(z), p.text_secondary, None)));
    let placed_by_user = |i: u8, end: &archforge_diagram::figure::End| {
        store.uuid(e).ok().is_some_and(|edge_id| d.end_drag.is_some_and(|(id, k, _)| id == edge_id && k == i))
            || refs(store, e, edge::LABEL).into_iter().any(|l| string(store, l, label::KIND) == end.slot)
    };
    let pair = match (&galleys[0], &galleys[1]) {
        (Some(a), Some(b)) if !placed_by_user(0, sides[0].0) && !placed_by_user(1, sides[1].0) => {
            Some(beside_pair([(sides[0].1, sides[0].2, a.size(), sides[0].0.mark), (sides[1].1, sides[1].2, b.size(), sides[1].0.mark)], z))
        }
        _ => None,
    };
    for (i, ((end, at, towards), g)) in sides.into_iter().zip(galleys).enumerate() {
        let Some(g) = g else { continue };
        let Ok(edge_id) = store.uuid(e) else { continue };
        let k = i as u8;
        let own = refs(store, e, edge::LABEL).into_iter().find(|l| string(store, *l, label::KIND) == end.slot);
        let centre = match (d.end_drag.filter(|(id, j, _)| *id == edge_id && *j == k), own) {
            (Some((_, _, centre)), _) => centre,
            (None, Some(l)) => at + vec2(real(store, l, label::DX) as f32, real(store, l, label::DY) as f32) * z,
            (None, None) => pair.map_or_else(|| beside(at, towards, g.size(), z, end.mark), |c| c[i]),
        };
        let i = k;
        let rect = Rect::from_center_size(centre, g.size());
        paint.galley(rect.min, g, p.text_secondary.into());
        d.ends.borrow_mut().push(EndText { edge: edge_id, end: i, rect: rect.expand(3.0), anchor: at });
    }
    // The roles near their ends: above a level line, left of an upright one, so neither sits on
    // the line or its mark.
    for (end, at, towards) in [(&line.ends[0], points[0], points[1]), (&line.ends[1], points[n - 1], points[n - 2])] {
        let Some(text) = end.role.as_deref() else { continue };
        if text.is_empty() || (towards - at).length() <= f32::EPSILON {
            continue;
        }
        let dir = (towards - at).normalized();
        let g = ux::galley(paint, text, types::SMALL.scaled(z), p.text_secondary, None);
        let near = at + dir * 44.0 * z;
        let at = if dir.y.abs() > dir.x.abs() {
            near - vec2(g.size().x + 8.0 * z, g.size().y / 2.0)
        } else {
            near - vec2(g.size().x / 2.0, g.size().y + 8.0 * z)
        };
        paint.galley(at, g, p.text_secondary.into());
    }
    if line.styled {
        for l in refs(store, e, edge::LABEL) {
            let kind = string(store, l, label::KIND);
            // The ends' texts draw above, with the ends.
            if line.ends.iter().any(|end| !end.slot.is_empty() && end.slot == kind) {
                continue;
            }
            let text = notation.map(|nt| nt.slot_text(store, element, &kind)).unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            let t = match store.get(l, label::AT) {
                Ok(Value::Enum(i)) => match LabelPlace::from_index(i) {
                    Some(LabelPlace::Source) => 0.0,
                    Some(LabelPlace::Target) => 1.0,
                    _ => 0.5,
                },
                _ => 0.5,
            };
            let at = along(&points, t) + vec2(real(store, l, label::DX) as f32, real(store, l, label::DY) as f32) * z;
            let g = ux::galley(paint, &text, types::SMALL.scaled(z), p.text_secondary, None);
            // A middle label sits above a level stretch of the line, right of an upright one.
            let upright = {
                let d = along(&points, 0.51) - along(&points, 0.49);
                d.y.abs() > d.x.abs()
            };
            let offset = match (t == 0.5, upright) {
                (true, true) => vec2(8.0 * z, -g.size().y / 2.0),
                (true, false) => vec2(-g.size().x / 2.0, -g.size().y - 6.0 * z),
                (false, _) => vec2(-g.size().x / 2.0, 6.0 * z),
            };
            if kind == "name" {
                name_at = Some(Rect::from_min_size(at + offset, g.size()));
            }
            paint.galley(at + offset, g, p.text_secondary.into());
        }
        if selected {
            // A square at each end, but where the end's round handle moved past its mark: the
            // handle marks that end, and a square at the tip would sit on the mark.
            let moved = end_handles_at(&points, [line.ends[0].mark, line.ends[1].mark], z);
            for (q, _) in [(points[0], moved[0]), (points[n - 1], moved[1])].into_iter().filter(|(_, at)| at.is_none()) {
                let h = Rect::from_center_size(q, Vec2::splat(6.0));
                paint.rect_filled(h, 1.0, p.surface);
                paint.rect_stroke(h, 1.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
            }
        }
    }
    if let Some(keyword) = line.keyword.as_deref() {
        let at = keyword_text(paint, d, keyword, &points, name_at);
        if let Ok(id) = store.uuid(e) {
            widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}.stereotypes"), "label", at);
        }
    }
    badge
}

/// A line, solid for an identifying relationship and dashed for the rest.
pub fn line(paint: &egui::Painter, points: &[Pos2], stroke: Stroke, identifying: bool, z: f32) {
    if identifying {
        paint.add(egui::Shape::line(points.to_vec(), stroke));
    } else {
        paint.add(egui::Shape::dashed_line(points, stroke, 6.0 * z.max(0.5), 4.0 * z.max(0.5)));
    }
}

/// A line's stereotypes, `«Reference»` (AR-135, F3), in the caption style: just above its name
/// label `name` where it shows one, else at its middle where a name would sit. Gives where it is.
fn keyword_text(paint: &egui::Painter, d: &Drawing<'_>, keyword: &str, points: &[Pos2], name: Option<Rect>) -> Rect {
    let (p, z) = (d.p, d.view.zoom);
    let g = ux::galley(paint, keyword, types::CAPTION.scaled(z), p.text_secondary, None);
    let at = match name {
        Some(r) => pos2(r.center().x - g.size().x / 2.0, r.top() - g.size().y - 1.0 * z),
        None => {
            let upright = {
                let v = along(points, 0.51) - along(points, 0.49);
                v.y.abs() > v.x.abs()
            };
            let offset = if upright { vec2(8.0 * z, -g.size().y / 2.0) } else { vec2(-g.size().x / 2.0, -g.size().y - 6.0 * z) };
            along(points, 0.5) + offset
        }
    };
    let rect = Rect::from_min_size(at, g.size());
    paint.galley(at, g, p.text_secondary.into());
    rect
}

/// The qualifier boxes of `line`'s ends (AR-127), each with its end's index, its rectangle and its
/// rows laid out: a box sits against the side of its node where the line leaves it, centred on the
/// line, and the line's end in `points` moves to the box's outer side, so the end's mark, role and
/// text sit there. A line from a node to itself draws none.
fn qualifier_boxes(
    paint: &egui::Painter,
    d: &Drawing<'_>,
    e: Obj,
    line: &archforge_diagram::figure::Line,
    points: &mut [Pos2],
) -> Vec<(usize, Rect, Vec<std::sync::Arc<egui::Galley>>)> {
    let (store, z, n) = (d.store, d.view.zoom, points.len());
    let nodes = [element_of(store, e, edge::SOURCE), element_of(store, e, edge::TARGET)];
    if n < 2 || nodes[0] == nodes[1] {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, end) in line.ends.iter().enumerate() {
        let (Some(node), false) = (nodes[i], end.qualifiers.is_empty()) else { continue };
        let k = if i == 0 { 0 } else { n - 1 };
        let rows: Vec<_> = end.qualifiers.iter().map(|t| ux::galley(paint, t, types::SMALL.scaled(z), d.p.text, None)).collect();
        let width = rows.iter().map(|g| g.size().x).fold(0.0_f32, f32::max) + 16.0 * z;
        let height = rows.iter().map(|g| g.size().y).sum::<f32>() + 8.0 * z;
        let (rect, outer) = qualifier_place(d.node_rect(node), points[k], vec2(width, height));
        d.qualified_ends.borrow_mut().push((points[k], outer));
        points[k] = outer;
        out.push((i, rect, rows));
    }
    out
}

/// A template's signature (AR-130): a dashed box on the surface over the card's top-right corner,
/// one parameter a row in their order; gives its rectangle, none for a card that is no template.
fn signature_box(paint: &egui::Painter, p: &Palette, rows: &[String], card: Rect, z: f32) -> Option<Rect> {
    if rows.is_empty() {
        return None;
    }
    let galleys: Vec<_> = rows.iter().map(|t| ux::galley(paint, t, types::CAPTION.scaled(z), p.text, None)).collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max) + 16.0 * z;
    let height = galleys.iter().map(|g| g.size().y).sum::<f32>() + 8.0 * z;
    let rect = signature_place(card, vec2(width, height), z);
    paint.rect_filled(rect, 0.0, p.surface);
    dashed_rect(paint, rect, Stroke::new(1.0, p.text_secondary), 4.0 * z, 3.0 * z);
    let mut y = rect.top() + 4.0 * z;
    for g in galleys {
        let h = g.size().y;
        paint.galley(pos2(rect.left() + 8.0 * z, y), g, p.text.into());
        y += h;
    }
    Some(rect)
}

/// A qualifier box: the surface, framed in the line's ink, its rows in model order.
fn draw_qualifier_box(paint: &egui::Painter, p: &Palette, rect: Rect, rows: Vec<std::sync::Arc<egui::Galley>>, stroke: Stroke, z: f32) {
    paint.rect_filled(rect, 1.0, p.surface);
    paint.rect_stroke(rect, 1.0, stroke, StrokeKind::Inside);
    let mut y = rect.top() + 4.0 * z;
    for g in rows {
        let h = g.size().y;
        paint.galley(pos2(rect.left() + 8.0 * z, y), g, p.text.into());
        y += h;
    }
}

/// A soft band under a line's whole route, so the line under the pointer stands out.
fn halo(paint: &egui::Painter, points: &[Pos2], stroke: Stroke, p: &Palette) {
    let band = Stroke::new(stroke.width + 6.0, p.accent.gamma_multiply(0.18));
    paint.add(egui::Shape::line(points.to_vec(), band));
}

/// A line's points on screen, from its source's edge to its target's, run as its style or its
/// diagram's says (straight, at right angles, around the other tables, or curved) through its
/// waypoints; a relationship of a table with itself loops out of its right side. Routed in diagram
/// coordinates from the rectangles drawn, so a table being dragged takes its lines along. None
/// when an end is missing.
pub(crate) fn edge_points(d: &Drawing<'_>, e: Obj) -> Option<Vec<Pos2>> {
    let (store, view) = (d.store, d.view);
    let (Some(source), Some(target)) = (element_of(store, e, edge::SOURCE), element_of(store, e, edge::TARGET)) else { return None };
    let dragged = store.uuid(e).ok().and_then(|id| d.segment_drag.as_ref().filter(|(s, _)| *s == id)).map(|(_, route)| route);
    let points: Vec<Pos2> = if source == target {
        self_loop(d.node_rect(source), view.zoom)
    } else if let Some(route) = dragged {
        route.iter().map(|(x, y)| view.to_screen(*x, *y)).collect()
    } else {
        let rect_of = |n: Obj| model_rect(store, view, d.overrides, n);
        let via: Vec<(f64, f64)> = refs(store, e, edge::WAYPOINT).into_iter().map(|w| (real(store, w, point::X), real(store, w, point::Y))).collect();
        let way = archforge_diagram::route::way_of(store, e);
        // The other tables and notes on the diagram, which an avoiding line goes around.
        let boxes: Vec<archforge_diagram::Rect> = if way == archforge_diagram::Way::Avoiding {
            d.boxes.iter().filter(|(n, _)| *n != source && *n != target).map(|(_, r)| *r).collect()
        } else {
            Vec::new()
        };
        let at = d.spreads.get(&e).copied().unwrap_or((0.5, 0.5));
        archforge_diagram::route(way, &rect_of(source), &rect_of(target), at, &via, &boxes).into_iter().map(|(x, y)| view.to_screen(x, y)).collect()
    };
    if points.len() < 2 || points.iter().any(|q| !q.is_finite()) {
        return None;
    }
    Some(points)
}

/// A note link (owner decision): dashed and straight, from the note's edge to the
/// edge of the table it points at, or to the middle of the relationship's line.
fn draw_link(paint: &egui::Painter, d: &Drawing<'_>, e: Obj, registry: &mut widgets::Registry, ui: &egui::Ui) {
    let (p, store, z) = (d.p, d.store, d.view.zoom);
    let (Some(source), Some(target)) = (element_of(store, e, edge::SOURCE), element_of(store, e, edge::TARGET)) else { return };
    let s_rect = d.node_rect(source);
    let (from, to) = if store.class_of(target).ok() == Some(archforge_diagram::EDGE) {
        let Some(line) = edge_points(d, target) else { return };
        let middle = along(&line, 0.5);
        (exit_point(s_rect, middle), middle)
    } else {
        let t_rect = d.node_rect(target);
        (exit_point(s_rect, t_rect.center()), exit_point(t_rect, s_rect.center()))
    };
    if !from.is_finite() || !to.is_finite() {
        return;
    }
    let selected = d.selection.contains(&e);
    let ink = if selected { p.accent } else { p.text_secondary };
    let stroke = Stroke::new((1.2 * z).clamp(1.0, 2.5), ink);
    paint.extend(egui::Shape::dashed_line(&[from, to], stroke, 5.0 * z.max(0.5), 4.0 * z.max(0.5)));
    // Where it meets a relationship's line, a dot says it points at the line.
    if store.class_of(target).ok() == Some(archforge_diagram::EDGE) {
        paint.circle_filled(to, 2.5 * z.max(0.6), ink);
    }
    if selected {
        for q in [from, to] {
            let h = Rect::from_center_size(q, Vec2::splat(6.0));
            paint.rect_filled(h, 1.0, p.surface);
            paint.rect_stroke(h, 1.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
    }
    if let Ok(id) = store.uuid(e) {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}"), "edge", Rect::from_two_pos(from, to).expand(6.0));
    }
}

/// A line's end mark at `end`, the line coming from `from`. Crow's foot: the outer mark is the
/// most (a bar for one, a crow's foot for many), the inner mark the fewest (a circle for zero, a
/// bar for one), so exactly one is two bars and one or more a bar and a crow's foot; the
/// cardinality is written under the line. A triangle is drawn with its line.
#[allow(clippy::too_many_arguments, reason = "an end is drawn from its line, its mark and the view")]
pub(crate) fn end_glyph(paint: &egui::Painter, p: &Palette, end: Pos2, from: Pos2, mark: EndMark, stroke: Stroke, z: f32, with_text: bool) {
    end_glyph_on(paint, p, end, from, mark, stroke, z, with_text, p.canvas);
}

/// [`end_glyph`] over a background `fill`, which the circle for zero is filled with.
#[allow(clippy::too_many_arguments, reason = "an end is drawn from its line, its mark, the view and what it is drawn on")]
pub(crate) fn end_glyph_on(
    paint: &egui::Painter,
    p: &Palette,
    end: Pos2,
    from: Pos2,
    mark: EndMark,
    stroke: Stroke,
    z: f32,
    with_text: bool,
    fill: ux::Colour,
) {
    let (many, zero, text) = match mark {
        EndMark::ExactlyOne => (false, false, "1"),
        EndMark::ZeroOrOne => (false, true, "0..1"),
        EndMark::ZeroOrMore => (true, true, "0..*"),
        EndMark::OneOrMore => (true, false, "1..*"),
        _ => return,
    };
    let d = from - end;
    if d.length() <= f32::EPSILON || !d.is_finite() {
        return;
    }
    let d = d.normalized();
    let n = vec2(-d.y, d.x);
    if many {
        let base = end + d * 14.0 * z;
        paint.line_segment([base, end + n * 7.0 * z], stroke);
        paint.line_segment([base, end - n * 7.0 * z], stroke);
    } else {
        let bar = end + d * 10.0 * z;
        paint.line_segment([bar + n * 7.0 * z, bar - n * 7.0 * z], stroke);
    }
    // The inner mark: a circle for zero, a bar for one.
    let inner = end + d * (if many { 20.0 } else { 18.0 }) * z;
    if zero {
        paint.circle_filled(inner, 4.0 * z, fill);
        paint.circle_stroke(inner, 4.0 * z, stroke);
    } else {
        paint.line_segment([inner + n * 7.0 * z, inner - n * 7.0 * z], stroke);
    }
    if !with_text {
        return;
    }
    let g = ux::galley(paint, text, types::SMALL.scaled(z), p.text_secondary, None);
    let at = end + d * 26.0 * z + vec2(0.0, 6.0 * z);
    paint.galley(at - vec2(g.size().x / 2.0, 0.0), g, p.text_secondary.into());
}

/// The minimap: the whole diagram small, the viewport over it, and the zoom control.
fn minimap(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, bounds: Option<Rect>, view: &View) {
    let paint = ui.painter().clone();
    let m = Rect::from_min_size(pos2(r.right() - MINIMAP_GAP.x - MINIMAP.x, r.bottom() - MINIMAP_GAP.y - MINIMAP.y), MINIMAP);
    if m.left() < r.left() || m.top() < r.top() {
        return;
    }
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "workspace.minimap", "panel", m);
    ux::text(&paint, m.min + vec2(12.0, 11.0), &tr("Minimap").to_uppercase(), types::EYEBROW, p.text_secondary, Some(120.0));
    let hide = Rect::from_min_size(pos2(m.right() - 12.0 - 28.0, m.top() + 3.0), vec2(28.0, 26.0));
    paint.rect_filled(hide, 7.0, p.page);
    ux::icon_in(&paint, Rect::from_center_size(hide.center(), vec2(6.0, 1.0)), &icons::MINUS, p.text_secondary);
    if control(shell, ui, "workspace.minimap.toggle", "button", hide, true).on_hover_text(tr("Hide minimap")).clicked() {
        shell.canvas.minimap_open = false;
    }
    // The overview: the nodes and the viewport, scaled into the box.
    let bx = Rect::from_min_size(m.min + vec2(8.0, 34.0), vec2(MINIMAP.x - 16.0, 88.0));
    paint.rect_filled(bx, 6.0, p.page);
    let viewport = Rect::from_min_max(pos2_of(view.to_canvas(r.min)), pos2_of(view.to_canvas(r.max)));
    let shown = match bounds {
        Some(b) => b.union(viewport),
        None => viewport,
    };
    let inner = bx.shrink(6.0);
    let scale = (inner.width() / shown.width().max(1.0)).min(inner.height() / shown.height().max(1.0));
    let map = |q: Pos2| inner.min + (q - shown.min) * scale + (inner.size() - shown.size() * scale) / 2.0;
    let boxed = paint.with_clip_rect(bx);
    if let Some(session) = shell.session.as_ref()
        && let Some(d) = shell.canvas.active.and_then(|id| session.doc().by_uuid(id))
    {
        let store = session.doc();
        for n in refs(store, d, diagram::NODE) {
            let b = bounds_of(store, n);
            let nr = Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32));
            if !nr.is_finite() {
                continue;
            }
            let small = Rect::from_min_max(map(nr.min), map(nr.max));
            boxed.rect_filled(Rect::from_min_size(small.min, small.size().max(Vec2::splat(2.0))), 1.0, p.accent);
        }
    }
    if bounds.is_none() {
        let g = ux::galley(&paint, &tr("Empty diagram"), types::SMALL, p.text_muted, Some(inner.width()));
        boxed.galley(bx.center() - g.size() / 2.0, g, p.text_muted.into());
    }
    let vp = Rect::from_min_max(map(viewport.min), map(viewport.max));
    boxed.rect_stroke(vp, 2.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    // Dragging the viewport moves the view; a press elsewhere in the box centres the view there
    // first and goes on from there. The box keeps its scale while the drag lasts.
    widgets::record_visible(ui, &mut shell.registry, "workspace.minimap.view", "map", bx);
    let resp = ui.interact(bx, widgets::egui_id("workspace.minimap.view"), Sense::click_and_drag());
    let usable = scale > 0.0 && scale.is_finite();
    let press = ui.input(|i| i.pointer.press_origin());
    if usable
        && (resp.drag_started() || resp.clicked())
        && let Some(at) = press.or(resp.interact_pointer_pos())
    {
        let grab = if vp.contains(at) { at - vp.center() } else { Vec2::ZERO };
        let origin = inner.min + (inner.size() - shown.size() * scale) / 2.0;
        shell.canvas.minimap_drag = Some(MinimapDrag { shown, scale, origin, grab });
    }
    if let Some(d) = shell.canvas.minimap_drag
        && (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let target = d.shown.min + (pos - d.grab - d.origin) / d.scale;
        let pan = r.size() / 2.0 - target.to_vec2() * shell.canvas.zoom;
        if pan.is_finite() {
            shell.canvas.pan = pan;
        }
    }
    if resp.drag_stopped() || resp.clicked() || !(resp.dragged() || resp.is_pointer_button_down_on()) {
        shell.canvas.minimap_drag = None;
    }
    if shell.canvas.minimap_drag.is_some() && resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if resp.hover_pos().is_some_and(|q| vp.contains(q)) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // The footer: Whole diagram, then − 100 % +.
    let fit_text = ux::text(&paint, m.min + vec2(12.0, 134.0), &tr("Whole diagram"), types::SMALL, p.text_secondary, Some(90.0));
    if control(shell, ui, "workspace.minimap.fit", "button", fit_text.expand(4.0), true).clicked() {
        fit(&mut shell.canvas, r.size(), bounds, ZOOM_RANGE.0);
    }
    let plus = Rect::from_center_size(pos2(m.right() - 20.0, m.top() + 142.0), Vec2::splat(20.0));
    let minus = Rect::from_center_size(pos2(m.right() - 76.0, m.top() + 142.0), Vec2::splat(20.0));
    ux::icon(&paint, plus.center() - Vec2::splat(5.0), &icons::PLUS, 10.0, p.text_secondary);
    ux::icon_in(&paint, Rect::from_center_size(minus.center(), vec2(8.0, 1.0)), &icons::MINUS, p.text_secondary);
    let percent = fmt(&tr("{n} %"), &[("n", &format!("{}", (shell.canvas.zoom * 100.0).round() as i32))]);
    let g = ux::galley(&paint, &percent, types::SMALL, p.text, None);
    paint.galley(pos2(m.right() - 48.0 - g.size().x / 2.0, m.top() + 142.0 - g.size().y / 2.0), g, p.text.into());
    let centre = r.size() / 2.0;
    if control(shell, ui, "workspace.zoom.in", "button", plus, true).on_hover_text(tr("Zoom in")).clicked() {
        zoom_by(&mut shell.canvas, ZOOM_STEP, centre);
    }
    if control(shell, ui, "workspace.zoom.out", "button", minus, true).on_hover_text(tr("Zoom out")).clicked() {
        zoom_by(&mut shell.canvas, 1.0 / ZOOM_STEP, centre);
    }
}

fn pos2_of((x, y): (f64, f64)) -> Pos2 {
    pos2(x as f32, y as f32)
}

/// The shared tools of the toolbar: automation id, icon, hover text. Select and Pan come first, a
/// diagram kind's own tools after them, then Note and Link note.
const TOOLS: [(&str, &ux::Icon, &str); 4] = [
    ("select", &icons::TOOL_SELECT, "Select"),
    ("pan", &icons::TOOL_PAN, "Pan"),
    ("note", &icons::TOOL_NOTE, "Note"),
    ("link", &icons::TOOL_LINK_NOTE, "Link note"),
];

/// Where the toolbar's parts sit, from its left, for the tools of the open diagram's kind: Select
/// and Pan, a divider, the kind's tools (each with a chevron followed by it, a wide one twice as wide),
/// Note and Link note, a divider, the view group (the status marks where the kind has them, and
/// the grid), a divider and Arrange.
struct Layout {
    /// Each kind tool's x, and the x after them, where Note goes.
    kind_tools: Vec<f32>,
    note: f32,
    /// The view group's x and its toggles, each with its hover text.
    group: f32,
    toggles: Vec<(&'static str, String)>,
    arrange: f32,
    width: f32,
}

/// The toolbar's layout for `kind`'s tools: where each sits and how wide the bar is.
fn layout(kind: Option<&'static dyn crate::kinds::DiagramKind>) -> Layout {
    let mut x = 86.0;
    let mut kind_tools = Vec::new();
    for t in kind.map(|k| k.tools()).unwrap_or(&[]) {
        kind_tools.push(x);
        x += match (t.chevron.is_some(), t.wide) {
            (true, true) => 82.0,
            (true, false) => 50.0,
            (false, _) => 34.0,
        };
    }
    let mut toggles = Vec::new();
    if let Some(hint) = kind.and_then(|k| k.status_marks()) {
        toggles.push(("key-status", hint));
    }
    toggles.push(("grid", tr("Grid")));
    let group = x + 79.0;
    let arrange = group + 2.0 + 34.0 * toggles.len() as f32 + 6.0 + 8.0;
    Layout { kind_tools, note: x, group, toggles, arrange, width: arrange + 32.0 + 7.0 }
}

/// Whether the minimap is drawn open: when the user has it open and the canvas has room for it
/// beside the toolbar; otherwise its button stands in, so the tools are never hidden.
fn minimap_shown(shell: &Shell, r: Rect) -> bool {
    shell.canvas.minimap_open && minimap_room(shell, r)
}

/// Whether the canvas `r` has room for the minimap, open or folded: wide enough beside the toolbar
/// and tall enough for its box above the margin.
pub(crate) fn minimap_room(shell: &Shell, r: Rect) -> bool {
    let wide = r.width() >= 16.0 + layout(crate::kinds::active_kind(shell)).width + 16.0 + MINIMAP.x + MINIMAP_GAP.x;
    wide && r.height() >= MINIMAP.y + MINIMAP_GAP.y
}

/// The minimap's box on the canvas `r` while it shows, for what must keep clear of it (the
/// toast with Undo).
pub(crate) fn minimap_box(shell: &Shell, r: Rect) -> Option<Rect> {
    let m = Rect::from_min_size(pos2(r.right() - MINIMAP_GAP.x - MINIMAP.x, r.bottom() - MINIMAP_GAP.y - MINIMAP.y), MINIMAP);
    (minimap_shown(shell, r) && m.left() >= r.left() && m.top() >= r.top()).then_some(m)
}

/// The floating toolbar at the canvas's bottom centre: Select and Pan, the open diagram kind's
/// tools, Note and Link note, the status marks and grid toggles, and Arrange diagram.
fn toolbar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let kind = crate::kinds::active_kind(shell);
    // A tool of another kind, from the diagram shown before, hands back to Select.
    if let Tool::Place(key) | Tool::Connect(key) = shell.canvas.tool
        && !kind.is_some_and(|k| k.tools().iter().any(|t| t.key == key))
    {
        shell.canvas.tool = Tool::Select;
        shell.canvas.popover = None;
    }
    // A popover of another kind's chevron, from the diagram shown before, closes: hidden, it
    // would still hold the keys its menu takes.
    if let Some(open) = shell.canvas.popover
        && !kind.is_some_and(|k| k.tools().iter().any(|t| t.chevron.is_some_and(|(key, _)| key == open)))
    {
        shell.canvas.popover = None;
        shell.canvas.menu_focus = None;
    }
    let at = layout(kind);
    let size = vec2(at.width, TOOLBAR_HEIGHT);
    // Kept to the canvas: in the narrowest window the toolbar's end is cut, never drawn over a panel.
    let paint = ui.painter().with_clip_rect(r);
    // Centred under the canvas; moved left of the minimap, or of its button; never hidden.
    let minimap_left = if minimap_shown(shell, r) { r.right() - MINIMAP_GAP.x - MINIMAP.x } else { r.right() - MINIMAP_GAP.x - 28.0 };
    let x = (r.center().x - size.x / 2.0).min(minimap_left - 16.0 - size.x).max(r.left() + 16.0);
    let t = Rect::from_min_size(pos2(x, r.bottom() - TOOLBAR_GAP - size.y), size);
    if t.top() < r.top() {
        return;
    }
    ux::shadow(&paint, t, 12.0, p.shadow);
    paint.rect_filled(t, 12.0, p.surface);
    paint.rect_stroke(t, 12.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let button = |x: f32| Rect::from_min_size(pos2(t.left() + x, t.top() + 9.0), Vec2::splat(32.0));
    let divider = |x: f32| Rect::from_min_size(pos2(t.left() + x, t.top() + 15.0), vec2(1.0, 20.0));
    // The shared tools.
    for ((key, icon, hint), x) in TOOLS.iter().zip([7.0, 41.0, at.note, at.note + 34.0]) {
        let b = button(x);
        let tool = match *key {
            "select" => Tool::Select,
            "pan" => Tool::Pan,
            "note" => Tool::Note,
            _ => Tool::Link,
        };
        let active = shell.canvas.tool == tool;
        if active {
            paint.rect_filled(b, 8.0, p.accent_tint);
        }
        let looks = matches!(tool, Tool::Select | Tool::Pan) || !shell.looking_only();
        ux::icon_in(
            &paint,
            b,
            icon,
            if active {
                p.accent
            } else if looks {
                p.text_secondary
            } else {
                p.text_muted
            },
        );
        if control(shell, ui, &format!("workspace.tools.{key}"), "button", b, looks).on_hover_text(tr(hint)).clicked() {
            shell.canvas.tool = tool;
        }
    }
    // The kind's own tools; a wide one is twice as wide, so what it draws reads.
    let mut chevrons = Vec::new();
    for (tool, x) in kind.map(|k| k.tools()).unwrap_or(&[]).iter().zip(at.kind_tools.iter().copied()) {
        let b = if tool.wide { Rect::from_min_size(pos2(t.left() + x, t.top() + 9.0), vec2(64.0, 32.0)) } else { button(x) };
        let picked = match tool.use_ {
            crate::kinds::ToolUse::Place => Tool::Place(tool.key),
            crate::kinds::ToolUse::Connect => Tool::Connect(tool.key),
        };
        let active = shell.canvas.tool == picked;
        if active {
            paint.rect_filled(b, 8.0, p.accent_tint);
        }
        let ink = if active {
            p.accent
        } else if shell.looking_only() {
            p.text_muted
        } else {
            p.text_secondary
        };
        if !kind.is_some_and(|k| k.tool_face(&shell.cx_ref(), &paint, p, tool.key, b, ink, active)) {
            ux::icon_in(&paint, b, tool.icon, ink);
        }
        let hint = kind.and_then(|k| k.tool_hint(&shell.cx_ref(), tool.key)).unwrap_or_else(|| tr(tool.hint));
        // Its name for assistive technology is its tooltip: what it makes now, and which way.
        let live = !shell.looking_only();
        if control(shell, ui, &format!("workspace.tools.{}", tool.key), "button", b, live)
            .on_hover_text(hint.as_str())
            .accessible_name(hint)
            .clicked()
        {
            shell.canvas.tool = picked;
        }
        if let Some(chevron) = tool.chevron {
            let at = t.left() + x + b.width() + 2.0;
            chevrons.push((chevron, picked, Rect::from_min_size(pos2(at, t.top() + 9.0), vec2(14.0, 32.0))));
        }
    }
    for x in [75.0, at.note + 68.0] {
        paint.rect_filled(divider(x), 0.0, p.border);
    }
    // A tool's chevron opens the kind's popover for it; another chevron's closes.
    let mut open_at = None;
    for ((key, hint), picked, chevron) in chevrons {
        let open = shell.canvas.popover == Some(key);
        ux::icon_in(&paint, chevron, &icons::CHEVRON_SMALL, if open { p.accent } else { p.text_secondary });
        let live = !shell.looking_only();
        if control(shell, ui, &format!("workspace.tools.{key}"), "button", chevron, live).on_hover_text(tr(hint)).clicked() {
            shell.canvas.popover = if open { None } else { Some(key) };
            shell.canvas.menu_focus = None;
            if !open {
                shell.canvas.tool = picked;
            }
        }
        if shell.canvas.popover == Some(key) {
            open_at = Some((key, chevron));
        }
    }
    // The view group: the status marks and the grid.
    let group = Rect::from_min_size(pos2(t.left() + at.group, t.top() + 7.0), vec2(2.0 + 34.0 * at.toggles.len() as f32, 36.0));
    paint.rect_filled(group, 8.0, p.page);
    for (i, (key, hint)) in at.toggles.iter().enumerate() {
        let b = button(at.group + 2.0 + 34.0 * i as f32);
        let on = if *key == "grid" { shell.canvas.grid } else { shell.canvas.key_status };
        if on {
            paint.rect_filled(b, 8.0, p.accent_tint);
        }
        let ink = if on { p.accent } else { p.text_secondary };
        if *key == "grid" {
            ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), &icons::GRID, ink);
        } else {
            let box_ = Rect::from_center_size(b.center(), vec2(14.0, 12.0));
            ux::icon_in(&paint, box_, &icons::KEY_STATUS, ink);
            paint.circle_filled(box_.min + vec2(2.5, 2.5), 2.5, p.green);
            paint.circle_filled(box_.min + vec2(11.5, 9.5), 2.5, p.key_foreign);
        }
        if control(shell, ui, &format!("workspace.tools.{key}"), "toggle", b, true).on_hover_text(hint.as_str()).clicked() {
            match *key {
                "grid" => shell.canvas.grid = !shell.canvas.grid,
                _ => shell.canvas.key_status = !shell.canvas.key_status,
            }
        }
    }
    // Arrange diagram: the boxes laid out along their lines, one undo step, then the whole diagram
    // shown.
    paint.rect_filled(divider(at.arrange - 8.0), 0.0, p.border);
    let arrange = button(at.arrange);
    let live = shell.canvas.active.is_some() && !shell.looking_only();
    let resp = control(shell, ui, "workspace.tools.arrange", "button", arrange, live).on_hover_text(tr("Arrange diagram"));
    if resp.hovered() && live {
        paint.rect_filled(arrange, 8.0, p.page);
    }
    ux::icon_in(
        &paint,
        Rect::from_center_size(arrange.center(), Vec2::splat(16.0)),
        &icons::ARRANGE_DIAGRAM,
        if live { p.text_secondary } else { p.text_muted },
    );
    if resp.clicked()
        && let Some(d) = shell.canvas.active
        && let Some(session) = shell.session.as_mut()
    {
        match session.execute("diagram.arrange", &json!({ "diagram": d.to_string() })) {
            Ok(_) => shell.canvas.fit_pending = true,
            Err(e) => shell.canvas.notice = Some(crate::panels::refusal(shell.notation_uis, &e)),
        }
    }
    if let (Some(kind), Some((key, chevron))) = (kind, open_at) {
        shell.cx(|cx| kind.popover(cx, ui, p, r, chevron, key));
    }
}

/// The height a command grows a node to, [`archforge_diagram::figure::Card::height`], is the one the
/// canvas lays a card's places out by: the last place's top, a row and the foot, or the header for
/// a card with none, whether its compartments are empty, full or both (AR-241).
#[cfg(test)]
#[test]
fn a_cards_height_is_where_the_canvas_ends_its_last_place() {
    use archforge_diagram::figure::{Card, Row, Text, Tone};
    let row = || Row { mark: None, text: "x".into(), linked: false, slanted: false, underlined: false };
    let card = |compartments: Vec<Vec<Row>>| Card {
        eyebrow: Text::said("Class"),
        name: "C".into(),
        compartments,
        more: ("", ""),
        hidden: 0,
        elsewhere: false,
        tone: Tone::Plain,
        slanted: false,
        braces: None,
        template: Vec::new(),
        owner_path: Vec::new(),
    };
    for compartments in [vec![], vec![vec![]], vec![vec![row()]], vec![vec![], vec![]], vec![vec![row(), row()], vec![], vec![row(); 3]]] {
        let c = card(compartments);
        let laid = slots(&c).last().map_or(ENTITY_HEADER, |(top, _, _)| top + ENTITY_ROW + archforge_diagram::figure::CARD_FOOT);
        assert_eq!(card_height(&c), laid, "{:?}", c.compartments.iter().map(Vec::len).collect::<Vec<_>>());
    }
}

/// Every hover text of the tables, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    TOOLS
        .iter()
        .map(|(_, _, hint)| *hint)
        .chain(["Grid", "complete", "disjoint", "overlapping", "Arrange diagram"])
        .chain(crate::kinds::KINDS.iter().flat_map(|k| k.tools().iter().flat_map(|t| std::iter::once(t.hint).chain(t.chevron.map(|(_, hint)| hint)))))
}
