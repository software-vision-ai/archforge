//! The Models explorer (slice W2 of the workspace chrome, from the design's page 03 frames and
//! the cards "Models + menu" and "First schema named in place"): the tree of the open project's
//! model over the session's store, starting at the schemas, with a schema's skills and tables
//! under it and a table's skills and columns under that. A click selects through `view.select`,
//! the `+` adds a schema, table or column through the ER commands and opens the name for typing
//! in place, Enter or F2 renames, Delete removes through `element.remove` after a dialog when other
//! things go with it. The filter narrows the rows to the names that contain it. Every change goes
//! through the engine, so undo and redo cover it.
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
}

/// A removal the dialog asks about: what goes, named, and what goes with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delete {
    pub ids: Vec<Uuid>,
    pub name: String,
    pub relationships: usize,
    pub keys: usize,
    pub foreign_keys: usize,
    /// The diagrams it disappears from, and the node the canvas asked from, when it did.
    pub diagrams: usize,
    pub node: Option<Uuid>,
}

/// What a row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Schema,
    Entity,
    Attribute,
    Skill,
    /// A schema's domain or enum type (slice S5).
    Domain,
    Enum,
    /// A schema's sequence or view (slice S7).
    Sequence,
    View,
}

/// One row of the tree as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: Uuid,
    pub obj: Obj,
    pub depth: u8,
    pub kind: Kind,
    pub label: String,
    /// A column's type, after its name.
    pub detail: Option<String>,
    pub expandable: bool,
    pub expanded: bool,
}

fn uuid_of(store: &Store, obj: Obj) -> Option<Uuid> {
    store.uuid(obj).ok()
}

fn name_of(store: &Store, obj: Obj) -> String {
    archforge_core::name_of(store, obj)
}

