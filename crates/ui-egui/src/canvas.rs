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

use archforge_diagram::grid::bounds_of;
use archforge_diagram::{DIAGRAM, Grid, LabelPlace, diagram, edge, label, node, point};
use archforge_er::Cardinality;
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::{fmt, tr, trn};
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
const ZOOM_RANGE: (f32, f32) = (0.1, 4.0);
const ZOOM_STEP: f32 = 1.25;
/// The margin "Whole diagram" leaves around the nodes.
const FIT_MARGIN: f32 = 48.0;
/// The minimap's box and the toolbar's pill.
const MINIMAP: Vec2 = vec2(200.0, 156.0);
const MINIMAP_GAP: Vec2 = vec2(16.0, 14.0);
const TOOLBAR: Vec2 = vec2(342.0, 50.0);
const TOOLBAR_GAP: f32 = 14.0;
/// An entity node: its header's height, its first row's baseline, the pitch between rows, the
/// text inset and the badge column.
pub(crate) const ENTITY_HEADER: f32 = 66.0;
pub(crate) const ENTITY_FIRST_ROW: f32 = 77.0;
pub(crate) const ENTITY_ROW: f32 = 20.0;
const ENTITY_INSET: f32 = 18.0;
const ENTITY_TEXT: f32 = 42.0;
/// Below this zoom a node shows its name only.
const DETAIL_ZOOM: f32 = 0.5;

/// The canvas tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Pan,
    /// Places a table where the canvas is clicked (slice E1).
    Entity,
    /// Drags a relationship from one table to another (slice E2).
    Relationship,
}

/// What the centre keeps between frames: the open tabs, the view and the toggles.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// The diagrams with a tab, in tab order.
    pub tabs: Vec<Uuid>,
    /// The tab whose diagram the canvas draws.
    pub active: Option<Uuid>,
    pub tool: Tool,
    /// Where diagram unit (0, 0) is, in logical pixels from the canvas's top left.
    pub pan: Vec2,
    pub zoom: f32,
    /// True until the diagram just opened has been fitted into the canvas once.
    pub fit_pending: bool,
    /// True when the active tab changed without the view: the next frame opens it through the
    /// engine, so the view's open diagram and the canvas agree.
    pub reopen_pending: bool,
    pub minimap_open: bool,
    pub grid: bool,
    pub key_status: bool,
    /// The last refusal, shown at the canvas's top left until the next action.
    pub notice: Option<String>,
    /// Editing (slice E1): a name typed in a node, the pointer gesture under way, the entity menu,
    /// and whether the canvas has the keyboard.
    pub rename: Option<crate::editing::NodeRename>,
    pub gesture: Option<crate::editing::Gesture>,
    pub menu: Option<crate::editing::EntityMenu>,
    pub focused: bool,
    /// Relationships (slice E2): the ends a new line gets, whether the cardinality popover is
    /// open, the mapping table offer and the toast with Undo.
    pub relationship: crate::connect::RelKind,
    pub popover: bool,
    pub mapping: Option<crate::connect::MappingOffer>,
    pub toast: Option<crate::connect::Toast>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            tool: Tool::Select,
            pan: Vec2::ZERO,
            zoom: 1.0,
            fit_pending: false,
            reopen_pending: false,
            minimap_open: true,
            grid: true,
            key_status: true,
            notice: None,
            rename: None,
            gesture: None,
            menu: None,
            focused: false,
            relationship: crate::connect::RelKind::default(),
            popover: false,
            mapping: None,
            toast: None,
        }
    }
}

fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

fn select(shell: &mut Shell, ids: Vec<Uuid>) {
    let ids: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    if let Err(e) = run(shell, "view.select", json!({ "ids": ids })) {
        shell.canvas.notice = Some(e);
    }
}

/// Opens a diagram in the centre: its tab, added when it has none, becomes the active one, the
/// view's open diagram follows, and the diagram is selected so Properties shows it.
pub fn open(shell: &mut Shell, id: Uuid) {
    shell.canvas.notice = None;
    if let Err(e) = run(shell, "view.open-diagram", json!({ "diagram": id.to_string() })) {
        shell.canvas.notice = Some(e);
        return;
    }
    select(shell, vec![id]);
    if !shell.canvas.tabs.contains(&id) {
        shell.canvas.tabs.push(id);
        shell.canvas.fit_pending = true;
    }
    shell.canvas.active = Some(id);
}

/// Closes a tab; when it was the active one, the tab before it (else after it) takes over.
pub fn close_tab(shell: &mut Shell, id: Uuid) {
    let Some(i) = shell.canvas.tabs.iter().position(|t| *t == id) else { return };
    shell.canvas.tabs.remove(i);
    if shell.canvas.active == Some(id) {
        let next = shell.canvas.tabs.get(i.saturating_sub(1)).or_else(|| shell.canvas.tabs.get(i)).copied();
        shell.canvas.active = None;
        if let Some(n) = next {
            open(shell, n);
        }
    }
}

