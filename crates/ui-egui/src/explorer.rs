//! The Models explorer (slice W2 of the workspace chrome, from the design's page 03 frames and
//! the cards "Models + menu" and "First schema named in place"): the tree of the open project's
//! model over the session's store, each notation's top elements in the model's order with the
//! rows its notation puts under them (ER's schemas, their skills and tables, a table's skills and
//! columns; `notation_ui`). A click selects through `view.select`, the `+` adds what the
//! selection's notation offers and opens the name for typing in place, Enter or F2 renames, Delete removes through `element.remove` after a dialog when other
//! things go with it or a diagram shows it. The filter narrows the rows to the names that contain
//! it. Every change goes through the engine, so undo and redo cover it.
//!
//! The geometry is the design's: rows 34 px apart from y 119, a level 13 px deeper than its
//! parent, the chevron at x 13, the icon at x 31 and the name at x 53 of the first level, the
//! selected row filled from x 8 to the column's right edge.

use std::collections::BTreeSet;

use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::{fmt, tr, trn};
use crate::notation_ui::{Added, NotationUi, Offer};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The first row's name baseline, the pitch between rows, a row's hit height, the indent per level.
const FIRST_ROW: f32 = 119.0;
const ROW: f32 = 34.0;
const ROW_HEIGHT: f32 = 32.0;
const INDENT: f32 = 13.0;
/// The most rows the tree draws: a model past this size gets the filter, not a longer list.
const MAX_ROWS: usize = 2000;
/// The most characters a name typed in place may have: the ER rule's.
const MAX_NAME: usize = archforge_er::rules::MAX_NAME;

/// What the explorer keeps between frames: nothing the model holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// The rows opened; everything else is folded, so a large model shows its schemas first and
    /// the user drills in.
    pub expanded: BTreeSet<Uuid>,
    /// The selection whose schema and table were last opened for it, so one made on the canvas or
    /// in the panels shows in the tree once, and folding them again stays folded.
    pub revealed: Option<Uuid>,
    /// The filter field's text.
    pub filter: String,
    /// A name being typed in place.
    pub rename: Option<Rename>,
    /// The `+` menu, open under its button.
    pub menu_open: bool,
    /// A row's context menu, open where it was asked for (AR-132, D1).
    pub context: Option<ContextMenu>,
    /// The removal waiting for the dialog's answer.
    pub delete: Option<Delete>,
    /// Whether the tree has the keyboard.
    pub focused: bool,
    /// The last refusal of an add or a removal, shown under the tree until the next action.
    pub notice: Option<String>,
    /// A table row being dragged towards the canvas, and where one was let go this frame, for the
    /// canvas to place (slice S7).
    pub dragging: Option<Uuid>,
    pub dropped: Option<(Uuid, egui::Pos2)>,
    /// How far the tree is scrolled, and the row it last brought into view.
    pub scroll: TreeScroll,
}

/// A tree's scroll: how far its rows are moved up, and the row last brought into view, so a row
/// newly selected (by the keys, on the canvas or in Properties) is shown once, and the wheel is
/// free again after.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreeScroll {
    pub offset: f32,
    pub followed: Option<Uuid>,
}

/// Scrolls a tree of `rows` rows `pitch` apart whose first row sits at `first` inside `tree`,
/// with `tail` points after the last: the wheel over it, the row `follow` (its index and id) into
/// view when it is newly selected, the offset kept within the content, and a thin thumb at the
/// edge while the rows overflow. Gives the offset to draw the rows at.
pub(crate) fn scroll_tree(
    ui: &egui::Ui,
    p: &Palette,
    tree: Rect,
    first: f32,
    rows: usize,
    pitch: f32,
    tail: f32,
    wheel: bool,
    follow: Option<(usize, Uuid)>,
    state: &mut TreeScroll,
) -> f32 {
    let view = (tree.bottom() - first - 8.0).max(pitch);
    let content = rows as f32 * pitch + tail;
    let room = (content - view).max(0.0);
    if wheel && ui.rect_contains_pointer(tree) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        state.offset -= dy;
    }
    match follow {
        Some((i, id)) if state.followed != Some(id) => {
            state.followed = Some(id);
            let top = i as f32 * pitch;
            if top < state.offset {
                state.offset = top;
            } else if top + pitch > state.offset + view {
                state.offset = top + pitch - view;
            }
        }
        Some(_) => {}
        None => state.followed = None,
    }
    state.offset = state.offset.clamp(0.0, room);
    if room > 0.0 {
        let track = Rect::from_min_max(pos2(tree.right() - 6.0, first - 6.0), pos2(tree.right() - 2.0, tree.bottom() - 6.0));
        let h = (track.height() * view / content).max(24.0).min(track.height());
        let thumb = Rect::from_min_size(pos2(track.left(), track.top() + state.offset / room * (track.height() - h)), vec2(4.0, h));
        ui.painter().rect_filled(thumb, 2.0, p.scroll_thumb);
    }
    state.offset
}

/// A row's context menu: the row's element, where it opens, whether its New inside ▸ submenu is
/// open, and the rows the arrow keys reached in it and in the submenu.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenu {
    pub target: Uuid,
    pub at: egui::Pos2,
    pub sub: bool,
    pub reached: Option<usize>,
    pub sub_reached: Option<usize>,
}

/// A name being typed in place: the element, the text so far, whether the element was just made
/// (its default name is then selected, so typing replaces it), and the engine's refusal, if any.
#[derive(Debug, Clone, PartialEq)]
pub struct Rename {
    pub target: Uuid,
    pub text: String,
    pub fresh: bool,
    pub refused: Option<String>,
    /// True until the field has taken the keyboard once.
    pub opened: bool,
}

impl Rename {
    fn new(target: Uuid, text: String, fresh: bool) -> Self {
        Self { target, text, fresh, refused: None, opened: true }
    }

    /// A name just made, selected whole so typing replaces it.
    pub(crate) fn fresh(target: Uuid, text: String) -> Self {
        Self::new(target, text, true)
    }
}

/// A removal the dialog asks about: what goes, named, and what goes with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delete {
    pub ids: Vec<Uuid>,
    pub name: String,
    pub relationships: usize,
    pub keys: usize,
    pub foreign_keys: usize,
    /// The diagrams it disappears from: those that show it as a card or a line, each once
    /// ([`crate::editing::diagrams_showing`]), not the diagrams that belong to it.
    pub diagrams: usize,
    /// The node the canvas asked from, when it did.
    pub node: Option<Uuid>,
    /// What goes with it by name, the dialog naming each: a UML classifier's nested classifiers
    /// (AR-132).
    pub along: Vec<String>,
    /// The stereotype applications that go with it and what it takes along, `«Entity» on Order`,
    /// the dialog naming each (AR-135).
    pub applications: Vec<String>,
    /// The package what it holds moves up to, a nested package's parent, and whether it holds
    /// anything: the dialog says so instead of saying what it holds goes with it, and the toast
    /// says where it went, as a removal without the dialog does.
    pub hands_to: Option<(String, bool)>,
}

/// One row of the tree as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: Uuid,
    pub obj: Obj,
    pub depth: u8,
    /// What the row is, as its notation names it (`schema`, `table`, `column`, `skill`), and its
    /// icon, drawn muted for a row under another's (a column).
    pub kind: &'static str,
    pub icon: &'static ux::Icon,
    pub muted: bool,
    pub label: String,
    /// A column's type, after its name.
    pub detail: Option<String>,
    /// What the row is, muted after its name (a UML generalization set's `generalization set`).
    pub note: Option<String>,
    pub expandable: bool,
    pub expanded: bool,
    /// Whether the row drags onto the canvas (a table).
    pub draggable: bool,
}

