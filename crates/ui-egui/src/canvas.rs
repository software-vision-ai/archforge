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

use archforge_diagram::figure::EndMark;
use archforge_diagram::grid::bounds_of;
use archforge_diagram::{DIAGRAM, Grid, LabelPlace, diagram, edge, label, node, point, style};
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
/// An entity node: its header's height, its first row's baseline, the pitch between rows, the
/// text inset and the badge column.
pub(crate) const ENTITY_HEADER: f32 = 66.0;
pub(crate) const ENTITY_FIRST_ROW: f32 = 77.0;
pub(crate) const ENTITY_ROW: f32 = 20.0;
const ENTITY_INSET: f32 = 18.0;
pub(crate) const ENTITY_TEXT: f32 = 42.0;
/// How strongly a table of another schema shows: the design's 60 %.
const FOREIGN_ALPHA: f32 = 0.6;
/// Below this zoom a node shows its name only.
pub(crate) const DETAIL_ZOOM: f32 = 0.5;

/// The canvas tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Pan,
    /// A diagram kind's tool that places an element where the canvas is clicked (ER's Entity,
    /// slice E1), by its key.
    Place(&'static str),
    /// A diagram kind's tool that drags a line from one element to another (ER's Relationship,
    /// slice E2), by its key.
    Connect(&'static str),
    /// Places a note where the canvas is clicked (slice E4).
    Note,
    /// Drags a link from a note to a table or a relationship's line.
    Link,
}

/// A drag of the minimap's viewport: the minimap's mapping frozen as the drag began (the part of
/// the diagram it shows, its scale and where that sits on screen), so the box does not rescale under
/// the pointer, and where on the viewport the pointer took hold of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapDrag {
    shown: Rect,
    scale: f32,
    origin: Pos2,
    grab: Vec2,
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
    /// A node the next frame brings to the canvas's centre (what the search found).
    pub centre_on: Option<Uuid>,
    /// True when the active tab changed without the view: the next frame opens it through the
    /// engine, so the view's open diagram and the canvas agree.
    pub reopen_pending: bool,
    pub minimap_open: bool,
    /// The minimap's viewport being dragged: its mapping as it was when the drag began.
    pub minimap_drag: Option<MinimapDrag>,
    pub grid: bool,
    pub key_status: bool,
    /// The last refusal, shown at the canvas's top left until the next action.
    pub notice: Option<String>,
    /// Editing (slice E1): a name typed in a node, the pointer gesture under way, the entity menu,
    /// and whether the canvas has the keyboard.
    pub rename: Option<crate::editing::NodeRename>,
    /// A class's attribute or operation typed in place on its card.
    pub(crate) inline: Option<crate::uml_inline::InlineMember>,
    pub gesture: Option<crate::editing::Gesture>,
    pub menu: Option<crate::editing::EntityMenu>,
    pub focused: bool,
    /// Relationships (slice E2): the ends a new line gets, the chevron whose popover is open (the
    /// cardinality popover, a menu of kinds), the mapping table offer and the toast with Undo.
    pub relationship: crate::connect::RelKind,
    pub popover: Option<&'static str>,
    /// The row of the open popover's menu the arrow keys are on, when they have moved.
    pub(crate) menu_focus: Option<usize>,
    /// A press closed the classifier's menu of kinds: the canvas ignores that press until the
    /// button lets go, so it places, selects and drags nothing.
    pub(crate) dismissing: bool,
    /// What a class diagram's Relationship tool draws.
    pub uml_relation: crate::uml_relate::Relate,
    /// What a class diagram's classifier tool places: kept while the window lives, a new window
    /// starting at Class.
    pub uml_classifier: crate::uml_classifier::Classifier,
    pub mapping: Option<crate::connect::MappingOffer>,
    pub toast: Option<crate::connect::Toast>,
    /// A note's text being typed in place (slice E4).
    pub note_edit: Option<crate::editing::NoteEdit>,
    /// The texts of the skills with a tab, as typed.
    pub skills: Vec<crate::skill_tab::Doc>,
    /// The cardinality texts the last frame drew, where a press grabs one.
    pub ends: Vec<EndText>,
    /// The packages' tabs the last frame drew, which take a press as their cards do (AR-113).
    pub package_tabs: Vec<PackageTab>,
    /// Moving between a package's diagram and its parent's (AR-95).
    pub(crate) nav: crate::package_nav::Nav,
}