/// The schemas of the model, in the order the model lists them.
fn schemas(store: &Store) -> Vec<Obj> {
    let Some(model) = archforge_core::skills::model_of(store) else { return Vec::new() };
    store
        .list(model, archforge_core::model::MEMBER)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| store.is_alive(*o) && store.class_of(*o).ok() == Some(archforge_er::SCHEMA))
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
pub fn rows(store: &Store, state: &State) -> Vec<Row> {
    let filter = state.filter.trim().to_lowercase();
    let mut out = Vec::new();
    let push = |out: &mut Vec<Row>, obj: Obj, depth: u8, kind: Kind, label: String, detail: Option<String>, expandable: bool, expanded: bool| {
        if let Some(id) = uuid_of(store, obj) {
            out.push(Row { id, obj, depth, kind, label, detail, expandable, expanded });
        }
    };
    let open = |obj: Obj| -> bool { !filter.is_empty() || uuid_of(store, obj).is_some_and(|id| state.expanded.contains(&id)) };
    let matches = |label: &str| filter.is_empty() || label.to_lowercase().contains(&filter);
    let skills = skills_by_subject(store);
    let none = Vec::new();
    for schema in schemas(store) {
        let schema_skills = skills.get(&schema).unwrap_or(&none);
        let entities = children_of(store, schema, archforge_er::schema::ENTITY);
        let mut rows_under = Vec::new();
        for s in schema_skills {
            let label = archforge_core::skills::name_of(store, *s);
            if matches(&label) {
                push(&mut rows_under, *s, 1, Kind::Skill, label, None, false, false);
            }
        }
        for d in archforge_er::rules::domains_of(store, schema) {
            let label = name_of(store, d);
            if matches(&label) {
                let base = Some(archforge_er::rules::text_of(store, d, archforge_er::domain::BASE_TYPE)).filter(|t| !t.is_empty());
                push(&mut rows_under, d, 1, Kind::Domain, label, base, false, false);
            }
        }
        for en in archforge_er::rules::enums_of(store, schema) {
            let label = name_of(store, en);
            if matches(&label) {
                let n = archforge_er::rules::literals_of(store, en).len();
                push(&mut rows_under, en, 1, Kind::Enum, label, Some(trn("{n} literal", "{n} literals", n as u64)), false, false);
            }
        }
        for q in archforge_er::rules::sequences_of(store, schema) {
            let label = name_of(store, q);
            if matches(&label) {
                push(&mut rows_under, q, 1, Kind::Sequence, label, None, false, false);
            }
        }
        for v in archforge_er::rules::views_of(store, schema) {
            let label = name_of(store, v);
            if matches(&label) {
                push(&mut rows_under, v, 1, Kind::View, label, None, false, false);
            }
        }
        for e in &entities {
            let e_label = name_of(store, *e);
            let entity_skills = skills.get(e).unwrap_or(&none);
            let attributes = children_of(store, *e, archforge_er::entity::ATTRIBUTE);
            let mut under_entity = Vec::new();
            for s in entity_skills {
                let label = archforge_core::skills::name_of(store, *s);
                if matches(&label) {
                    push(&mut under_entity, *s, 2, Kind::Skill, label, None, false, false);
                }
            }
            for a in &attributes {
                let label = name_of(store, *a);
                let ty = Some(archforge_er::rules::type_display(store, *a)).filter(|t| !t.is_empty());
                if matches(&label) {
                    push(&mut under_entity, *a, 2, Kind::Attribute, label, ty, false, false);
                }
            }
            let expandable = if filter.is_empty() { !entity_skills.is_empty() || !attributes.is_empty() } else { !under_entity.is_empty() };
            if matches(&e_label) || !under_entity.is_empty() {
                let expanded = expandable && open(*e);
                push(&mut rows_under, *e, 1, Kind::Entity, e_label, None, expandable, expanded);
                if expanded {
                    rows_under.extend(under_entity);
                }
            }
        }
        let s_label = name_of(store, schema);
        let has_types = !archforge_er::rules::domains_of(store, schema).is_empty()
            || !archforge_er::rules::enums_of(store, schema).is_empty()
            || !archforge_er::rules::sequences_of(store, schema).is_empty()
            || !archforge_er::rules::views_of(store, schema).is_empty();
        let expandable = if filter.is_empty() { !schema_skills.is_empty() || !entities.is_empty() || has_types } else { !rows_under.is_empty() };
        if matches(&s_label) || !rows_under.is_empty() {
            let expanded = expandable && open(schema);
            push(&mut out, schema, 0, Kind::Schema, s_label, None, expandable, expanded);
            if expanded {
                out.extend(rows_under);
            }
        }
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

/// What `+` offers: the kinds that can be added at the selection, the first being the default.
fn offers(store: &Store, selection: Option<Obj>) -> Vec<Kind> {
    match selection.and_then(|o| store.class_of(o).ok()) {
        Some(c) if c == archforge_er::SCHEMA => vec![Kind::Entity, Kind::Schema],
        Some(c) if c == archforge_er::ENTITY || c == archforge_er::ATTRIBUTE => vec![Kind::Attribute, Kind::Entity],
        _ => vec![Kind::Schema],
    }
}

fn kind_word(kind: Kind) -> String {
    match kind {
        Kind::Schema => tr("Schema"),
        Kind::Entity => tr("Table"),
        Kind::Attribute => tr("Column"),
        Kind::Skill => tr("Skill"),
        Kind::Domain => tr("Domain"),
        Kind::Enum => tr("Enum type"),
        Kind::Sequence => tr("Sequence"),
        Kind::View => tr("View"),
    }
}

/// Runs a command on the open project's session, keeping the engine's refusal as text.
fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
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

/// Adds an element of `kind` at the selection with a fresh default name, selects it and opens its
/// name for typing. A schema or table needs a parent of the right class.
pub fn add(shell: &mut Shell, kind: Kind) {
    if shell.explorer.rename.is_some() {
        commit_rename(shell);
    }
    shell.explorer.notice = None;
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let sel = selected(shell);
    // The parents the new row sits under, opened so it is seen; the filter goes, so it is listed.
    let mut parents: Vec<Uuid> = Vec::new();
    // The table a column is added to: its nodes grow with the column, in the same step.
    let mut grow: Option<Uuid> = None;
    let no_name = || tr("No free name is left");
    let (cmd, params) = match kind {
        Kind::Schema => {
            let Some(name) = fresh_name("Schema", schemas(store).into_iter().map(|s| name_of(store, s)), false) else {
                shell.explorer.notice = Some(no_name());
                return;
            };
            ("er.schema.add", json!({ "name": name }))
        }
        Kind::Entity => {
            let Some(schema) = sel.and_then(|o| archforge_er::rules::schema_of(store, o)) else { return };
            let taken = children_of(store, schema, archforge_er::schema::ENTITY).into_iter().map(|e| name_of(store, e));
            let Some(name) = fresh_name("Table", taken, false) else {
                shell.explorer.notice = Some(no_name());
                return;
            };
            let Some(schema_id) = uuid_of(store, schema) else { return };
            parents.push(schema_id);
            ("er.entity.add", json!({ "schema": schema_id.to_string(), "name": name }))
        }
        Kind::Attribute => {
            let entity = sel.and_then(|o| match store.class_of(o).ok() {
                Some(c) if c == archforge_er::ENTITY => Some(o),
                Some(c) if c == archforge_er::ATTRIBUTE => archforge_er::rules::entity_of(store, o),
                _ => None,
            });
            let Some(entity) = entity else { return };
            let taken = children_of(store, entity, archforge_er::entity::ATTRIBUTE).into_iter().map(|a| name_of(store, a));
            let Some(name) = fresh_name("column", taken, true) else {
                shell.explorer.notice = Some(no_name());
                return;
            };
            let Some(entity_id) = uuid_of(store, entity) else { return };
            parents.extend(archforge_er::rules::schema_of(store, entity).and_then(|s| uuid_of(store, s)));
            parents.push(entity_id);
            grow = Some(entity_id);
            ("er.attribute.add", json!({ "entity": entity_id.to_string(), "name": name, "type": "TEXT" }))
        }
        Kind::Skill | Kind::Domain | Kind::Enum | Kind::Sequence | Kind::View => return,
    };
    let out = match (kind, grow) {
        // A table lands on an open diagram of its schema too, in the same step.
        (Kind::Entity, _) => {
            let schema = params.get("schema").and_then(|v| v.as_str()).and_then(Uuid::parse);
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            match schema {
                Some(schema) => crate::editing::add_entity(shell, schema, &name).map(|id| json!({ "id": id.to_string() })),
                None => run(shell, cmd, params),
            }
        }
        (_, Some(entity)) => crate::editing::run_growing(shell, &tr("Add column"), cmd, params, entity),
        _ => run(shell, cmd, params),
    };
    match out {
        Ok(out) => {
            if let Some(id) = out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse) {
                shell.explorer.expanded.extend(parents);
                shell.explorer.filter.clear();
                let name = shell.session.as_ref().and_then(|s| s.doc().by_uuid(id).map(|o| name_of(s.doc(), o))).unwrap_or_default();
                shell.explorer.rename = Some(Rename::new(id, name, true));
                shell.explorer.focused = true;
            }
        }
        Err(e) => shell.explorer.notice = Some(e),
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
    let cmd = match store.class_of(obj).ok() {
        Some(c) if c == archforge_er::SCHEMA => Some(("er.schema.rename", "schema")),
        Some(c) if c == archforge_er::ENTITY => Some(("er.entity.rename", "entity")),
        Some(c) if c == archforge_er::ATTRIBUTE => Some(("er.attribute.rename", "attribute")),
        _ => None,
    };
    let Some((cmd, key)) = cmd else {
        shell.explorer.rename = None;
        return;
    };
    match run(shell, cmd, json!({ key: r.target.to_string(), "name": name })) {
        Ok(_) => shell.explorer.rename = None,
        Err(e) => {
            if let Some(r) = shell.explorer.rename.as_mut() {
                r.refused = Some(e);
            }
        }
    }
}

/// What goes with `obj` when it is removed: the relationships that reach it, and the keys and
/// foreign keys the ER cascade takes.
pub(crate) fn consequences(store: &Store, obj: Obj) -> (usize, usize, usize) {
    let relationships = match store.class_of(obj).ok() {
        Some(c) if c == archforge_er::ENTITY => store
            .referrers(obj)
            .into_iter()
            .filter(|(_, p)| *p == archforge_er::relationship::FROM || *p == archforge_er::relationship::TO)
            .map(|(r, _)| r)
            .collect::<BTreeSet<_>>()
            .len(),
        Some(c) if c == archforge_er::SCHEMA => {
            let own: BTreeSet<Obj> = children_of(store, obj, archforge_er::schema::RELATIONSHIP).into_iter().collect();
            let reaching: BTreeSet<Obj> = children_of(store, obj, archforge_er::schema::ENTITY)
                .into_iter()
                .flat_map(|e| store.referrers(e))
                .filter(|(_, p)| *p == archforge_er::relationship::FROM || *p == archforge_er::relationship::TO)
                .map(|(r, _)| r)
                .collect();
            own.union(&reaching).count()
        }
        _ => 0,
    };
    let (keys, fks) = archforge_er::commands::dependents(store, obj).unwrap_or_default();
    (relationships, keys.len(), fks.len())
}

/// Removes the selection: at once when nothing else goes with it, else after the dialog.
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
    let (relationships, keys, foreign_keys) = consequences(store, obj);
    // A schema's types, sequences and views count as what goes with it too (slices S5 and S7).
    let has_children = [
        archforge_er::schema::ENTITY,
        archforge_er::schema::DOMAIN,
        archforge_er::schema::ENUM_TYPE,
        archforge_er::schema::SEQUENCE,
        archforge_er::schema::VIEW,
        archforge_er::entity::ATTRIBUTE,
    ]
    .into_iter()
    .any(|prop| !children_of(store, obj, prop).is_empty());
    if relationships + keys + foreign_keys == 0 && !has_children {
        if let Err(e) = run(shell, "element.remove", json!({ "ids": [id.to_string()] })) {
            shell.explorer.notice = Some(e);
        }
        return;
    }
    let (diagrams, _, _) = archforge_diagram::views_of(store, obj);
    shell.explorer.delete =
        Some(Delete { ids: vec![id], name: name_of(store, obj), relationships, keys, foreign_keys, diagrams: diagrams.len(), node: None });
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

/// Every row that can open: the schemas and their tables.
fn foldable(store: &Store) -> Vec<Uuid> {
    let mut out = Vec::new();
    for schema in schemas(store) {
        out.extend(uuid_of(store, schema));
        out.extend(children_of(store, schema, archforge_er::schema::ENTITY).into_iter().filter_map(|e| uuid_of(store, e)));
    }
    out
}

/// Opens the schema and the table of the element selected, once per selection, so its row shows.
fn reveal(state: &mut State, store: &Store, selected: Option<Obj>) {
    let id = selected.and_then(|o| uuid_of(store, o));
    if id == state.revealed {
        return;
    }
    state.revealed = id;
    if let Some(o) = selected {
        let parents = [archforge_er::rules::entity_of(store, o).filter(|e| *e != o), archforge_er::rules::schema_of(store, o).filter(|s| *s != o)];
        state.expanded.extend(parents.into_iter().flatten().filter_map(|p| uuid_of(store, p)));
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
    let folds = shell.session.as_ref().map(|s| foldable(s.doc())).unwrap_or_default();
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
        Some(s) => rows(s.doc(), &shell.explorer),
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
        reveal(&mut shell.explorer, store, selected_obj);
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
    if shell.explorer.focused && no_widget_has_keys && !typing_at_start && shell.explorer.rename.is_none() && shell.explorer.delete.is_none() {
        keyboard(shell, ui, &rows, selected_obj);
    }
    if shell.explorer.menu_open {
        add_menu(shell, ui, p, add);
    }
    drag_ghost(shell, ui, p, &rows);
    // A drag whose row was not drawn when the button went up has ended all the same.
    if shell.explorer.dragging.is_some() && !ui.input(|i| i.pointer.primary_down()) {
        shell.explorer.dragging = None;
    }
}

/// The table being dragged, as a chip at the pointer above everything.
fn drag_ghost(shell: &Shell, ui: &egui::Ui, p: &Palette, rows: &[Row]) {
    let (Some(id), Some(at)) = (shell.explorer.dragging, ui.input(|i| i.pointer.latest_pos())) else { return };
    let Some(row) = rows.iter().find(|r| r.id == id) else { return };
    let paint = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, widgets::egui_id("workspace.explorer.drag")));
    let g = ux::galley(&paint, &row.label, types::SMALL, p.text, Some(220.0));
    let chip = Rect::from_min_size(at + vec2(12.0, 10.0), vec2(36.0 + g.size().x, 28.0));
    ux::shadow(&paint, chip, 6.0, p.shadow);
    paint.rect_filled(chip, 6.0, p.surface);
    paint.rect_stroke(chip, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    ux::icon_in(&paint, Rect::from_min_size(chip.min + vec2(10.0, 7.0), Vec2::splat(14.0)), &icons::TABLE, p.accent);
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
    if row.kind == Kind::Entity {
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
    let icon = match row.kind {
        Kind::Schema => &icons::SCHEMA,
        Kind::Entity => &icons::TABLE,
        Kind::Skill => &icons::SKILL,
        Kind::Domain => &icons::DOMAIN,
        Kind::Enum => &icons::ENUM,
        Kind::Sequence => &icons::SEQUENCE,
        Kind::View => &icons::VIEW,
        Kind::Attribute => &icons::COLUMN,
    };
    ux::icon_mid(&paint, pos2(column.left() + 38.5 + indent, mid), icon, if row.kind == Kind::Attribute { p.text_muted } else { icon_ink });
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
    ux::text_mid(&paint, name_x, mid, &label, types::UI, ink, Some((column.right() - 16.0 - name_x).max(0.0)));
    if resp.clicked() {
        shell.explorer.focused = true;
        shell.explorer.menu_open = false;
        if shell.explorer.rename.is_some() {
            commit_rename(shell);
        }
        if is_selected && !resp.double_clicked() {
            // A second click on the selected row opens its name, as the design's inline edit.
            shell.explorer.rename = Some(Rename::new(row.id, row.label.clone(), false));
        } else {
            select(shell, Some(row.id));
        }
    }
    if resp.double_clicked() && row.expandable {
        toggle(&mut shell.explorer, row);
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
        } else if rename && row.kind != Kind::Skill {
            shell.explorer.rename = Some(Rename::new(row.id, row.label.clone(), false));
        } else if delete {
            remove_selection(shell);
        }
    }
}

/// The `+` menu under its button: what can be added at the selection, Folder shown but not yet
/// live.
fn add_menu(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, anchor: Rect) {
    let store_offers = shell.session.as_ref().map(|s| offers(s.doc(), selected(shell))).unwrap_or_default();
    let mut items: Vec<(Option<Kind>, String)> = store_offers.iter().map(|k| (Some(*k), kind_word(*k))).collect();
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
fn add_menu_rows(shell: &mut Shell, ui: &egui::Ui, p: &Palette, menu: Rect, items: &[(Option<Kind>, String)]) -> Option<Option<Kind>> {
    let width = menu.width();
    let paint = ui.painter().clone();
    ux::shadow(&paint, menu, 8.0, p.shadow);
    paint.rect_filled(menu, 8.0, p.surface);
    paint.rect_stroke(menu, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    for (i, (kind, label)) in items.iter().enumerate() {
        let row = Rect::from_min_size(pos2(menu.left() + 4.0, menu.top() + 4.0 + i as f32 * 30.0), vec2(width - 8.0, 30.0));
        let id = match kind {
            Some(Kind::Schema) => "workspace.explorer.add.schema",
            Some(Kind::Entity) => "workspace.explorer.add.table",
            Some(Kind::Attribute) => "workspace.explorer.add.column",
            Some(Kind::Skill) => "workspace.explorer.add.skill",
            Some(Kind::Domain) => "workspace.explorer.add.domain",
            Some(Kind::Enum) => "workspace.explorer.add.enum",
            Some(Kind::Sequence) => "workspace.explorer.add.sequence",
            Some(Kind::View) => "workspace.explorer.add.view",
            None => "workspace.explorer.add.folder",
        };
        let resp = control(shell, ui, id, "menu-item", row, kind.is_some());
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
    lines.push(tr("What it holds goes with it."));
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
        let g = ux::galley(paint, &label, types::SMALL, p.text, None);
        let b = Rect::from_min_max(pos2(x - g.size().x - 24.0, dialog.bottom() - 48.0), pos2(x, dialog.bottom() - 20.0));
        paint.rect_filled(b, 6.0, if primary { p.danger } else { p.field });
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, if primary { p.on_accent } else { p.text });
        if control(shell, ui, id, "button", b, true).clicked() {
            shell.explorer.delete = None;
            if primary {
                let ids: Vec<String> = d.ids.iter().map(|i| i.to_string()).collect();
                if let Err(e) = run(shell, "element.remove", json!({ "ids": ids })) {
                    shell.explorer.notice = Some(e);
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