/// What a notation's rows are made with: the filter, the rows opened and the skills by subject.
pub(crate) struct Tree<'a> {
    store: &'a Store,
    filter: String,
    expanded: &'a BTreeSet<Uuid>,
    skills: std::collections::BTreeMap<Obj, Vec<Obj>>,
    none: Vec<Obj>,
}

impl Tree<'_> {
    /// Whether a filter narrows the rows: then every row with a match under it is open.
    pub(crate) fn filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    /// Whether a name holds the filter.
    pub(crate) fn matches(&self, label: &str) -> bool {
        self.filter.is_empty() || label.to_lowercase().contains(&self.filter)
    }

    /// Whether the row of `obj` is open.
    pub(crate) fn open(&self, obj: Obj) -> bool {
        self.filtering() || uuid_of(self.store, obj).is_some_and(|id| self.expanded.contains(&id))
    }

    /// The skills that sit on `obj`.
    pub(crate) fn skills(&self, obj: Obj) -> &[Obj] {
        self.skills.get(&obj).unwrap_or(&self.none)
    }

    /// A row of `obj`, closed, without a detail.
    pub(crate) fn row(&self, obj: Obj, depth: u8, kind: &'static str, icon: &'static ux::Icon, label: String) -> Option<Row> {
        let id = uuid_of(self.store, obj)?;
        Some(Row { id, obj, depth, kind, icon, muted: false, label, detail: None, note: None, expandable: false, expanded: false, draggable: false })
    }

    /// The rows of the skills on `obj` whose names hold the filter.
    pub(crate) fn skill_rows(&self, obj: Obj, depth: u8) -> Vec<Row> {
        self.skills(obj)
            .iter()
            .filter_map(|s| {
                let label = archforge_core::skills::name_of(self.store, *s);
                if self.matches(&label) { self.row(*s, depth, "skill", &icons::SKILL, label) } else { None }
            })
            .collect()
    }
}

fn uuid_of(store: &Store, obj: Obj) -> Option<Uuid> {
    store.uuid(obj).ok()
}

fn name_of(store: &Store, obj: Obj) -> String {
    archforge_core::name_of(store, obj)
}