/// Drops the tabs of diagrams the store no longer has.
pub fn retain_valid(state: &mut State, store: &Store) {
    state.tabs.retain(|id| store.by_uuid(*id).is_some_and(|o| store.is_alive(o)));
    if state.active.is_some_and(|a| !state.tabs.contains(&a)) {
        state.active = state.tabs.last().copied();
        state.reopen_pending = state.active.is_some();
    }
    crate::editing::retain_valid(state, store);
    if state.mapping.as_ref().is_some_and(|m| store.by_uuid(m.relationship).is_none_or(|o| !store.is_alive(o))) {
        state.mapping = None;
    }
}

/// The active diagram, when the store still has it.
fn active_diagram(shell: &Shell) -> Option<Obj> {
    let store = shell.session.as_ref()?.doc();
    let d = store.by_uuid(shell.canvas.active?)?;
    (store.is_alive(d) && store.class_of(d).ok() == Some(DIAGRAM)).then_some(d)
}

fn real(store: &Store, o: Obj, prop: archforge_mof::PropId) -> f64 {
    match store.get(o, prop) {
        Ok(Value::Real(v)) if v.is_finite() => v,
        _ => 0.0,
    }
}

fn string(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|r| store.is_alive(*r)).collect()
}

fn element_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e))
}

/// The rectangle around every node of the diagram, in diagram units, or none when it has none.
fn diagram_bounds(store: &Store, d: Obj) -> Option<Rect> {
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

/// The view's mapping between diagram units and the screen.
#[derive(Debug, Clone, Copy)]
pub(crate) struct View {
    pub origin: Pos2,
    pub pan: Vec2,
    pub zoom: f32,
}

impl View {
    pub fn to_screen(self, x: f64, y: f64) -> Pos2 {
        self.origin + self.pan + vec2(x as f32, y as f32) * self.zoom
    }
    pub fn rect(self, b: &archforge_diagram::Rect) -> Rect {
        Rect::from_min_size(self.to_screen(b.x, b.y), vec2(b.width as f32, b.height as f32) * self.zoom)
    }
    pub fn to_canvas(self, at: Pos2) -> (f64, f64) {
        let v = (at - self.origin - self.pan) / self.zoom;
        (v.x as f64, v.y as f64)
    }
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

/// Fits the whole diagram into the canvas, or shows an empty one at 100 % from its top left.
fn fit(state: &mut State, size: Vec2, bounds: Option<Rect>) {
    match bounds {
        Some(b) if b.width() > 0.0 && b.height() > 0.0 => {
            let room = (size - Vec2::splat(2.0 * FIT_MARGIN)).max(Vec2::splat(1.0));
            let z = (room.x / b.width()).min(room.y / b.height()).min(1.0).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
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

/// Draws the centre: the tab strip over the canvas.
pub fn centre(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    if std::mem::take(&mut shell.canvas.reopen_pending)
        && let Some(id) = shell.canvas.active
    {
        open(shell, id);
    }
    let tabs = Rect::from_min_size(r.min, vec2(r.width(), TABS));
    let canvas_rect = Rect::from_min_max(pos2(r.left(), tabs.bottom()), r.max);
    tab_strip(shell, ui, p, tabs);
    canvas(shell, ui, p, canvas_rect);
}

/// The tab strip: the new-diagram `+` at the left, then a tab per open diagram.
fn tab_strip(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.page);
    paint.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 1.0), r.max), 0.0, p.border);
    let add = Rect::from_min_size(pos2(r.left() + 8.0, r.top() + 4.0), Vec2::splat(28.0));
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
    let live = shell.session.is_some();
    if control(shell, ui, "workspace.tabs.add", "button", add, live).on_hover_text(tr("New diagram")).clicked() {
        crate::diagrams::open_new(shell);
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let tabs: Vec<(Uuid, String)> = shell
        .canvas
        .tabs
        .iter()
        .filter_map(|id| {
            let d = store.by_uuid(*id)?;
            let name = crate::diagrams::diagram_name(store, d);
            Some((*id, if name.is_empty() { tr("Unnamed diagram") } else { name }))
        })
        .collect();
    let active = shell.canvas.active;
    let strip = Rect::from_min_max(pos2(r.left() + FIRST_TAB, r.top()), r.max);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(strip));
    clip.set_clip_rect(strip);
    let mut x = strip.left();
    let mut action: Option<(Uuid, bool)> = None;
    for (id, name) in &tabs {
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
        ux::icon_in(&paint, Rect::from_min_size(pos2(tab.left() + 11.0, tab.top() + 9.0), Vec2::splat(15.0)), &icons::ER_DIAGRAM, p.text_secondary);
        paint.galley(pos2(tab.left() + 34.0, tab.top() + (TAB_HEIGHT - g.size().y) / 2.0), g, ink);
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
/// toolbar, with the pointer panning, zooming and selecting.
fn canvas(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.canvas);
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return;
    }
    let Some(d) = active_diagram(shell) else {
        return;
    };
    widgets::record_visible(ui, &mut shell.registry, "workspace.canvas", "canvas", r);
    let bounds = shell.session.as_ref().map(|s| diagram_bounds(s.doc(), d)).unwrap_or_default();
    if shell.canvas.fit_pending {
        fit(&mut shell.canvas, r.size(), bounds);
        shell.canvas.fit_pending = false;
    }
    // The pointer: the pan tool or the middle button drag the view, the wheel scrolls it, a pinch
    // or the wheel with the command key zoom it about the pointer, a click selects what it hits.
    let resp = ui.interact(r, widgets::egui_id("workspace.canvas"), Sense::click_and_drag());
    match shell.canvas.tool {
        Tool::Pan => {
            resp.clone().on_hover_cursor(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
        }
        Tool::Entity | Tool::Relationship => {
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
            zoom_about(&mut shell.canvas, pinch, at);
        } else if scroll != Vec2::ZERO {
            shell.canvas.pan += scroll;
        }
    }
    if !shell.canvas.pan.is_finite() {
        shell.canvas.pan = Vec2::ZERO;
    }
    let view = View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    let clip = paint.with_clip_rect(r);
    if shell.canvas.grid {
        grid_dots(&clip, p, r, &view);
    }
    let selection: Vec<Obj> = shell.session.as_ref().map(|s| s.view().selection.clone()).unwrap_or_default();
    // A gesture under way is previewed: its nodes draw where the pointer has them.
    let pointer = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos()));
    let overrides = crate::editing::preview_rects(shell, view, pointer);
    let renaming = shell.canvas.rename.as_ref().map(|rn| rn.node);
    let grid = shell.session.as_ref().map(|s| Grid::build(s.doc(), d));
    let mut pending_badges: Vec<(Pos2, Uuid)> = Vec::new();
    if let (Some(session), Some(grid)) = (shell.session.as_ref(), grid.as_ref()) {
        let store = session.doc();
        let (x0, y0) = view.to_canvas(r.min);
        let (x1, y1) = view.to_canvas(r.max);
        let mut visible = grid.visible(archforge_diagram::Rect::new(x0, y0, x1 - x0, y1 - y0));
        // Nodes dragged into view draw too.
        for n in refs(store, d, diagram::NODE) {
            if !visible.nodes.contains(&n) && store.uuid(n).ok().is_some_and(|id| overrides.contains_key(&id)) {
                visible.nodes.push(n);
            }
        }
        let mut badges: Vec<(Pos2, Uuid)> = Vec::new();
        let draw = Drawing { p, store, view: &view, selection: &selection, overrides: &overrides, key_status: shell.canvas.key_status, renaming };
        for e in &visible.edges {
            if let Some(badge) = draw_edge(&clip, &draw, *e, &mut shell.registry, ui) {
                badges.push(badge);
            }
        }
        for n in &visible.nodes {
            draw_node(&clip, &draw, *n, &mut shell.registry, ui);
        }
        pending_badges = badges;
    }
    for (at, rel) in pending_badges {
        crate::connect::badge(shell, ui, p, at, rel);
    }
    if let Some(grid) = grid.as_ref() {
        crate::editing::pointer(shell, ui, &resp, view, grid, d);
    }
    crate::editing::overlays(shell, ui, p, r, view, d, &overrides);
    crate::connect::mapping_offer(shell, ui, p, r);
    crate::connect::toast(shell, ui, p, r);
    if shell.canvas.minimap_open {
        minimap(shell, ui, p, r, bounds, &view);
    } else {
        let b = Rect::from_min_size(pos2(r.right() - MINIMAP_GAP.x - 28.0, r.bottom() - MINIMAP_GAP.y - 26.0), vec2(28.0, 26.0));
        paint.rect_filled(b, 7.0, p.surface);
        paint.rect_stroke(b, 7.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), &icons::MAP, p.text_secondary);
        if control(shell, ui, "workspace.minimap.toggle", "button", b, true).on_hover_text(tr("Show minimap")).clicked() {
            shell.canvas.minimap_open = true;
        }
    }
    toolbar(shell, ui, p, r);
    if let Some(notice) = shell.canvas.notice.clone() {
        ux::text(&paint, r.min + vec2(16.0, 12.0), &notice, types::CAPTION, p.amber, Some((r.width() - 32.0).max(0.0)));
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

/// The badge of a column: PK when it is in the primary key, FK when a foreign key holds it.
fn badge(store: &Store, entity: Obj, column: Obj) -> Option<&'static str> {
    if archforge_er::rules::in_primary_key(store, column) {
        return Some("PK");
    }
    let foreign =
        refs(store, entity, archforge_er::entity::FOREIGN_KEY).iter().any(|fk| refs(store, *fk, archforge_er::foreign_key::COLUMN).contains(&column));
    foreign.then_some("FK")
}

