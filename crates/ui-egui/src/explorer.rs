//! The Models explorer (slice W2 of the workspace chrome, from the design's page 03 frames and
//! the cards "Models + menu" and "First schema named in place"): the tree of the open project's
//! model over the session's store, starting at the schemas, with a schema's skills and tables
//! under it and a table's skills and columns under that. A click selects through `view.select`,
//! the `+` adds a schema, table or column through the ER commands and opens the name for typing
//! in place, Enter or F2 renames, Delete removes through `er.remove` after a dialog when other
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
    /// The rows folded shut; everything else is open.
    pub collapsed: BTreeSet<Uuid>,
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
}

/// What a row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Schema,
    Entity,
    Attribute,
    Skill,
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
    archforge_er::rules::name_of(store, obj)
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

/// The tree's rows for the store, the collapsed set and the filter: a filter shows the rows whose
/// name holds it and their ancestors, all open.
pub fn rows(store: &Store, state: &State) -> Vec<Row> {
    let filter = state.filter.trim().to_lowercase();
    let mut out = Vec::new();
    let push = |out: &mut Vec<Row>, obj: Obj, depth: u8, kind: Kind, label: String, detail: Option<String>, expandable: bool, expanded: bool| {
        if let Some(id) = uuid_of(store, obj) {
            out.push(Row { id, obj, depth, kind, label, detail, expandable, expanded });
        }
    };
    let open = |obj: Obj| -> bool { !filter.is_empty() || uuid_of(store, obj).is_none_or(|id| !state.collapsed.contains(&id)) };
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
                let ty = store.get(*a, archforge_er::attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string)).filter(|t| !t.is_empty());
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
        let expandable = if filter.is_empty() { !schema_skills.is_empty() || !entities.is_empty() } else { !rows_under.is_empty() };
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
            ("er.attribute.add", json!({ "entity": entity_id.to_string(), "name": name, "type": "TEXT" }))
        }
        Kind::Skill => return,
    };
    match run(shell, cmd, params) {
        Ok(out) => {
            if let Some(id) = out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse) {
                for parent in parents {
                    shell.explorer.collapsed.remove(&parent);
                }
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
fn consequences(store: &Store, obj: Obj) -> (usize, usize, usize) {
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
    let has_children =
        !children_of(store, obj, archforge_er::schema::ENTITY).is_empty() || !children_of(store, obj, archforge_er::entity::ATTRIBUTE).is_empty();
    if relationships + keys + foreign_keys == 0 && !has_children {
        if let Err(e) = run(shell, "er.remove", json!({ "ids": [id.to_string()] })) {
            shell.explorer.notice = Some(e);
        }
        return;
    }
    shell.explorer.delete = Some(Delete { ids: vec![id], name: name_of(store, obj), relationships, keys, foreign_keys });
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
    state.collapsed.retain(|id| store.by_uuid(*id).is_some());
}

/// Draws the Models explorer into `r`: the title with `+`, the filter field, the tree.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    let at = |x: f32, y: f32| r.min + vec2(x, y);
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
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, row) in rows.iter().enumerate() {
        let y = r.top() + FIRST_ROW + i as f32 * ROW;
        if y + ROW_HEIGHT < tree.top() || y - 7.0 > tree.bottom() {
            continue;
        }
        tree_row(shell, &mut clip, p, r, y, row, selected_obj == Some(row.obj));
    }
    if rows.len() >= MAX_ROWS {
        let y = r.top() + FIRST_ROW + rows.len() as f32 * ROW;
        ux::text(
            &paint,
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
}

/// The filter field: a real text field narrowing the tree as it is typed.
fn filter_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 8.0, p.page);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon(&paint, field.min + vec2(10.0, 9.0), &icons::SEARCH, 13.0, p.text_secondary);
    widgets::record_visible(ui, &mut shell.registry, "workspace.explorer.filter", "field", field);
    let inner = Rect::from_min_max(pos2(field.left() + 30.0, field.top() + 4.0), pos2(field.right() - 8.0, field.bottom() - 4.0));
    let edit = egui::TextEdit::singleline(&mut shell.explorer.filter)
        .id(widgets::egui_id("workspace.explorer.filter"))
        .frame(egui::Frame::NONE)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(tr("Filter models…")).color(p.text_secondary).font(types::SMALL.font()))
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let resp = ui.put(inner, edit);
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
    let resp = control(shell, ui, &id, "row", hit, true);
    if row.expandable {
        let chevron = Rect::from_min_size(pos2(column.left() + 13.0 + indent, y), Vec2::splat(16.0));
        ux::icon_in(&paint, chevron, if row.expanded { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, p.text_secondary);
        if ui.interact(chevron.expand(4.0), widgets::egui_id(&format!("{id}.fold")), Sense::click()).clicked() {
            if row.expanded {
                shell.explorer.collapsed.insert(row.id);
            } else {
                shell.explorer.collapsed.remove(&row.id);
            }
            return;
        }
    }
    let ink = if is_selected { p.accent } else { p.text };
    let icon_at = Rect::from_min_size(pos2(column.left() + 31.0 + indent, y), Vec2::splat(15.0));
    match row.kind {
        Kind::Schema => ux::icon_in(&paint, icon_at, &icons::SCHEMA, if is_selected { p.accent } else { p.text_secondary }),
        Kind::Entity => ux::icon_in(&paint, icon_at, &icons::TABLE, if is_selected { p.accent } else { p.text_secondary }),
        Kind::Skill => ux::icon_in(&paint, icon_at, &icons::SKILL, if is_selected { p.accent } else { p.text_secondary }),
        Kind::Attribute => {
            ux::text(&paint, pos2(icon_at.left() + 1.0, y - 1.0), "a", types::CAPTION, p.text_muted, None);
        }
    }
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
    ux::text(&paint, pos2(name_x, y - 1.0), &label, types::UI, ink, Some((column.right() - 16.0 - name_x).max(0.0)));
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
        if row.expanded {
            shell.explorer.collapsed.insert(row.id);
        } else {
            shell.explorer.collapsed.remove(&row.id);
        }
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
    let inner = Rect::from_min_max(pos2(field.left() + 6.0, field.top() + 4.0), pos2(field.right() - 6.0, field.bottom() - 4.0));
    let edit = egui::TextEdit::singleline(&mut r.text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(types::UI.font())
        .text_color(p.text)
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
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
                shell.explorer.collapsed.insert(row.id);
            } else if let Some(parent) = rows.get(..i).unwrap_or(&[]).iter().rev().find(|r| r.depth < row.depth) {
                select(shell, Some(parent.id));
            }
        } else if right && row.expandable {
            shell.explorer.collapsed.remove(&row.id);
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
    let paint = ui.painter().clone();
    paint.rect_filled(menu.expand(2.0), 10.0, p.shadow);
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
    paint.rect_filled(dialog.expand(4.0), 14.0, p.shadow);
    paint.rect_filled(dialog, 10.0, p.surface);
    paint.rect_stroke(dialog, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    let at = |x: f32, y: f32| dialog.min + vec2(x, y);
    ux::text(paint, at(20.0, 20.0), &fmt(&tr("Delete {name} from the model?"), &[("name", &d.name)]), types::CARD_TITLE, p.text, Some(size.x - 40.0));
    for (i, line) in lines.iter().enumerate() {
        ux::text(paint, at(20.0, 58.0 + i as f32 * 20.0), line, types::UI, p.text_secondary, Some(size.x - 40.0));
    }
    let mut x = dialog.right() - 20.0;
    for (id, label, primary) in [("workspace.explorer.delete.confirm", tr("Delete"), true), ("workspace.explorer.delete.cancel", tr("Cancel"), false)]
    {
        let g = ux::galley(paint, &label, types::SMALL, p.text, None);
        let b = Rect::from_min_max(pos2(x - g.size().x - 24.0, dialog.bottom() - 48.0), pos2(x, dialog.bottom() - 20.0));
        paint.rect_filled(b, 6.0, if primary { p.danger } else { p.field });
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, if primary { p.on_accent } else { p.text });
        if control(shell, ui, id, "button", b, true).clicked() {
            shell.explorer.delete = None;
            if primary {
                let ids: Vec<String> = d.ids.iter().map(|i| i.to_string()).collect();
                if let Err(e) = run(shell, "er.remove", json!({ "ids": ids })) {
                    shell.explorer.notice = Some(e);
                }
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