/// The model's members each notation lists at the top, with their notation, in the model's order.
fn tops(uis: &[&'static dyn NotationUi], store: &Store) -> Vec<(Obj, &'static dyn NotationUi)> {
    let Some(model) = archforge_core::skills::model_of(store) else { return Vec::new() };
    children_of(store, model, archforge_core::model::MEMBER)
        .into_iter()
        .filter_map(|m| uis.iter().copied().find(|u| u.is_top(store, m)).map(|u| (m, u)))
        .collect()
}

fn children_of(store: &Store, obj: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(obj, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|o| store.is_alive(*o)).collect()
}

/// The skills by the element they sit on, read once per frame.
fn skills_by_subject(store: &Store) -> std::collections::BTreeMap<Obj, Vec<Obj>> {
    let mut out: std::collections::BTreeMap<Obj, Vec<Obj>> = std::collections::BTreeMap::new();
    for s in archforge_core::skills::skills(store) {
        if let Some(subject) = archforge_core::skills::subject_of(store, s) {
            out.entry(subject).or_default().push(s);
        }
    }
    out
}

/// The tree's rows for the store, the opened rows and the filter: a filter shows the rows whose
/// name holds it and their ancestors, all open.
pub(crate) fn rows(uis: &[&'static dyn NotationUi], store: &Store, state: &State) -> Vec<Row> {
    let t = Tree { store, filter: state.filter.trim().to_lowercase(), expanded: &state.expanded, skills: skills_by_subject(store), none: Vec::new() };
    let mut out = Vec::new();
    for (top, ui) in tops(uis, store) {
        out.extend(ui.tree(store, top, &t));
        if out.len() >= MAX_ROWS {
            out.truncate(MAX_ROWS);
            break;
        }
    }
    out
}

/// A default name no sibling has: `Schema 1`, `Table 2`, `column_3`; none past ten thousand.
pub fn fresh_name(stem: &str, taken: impl Iterator<Item = String>, snake: bool) -> Option<String> {
    let taken: BTreeSet<String> = taken.map(|n| n.to_lowercase()).collect();
    (1..=10_000u32).map(|n| if snake { format!("{stem}_{n}") } else { format!("{stem} {n}") }).find(|name| !taken.contains(&name.to_lowercase()))
}

/// What `+` offers at the selection, the first being the default: what the selection's notation
/// adds there, or what every notation adds at the model's root.
fn offers(uis: &[&'static dyn NotationUi], store: &Store, selection: Option<Obj>) -> Vec<(&'static dyn NotationUi, Offer)> {
    match selection.and_then(|o| crate::notation_ui::of(uis, store, o)) {
        Some(ui) => ui.offers(store, selection).into_iter().map(|o| (ui, o)).collect(),
        None => uis.iter().flat_map(|ui| ui.offers(store, None).into_iter().map(move |o| (*ui, o))).collect(),
    }
}

/// Runs a command on the open project's session, keeping the engine's refusal as text.
pub(crate) fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(crate::i18n::tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

fn selected(shell: &Shell) -> Option<Obj> {
    shell.session.as_ref().and_then(|s| s.view().selection.first().copied())
}

fn select(shell: &mut Shell, id: Option<Uuid>) {
    let ids: Vec<String> = id.into_iter().map(|u| u.to_string()).collect();
    let _ = run(shell, "view.select", json!({ "ids": ids }));
}

/// Adds what the offer `key` names at the selection with a fresh default name, selects it and
/// opens its name for typing; the rows it sits under open, and the filter goes, so it is listed.
pub fn add(shell: &mut Shell, key: &str) {
    if shell.explorer.rename.is_some() {
        commit_rename(shell);
    }
    shell.explorer.notice = None;
    let Some(session) = shell.session.as_ref() else { return };
    let sel = selected(shell);
    let Some(ui) = offers(shell.notation_uis, session.doc(), sel).into_iter().find(|(_, o)| o.key == key).map(|(ui, _)| ui) else { return };
    match ui.add(shell, key, sel) {
        None => {}
        Some(Ok(Added { id, parents })) => {
            shell.explorer.expanded.extend(parents);
            shell.explorer.filter.clear();
            let name = shell.session.as_ref().and_then(|s| s.doc().by_uuid(id).map(|o| name_of(s.doc(), o))).unwrap_or_default();
            shell.explorer.rename = Some(Rename::new(id, name, true));
            shell.explorer.focused = true;
        }
        Some(Err(e)) => shell.explorer.notice = Some(e),
    }
}

/// Commits a name typed in place through the element's rename command; a refusal stays with the
/// field.
fn commit_rename(shell: &mut Shell) {
    let Some(r) = shell.explorer.rename.clone() else { return };
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let Some(obj) = store.by_uuid(r.target) else {
        shell.explorer.rename = None;
        return;
    };
    let name = r.text.trim().to_string();
    if name == name_of(store, obj) {
        shell.explorer.rename = None;
        return;
    }
    let ui = crate::notation_ui::of(shell.notation_uis, store, obj);
    let Some((cmd, key)) = ui.and_then(|ui| ui.rename(store, obj)) else {
        shell.explorer.rename = None;
        return;
    };
    // A refusal the notation says in the user's words comes first, and nothing runs.
    if let Some(why) = ui.and_then(|ui| ui.rename_refusal(store, obj, &name)) {
        if let Some(r) = shell.explorer.rename.as_mut() {
            r.refused = Some(why);
        }
        return;
    }
    match run(shell, cmd, json!({ key: r.target.to_string(), "name": name })) {
        Ok(_) => shell.explorer.rename = None,
        Err(e) => {
            if let Some(r) = shell.explorer.rename.as_mut() {
                r.refused = Some(e);
            }
        }
    }
}

/// Whether the notation of `obj` renames it in place, so its row opens its name for typing.
fn renames(shell: &Shell, obj: Obj) -> bool {
    shell.session.as_ref().is_some_and(|s| {
        let store = s.doc();
        crate::notation_ui::of(shell.notation_uis, store, obj).is_some_and(|ui| ui.rename(store, obj).is_some())
    })
}

/// Removes the selection: at once when nothing else goes with it and no diagram shows it, else
/// after the dialog, which counts the diagrams that show it as the canvas's Delete from the model
/// does ([`crate::editing::diagrams_showing`]).
pub fn remove_selection(shell: &mut Shell) {
    shell.explorer.notice = None;
    let Some(obj) = selected(shell) else { return };
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    if store.class_of(obj).ok() == Some(archforge_core::SKILL) {
        // A skill is removed from the Skills tab (slice W3), not from the tree.
        return;
    }
    let Some(id) = uuid_of(store, obj) else { return };
    let ui = crate::notation_ui::of(shell.notation_uis, store, obj);
    // What its notation keeps (UML's package Types with the built-in types) is said, not removed.
    if let Some(why) = ui.and_then(|u| u.remove_refusal(store, obj)) {
        shell.explorer.notice = Some(why);
        return;
    }
    // Stereotype applications go with it: removing them is never silent (AR-135).
    let applications = ui.map(|u| u.applications_along(store, obj)).unwrap_or_default();
    // What it holds moves up to its parent (a nested package): no dialog, the toast says where;
    // the dialog when its stereotype applications would go with it.
    if let Some(parent) = ui.and_then(|u| u.hands_up(store, obj)).filter(|_| applications.is_empty()) {
        let (name, to) = (crate::notation_ui::label(shell.notation_uis, store, obj), name_of(store, parent));
        let held = ui.is_some_and(|u| u.has_children(store, obj));
        let said = crate::uml_constraints::before_removal(shell, &[id.to_string()]);
        match run(shell, "element.remove", json!({ "ids": [id.to_string()] })) {
            Ok(_) => {
                let text = if held {
                    fmt(&tr("{name} removed; what it held moved to {parent}"), &[("name", &name), ("parent", &to)])
                } else {
                    fmt(&tr("{name} removed"), &[("name", &name)])
                };
                let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
                shell.canvas.toast = Some(crate::connect::Toast { text, until: shell.frames.saturating_add(300), undo_label });
                crate::uml_constraints::say_dropped(shell, said, &name);
            }
            Err(e) => shell.explorer.notice = Some(e),
        }
        return;
    }
    let (relationships, keys, foreign_keys) = ui.map(|u| u.consequences(store, obj)).unwrap_or_default();
    let has_children = ui.is_some_and(|u| u.has_children(store, obj));
    // The diagrams that show it, counted as the canvas counts them (AR-187).
    let diagrams = crate::editing::diagrams_showing(store, obj);
    if [relationships, keys, foreign_keys, diagrams].iter().all(|n| *n == 0) && !has_children && applications.is_empty() {
        let lead = fmt(&tr("{name} deleted"), &[("name", &crate::notation_ui::label(shell.notation_uis, store, obj))]);
        let said = crate::uml_constraints::before_removal(shell, &[id.to_string()]);
        match run(shell, "element.remove", json!({ "ids": [id.to_string()] })) {
            Ok(_) => crate::uml_constraints::say_dropped(shell, said, &lead),
            Err(e) => shell.explorer.notice = Some(e),
        }
        return;
    }
    let along = ui.map(|u| u.goes_along(store, obj)).unwrap_or_default();
    let hands_to = ui.and_then(|u| u.hands_up(store, obj)).map(|p| (name_of(store, p), has_children));
    shell.explorer.delete = Some(Delete {
        ids: vec![id],
        // Named as the canvas names it, an unnamed relationship by its ends (AR-216).
        name: crate::notation_ui::label(shell.notation_uis, store, obj),
        relationships,
        keys,
        foreign_keys,
        diagrams,
        node: None,
        along,
        applications,
        hands_to,
    });
}

/// Drops what the store no longer has, so a rename or a dialog never points at a gone element.
pub fn retain_valid(state: &mut State, store: &Store) {
    if state.rename.as_ref().is_some_and(|r| store.by_uuid(r.target).is_none()) {
        state.rename = None;
    }
    if state.rename.is_none() && state.delete.is_none() {
        // Nothing waits for the keyboard: a notice is cleared on the next action, not here.
    }
    if state.delete.as_ref().is_some_and(|d| d.ids.iter().any(|i| store.by_uuid(*i).is_none())) {
        state.delete = None;
    }
    state.expanded.retain(|id| store.by_uuid(*id).is_some());
}

/// Opens a folded row, folds an open one.
fn toggle(state: &mut State, row: &Row) {
    if row.expanded {
        state.expanded.remove(&row.id);
    } else {
        state.expanded.insert(row.id);
    }
}

/// Every row that can open (ER's schemas and their tables).
fn foldable(uis: &[&'static dyn NotationUi], store: &Store) -> Vec<Uuid> {
    tops(uis, store).into_iter().flat_map(|(top, ui)| ui.foldable(store, top)).filter_map(|o| uuid_of(store, o)).collect()
}

/// Opens the rows the selected element sits under (ER's schema and table), once per selection, so
/// its row shows.
fn reveal(uis: &[&'static dyn NotationUi], state: &mut State, store: &Store, selected: Option<Obj>) {
    let id = selected.and_then(|o| uuid_of(store, o));
    if id == state.revealed {
        return;
    }
    state.revealed = id;
    if let Some(o) = selected
        && let Some(ui) = crate::notation_ui::of(uis, store, o)
    {
        state.expanded.extend(ui.parents(store, o).into_iter().filter_map(|p| uuid_of(store, p)));
    }
}

/// Draws the Models explorer into `r`: the title with `+`, the filter field, the tree.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    // A drop the canvas did not take last frame is gone.
    shell.explorer.dropped = None;
    ux::text(&paint, at(20.0, 25.0), &tr("Models").to_uppercase(), types::EYEBROW, p.text, Some((r.width() - 80.0).max(0.0)));
    let add = Rect::from_min_size(pos2(r.right() - 40.0, r.top() + 20.0), Vec2::splat(24.0));
    let menu_open = shell.explorer.menu_open;
    if menu_open {
        paint.rect_filled(add, 6.0, p.accent_tint);
    }
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(16.0)), &icons::PLUS, if menu_open { p.accent } else { p.text_secondary });
    let live = shell.session.is_some();
    if control(shell, ui, "workspace.explorer.add", "button", add, live).on_hover_text(tr("Add")).clicked() {
        shell.explorer.menu_open = !menu_open;
    }
    // Expand all, or Collapse all once every row is open; a filter opens everything it shows.
    let folds = shell.session.as_ref().map(|s| foldable(shell.notation_uis, s.doc())).unwrap_or_default();
    let all_open = !folds.is_empty() && folds.iter().all(|id| shell.explorer.expanded.contains(id));
    let unfiltered = shell.explorer.filter.trim().is_empty();
    let fold_all = Rect::from_min_size(pos2(r.right() - 68.0, r.top() + 20.0), Vec2::splat(24.0));
    let can_fold = live && unfiltered && !folds.is_empty();
    let icon = if all_open { &icons::COLLAPSE_ALL } else { &icons::EXPAND_ALL };
    ux::icon_mid(&paint, fold_all.center(), icon, if can_fold { p.text_secondary } else { p.text_muted });
    let hint = if all_open { tr("Collapse all") } else { tr("Expand all") };
    if control(shell, ui, "workspace.explorer.fold-all", "button", fold_all, can_fold).on_hover_text(hint).clicked() {
        if all_open {
            shell.explorer.expanded.clear();
        } else {
            shell.explorer.expanded.extend(folds);
        }
    }
    filter_field(shell, ui, p, Rect::from_min_max(at(16.0, 65.0), pos2(r.right() - 16.0, r.top() + 97.0)));
    // The keys act only when no name was open at the frame's start: the Enter that commits a name
    // must not open the next one.
    let typing_at_start = shell.explorer.rename.is_some();
    let tree = Rect::from_min_max(at(0.0, FIRST_ROW - 12.0), r.max);
    let rows = match &shell.session {
        Some(s) => rows(shell.notation_uis, s.doc(), &shell.explorer),
        None => Vec::new(),
    };
    if rows.is_empty() {
        let empty = if shell.explorer.filter.trim().is_empty() { tr("No schemas yet") } else { tr("Nothing matches the filter") };
        ux::text(&paint, at(36.0, FIRST_ROW), &empty, types::SMALL, p.text_muted, Some((r.width() - 52.0).max(0.0)));
    }
    // A name open on a row the tree does not list (filtered out, or past the most rows drawn) is
    // committed, so the keys never go dead.
    if let Some(target) = shell.explorer.rename.as_ref().map(|r| r.target)
        && !rows.iter().any(|row| row.id == target)
    {
        commit_rename(shell);
        shell.explorer.rename = None;
    }
    if ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|pos| !r.contains(pos))) {
        shell.explorer.focused = false;
    }
    // A click on the tree's empty part takes the keyboard and clears the selection.
    let tree_resp = ui.interact(tree, widgets::egui_id("workspace.explorer.tree"), Sense::click());
    if tree_resp.clicked() {
        shell.explorer.focused = true;
        shell.explorer.menu_open = false;
        if shell.explorer.rename.is_some() {
            commit_rename(shell);
        }
        select(shell, None);
    }
    let selected_obj = selected(shell);
    if let Some(store) = shell.session.as_ref().map(|s| s.doc()) {
        reveal(shell.notation_uis, &mut shell.explorer, store, selected_obj);
    }
    // The rows scroll under the filter; a row newly selected is brought into view.
    let follow = selected_obj.and_then(|o| rows.iter().position(|row| row.obj == o).map(|i| (i, rows[i].id)));
    let tail = if rows.len() >= MAX_ROWS { 40.0 } else { 12.0 };
    let wheel = !shell.explorer.menu_open && shell.explorer.dragging.is_none();
    let offset = scroll_tree(ui, p, tree, r.top() + FIRST_ROW, rows.len(), ROW, tail, wheel, follow, &mut shell.explorer.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, row) in rows.iter().enumerate() {
        let y = r.top() + FIRST_ROW + i as f32 * ROW - offset;
        if y + ROW_HEIGHT < tree.top() || y - 7.0 > tree.bottom() {
            continue;
        }
        tree_row(shell, &mut clip, p, r, y, row, selected_obj == Some(row.obj));
    }
    // A row dragged onto another row: what the drop means there (a class into a package), its
    // target ringed while it is held over it.
    let row_at = |at: egui::Pos2| -> Option<&Row> {
        if !tree.contains(at) {
            return None;
        }
        let i = ((at.y + 7.0 - (r.top() + FIRST_ROW - offset)) / ROW).floor();
        (i >= 0.0).then(|| rows.get(i as usize)).flatten()
    };
    let drop_of = |shell: &Shell, dragged: Uuid, target: &Row| {
        let store = shell.session.as_ref()?.doc();
        let d = store.by_uuid(dragged)?;
        crate::notation_ui::of(shell.notation_uis, store, target.obj)?.drop_on(store, d, target.obj)
    };
    if let (Some(dragged), Some(at)) = (shell.explorer.dragging, ui.input(|i| i.pointer.latest_pos()))
        && let Some(target) = row_at(at).filter(|t| t.id != dragged && drop_of(shell, dragged, t).is_some())
    {
        let i = rows.iter().position(|x| x.id == target.id).unwrap_or(0);
        let y = r.top() + FIRST_ROW + i as f32 * ROW - offset;
        let hit = Rect::from_min_max(pos2(r.left() + 8.0, y - 7.0), pos2(r.right() - 8.0, y - 7.0 + ROW_HEIGHT));
        let refused = shell.session.as_ref().is_some_and(|s| {
            let store = s.doc();
            store
                .by_uuid(dragged)
                .zip(crate::notation_ui::of(shell.notation_uis, store, target.obj))
                .is_some_and(|(d, ui)| ui.drop_refused(store, d, target.obj))
        });
        clip.painter().rect_stroke(hit, 6.0, Stroke::new(1.5, if refused { p.amber } else { p.accent }), StrokeKind::Inside);
    }
    if let Some((dragged, at)) = shell.explorer.dropped
        && tree.contains(at)
    {
        shell.explorer.dropped = None;
        let todo = row_at(at).filter(|t| t.id != dragged).and_then(|t| drop_of(shell, dragged, t));
        // A package moved here goes as Properties and the canvas move it: shown on a diagram, it
        // goes onto its new home's diagram too (AR-102).
        let package_to = todo.as_ref().filter(|(cmd, _)| *cmd == "uml.package.move").and_then(|(_, params)| {
            let store = shell.session.as_ref()?.doc();
            Some((store.by_uuid(dragged)?, store.by_uuid(Uuid::parse(params.get("to")?.as_str()?)?)?))
        });
        let out = match (package_to, todo) {
            (Some((package, to)), _) => crate::package_nav::move_package(shell, package, to),
            (None, Some((cmd, params))) => run(shell, cmd, params).map(|_| ()),
            (None, None) => Ok(()),
        };
        if let Err(e) = out {
            shell.explorer.notice = Some(e);
        }
    }
    if rows.len() >= MAX_ROWS {
        let y = r.top() + FIRST_ROW + rows.len() as f32 * ROW - offset;
        ux::text(
            &clip.painter().clone(),
            pos2(r.left() + 36.0, y),
            &tr("Narrow the filter to see more"),
            types::SMALL,
            p.text_muted,
            Some((r.width() - 52.0).max(0.0)),
        );
    }
    if let Some(notice) = shell.explorer.notice.clone() {
        ux::text(&paint, pos2(r.left() + 16.0, r.bottom() - 40.0), &notice, types::CAPTION, p.amber, Some((r.width() - 32.0).max(0.0)));
    }
    let no_widget_has_keys = ui.memory(|m| m.focused().is_none());
    let tree_keys =
        shell.explorer.focused && no_widget_has_keys && !typing_at_start && shell.explorer.rename.is_none() && shell.explorer.delete.is_none();
    // ⇧F10 opens the selected row's menu under it, as a right click does (D1).
    if tree_keys
        && shell.explorer.context.is_none()
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F10))
        && let Some(i) = selected_obj.and_then(|o| rows.iter().position(|row| row.obj == o))
    {
        let y = r.top() + FIRST_ROW + i as f32 * ROW - offset;
        let indent = f32::from(rows[i].depth) * INDENT;
        shell.explorer.context = Some(ContextMenu {
            target: rows[i].id,
            at: pos2(r.left() + 53.0 + indent, y + ROW_HEIGHT - 6.0),
            sub: false,
            reached: Some(0),
            sub_reached: None,
        });
    }
    // The Stereotypes picker takes typing, Backspace, the arrows and Enter (AR-135).
    if tree_keys && shell.explorer.context.is_none() && shell.panels.menu != Some(crate::panels::Menu::StereotypePick) {
        keyboard(shell, ui, &rows, selected_obj);
    }
    if shell.explorer.menu_open {
        add_menu(shell, ui, p, add);
    }
    if shell.explorer.context.is_some() {
        context_menu(shell, ui, p);
    }
    drag_ghost(shell, ui, p, &rows);
    // A drag whose row was not drawn when the button went up has ended all the same.
    if shell.explorer.dragging.is_some() && !ui.input(|i| i.pointer.primary_down()) {
        shell.explorer.dragging = None;
    }
}