/// A cardinality text as drawn: its line, which end (0 the line's start, 1 its end), where it
/// was and the end of the line it belongs to, on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct EndText {
    pub edge: Uuid,
    pub end: u8,
    pub rect: Rect,
    pub anchor: Pos2,
}

/// A package's tab as drawn (AR-113): where it was on screen, its node, and the whole name when
/// the tab could show it only cut.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageTab {
    pub rect: Rect,
    pub node: Uuid,
    pub cut: Option<String>,
}

impl Default for State {
    /// A canvas with no diagram open: Select, the grid and minimap on, no menu open, Class as the classifier kind.
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            tool: Tool::Select,
            pan: Vec2::ZERO,
            zoom: 1.0,
            fit_pending: false,
            centre_on: None,
            reopen_pending: false,
            minimap_open: true,
            minimap_drag: None,
            grid: true,
            key_status: true,
            notice: None,
            rename: None,
            inline: None,
            gesture: None,
            menu: None,
            focused: false,
            relationship: crate::connect::RelKind::default(),
            popover: None,
            menu_focus: None,
            dismissing: false,
            uml_relation: crate::uml_relate::Relate::default(),
            uml_classifier: crate::uml_classifier::Classifier::default(),
            mapping: None,
            toast: None,
            note_edit: None,
            skills: Vec::new(),
            ends: Vec::new(),
            package_tabs: Vec::new(),
            nav: crate::package_nav::Nav::default(),
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
/// view's open diagram follows, and the diagram is selected so Properties shows it. A skill opens
/// in its own tab the same way, with its text in place of the canvas.
pub fn open(shell: &mut Shell, id: Uuid) {
    if shell.session.as_ref().is_some_and(|s| crate::skill_tab::is_skill(s.doc(), id)) {
        crate::skill_tab::open(shell, id);
        return;
    }
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

/// Closes a tab; when it was the active one, the tab before it (else after it) takes over. A
/// skill's tab whose text the store refused stays open on it, showing why, so nothing typed is lost.
pub fn close_tab(shell: &mut Shell, id: Uuid) {
    let Some(i) = shell.canvas.tabs.iter().position(|t| *t == id) else { return };
    crate::skill_tab::sync(shell, 0.0, true, Some(id));
    if shell.canvas.skills.iter().any(|d| d.skill == id && d.text != d.saved) {
        crate::skill_tab::open(shell, id);
        return;
    }
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

/// A node's or an edge's own look on this diagram (the Appearance panel): its colours and width,
/// each none when the diagram's look holds.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Look {
    pub fill: Option<egui::Color32>,
    pub stroke: Option<egui::Color32>,
    pub text: Option<egui::Color32>,
    /// The border's or the line's width; none for the diagram's.
    pub width: Option<f32>,
}

/// A colour `#rrggbb` as drawn, or none for anything else.
pub(crate) fn colour(hex: &str) -> Option<egui::Color32> {
    let h = hex.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii())?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some(egui::Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?))
}