/// What one frame's drawing reads: the palette, the store, the view, the selection, the rectangles
/// a gesture previews, the key status toggle and the node whose name is being typed.
struct Drawing<'a> {
    p: &'a Palette,
    store: &'a Store,
    view: &'a View,
    selection: &'a [Obj],
    overrides: &'a std::collections::BTreeMap<Uuid, Rect>,
    key_status: bool,
    renaming: Option<Uuid>,
}

impl Drawing<'_> {
    /// A node's rectangle on screen: the gesture's preview when it has one, else as stored.
    fn node_rect(&self, n: Obj) -> Rect {
        self.store.uuid(n).ok().and_then(|id| self.overrides.get(&id).copied()).unwrap_or_else(|| self.view.rect(&bounds_of(self.store, n)))
    }
}

/// One node: an entity card with its eyebrow, name and columns, a note, or a plain card.
fn draw_node(paint: &egui::Painter, d: &Drawing<'_>, n: Obj, registry: &mut widgets::Registry, ui: &egui::Ui) {
    let (p, store, view, selection, key_status) = (d.p, d.store, d.view, d.selection, d.key_status);
    let rect = d.node_rect(n);
    if !rect.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let z = view.zoom;
    let id = store.uuid(n).ok();
    if let Some(id) = id {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}"), "node", rect);
    }
    let renaming = id.is_some() && id == d.renaming;
    let element = element_of(store, n, node::ELEMENT);
    let selected = selection.contains(&n) || element.is_some_and(|e| selection.contains(&e));
    let inner = paint.with_clip_rect(rect.intersect(paint.clip_rect()));
    match element {
        Some(e) if store.class_of(e).ok() == Some(archforge_er::ENTITY) => {
            paint.rect_filled(rect, 6.0 * z, p.entity);
            paint.rect_stroke(rect, 6.0 * z, Stroke::new(1.0, p.entity_edge), StrokeKind::Inside);
            let name = archforge_er::rules::name_of(store, e);
            let room = Some((rect.width() - 2.0 * ENTITY_INSET * z).max(0.0));
            if z >= DETAIL_ZOOM {
                ux::text(&inner, rect.min + vec2(ENTITY_INSET, 12.0) * z, &tr("Entity").to_uppercase(), types::EYEBROW.scaled(z), p.entity_ink, room);
                if !renaming {
                    ux::text(&inner, rect.min + vec2(ENTITY_INSET, 30.0) * z, &name, types::SECTION.scaled(z), p.text, room);
                }
                let line_y = rect.top() + ENTITY_HEADER * z;
                if line_y < rect.bottom() {
                    inner.rect_filled(Rect::from_min_size(pos2(rect.left(), line_y), vec2(rect.width(), 1.0)), 0.0, p.entity_line);
                }
                let columns = refs(store, e, archforge_er::entity::ATTRIBUTE);
                for (i, column) in columns.iter().enumerate() {
                    let y = rect.top() + (ENTITY_FIRST_ROW + i as f32 * ENTITY_ROW) * z;
                    let last_shown = i + 1 < columns.len() && y + 2.0 * ENTITY_ROW * z > rect.bottom();
                    if y + ENTITY_ROW * z > rect.bottom() + z {
                        break;
                    }
                    if last_shown {
                        // The columns past the node's height, said in one row.
                        let more = columns.len() - i;
                        let text = trn("+ {n} more column", "+ {n} more columns", more as u64);
                        ux::text(&inner, pos2(rect.left() + ENTITY_TEXT * z, y), &text, types::UI.scaled(z), p.text_muted, room);
                        break;
                    }
                    if key_status && let Some(b) = badge(store, e, *column) {
                        ux::text(&inner, pos2(rect.left() + ENTITY_INSET * z, y + z), b, types::CAPTION.scaled(z), p.entity_ink, None);
                    }
                    let ty = string(store, *column, archforge_er::attribute::TYPE);
                    let text = if ty.is_empty() {
                        archforge_er::rules::name_of(store, *column)
                    } else {
                        format!("{} : {ty}", archforge_er::rules::name_of(store, *column))
                    };
                    let text_x = rect.left() + ENTITY_TEXT * z;
                    ux::text(&inner, pos2(text_x, y), &text, types::UI.scaled(z), p.text, Some((rect.right() - ENTITY_INSET * z - text_x).max(0.0)));
                }
            } else {
                let g = ux::galley(&inner, &name, types::SECTION.scaled(z.max(DETAIL_ZOOM)), p.text, room);
                inner.galley(rect.center() - g.size() / 2.0, g, p.text);
            }
        }
        Some(e) => {
            paint.rect_filled(rect, 6.0 * z, p.surface);
            paint.rect_stroke(rect, 6.0 * z, Stroke::new(1.0, p.border), StrokeKind::Inside);
            let name = archforge_er::rules::name_of(store, e);
            ux::text(
                &inner,
                rect.min + vec2(ENTITY_INSET, 12.0) * z,
                &name,
                types::UI.scaled(z),
                p.text,
                Some((rect.width() - 2.0 * ENTITY_INSET * z).max(0.0)),
            );
        }
        None => {
            // A note: its own text on the amber tint.
            paint.rect_filled(rect, 4.0 * z, p.amber_tint);
            paint.rect_stroke(rect, 4.0 * z, Stroke::new(1.0, p.amber), StrokeKind::Inside);
            let text = string(store, n, node::TEXT);
            ux::text(&inner, rect.min + vec2(12.0, 10.0) * z, &text, types::UI.scaled(z), p.text, Some((rect.width() - 24.0 * z).max(0.0)));
        }
    }
    if selected {
        selection_ring(paint, p, rect);
    }
}