/// The element being dragged, as a chip with its icon at the pointer above everything.
fn drag_ghost(shell: &Shell, ui: &egui::Ui, p: &Palette, rows: &[Row]) {
    let (Some(id), Some(at)) = (shell.explorer.dragging, ui.input(|i| i.pointer.latest_pos())) else { return };
    let Some(row) = rows.iter().find(|r| r.id == id) else { return };
    let paint = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, widgets::egui_id("workspace.explorer.drag")));
    let g = ux::galley(&paint, &row.label, types::SMALL, p.text, Some(220.0));
    let chip = Rect::from_min_size(at + vec2(12.0, 10.0), vec2(36.0 + g.size().x, 28.0));
    ux::shadow(&paint, chip, 6.0, p.shadow);
    paint.rect_filled(chip, 6.0, p.surface);
    paint.rect_stroke(chip, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    ux::icon_in(&paint, Rect::from_min_size(chip.min + vec2(10.0, 7.0), Vec2::splat(14.0)), row.icon, p.accent);
    paint.galley(chip.min + vec2(30.0, (28.0 - g.size().y) / 2.0), g, p.text);
}

/// The filter field: a real text field narrowing the tree as it is typed.
fn filter_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 8.0, p.page);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon_mid(&paint, pos2(field.left() + 17.0, field.center().y), &icons::SEARCH, p.text_secondary);
    widgets::record_visible(ui, &mut shell.registry, "workspace.explorer.filter", "field", field);
    let inner = crate::panels::field_inner(ui, field.with_min_x(field.left() + 22.0));
    let edit = egui::TextEdit::singleline(&mut shell.explorer.filter)
        .id(widgets::egui_id("workspace.explorer.filter"))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(tr("Filter models…")).color(p.text_secondary).font(types::SMALL.font()))
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let resp = ui.put(inner, edit);
    let _ = resp.clone().accessible_name(tr("Filter models…"));
    if resp.gained_focus() {
        shell.explorer.focused = false;
    }
}