/// A colour as `#rrggbb`.
pub(crate) fn hex(c: egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// The palette whose text reads on `fill`: Light's dark text on a light fill, Dark's light text on
/// a dark one, so a node's own fill never hides its words.
pub(crate) fn ink_for(fill: egui::Color32) -> &'static Palette {
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

/// A zoom the user makes by `factor` about `at`: the canvas zooms, and package navigation hears
/// of it.
pub(crate) fn zoom_by(state: &mut State, factor: f32, at: Vec2) {
    zoom_about(state, factor, at);
    crate::package_nav::intend(&mut state.nav, factor);
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
    match shell.canvas.active.filter(|id| shell.session.as_ref().is_some_and(|s| crate::skill_tab::is_skill(s.doc(), *id))) {
        Some(skill) => crate::skill_tab::draw(shell, ui, p, canvas_rect, skill),
        None => canvas(shell, ui, p, canvas_rect),
    }
}

/// The tab strip: the new-diagram `+` at the left, then a tab per open diagram or skill.
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
    // A zoom across a package's threshold, ⌘↓ or ⌘↑ moves to another level (AR-95).
    crate::package_nav::before_draw(shell, ui, r, d);
    let view = View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    // An element dragged from the Models explorer lands where it was let go (slice S7), as the
    // diagram's kind places it.
    if let Some((element, at)) = shell.explorer.dropped.take()
        && r.contains(at)
        && let Some(kind) = shell.session.as_ref().and_then(|s| crate::kinds::kind_of(shell, s.doc(), d))
    {
        kind.drop(shell, d, element, view.to_canvas(at));
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
    let grid = shell.session.as_ref().map(|s| Grid::build(s.doc(), d));
    let mut pending_badges: Vec<(Pos2, Uuid, Uuid, Obj, bool)> = Vec::new();
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
        let mut badges: Vec<(Pos2, Uuid, Uuid, Obj, bool)> = Vec::new();
        let draw = Drawing {
            p,
            store,
            view: &view,
            selection: &selection,
            overrides: &overrides,
            key_status: shell.canvas.key_status,
            renaming,
            problems: &shell.problems,
            diagram: d,
            notations: session.view(),
            boxes: refs(store, d, diagram::NODE).into_iter().map(|n| (n, model_rect(store, &view, &overrides, n))).collect(),
            spreads: archforge_diagram::route::spreads(store, d, |n| model_rect(store, &view, &overrides, n)),
            // A line under the pointer, while no gesture runs and the pointer is on the canvas.
            hovered: ui
                .input(|i| i.pointer.hover_pos())
                .filter(|at| r.contains(*at) && shell.canvas.gesture.is_none() && ui.rect_contains_pointer(r))
                .and_then(|at| {
                    let (x, y) = view.to_canvas(at);
                    match grid.hit(x, y) {
                        Some(archforge_diagram::Hit::Edge(e)) => Some(e),
                        _ => None,
                    }
                }),
            end_drag: crate::editing::end_preview(shell, pointer),
            segment_drag: crate::editing::segment_preview(shell, view, pointer),
            ends: std::cell::RefCell::new(Vec::new()),
            draft: crate::uml_inline::draft(shell, store),
            rename_text: shell.canvas.rename.as_ref().map(|rn| rn.text.as_str()),
            tabs: std::cell::RefCell::new(Vec::new()),
            led: std::cell::OnceCell::new(),
        };
        for e in &visible.edges {
            if let Some(badge) = draw_edge(&clip, &draw, *e, &mut shell.registry, ui) {
                badges.push(badge);
            }
        }
        shell.canvas.ends = draw.ends.take();
        for n in &visible.nodes {
            if let Some(badge) = draw_node(&clip, &draw, *n, &mut shell.registry, ui) {
                badges.push(badge);
            }
        }
        shell.canvas.package_tabs = draw.tabs.take();
        // A name its tab shows cut is shown whole while the pointer is on the tab.
        if let Some(at) = ui.input(|i| i.pointer.hover_pos())
            && let Some(name) = shell.canvas.package_tabs.iter().rev().find(|t| t.rect.contains(at)).and_then(|t| t.cut.clone())
        {
            let _ = resp.clone().on_hover_text_at_pointer(name);
        }
        pending_badges = badges;
    }
    for (at, control_id, element, element_obj, many) in pending_badges {
        let texts = shell.problems.get(&element_obj).cloned().unwrap_or_default();
        crate::connect::badge(shell, ui, p, at, control_id, element, &texts, many);
    }
    if let Some(grid) = grid.as_ref() {
        crate::editing::pointer(shell, ui, &resp, view, grid, d);
    }
    crate::editing::overlays(shell, ui, p, r, view, d, &overrides);
    let below_path = crate::package_nav::after_draw(shell, ui, p, r, d);
    crate::connect::mapping_offer(shell, ui, p, r);
    crate::connect::toast(shell, ui, p, r);
    if minimap_shown(shell, r) {
        minimap(shell, ui, p, r, bounds, &view);
    } else if r.width() >= 16.0 + layout(crate::kinds::active_kind(shell)).width + 16.0 + 28.0 + MINIMAP_GAP.x {
        // Its button, where the toolbar leaves room for it: live when the user folded the minimap,
        // not available when the canvas is too narrow for it.
        let b = Rect::from_min_size(pos2(r.right() - MINIMAP_GAP.x - 28.0, r.bottom() - MINIMAP_GAP.y - 26.0), vec2(28.0, 26.0));
        let folded = !shell.canvas.minimap_open;
        paint.rect_filled(b, 7.0, p.surface);
        paint.rect_stroke(b, 7.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), &icons::MAP, if folded { p.text_secondary } else { p.text_muted });
        let resp = control(shell, ui, "workspace.minimap.toggle", "button", b, true);
        let hint = if folded { tr("Show minimap") } else { tr("Widen the canvas to show the minimap") };
        if resp.on_hover_text(hint).clicked() && folded {
            shell.canvas.minimap_open = true;
        }
    }
    toolbar(shell, ui, p, r);
    if let Some(notice) = shell.canvas.notice.clone() {
        ux::text(&paint, r.min + vec2(16.0, 12.0 + below_path), &notice, types::CAPTION, p.amber, Some((r.width() - 32.0).max(0.0)));
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
    /// The problems the rules find, by the element they sit on.
    problems: &'a std::collections::BTreeMap<Obj, Vec<String>>,
    /// The diagram drawn, and the notations that describe its nodes and lines.
    diagram: Obj,
    notations: &'a archforge_diagram::ProjectView,
    /// The diagram's tables and notes in diagram coordinates (a drag's preview where it has one),
    /// and where each line's ends sit along their sides: worked out once a frame.
    boxes: Vec<(Obj, archforge_diagram::Rect)>,
    spreads: std::collections::HashMap<Obj, (f64, f64)>,
    /// The line under the pointer, drawn with a halo so its route reads.
    hovered: Option<Obj>,
    /// A drag under way: a cardinality text's line, end and centre, or a line's route with a
    /// segment moved.
    end_drag: Option<(Uuid, u8, Pos2)>,
    segment_drag: Option<(Uuid, Vec<(f64, f64)>)>,
    /// The cardinality texts drawn this frame, for the next frame's press.
    ends: std::cell::RefCell<Vec<EndText>>,
    /// A row typed in place on a card: its element, compartment and row (none for a new one).
    draft: Option<(Obj, usize, Option<usize>)>,
    /// The name being typed in place, which a package's tab follows as it is typed.
    rename_text: Option<&'a str>,
    /// The packages' tabs drawn this frame, for the next frame's press and hover.
    tabs: std::cell::RefCell<Vec<PackageTab>>,
    /// The diagram each element leads to, by the element: worked out once a frame, when a card
    /// first shows one (a package's own diagram, AR-113).
    led: std::cell::OnceCell<std::collections::HashMap<Obj, Obj>>,
}