/// The accent ring and the four corner handles of a selected node.
fn selection_ring(paint: &egui::Painter, p: &Palette, rect: Rect) {
    let ring = rect.expand(4.0);
    paint.rect_stroke(ring, 4.0, Stroke::new(1.5, p.accent), StrokeKind::Inside);
    for corner in [ring.left_top(), ring.right_top(), ring.left_bottom(), ring.right_bottom()] {
        let h = Rect::from_center_size(corner, Vec2::splat(6.0));
        paint.rect_filled(h, 1.0, p.surface);
        paint.rect_stroke(h, 1.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    }
}

/// Where a line from a rectangle's centre towards `toward` leaves the rectangle.
pub(crate) fn exit_point(rect: Rect, toward: Pos2) -> Pos2 {
    let c = rect.center();
    let d = toward - c;
    if d == Vec2::ZERO || !d.is_finite() {
        return c;
    }
    let tx = if d.x.abs() > f32::EPSILON { (rect.width() / 2.0) / d.x.abs() } else { f32::INFINITY };
    let ty = if d.y.abs() > f32::EPSILON { (rect.height() / 2.0) / d.y.abs() } else { f32::INFINITY };
    let t = tx.min(ty);
    if t.is_finite() { c + d * t } else { c }
}

/// The text of a relationship's slot a label shows.
fn slot_text(store: &Store, rel: Obj, kind: &str) -> String {
    use archforge_er::relationship as r;
    match kind {
        "name" => archforge_er::rules::name_of(store, rel),
        "from_end" => archforge_er::rules::cardinality_of(store, rel, r::FROM_END).map(cardinality_text).unwrap_or_default(),
        "to_end" => archforge_er::rules::cardinality_of(store, rel, r::TO_END).map(cardinality_text).unwrap_or_default(),
        "from_role" => string(store, rel, r::FROM_ROLE),
        "to_role" => string(store, rel, r::TO_ROLE),
        _ => String::new(),
    }
}

/// A cardinality as the design writes it beside an end.
pub(crate) fn cardinality_text(c: Cardinality) -> String {
    match c {
        Cardinality::ZeroOrOne => "0..1",
        Cardinality::ExactlyOne => "1",
        Cardinality::ZeroOrMore => "0..*",
        Cardinality::OneOrMore => "1..*",
    }
    .to_string()
}

/// A point along a polyline at `t` of its length, 0 at its start and 1 at its end.
fn along(points: &[Pos2], t: f32) -> Pos2 {
    let total: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if points.is_empty() {
        return Pos2::ZERO;
    }
    if total <= 0.0 || points.len() == 1 {
        return points[0];
    }
    let mut left = (total * t.clamp(0.0, 1.0)).max(0.0);
    for w in points.windows(2) {
        let len = (w[1] - w[0]).length();
        if left <= len {
            return w[0] + (w[1] - w[0]) * if len > 0.0 { left / len } else { 0.0 };
        }
        left -= len;
    }
    points[points.len() - 1]
}

/// One edge: the line between its nodes' borders through its waypoints, the ER end glyphs with
/// their cardinalities, and its labels.
/// Gives the point and relationship of a many-to-many badge to draw, when the line needs one.
fn draw_edge(paint: &egui::Painter, d: &Drawing<'_>, e: Obj, registry: &mut widgets::Registry, ui: &egui::Ui) -> Option<(Pos2, Uuid)> {
    let (p, store, view, selection) = (d.p, d.store, d.view, d.selection);
    let (Some(source), Some(target)) = (element_of(store, e, edge::SOURCE), element_of(store, e, edge::TARGET)) else { return None };
    let z = view.zoom;
    let s_rect = d.node_rect(source);
    let t_rect = d.node_rect(target);
    let element = element_of(store, e, edge::ELEMENT);
    let relationship = element.filter(|el| store.class_of(*el).ok() == Some(archforge_er::RELATIONSHIP));
    let mut points: Vec<Pos2> = if source == target {
        // A relationship of a table with itself loops out of its right side.
        crate::connect::self_loop(s_rect, z)
    } else {
        let mut points = vec![s_rect.center()];
        for w in refs(store, e, edge::WAYPOINT) {
            points.push(view.to_screen(real(store, w, point::X), real(store, w, point::Y)));
        }
        points.push(t_rect.center());
        points
    };
    if points.iter().any(|q| !q.is_finite()) {
        return None;
    }
    let n = points.len();
    if source != target {
        points[0] = exit_point(s_rect, points[1]);
        points[n - 1] = exit_point(t_rect, points[n - 2]);
    }
    let selected = selection.contains(&e) || element.is_some_and(|el| selection.contains(&el));
    // The line's colour says its key status while the toggle is on: green with its foreign key,
    // orange without one or while many-to-many waits for its mapping table.
    let (identifying, many_to_many, with_key) = match relationship {
        Some(rel) => (
            matches!(store.get(rel, archforge_er::relationship::IDENTIFYING), Ok(Value::Bool(true))),
            archforge_er::rules::is_many_to_many(store, rel),
            crate::connect::has_foreign_key(store, rel),
        ),
        None => (true, false, true),
    };
    let ink = if selected {
        p.accent
    } else if !d.key_status {
        p.text_secondary
    } else if with_key && !many_to_many {
        p.green
    } else {
        p.key_foreign
    };
    let stroke = Stroke::new((1.5 * z).clamp(1.0, 3.0), ink);
    crate::connect::line(paint, &points, stroke, identifying, z);
    let mut badge = None;
    if many_to_many
        && let Some(rel) = relationship
        && let Ok(id) = store.uuid(rel)
    {
        badge = Some((along(&points, 0.5) + vec2(0.0, -14.0 * z.max(0.5)), id));
    }
    if let Ok(id) = store.uuid(e) {
        let bounds = Rect::from_points(&points).expand(6.0);
        widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}"), "edge", bounds);
    }
    // The ends: the relationship's cardinalities as glyphs and as text.
    if let Some(rel) = element.filter(|el| store.class_of(*el).ok() == Some(archforge_er::RELATIONSHIP)) {
        let from = archforge_er::rules::cardinality_of(store, rel, archforge_er::relationship::FROM_END);
        let to = archforge_er::rules::cardinality_of(store, rel, archforge_er::relationship::TO_END);
        end_glyph(paint, p, points[0], points[1], from, stroke, z, true);
        end_glyph(paint, p, points[n - 1], points[n - 2], to, stroke, z, true);
        // The roles, above the line near their ends.
        for (role, end, towards) in
            [(archforge_er::relationship::FROM_ROLE, points[0], points[1]), (archforge_er::relationship::TO_ROLE, points[n - 1], points[n - 2])]
        {
            let text = string(store, rel, role);
            if text.is_empty() || (towards - end).length() <= f32::EPSILON {
                continue;
            }
            let dir = (towards - end).normalized();
            let g = ux::galley(paint, &text, types::SMALL.scaled(z), p.text_secondary, None);
            let at = end + dir * 44.0 * z - vec2(g.size().x / 2.0, g.size().y + 8.0 * z);
            paint.galley(at, g, p.text_secondary);
        }
        for l in refs(store, e, edge::LABEL) {
            let kind = string(store, l, label::KIND);
            let text = slot_text(store, rel, &kind);
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
            let offset = if t == 0.5 { vec2(-g.size().x / 2.0, -g.size().y - 6.0 * z) } else { vec2(-g.size().x / 2.0, 6.0 * z) };
            paint.galley(at + offset, g, p.text_secondary);
        }
    }
    if selected {
        for q in [points[0], points[n - 1]] {
            let h = Rect::from_center_size(q, Vec2::splat(6.0));
            paint.rect_filled(h, 1.0, p.surface);
            paint.rect_stroke(h, 1.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
    }
    badge
}

/// An ER end at `end`, the line coming from `from`: a bar for one, a crow's foot for many, a
/// circle for zero, and the cardinality written under the line.
#[allow(clippy::too_many_arguments, reason = "an end is drawn from its line, its cardinality and the view")]
pub(crate) fn end_glyph(paint: &egui::Painter, p: &Palette, end: Pos2, from: Pos2, c: Option<Cardinality>, stroke: Stroke, z: f32, with_text: bool) {
    let Some(c) = c else { return };
    let d = from - end;
    if d.length() <= f32::EPSILON || !d.is_finite() {
        return;
    }
    let d = d.normalized();
    let n = vec2(-d.y, d.x);
    let many = matches!(c, Cardinality::ZeroOrMore | Cardinality::OneOrMore);
    let zero = matches!(c, Cardinality::ZeroOrOne | Cardinality::ZeroOrMore);
    if many {
        let base = end + d * 14.0 * z;
        paint.line_segment([base, end + n * 7.0 * z], stroke);
        paint.line_segment([base, end - n * 7.0 * z], stroke);
    } else {
        let bar = end + d * 10.0 * z;
        paint.line_segment([bar + n * 7.0 * z, bar - n * 7.0 * z], stroke);
    }
    if zero {
        let centre = end + d * (if many { 20.0 } else { 18.0 }) * z;
        paint.circle_filled(centre, 4.0 * z, p.canvas);
        paint.circle_stroke(centre, 4.0 * z, stroke);
    }
    if !with_text {
        return;
    }
    let text = cardinality_text(c);
    let g = ux::galley(paint, &text, types::SMALL.scaled(z), p.text_secondary, None);
    let at = end + d * 26.0 * z + vec2(0.0, 6.0 * z);
    paint.galley(at - vec2(g.size().x / 2.0, 0.0), g, p.text_secondary);
}

/// The minimap: the whole diagram small, the viewport over it, and the zoom control.
fn minimap(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, bounds: Option<Rect>, view: &View) {
    let paint = ui.painter().clone();
    let m = Rect::from_min_size(pos2(r.right() - MINIMAP_GAP.x - MINIMAP.x, r.bottom() - MINIMAP_GAP.y - MINIMAP.y), MINIMAP);
    if m.left() < r.left() || m.top() < r.top() {
        return;
    }
    paint.rect_filled(m.expand(2.0), 10.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
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
        boxed.galley(bx.center() - g.size() / 2.0, g, p.text_muted);
    }
    let vp = Rect::from_min_max(map(viewport.min), map(viewport.max));
    boxed.rect_stroke(vp, 2.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    // A click in the box centres the view there.
    let resp = control(shell, ui, "workspace.minimap.view", "map", bx, true);
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && scale > 0.0
        && scale.is_finite()
    {
        let centre_offset = (inner.size() - shown.size() * scale) / 2.0;
        let target = shown.min + (pos - inner.min - centre_offset) / scale;
        shell.canvas.pan = r.size() / 2.0 - target.to_vec2() * shell.canvas.zoom;
    }
    // The footer: Whole diagram, then − 100 % +.
    let fit_text = ux::text(&paint, m.min + vec2(12.0, 134.0), &tr("Whole diagram"), types::SMALL, p.text_secondary, Some(90.0));
    if control(shell, ui, "workspace.minimap.fit", "button", fit_text.expand(4.0), true).clicked() {
        fit(&mut shell.canvas, r.size(), bounds);
    }
    let plus = Rect::from_center_size(pos2(m.right() - 20.0, m.top() + 142.0), Vec2::splat(20.0));
    let minus = Rect::from_center_size(pos2(m.right() - 76.0, m.top() + 142.0), Vec2::splat(20.0));
    ux::icon(&paint, plus.center() - Vec2::splat(5.0), &icons::PLUS, 10.0, p.text_secondary);
    ux::icon_in(&paint, Rect::from_center_size(minus.center(), vec2(8.0, 1.0)), &icons::MINUS, p.text_secondary);
    let percent = fmt(&tr("{n} %"), &[("n", &format!("{}", (shell.canvas.zoom * 100.0).round() as i32))]);
    let g = ux::galley(&paint, &percent, types::SMALL, p.text, None);
    paint.galley(pos2(m.right() - 48.0 - g.size().x / 2.0, m.top() + 142.0 - g.size().y / 2.0), g, p.text);
    let centre = r.size() / 2.0;
    if control(shell, ui, "workspace.zoom.in", "button", plus, true).on_hover_text(tr("Zoom in")).clicked() {
        zoom_about(&mut shell.canvas, ZOOM_STEP, centre);
    }
    if control(shell, ui, "workspace.zoom.out", "button", minus, true).on_hover_text(tr("Zoom out")).clicked() {
        zoom_about(&mut shell.canvas, 1.0 / ZOOM_STEP, centre);
    }
}

fn pos2_of((x, y): (f64, f64)) -> Pos2 {
    pos2(x as f32, y as f32)
}

/// The tools of the toolbar: automation id, icon, hover text, whether it works in this slice.
const TOOLS: [(&str, &ux::Icon, &str, bool); 6] = [
    ("select", &icons::TOOL_SELECT, "Select", true),
    ("pan", &icons::TOOL_PAN, "Pan", true),
    ("entity", &icons::TOOL_ENTITY, "Entity", true),
    ("relationship", &icons::TOOL_RELATIONSHIP, "Relationship", true),
    ("note", &icons::TOOL_NOTE, "Note", false),
    ("group", &icons::TOOL_GROUP, "Group", false),
];

/// The floating toolbar at the canvas's bottom centre: Select and Pan, the drawing tools, and
/// the key status and grid toggles.
fn toolbar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    // Centred under the canvas; moved left when the minimap would cover it.
    let minimap_left = if shell.canvas.minimap_open { r.right() - MINIMAP_GAP.x - MINIMAP.x } else { r.right() - MINIMAP_GAP.x - 28.0 };
    let x = (r.center().x - TOOLBAR.x / 2.0).min(minimap_left - 16.0 - TOOLBAR.x).max(r.left() + 16.0);
    let t = Rect::from_min_size(pos2(x, r.bottom() - TOOLBAR_GAP - TOOLBAR.y), TOOLBAR);
    if t.right() > minimap_left - 8.0 || t.top() < r.top() {
        return;
    }
    paint.rect_filled(t.expand(2.0), 14.0, p.shadow);
    paint.rect_filled(t, 12.0, p.surface);
    paint.rect_stroke(t, 12.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let button = |x: f32| Rect::from_min_size(pos2(t.left() + x, t.top() + 9.0), Vec2::splat(32.0));
    let divider = |x: f32| Rect::from_min_size(pos2(t.left() + x, t.top() + 15.0), vec2(1.0, 20.0));
    let xs = [7.0, 41.0, 86.0, 120.0, 170.0, 204.0];
    for ((key, icon, hint, live), x) in TOOLS.iter().zip(xs) {
        let b = button(x);
        let active = matches!(
            (*key, shell.canvas.tool),
            ("select", Tool::Select) | ("pan", Tool::Pan) | ("entity", Tool::Entity) | ("relationship", Tool::Relationship)
        );
        if active {
            paint.rect_filled(b, 8.0, p.accent_tint);
        }
        let ink = if active {
            p.accent
        } else if *live {
            p.text_secondary
        } else {
            p.text_muted
        };
        ux::icon_in(&paint, b, icon, ink);
        if control(shell, ui, &format!("workspace.tools.{key}"), "button", b, *live).on_hover_text(tr(hint)).clicked() {
            match *key {
                "select" => shell.canvas.tool = Tool::Select,
                "pan" => shell.canvas.tool = Tool::Pan,
                "entity" => shell.canvas.tool = Tool::Entity,
                "relationship" => shell.canvas.tool = Tool::Relationship,
                _ => {}
            }
        }
    }
    for x in [75.0, 254.0] {
        paint.rect_filled(divider(x), 0.0, p.border);
    }
    let mut chevron = None;
    for (key, x, hint, live) in [("relationship-types", 154.0, "Relationship types", true), ("group-kinds", 238.0, "Group kinds", false)] {
        let b = Rect::from_min_size(pos2(t.left() + x, t.top() + 9.0), vec2(14.0, 32.0));
        let open = live && shell.canvas.popover;
        ux::icon_in(
            &paint,
            b,
            &icons::CHEVRON_SMALL,
            if open {
                p.accent
            } else if live {
                p.text_secondary
            } else {
                p.text_muted
            },
        );
        if control(shell, ui, &format!("workspace.tools.{key}"), "button", b, live).on_hover_text(tr(hint)).clicked() && live {
            shell.canvas.popover = !shell.canvas.popover;
            if shell.canvas.popover {
                shell.canvas.tool = Tool::Relationship;
            }
        }
        if live {
            chevron = Some(b);
        }
    }
    // The view group: key status and grid.
    let group = Rect::from_min_size(pos2(t.left() + 265.0, t.top() + 7.0), vec2(70.0, 36.0));
    paint.rect_filled(group, 8.0, p.page);
    for (key, x, hint, on) in [("key-status", 267.0, "Key status", shell.canvas.key_status), ("grid", 301.0, "Grid", shell.canvas.grid)] {
        let b = button(x);
        if on {
            paint.rect_filled(b, 8.0, p.accent_tint);
        }
        let ink = if on { p.accent } else { p.text_secondary };
        if key == "grid" {
            ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), &icons::GRID, ink);
        } else {
            let box_ = Rect::from_center_size(b.center(), vec2(14.0, 12.0));
            ux::icon_in(&paint, box_, &icons::KEY_STATUS, ink);
            paint.circle_filled(box_.min + vec2(2.5, 2.5), 2.5, p.green);
            paint.circle_filled(box_.min + vec2(11.5, 9.5), 2.5, p.key_foreign);
        }
        if control(shell, ui, &format!("workspace.tools.{key}"), "toggle", b, true).on_hover_text(tr(hint)).clicked() {
            match key {
                "grid" => shell.canvas.grid = !shell.canvas.grid,
                _ => shell.canvas.key_status = !shell.canvas.key_status,
            }
        }
    }
    if shell.canvas.popover
        && let Some(anchor) = chevron
    {
        crate::connect::popover(shell, ui, p, r, anchor);
    }
}

/// Every hover text of the tables, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    TOOLS.iter().map(|(_, _, hint, _)| *hint).chain(["Relationship types", "Group kinds", "Key status", "Grid"])
}