/// One row: its chevron, icon, name and type; a click selects, a second click on the name or
/// Enter renames, the chevron folds.
fn tree_row(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, column: Rect, y: f32, row: &Row, is_selected: bool) {
    let paint = ui.painter().clone();
    let indent = row.depth as f32 * INDENT;
    let hit = Rect::from_min_max(pos2(column.left() + 8.0, y - 7.0), pos2(column.right() - 8.0, y - 7.0 + ROW_HEIGHT));
    if is_selected {
        paint.rect_filled(hit, 6.0, p.accent_tint);
    }
    let id = format!("workspace.explorer.row.{}", row.id);
    // A table row drags onto the canvas; its click still selects.
    if row.draggable {
        let drag = ui.interact(hit, widgets::egui_id(&format!("{id}.drag")), Sense::drag());
        if drag.drag_started() {
            shell.explorer.dragging = Some(row.id);
        }
        if drag.drag_stopped() && shell.explorer.dragging == Some(row.id) {
            shell.explorer.dragging = None;
            if let Some(at) = ui.input(|i| i.pointer.latest_pos()) {
                shell.explorer.dropped = Some((row.id, at));
            }
        }
    }
    let resp = control(shell, ui, &id, "row", hit, true);
    if row.expandable {
        let chevron = Rect::from_center_size(pos2(column.left() + 21.0 + indent, hit.center().y), Vec2::splat(16.0));
        ux::icon_in(&paint, chevron, if row.expanded { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, p.text_secondary);
        if ui.interact(chevron.expand(4.0), widgets::egui_id(&format!("{id}.fold")), Sense::click()).clicked() {
            toggle(&mut shell.explorer, row);
            return;
        }
    }
    let ink = if is_selected { p.accent } else { p.text };
    // The icon and the name share the row's middle line.
    let mid = hit.center().y;
    let icon_ink = if is_selected { p.accent } else { p.text_secondary };
    ux::icon_mid(&paint, pos2(column.left() + 38.5 + indent, mid), row.icon, if row.muted { p.text_muted } else { icon_ink });
    let name_x = column.left() + 53.0 + indent;
    let renaming = shell.explorer.rename.as_ref().is_some_and(|r| r.target == row.id);
    if renaming {
        rename_field(shell, ui, p, Rect::from_min_max(pos2(name_x - 4.0, y - 6.0), pos2(column.right() - 16.0, y + 22.0)));
        return;
    }
    let label = match &row.detail {
        Some(ty) => format!("{} : {ty}", row.label),
        None => row.label.clone(),
    };
    let drawn = ux::text_mid(&paint, name_x, mid, &label, types::UI, ink, Some((column.right() - 16.0 - name_x).max(0.0)));
    if let Some(note) = &row.note {
        let room = column.right() - 16.0 - drawn.right() - 6.0;
        if room > 0.0 {
            ux::text_mid(&paint, drawn.right() + 6.0, mid, note, types::CAPTION, p.text_muted, Some(room));
        }
    }
    if resp.clicked() {
        shell.explorer.focused = true;
        shell.explorer.menu_open = false;
        if shell.explorer.rename.is_some() {
            commit_rename(shell);
        }
        if is_selected && !resp.double_clicked() {
            // A second click on the selected row opens its name, as the design's inline edit,
            // when its notation renames it.
            if renames(shell, row.obj) {
                shell.explorer.rename = Some(Rename::new(row.id, row.label.clone(), false));
            }
        } else {
            select(shell, Some(row.id));
        }
    }
    if resp.double_clicked() && row.expandable {
        toggle(&mut shell.explorer, row);
    }
    // A right click selects the row and opens its menu where it was pressed (D1).
    if resp.secondary_clicked() {
        shell.explorer.focused = true;
        shell.explorer.menu_open = false;
        if shell.explorer.rename.is_some() {
            commit_rename(shell);
        }
        select(shell, Some(row.id));
        let at = ui.input(|i| i.pointer.interact_pos()).unwrap_or(pos2(hit.left() + 45.0 + indent, hit.bottom()));
        shell.explorer.context = Some(ContextMenu { target: row.id, at, sub: false, reached: None, sub_reached: None });
    }
}

/// The name typed in place: a bordered field with the text selected when the element is new;
/// Enter commits, Escape closes, a refusal stays under it in the engine's words.
fn rename_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 5.0, p.surface);
    paint.rect_stroke(field, 5.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let id = widgets::egui_id("workspace.explorer.rename");
    widgets::record_visible(ui, &mut shell.registry, "workspace.explorer.rename", "field", field);
    let Some(r) = shell.explorer.rename.as_mut() else { return };
    let fresh = r.fresh;
    let inner = crate::panels::field_inner(ui, field.expand2(vec2(2.0, 0.0)));
    let edit = egui::TextEdit::singleline(&mut r.text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::UI.font())
        .text_color(p.text)
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(tr("Name"));
    if r.opened {
        // The first frame takes the keyboard once; a new element's default name is selected whole,
        // so typing replaces it. Nothing asks for the keyboard again: a click elsewhere commits.
        out.response.request_focus();
        if fresh {
            let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(r.text.chars().count()));
            let mut state = out.state;
            state.cursor.set_char_range(Some(range));
            state.store(ui.ctx(), id);
        }
        r.opened = false;
        r.fresh = false;
    }
    if let Some(why) = &r.refused {
        ux::text(&paint, pos2(field.left(), field.bottom() + 4.0), why, types::CAPTION, p.amber, Some(field.width()));
    }
    let has_keys = out.response.has_focus() || out.response.lost_focus();
    let enter = has_keys && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let escape = has_keys && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if escape {
        shell.explorer.rename = None;
    } else if enter || out.response.lost_focus() {
        commit_rename(shell);
    }
}