impl Drawing<'_> {
    /// A node's rectangle on screen: the gesture's preview when it has one, else as stored.
    fn node_rect(&self, n: Obj) -> Rect {
        self.store.uuid(n).ok().and_then(|id| self.overrides.get(&id).copied()).unwrap_or_else(|| self.view.rect(&bounds_of(self.store, n)))
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
    let id = store.uuid(n).ok();
    if let Some(id) = id {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}"), "node", rect);
    }
    let renaming = id.is_some() && id == d.renaming;
    let element = element_of(store, n, node::ELEMENT);
    let selected = selection.contains(&n) || element.is_some_and(|e| selection.contains(&e));
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
    match element {
        Some(_) if let Some(card) = card => {
            // An element of another scope: dimmed, dashed, its scope named in the eyebrow.
            let dim = |c: egui::Color32| if card.elsewhere { c.gamma_multiply(FOREIGN_ALPHA) } else { c };
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
                    paint.galley(tab.min + vec2(PACKAGE_TAB_PAD * z, (tab.height() - name.size().y) / 2.0), name, p.text);
                }
                if let Some(id) = id {
                    widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.tab"), "tab", tab);
                    d.tabs.borrow_mut().push(PackageTab { rect: tab, node: id, cut: cut.then(|| card.name.clone()) });
                }
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
                    name(&inner, rect.min + vec2(ENTITY_INSET, 30.0) * z, &card.name, types::SECTION.scaled(z), p.text, room);
                }
                let line_y = rect.top() + ENTITY_HEADER * z;
                if line_y < rect.bottom() {
                    inner.rect_filled(Rect::from_min_size(pos2(rect.left(), line_y), vec2(rect.width(), 1.0)), 0.0, p.entity_line);
                }
                draw_rows(&inner, p, &card, rect, z, key_status, room);
            } else {
                let g = ux::galley(&inner, &card.name, types::SECTION.scaled(z.max(DETAIL_ZOOM)), p.text, room);
                inner.galley(rect.center() - g.size() / 2.0, g, p.text);
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
                inner.galley(rect.min + vec2(12.0, 10.0) * z, g, ink);
            }
        }
    }
    if selected {
        selection_ring(paint, p, rect);
    }
    let e = element.filter(|e| d.problems.contains_key(e))?;
    Some((rect.right_top() + vec2(-2.0, 2.0), id?, store.uuid(e).ok()?, e, false))
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
pub(crate) fn package_tab(paint: &egui::Painter, rect: Rect, name: &str, z: f32, ink: egui::Color32) -> (Rect, std::sync::Arc<egui::Galley>, bool) {
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

/// How a card of a thumbnail is drawn: as the card it stands for is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MiniLook {
    Plain,
    Tinted,
    Tabbed,
    Note,
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
    let grid = Grid::build(store, d);
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
        paint.galley(area.center() - g.size() / 2.0, g, p.text_muted);
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
    let (fill, edge) = match c.look {
        MiniLook::Tinted => (p.entity, p.entity_edge),
        MiniLook::Note => (p.amber_tint, p.amber),
        MiniLook::Plain | MiniLook::Tabbed => (p.surface, p.border),
    };
    if c.look == MiniLook::Tabbed {
        let tab =
            Rect::from_min_size(pos2(r.left(), r.top() - (PACKAGE_TAB_HEIGHT - 1.0) * s), vec2((r.width() * 0.4).max(2.0), PACKAGE_TAB_HEIGHT * s));
        paint.rect_filled(tab, 2.0, fill);
        paint.rect_stroke(tab, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    paint.rect_filled(r, 2.0, fill);
    paint.rect_stroke(r, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    if c.look == MiniLook::Note || c.look == MiniLook::Tabbed {
        return;
    }
    let bar = |y: f32, w: f32, h: f32, colour: egui::Color32| {
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
            y += 0.5 * ENTITY_ROW * s;
        }
        for _ in 0..*rows {
            bar(y + 4.0 * s, 0.45, 5.0 * s, p.border);
            y += ENTITY_ROW * s;
        }
    }
}

/// The row of `card` at `y` units below its top, counted across its compartments as
/// [`draw_rows`] lays them out; none on a dividing line or past the last row.
pub(crate) fn row_at(card: &archforge_diagram::figure::Card, y: f32) -> Option<usize> {
    let mut slot = 0.0_f32;
    let mut index = 0;
    for (c, rows) in card.compartments.iter().enumerate() {
        if c > 0 {
            slot += 0.5;
        }
        if rows.is_empty() && card.compartments.len() > 1 {
            slot += 1.0;
        }
        for _ in rows {
            let top = ENTITY_FIRST_ROW - 4.0 + slot * ENTITY_ROW;
            if (top..top + ENTITY_ROW).contains(&y) {
                return Some(index);
            }
            index += 1;
            slot += 1.0;
        }
    }
    None
}

/// A card's places under its header, each with its top in units below the card's and the
/// compartment it is in: a row a pitch apart, half a pitch and a dividing line between
/// compartments, an empty compartment of several still a row.
fn slots(card: &archforge_diagram::figure::Card) -> Vec<(f32, Slot<'_>, usize)> {
    let mut out = Vec::new();
    // Where the next row sits, in pitches below the first.
    let mut at = 0.0_f32;
    for (c, rows) in card.compartments.iter().enumerate() {
        if c > 0 {
            out.push((ENTITY_FIRST_ROW + at * ENTITY_ROW - 4.0, Slot::Line, c));
            at += 0.5;
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

/// How tall `card` is to show every place it has.
pub(crate) fn card_height(card: &archforge_diagram::figure::Card) -> f32 {
    slots(card).last().map_or(ENTITY_HEADER, |(top, _, _)| top + ENTITY_ROW + 9.0)
}

/// `card` with a row being typed in place: a new, blank one at the end of compartment
/// `compartment`, or row `row` (counted across the compartments) blanked under its field.
pub(crate) fn with_draft(card: &mut archforge_diagram::figure::Card, compartment: usize, row: Option<usize>) {
    match row {
        None => {
            if let Some(rows) = card.compartments.get_mut(compartment) {
                rows.push(archforge_diagram::figure::Row { mark: None, text: String::new(), linked: false, slanted: false });
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
                let text = if row.slanted { ux::text_slanted } else { ux::text };
                let drawn = text(inner, pos2(text_x, y), &row.text, types::UI.scaled(z), p.text, Some(room_x));
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

/// Where a mark the line stops short of sits at `tip`, coming from `back`: the line's direction,
/// the mark's base, half its width across, and its length; a triangle stops the line at its base,
/// a diamond at its far corner.
fn mark_shape(mark: EndMark, tip: Pos2, back: Pos2, z: f32) -> (Vec2, Pos2, Vec2, f32) {
    let dir = if (tip - back).length() > f32::EPSILON { (tip - back).normalized() } else { vec2(0.0, -1.0) };
    let s = z.max(0.6);
    let (len, half) = match mark {
        EndMark::Triangle => (12.0 * s, 7.5 * s),
        EndMark::HollowDiamond | EndMark::FilledDiamond => (20.0 * s, 6.0 * s),
        _ => (0.0, 6.0 * s),
    };
    (dir, tip - dir * len, vec2(-dir.y, dir.x) * half, len)
}

/// Draws a line's triangle, diamond or open arrow at `tip`, coming from `back`, a hollow one filled
/// with `bg`.
#[allow(clippy::too_many_arguments, reason = "a mark is drawn at its end, in its stroke and over what it is drawn on")]
pub(crate) fn line_mark(paint: &egui::Painter, mark: EndMark, tip: Pos2, back: Pos2, stroke: Stroke, z: f32, bg: egui::Color32) {
    let (dir, base, side, len) = mark_shape(mark, tip, back, z);
    match mark {
        EndMark::Triangle => {
            paint.add(egui::Shape::convex_polygon(vec![tip, base + side, base - side], bg, stroke));
        }
        EndMark::HollowDiamond | EndMark::FilledDiamond => {
            let middle = tip - dir * (len / 2.0);
            let fill = if mark == EndMark::FilledDiamond { stroke.color } else { bg };
            paint.add(egui::Shape::convex_polygon(vec![tip, middle + side, base, middle - side], fill, stroke));
        }
        EndMark::OpenArrow => {
            let back = tip - dir * 12.0 * z.max(0.6);
            paint.line_segment([tip, back + side], stroke);
            paint.line_segment([tip, back - side], stroke);
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
    let points = edge_points(d, e)?;
    let n = points.len();
    let selected = selection.contains(&e) || selection.contains(&element);
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
    crate::connect::line(paint, &drawn, stroke, !line.dashed, z);
    for (end, tip, back) in [(&line.ends[0], points[0], points[1]), (&line.ends[1], points[n - 1], points[n - 2])] {
        let (dir, base, _, _) = shape(end.mark, tip, back);
        line_mark(paint, end.mark, tip, back, stroke, z, p.canvas);
        // A note beside the target's triangle, such as a generalization set's facts.
        if end.mark == EndMark::Triangle
            && let Some(note) = line.note.as_ref().filter(|_| std::ptr::eq(end, &line.ends[1]))
        {
            let text = note.iter().map(say).collect::<Vec<_>>().join(" · ");
            let gal = ux::galley(paint, &text, types::SMALL.scaled(z), p.text_secondary, None);
            paint.galley(base + vec2(10.0 * z, -gal.size().y / 2.0) - dir * 14.0 * z, gal, p.text_secondary);
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
    // The ends' texts: beside the line near its end, or where the user put them.
    for (i, (end, at, towards)) in [(&line.ends[0], points[0], points[1]), (&line.ends[1], points[n - 1], points[n - 2])].into_iter().enumerate() {
        let Some(text) = end.text.as_deref() else { continue };
        let Ok(edge_id) = store.uuid(e) else { continue };
        let i = i as u8;
        let own = refs(store, e, edge::LABEL).into_iter().find(|l| string(store, *l, label::KIND) == end.slot);
        let g = ux::galley(paint, text, types::SMALL.scaled(z), p.text_secondary, None);
        let centre = match (d.end_drag.filter(|(id, k, _)| *id == edge_id && *k == i), own) {
            (Some((_, _, centre)), _) => centre,
            (None, Some(l)) => at + vec2(real(store, l, label::DX) as f32, real(store, l, label::DY) as f32) * z,
            (None, None) => beside(at, towards, g.size(), z),
        };
        let rect = Rect::from_center_size(centre, g.size());
        paint.galley(rect.min, g, p.text_secondary);
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
        paint.galley(at, g, p.text_secondary);
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
            paint.galley(at + offset, g, p.text_secondary);
        }
        if selected {
            for q in [points[0], points[n - 1]] {
                let h = Rect::from_center_size(q, Vec2::splat(6.0));
                paint.rect_filled(h, 1.0, p.surface);
                paint.rect_stroke(h, 1.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
            }
        }
    }
    badge
}

/// Where a cardinality text sits by default: a little along the line from its end, beside the
/// line rather than on it: under a level line (a line's name sits above it), right of an upright
/// one.
fn beside(end: Pos2, towards: Pos2, size: Vec2, z: f32) -> Pos2 {
    let d = towards - end;
    if d.length() <= f32::EPSILON || !d.is_finite() {
        return end;
    }
    let d = d.normalized();
    let n = if d.x.abs() >= d.y.abs() { vec2(0.0, 1.0) } else { vec2(1.0, 0.0) };
    // Far enough off the line that the text's box clears it, whichever way the line runs.
    let reach = n.x.abs() * size.x / 2.0 + n.y.abs() * size.y / 2.0 + 6.0 * z;
    end + d * 30.0 * z + n * reach
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
fn edge_points(d: &Drawing<'_>, e: Obj) -> Option<Vec<Pos2>> {
    let (store, view) = (d.store, d.view);
    let (Some(source), Some(target)) = (element_of(store, e, edge::SOURCE), element_of(store, e, edge::TARGET)) else { return None };
    let dragged = store.uuid(e).ok().and_then(|id| d.segment_drag.as_ref().filter(|(s, _)| *s == id)).map(|(_, route)| route);
    let points: Vec<Pos2> = if source == target {
        crate::connect::self_loop(d.node_rect(source), view.zoom)
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
    fill: egui::Color32,
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
    paint.galley(at - vec2(g.size().x / 2.0, 0.0), g, p.text_secondary);
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
        boxed.galley(bx.center() - g.size() / 2.0, g, p.text_muted);
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
    /// The view group's x and its toggles.
    group: f32,
    toggles: Vec<(&'static str, &'static str)>,
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
    if kind.is_some_and(|k| k.status_marks()) {
        toggles.push(("key-status", "Key status"));
    }
    toggles.push(("grid", "Grid"));
    let group = x + 79.0;
    let arrange = group + 2.0 + 34.0 * toggles.len() as f32 + 6.0 + 8.0;
    Layout { kind_tools, note: x, group, toggles, arrange, width: arrange + 32.0 + 7.0 }
}

/// Whether the minimap is drawn open: when the user has it open and the canvas has room for it
/// beside the toolbar; otherwise its button stands in, so the tools are never hidden.
fn minimap_shown(shell: &Shell, r: Rect) -> bool {
    let width = layout(crate::kinds::active_kind(shell)).width;
    shell.canvas.minimap_open && r.width() >= 16.0 + width + 16.0 + MINIMAP.x + MINIMAP_GAP.x
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
        ux::icon_in(&paint, b, icon, if active { p.accent } else { p.text_secondary });
        if control(shell, ui, &format!("workspace.tools.{key}"), "button", b, true).on_hover_text(tr(hint)).clicked() {
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
        let ink = if active { p.accent } else { p.text_secondary };
        if !kind.is_some_and(|k| k.tool_face(shell, &paint, p, tool.key, b, ink, active)) {
            ux::icon_in(&paint, b, tool.icon, ink);
        }
        let hint = kind.and_then(|k| k.tool_hint(shell, tool.key)).unwrap_or_else(|| tr(tool.hint));
        // Its name for assistive technology is its tooltip: what it makes now, and which way.
        if control(shell, ui, &format!("workspace.tools.{}", tool.key), "button", b, true)
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
        if control(shell, ui, &format!("workspace.tools.{key}"), "button", chevron, true).on_hover_text(tr(hint)).clicked() {
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
        if control(shell, ui, &format!("workspace.tools.{key}"), "toggle", b, true).on_hover_text(tr(hint)).clicked() {
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
    let live = shell.canvas.active.is_some();
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
            Err(e) => shell.canvas.notice = Some(e.to_string()),
        }
    }
    if let (Some(kind), Some((key, chevron))) = (kind, open_at) {
        kind.popover(shell, ui, p, r, chevron, key);
    }
}

/// Every hover text of the tables, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    TOOLS
        .iter()
        .map(|(_, _, hint)| *hint)
        .chain(["Key status", "Grid", "complete", "disjoint", "overlapping", "Arrange diagram"])
        .chain(crate::kinds::KINDS.iter().flat_map(|k| k.tools().iter().flat_map(|t| std::iter::once(t.hint).chain(t.chevron.map(|(_, hint)| hint)))))
        .chain(crate::kinds::KINDS.iter().filter_map(|k| k.adorner().map(|(_, hint)| hint)))
        .chain(archforge_er::figures::WORDS.iter().copied())
        .chain(archforge_uml::WORDS.iter().copied())
}