/// The keys when the tree has them: up and down move the selection, left folds or goes to the
/// parent, right unfolds, Enter and F2 rename, Delete removes.
fn keyboard(shell: &mut Shell, ui: &egui::Ui, rows: &[Row], selected_obj: Option<Obj>) {
    let (up, down, left, right, rename, delete) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
            i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::F2),
            i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
        )
    });
    let at = rows.iter().position(|r| Some(r.obj) == selected_obj);
    if down {
        let next = at.map_or(0, |i| (i + 1).min(rows.len().saturating_sub(1)));
        if let Some(row) = rows.get(next) {
            select(shell, Some(row.id));
        }
    } else if up {
        let prev = at.map_or(0, |i| i.saturating_sub(1));
        if let Some(row) = rows.get(prev) {
            select(shell, Some(row.id));
        }
    } else if let Some(i) = at
        && let Some(row) = rows.get(i)
    {
        if left {
            if row.expanded {
                shell.explorer.expanded.remove(&row.id);
            } else if let Some(parent) = rows.get(..i).unwrap_or(&[]).iter().rev().find(|r| r.depth < row.depth) {
                select(shell, Some(parent.id));
            }
        } else if right && row.expandable {
            shell.explorer.expanded.insert(row.id);
        } else if rename && row.kind != "skill" && renames(shell, row.obj) {
            shell.explorer.rename = Some(Rename::new(row.id, row.label.clone(), false));
        } else if delete {
            remove_selection(shell);
        }
    }
}

/// What a row of the context menu does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextAct {
    Inside,
    Rename,
    Show,
    Delete,
}

/// A row of the context menu or its submenu: its id, words, the detail at its right (a shortcut, a
/// reason), whether it may be picked, and the whole reason when it may not.
struct MenuRow {
    id: String,
    label: String,
    detail: String,
    live: bool,
    why: Option<String>,
    icon: Option<&'static ux::Icon>,
}

const CONTEXT_WIDTH: f32 = 232.0;
const CONTEXT_ROW: f32 = 30.0;

/// The rows of a menu at `m`, from `top`, a line before row `separator`: each live row a control,
/// each other one saying why on hover and to a screen reader; the row `reached` (the arrow keys')
/// marked as a hovered one is. Gives the row clicked and the row under the pointer.
#[allow(clippy::too_many_arguments, reason = "a menu's rows are drawn for their frame, layer and marks")]
fn menu_rows(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    m: Rect,
    top: f32,
    rows: &[MenuRow],
    separator: Option<usize>,
    reached: Option<usize>,
    danger: Option<usize>,
) -> (Option<usize>, Option<usize>) {
    let paint = ui.painter().clone();
    let (mut clicked, mut hovered) = (None, None);
    let mut y = top;
    for (i, item) in rows.iter().enumerate() {
        if separator == Some(i) {
            paint.rect_filled(Rect::from_min_size(pos2(m.left() + 8.0, y + 4.0), vec2(m.width() - 16.0, 1.0)), 0.0, p.border);
            y += 9.0;
        }
        let row = Rect::from_min_size(pos2(m.left() + 4.0, y), vec2(m.width() - 8.0, CONTEXT_ROW));
        y += CONTEXT_ROW;
        let resp = if item.live {
            control(shell, ui, &item.id, "menu-item", row, true).accessible_name(item.label.as_str())
        } else {
            widgets::record_visible(ui, &mut shell.registry, &item.id, "menu-item", row);
            let resp = ui.interact(row, widgets::egui_id(&item.id), Sense::hover());
            let why = item.why.clone().unwrap_or_else(|| item.detail.clone());
            ui.ctx().accesskit_node_builder(widgets::egui_id(&item.id), |node| {
                node.set_role(egui::accesskit::Role::MenuItem);
                node.set_disabled();
                node.set_description(why.clone());
            });
            resp.on_hover_text(why).accessible_name(item.label.as_str())
        };
        if resp.hovered() {
            hovered = Some(i);
        }
        if item.live && (resp.hovered() || reached == Some(i)) {
            paint.rect_filled(row, 5.0, p.accent_tint);
        }
        let ink = if !item.live {
            p.text_muted
        } else if danger == Some(i) {
            p.danger
        } else {
            p.text
        };
        let mut x = row.left() + 10.0;
        if let Some(icon) = item.icon {
            ux::icon_mid(&paint, pos2(x + 7.0, row.center().y), icon, if item.live { p.text_secondary } else { p.text_muted });
            x += 26.0;
        }
        let detail = ux::galley(&paint, &item.detail, types::CAPTION, p.text_muted, Some(m.width() / 2.0));
        let dw = detail.size().x;
        paint.galley(pos2(row.right() - 10.0 - dw, row.center().y - detail.size().y / 2.0), detail, p.text_muted);
        ux::text_mid(&paint, x, row.center().y, &item.label, types::SMALL, ink, Some((row.right() - 18.0 - dw - x).max(0.0)));
        if item.live && resp.clicked() {
            clicked = Some(i);
        }
    }
    (clicked, hovered)
}

/// A row's context menu (D1): New inside {name} ▸ on a classifier, its submenu the kinds it may
/// nest, then Rename, Show in diagram and Delete from model. The arrows walk it, → opens the
/// submenu and ← closes it, Enter picks, Escape closes; a press elsewhere closes it.
fn context_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette) {
    let Some(menu) = shell.explorer.context.clone() else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(obj) = store.by_uuid(menu.target).filter(|o| store.is_alive(*o)) else {
        shell.explorer.context = None;
        return;
    };
    let ui_of = crate::notation_ui::of(shell.notation_uis, store, obj);
    let inside = ui_of.and_then(|u| u.inside(store, obj));
    let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
    let mut rows: Vec<(MenuRow, ContextAct)> = Vec::new();
    if let Some(inside) = &inside {
        rows.push((
            MenuRow {
                id: "workspace.explorer.contextMenu.newInside".into(),
                label: inside.word.clone(),
                detail: "▸".into(),
                live: inside.why_not.is_none(),
                why: inside.why_not.clone(),
                icon: None,
            },
            ContextAct::Inside,
        ));
    }
    let skill = store.class_of(obj).ok() == Some(archforge_core::SKILL);
    let label = name_of(store, obj);
    let shown = !skill && crate::editing::diagrams_showing(store, ui_of.and_then(|u| u.shown(store, obj)).unwrap_or(obj)) > 0;
    rows.push((
        MenuRow {
            id: "workspace.explorer.contextMenu.rename".into(),
            label: tr("Rename"),
            detail: "F2".into(),
            live: renames(shell, obj),
            why: None,
            icon: None,
        },
        ContextAct::Rename,
    ));
    rows.push((
        MenuRow {
            id: "workspace.explorer.contextMenu.show".into(),
            label: tr("Show in diagram"),
            detail: String::new(),
            live: shown,
            why: (!shown).then(|| tr("No diagram shows it yet")),
            icon: None,
        },
        ContextAct::Show,
    ));
    let separator = rows.len();
    rows.push((
        MenuRow {
            id: "workspace.explorer.contextMenu.delete".into(),
            label: tr("Delete from model"),
            detail: (if mac { "⌘⌫" } else { "Del" }).into(),
            live: !skill,
            why: skill.then(|| tr("A skill is removed from its tab")),
            icon: None,
        },
        ContextAct::Delete,
    ));
    let window = ui.ctx().content_rect();
    let height = 8.0 + rows.len() as f32 * CONTEXT_ROW + 9.0;
    let x = menu.at.x.min(window.right() - CONTEXT_WIDTH - 8.0).max(window.left() + 8.0);
    let y = menu.at.y.min(window.bottom() - height - 8.0).max(window.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), vec2(CONTEXT_WIDTH, height));
    // The submenu beside it, at its first row, on the side with room.
    let sub_rows: Vec<MenuRow> = inside
        .as_ref()
        .map(|inside| {
            let kinds = inside.kinds.iter().map(|(key, word, icon)| MenuRow {
                id: format!("workspace.explorer.contextMenu.newInside.{key}"),
                label: word.clone(),
                detail: String::new(),
                live: true,
                why: None,
                icon: Some(*icon),
            });
            let refused = inside.refused.iter().map(|(key, word, why)| MenuRow {
                id: format!("workspace.explorer.contextMenu.newInside.{key}"),
                label: word.clone(),
                detail: why.clone(),
                live: false,
                why: Some(why.clone()),
                icon: None,
            });
            kinds.chain(refused).collect()
        })
        .unwrap_or_default();
    let kinds_n = inside.as_ref().map_or(0, |i| i.kinds.len());
    let split = inside.as_ref().and_then(|i| i.split).unwrap_or(kinds_n);
    let sub_height = 8.0 + 26.0 + sub_rows.len() as f32 * CONTEXT_ROW + 9.0;
    let sub_x = if m.right() + 4.0 + CONTEXT_WIDTH <= window.right() - 8.0 {
        m.right() + 4.0
    } else {
        (m.left() - 4.0 - CONTEXT_WIDTH).max(window.left() + 8.0)
    };
    let sub_y = m.top().min(window.bottom() - sub_height - 8.0).max(window.top() + 8.0);
    let sub = Rect::from_min_size(pos2(sub_x, sub_y), vec2(CONTEXT_WIDTH, sub_height));
    // The keys: the arrows walk the live rows, → and ← open and close the submenu.
    let live: Vec<usize> = rows.iter().enumerate().filter(|(_, (r, _))| r.live).map(|(i, _)| i).collect();
    let (down, up, right, left, enter, escape) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    let mut state = menu.clone();
    let step = |at: Option<usize>, n: usize, down: bool| -> Option<usize> {
        if n == 0 {
            return None;
        }
        Some(match at {
            None => 0,
            Some(i) if down => (i + 1).min(n - 1),
            Some(i) => i.saturating_sub(1),
        })
    };
    if state.sub {
        if down || up {
            state.sub_reached = step(state.sub_reached, kinds_n, down);
        }
        if left || escape {
            state.sub = false;
            state.sub_reached = None;
        }
    } else {
        if down || up {
            let at = state.reached.and_then(|r| live.iter().position(|l| *l == r));
            state.reached = step(at, live.len(), down).and_then(|i| live.get(i).copied());
        }
        let on_inside = state.reached.and_then(|i| rows.get(i)).is_some_and(|(_, a)| *a == ContextAct::Inside);
        if on_inside && (right || enter) {
            state.sub = true;
            state.sub_reached = Some(0);
        }
        if escape {
            shell.explorer.context = None;
            return;
        }
    }
    let p2 = *p;
    let main_rows: Vec<MenuRow> = rows
        .iter()
        .map(|(r, _)| MenuRow { id: r.id.clone(), label: r.label.clone(), detail: r.detail.clone(), live: r.live, why: r.why.clone(), icon: r.icon })
        .collect();
    let (clicked, hovered) = ux::overlay(ui, widgets::egui_id("workspace.explorer.contextMenu.layer"), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p2.shadow);
        paint.rect_filled(m, 8.0, p2.surface);
        paint.rect_stroke(m, 8.0, Stroke::new(1.0, p2.border), StrokeKind::Inside);
        let danger = main_rows.len().checked_sub(1);
        menu_rows(shell, ui, &p2, m, m.top() + 4.0, &main_rows, Some(separator), state.reached, danger)
    });
    // Hovering New inside ▸ opens the submenu, as a click does.
    if let Some(i) = hovered.or(clicked) {
        state.reached = rows.get(i).filter(|(r, _)| r.live).map(|_| i);
        let over_inside = rows.get(i).is_some_and(|(r, a)| *a == ContextAct::Inside && r.live);
        if over_inside {
            state.sub = true;
        } else if hovered.is_some() {
            state.sub = false;
        }
    }
    let mut sub_clicked = None;
    let mut area = m;
    if state.sub && !sub_rows.is_empty() {
        let heading = inside.as_ref().map(|i| crate::canvas::eyebrow_case(&i.heading)).unwrap_or_default();
        let sub_reached = state.sub_reached;
        let (c, _) = ux::overlay(ui, widgets::egui_id("workspace.explorer.contextMenu.sub.layer"), sub, |ui| {
            let paint = ui.painter().clone();
            ux::shadow(&paint, sub, 8.0, p2.shadow);
            paint.rect_filled(sub, 8.0, p2.surface);
            paint.rect_stroke(sub, 8.0, Stroke::new(1.0, p2.border), StrokeKind::Inside);
            ux::text(&paint, pos2(sub.left() + 12.0, sub.top() + 12.0), &heading, types::EYEBROW, p2.text_muted, Some(CONTEXT_WIDTH - 24.0));
            menu_rows(shell, ui, &p2, sub, sub.top() + 4.0 + 26.0, &sub_rows, Some(split), sub_reached, None)
        });
        sub_clicked = c;
        if enter && state.sub_reached.is_some() && menu.sub {
            sub_clicked = state.sub_reached;
        }
        area = area.union(sub);
    }
    let picked = clicked
        .and_then(|i| rows.get(i))
        .map(|(_, a)| *a)
        .or_else(|| (enter && !state.sub && !menu.sub).then(|| state.reached.and_then(|i| rows.get(i)).map(|(_, a)| *a)).flatten());
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| area.contains(pos)));
    shell.explorer.context = (!outside).then_some(state);
    if let Some(k) = sub_clicked.and_then(|i| inside.as_ref()?.kinds.get(i)).map(|(key, _, _)| *key) {
        shell.explorer.context = None;
        if let Some(Err(e)) = ui_of.and_then(|u| u.add_inside(shell, k, obj)) {
            shell.explorer.notice = Some(e);
        }
        return;
    }
    match picked {
        Some(ContextAct::Rename) => {
            shell.explorer.context = None;
            shell.explorer.rename = Some(Rename::new(menu.target, label, false));
        }
        Some(ContextAct::Show) => {
            shell.explorer.context = None;
            crate::project_search::go(shell, crate::project_search::Found::Element(menu.target));
        }
        Some(ContextAct::Delete) => {
            shell.explorer.context = None;
            select(shell, Some(menu.target));
            remove_selection(shell);
        }
        Some(ContextAct::Inside) | None => {}
    }
}

/// The `+` menu under its button: what can be added at the selection, Folder shown but not yet
/// live.
fn add_menu(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, anchor: Rect) {
    let store_offers = shell.session.as_ref().map(|s| offers(shell.notation_uis, s.doc(), selected(shell))).unwrap_or_default();
    let mut items: Vec<(Option<&'static str>, String)> = store_offers.into_iter().map(|(_, o)| (Some(o.key), o.word)).collect();
    items.push((None, tr("Folder")));
    let width = 200.0;
    let height = 8.0 + items.len() as f32 * 30.0;
    let menu = Rect::from_min_size(pos2((anchor.right() - width).max(0.0), anchor.bottom() + 4.0), vec2(width, height));
    // On a layer of its own, over the tree and the columns drawn after the explorer.
    let picked = ux::overlay(ui, crate::widgets::egui_id("workspace.explorer.add.layer"), menu, |ui| add_menu_rows(shell, ui, p, menu, &items));
    if let Some(kind) = picked {
        shell.explorer.menu_open = false;
        if let Some(kind) = kind {
            add(shell, kind);
        }
    } else {
        let outside =
            ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| menu.contains(pos) || anchor.contains(pos)));
        if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            shell.explorer.menu_open = false;
        }
    }
}

/// The add menu's rows; gives the row picked, a kind or the folder.
fn add_menu_rows(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    menu: Rect,
    items: &[(Option<&'static str>, String)],
) -> Option<Option<&'static str>> {
    let width = menu.width();
    let paint = ui.painter().clone();
    ux::shadow(&paint, menu, 8.0, p.shadow);
    paint.rect_filled(menu, 8.0, p.surface);
    paint.rect_stroke(menu, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    for (i, (kind, label)) in items.iter().enumerate() {
        let row = Rect::from_min_size(pos2(menu.left() + 4.0, menu.top() + 4.0 + i as f32 * 30.0), vec2(width - 8.0, 30.0));
        let id = format!("workspace.explorer.add.{}", kind.unwrap_or("folder"));
        let resp = control(shell, ui, &id, "menu-item", row, kind.is_some());
        if resp.hovered() && kind.is_some() {
            paint.rect_filled(row, 5.0, p.page);
        }
        ux::text(&paint, row.min + vec2(10.0, 7.5), label, types::SMALL, if kind.is_some() { p.text } else { p.text_muted }, Some(width - 28.0));
        if resp.clicked() {
            picked = Some(*kind);
        }
    }
    picked
}

/// The dialog before a removal that takes other things with it, as the design's "Delete from the
/// model": drawn over the whole window on its own layer, with a veil that takes every click.
pub(crate) fn delete_dialog(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette) {
    let Some(d) = shell.explorer.delete.clone() else { return };
    let window = ui.ctx().content_rect();
    let p = *p;
    egui::Area::new(widgets::egui_id("workspace.explorer.delete")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        let paint = ui.painter().clone();
        paint.rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("workspace.explorer.delete.veil"), Sense::click_and_drag());
        delete_dialog_body(shell, ui, &p, &paint, window, &d);
    });
}

fn delete_dialog_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, paint: &egui::Painter, window: Rect, d: &Delete) {
    let mut lines: Vec<String> = Vec::new();
    if d.diagrams > 0 {
        lines.push(trn("It disappears from {n} diagram.", "It disappears from {n} diagrams.", d.diagrams as u64));
    }
    if d.relationships > 0 {
        lines.push(trn("{n} relationship goes with it.", "{n} relationships go with it.", d.relationships as u64));
    }
    if d.keys > 0 {
        lines.push(trn("{n} key goes with it.", "{n} keys go with it.", d.keys as u64));
    }
    if d.foreign_keys > 0 {
        lines.push(trn("{n} foreign key goes with it.", "{n} foreign keys go with it.", d.foreign_keys as u64));
    }
    // What goes with it by name: a classifier's nested classifiers (AR-132).
    if !d.along.is_empty() {
        let n = u64::try_from(d.along.len()).unwrap_or(u64::MAX);
        lines.push(fmt(
            &trn("{n} nested classifier goes with it: {names}.", "{n} nested classifiers go with it: {names}.", n),
            &[("names", &d.along.join(", "))],
        ));
    }
    // The stereotype applications that go with it, by name (AR-135): their values go too.
    if !d.applications.is_empty() {
        let n = u64::try_from(d.applications.len()).unwrap_or(u64::MAX);
        lines.push(fmt(
            &trn("{n} stereotype application goes with it: {names}.", "{n} stereotype applications go with it: {names}.", n),
            &[("names", &d.applications.join(", "))],
        ));
    }
    lines.push(match &d.hands_to {
        Some((parent, _)) => fmt(&tr("What it holds moves to {parent}."), &[("parent", parent)]),
        None => tr("What it holds goes with it."),
    });
    let size = vec2(440.0_f32.min(window.width() - 32.0), 110.0 + lines.len() as f32 * 20.0);
    let dialog = Rect::from_center_size(window.center(), size);
    ux::shadow(paint, dialog, 10.0, p.shadow);
    paint.rect_filled(dialog, 10.0, p.surface);
    paint.rect_stroke(dialog, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    let at = |x: f32, y: f32| dialog.min + vec2(x, y);
    ux::text(paint, at(20.0, 20.0), &fmt(&tr("Delete {name} from the model?"), &[("name", &d.name)]), types::CARD_TITLE, p.text, Some(size.x - 40.0));
    for (i, line) in lines.iter().enumerate() {
        ux::text(paint, at(20.0, 58.0 + i as f32 * 20.0), line, types::UI, p.text_secondary, Some(size.x - 40.0));
    }
    let mut x = dialog.right() - 20.0;
    let mut buttons = vec![("workspace.explorer.delete.confirm", tr("Delete"), true), ("workspace.explorer.delete.cancel", tr("Cancel"), false)];
    if d.node.is_some() {
        buttons.push(("workspace.explorer.delete.unplace", tr("Remove from this diagram only"), false));
    }
    for (id, label, primary) in buttons {
        // Laid out in the ink it is drawn in: a galley keeps the colour it was laid out with.
        let ink = if primary { p.on_accent } else { p.text };
        let g = ux::galley(paint, &label, types::SMALL, ink, None);
        let b = Rect::from_min_max(pos2(x - g.size().x - 24.0, dialog.bottom() - 48.0), pos2(x, dialog.bottom() - 20.0));
        paint.rect_filled(b, 6.0, if primary { p.danger } else { p.field });
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, ink);
        if control(shell, ui, id, "button", b, true).clicked() {
            shell.explorer.delete = None;
            if primary {
                let ids: Vec<String> = d.ids.iter().map(|i| i.to_string()).collect();
                let said = crate::uml_constraints::before_removal(shell, &ids);
                match run(shell, "element.remove", json!({ "ids": ids })) {
                    Ok(_) => match &d.hands_to {
                        // A nested package's contents moved up: the toast says where, with Undo.
                        Some((parent, held)) => {
                            let text = if *held {
                                fmt(&tr("{name} removed; what it held moved to {parent}"), &[("name", &d.name), ("parent", parent)])
                            } else {
                                fmt(&tr("{name} removed"), &[("name", &d.name)])
                            };
                            let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
                            shell.canvas.toast = Some(crate::connect::Toast { text, until: shell.frames.saturating_add(300), undo_label });
                            crate::uml_constraints::say_dropped(shell, said, &d.name);
                        }
                        None => crate::uml_constraints::say_dropped(shell, said, &fmt(&tr("{name} deleted"), &[("name", &d.name)])),
                    },
                    Err(e) => shell.explorer.notice = Some(e),
                }
            } else if id == "workspace.explorer.delete.unplace"
                && let Some(node) = d.node
                && let Err(e) = run(shell, "diagram.unplace", json!({ "nodes": [node.to_string()] }))
            {
                shell.canvas.notice = Some(e);
            }
            return;
        }
        x = b.left() - 8.0;
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        shell.explorer.delete = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_names_skip_what_is_taken_whatever_the_case() {
        assert_eq!(fresh_name("Schema", std::iter::empty(), false).as_deref(), Some("Schema 1"));
        assert_eq!(fresh_name("Schema", ["schema 1".to_string(), "Other".to_string()].into_iter(), false).as_deref(), Some("Schema 2"));
        assert_eq!(fresh_name("column", ["column_1".to_string(), "column_2".to_string()].into_iter(), true).as_deref(), Some("column_3"));
        assert_eq!(fresh_name("Table", (1..=10_000u32).map(|n| format!("Table {n}")), false), None);
    }
}
