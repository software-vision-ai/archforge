//! The right panels (slice W3 of the workspace chrome, from the design's pages 03 and 04):
//! Properties shows the selection as the ER design does — a schema's name; a table's name and
//! kind, its columns with the in-place column editor, its keys — and Skills shows the skills on
//! the selection, the ones it inherits, and the skill editor. Every change goes through the
//! engine's commands, so undo covers it; a refusal stays with the field in the engine's words.
//! Agent and Appearance say what is true until their slices.
//!
//! The geometry is the design's: a section is 16 px in from the panel's edges with a full-width
//! line over it, a field is 28 px tall and filled, its label 11 px above, a row 36 px, an editor
//! a bordered card.

use archforge_core::SkillTrigger;
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::state::Panel;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

const PAD: f32 = 16.0;
const FIELD: f32 = 28.0;
const ROW: f32 = 36.0;
const MAX_NAME: usize = archforge_er::rules::MAX_NAME;
const MAX_TYPE: usize = archforge_er::rules::MAX_TYPE;

/// What the panels keep between frames: the open editors and the last refusal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// A name field being typed: the element and the text so far.
    pub name: Option<NameEdit>,
    /// The column editor, open on a column or on a column about to be added.
    pub column: Option<ColumnEdit>,
    /// The skill editor, open on a skill or on one about to be added.
    pub skill: Option<SkillEdit>,
    /// The select menu that is open.
    pub menu: Option<Menu>,
    /// The last refusal of a change, shown at the panel's foot until the next change.
    pub notice: Option<String>,
    /// The column whose selection opened the editor, so Cancel and + are not undone next frame.
    pub selected_column: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEdit {
    pub target: Uuid,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnEdit {
    /// The column, or none for one being added to `entity`.
    pub target: Option<Uuid>,
    pub entity: Uuid,
    pub name: String,
    pub ty: String,
    pub primary_key: bool,
    pub not_null: bool,
    pub unique: bool,
    pub refused: Option<String>,
    pub opened: bool,
}

/// What a skill aims at, as the editor's select offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    Subject,
    Tables,
    Relationships,
    Columns,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEdit {
    /// The skill, or none for one being added on `subject`.
    pub target: Option<Uuid>,
    /// The element the skill sits on; none for the model.
    pub subject: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub body: String,
    pub triggers: [bool; 4],
    pub applies: Applies,
    pub type_pattern: String,
    pub dialect: String,
    pub refused: Option<String>,
    pub opened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Kind,
    Applies,
}

/// The selection the panels describe.
#[derive(Debug, Clone, PartialEq)]
struct Subject {
    obj: Option<Obj>,
    id: Option<Uuid>,
    class: Option<archforge_mof::ClassId>,
    name: String,
    /// The kind line under the heading and the word for the selection.
    kind: String,
    word: String,
    /// "This table": the selection itself, as the applies-to choice and the own skill rows say it.
    this: String,
    /// "On this table · {n}": the title of the own skills section.
    on_this: String,
}

fn subject(shell: &Shell, project: &str) -> Subject {
    let Some(s) = shell.session.as_ref() else {
        return Subject::root(project);
    };
    let store = s.doc();
    match s.view().selection.first().copied().filter(|o| store.is_alive(*o)) {
        Some(o) => {
            let class = store.class_of(o).ok();
            let (kind, word, this, on_this) = match class {
                Some(c) if c == archforge_er::SCHEMA => (tr("Schema · model"), tr("Schema"), tr("This schema"), tr("On this schema · {n}")),
                Some(c) if c == archforge_er::ENTITY => {
                    (tr("Table · model, in every diagram"), tr("Table"), tr("This table"), tr("On this table · {n}"))
                }
                Some(c) if c == archforge_er::ATTRIBUTE => (tr("Column · model"), tr("Column"), tr("This column"), tr("On this column · {n}")),
                Some(c) if c == archforge_core::SKILL => (tr("Skill · model"), tr("Skill"), tr("This skill"), tr("On this skill · {n}")),
                Some(c) if c == archforge_er::RELATIONSHIP => {
                    let end = |p| store.get(o, p).ok().and_then(|v| v.as_ref()).map(|e| archforge_er::rules::name_of(store, e)).unwrap_or_default();
                    let kind = fmt(
                        &tr("Relationship · {from} to {to}"),
                        &[("from", &end(archforge_er::relationship::FROM)), ("to", &end(archforge_er::relationship::TO))],
                    );
                    (kind, tr("Relationship"), tr("This relationship"), tr("On this relationship · {n}"))
                }
                Some(c) if c == archforge_diagram::DIAGRAM => {
                    let schema = crate::diagrams::schema_of(store, o).map(|s| archforge_er::rules::name_of(store, s)).unwrap_or_default();
                    (fmt(&tr("Diagram · of {schema}"), &[("schema", &schema)]), tr("Diagram"), tr("This diagram"), tr("On this diagram · {n}"))
                }
                _ => (tr("Element · model"), tr("Element"), tr("This element"), tr("On this element · {n}")),
            };
            let name = match class {
                Some(c) if c == archforge_core::SKILL => archforge_core::skills::name_of(store, o),
                Some(c) if c == archforge_diagram::DIAGRAM => crate::diagrams::diagram_name(store, o),
                _ => archforge_er::rules::name_of(store, o),
            };
            Subject { obj: Some(o), id: store.uuid(o).ok(), class, name, kind, word, this, on_this }
        }
        None => Subject::root(project),
    }
}

impl Subject {
    /// The model root, which the project's name names.
    fn root(project: &str) -> Subject {
        Subject {
            obj: None,
            id: None,
            class: None,
            name: project.to_string(),
            kind: tr("Model · the project's root"),
            word: tr("Model"),
            this: tr("This model"),
            on_this: tr("On this model · {n}"),
        }
    }
}

fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

fn string_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn children(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|c| store.is_alive(*c)).collect()
}

/// Drops editors whose elements are gone.
pub fn retain_valid(state: &mut State, store: &Store) {
    if state.name.as_ref().is_some_and(|n| store.by_uuid(n.target).is_none()) {
        state.name = None;
    }
    if state.column.as_ref().is_some_and(|c| store.by_uuid(c.entity).is_none() || c.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.column = None;
    }
    if state
        .skill
        .as_ref()
        .is_some_and(|s| s.target.is_some_and(|t| store.by_uuid(t).is_none()) || s.subject.is_some_and(|t| store.by_uuid(t).is_none()))
    {
        state.skill = None;
    }
}

// Shared drawing.

/// A section's full-width top line and its title row with an optional `+`; gives the y under it,
/// or none when the `+` was clicked this frame.
fn section(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, title: &str, add: Option<(&str, &str)>) -> Option<f32> {
    let paint = ui.painter();
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)), 0.0, p.border);
    ux::text(paint, pos2(r.left() + PAD, y + 17.0), title, types::UI, p.text, Some((r.width() - 2.0 * PAD - 40.0).max(0.0)));
    if let Some((id, hint)) = add {
        let b = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
        ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
        if control(shell, ui, id, "button", b, true).on_hover_text(tr(hint)).clicked() {
            return None;
        }
    }
    Some(y + 50.0)
}

/// A label over a filled text field that commits on Enter or when it loses the keyboard; gives
/// the text as typed and whether it was committed this frame.
fn text_field(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    text: &mut String,
    limit: usize,
    take_focus: bool,
    commit_on_blur: bool,
) -> bool {
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, id, "field", field);
    let inner =
        Rect::from_min_max(pos2(field.left() + 8.0, field.top() + 4.0), pos2((field.right() - 8.0).max(field.left() + 8.0), field.bottom() - 4.0));
    let eid = widgets::egui_id(id);
    let edit = egui::TextEdit::singleline(text)
        .id(eid)
        .frame(egui::Frame::NONE)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(limit)
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    if take_focus {
        out.response.request_focus();
    }
    if out.response.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    let enter = out.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    enter || (commit_on_blur && out.response.lost_focus())
}

/// A filled select showing `value`; a click opens or closes `menu`.
fn select_field(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, width: f32, id: &str, label: &str, value: &str, menu: Menu) -> Rect {
    let paint = ui.painter();
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text(paint, field.min + vec2(8.0, 5.0), value, types::SMALL, p.text, Some((width - 36.0).max(0.0)));
    ux::icon(paint, pos2(field.right() - 22.0, field.top() + 6.0), &icons::CHEVRON_DOWN, 16.0, p.text_secondary);
    if control(shell, ui, id, "select", field, true).clicked() {
        shell.panels.menu = if shell.panels.menu == Some(menu) { None } else { Some(menu) };
    }
    field
}

/// A menu under `anchor` with `items`; gives the index picked, closing the menu on a pick or a
/// click elsewhere.
fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String]) -> Option<usize> {
    let paint = ui.painter();
    let height = 8.0 + items.len() as f32 * 30.0;
    let r = Rect::from_min_size(pos2(anchor.left(), anchor.bottom() + 4.0), vec2(anchor.width(), height));
    paint.rect_filled(r.expand(2.0), 10.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    for (i, item) in items.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * 30.0), vec2(r.width() - 8.0, 30.0));
        let resp = control(shell, ui, &format!("{id}.{i}"), "menu-item", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 5.0, p.page);
        }
        ux::text(paint, row.min + vec2(10.0, 7.5), item, types::SMALL, p.text, Some((row.width() - 20.0).max(0.0)));
        if resp.clicked() {
            picked = Some(i);
        }
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
    }
    picked
}

/// A check box with its label; a click flips `on`.
fn check(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, id: &str, label: &str, on: &mut bool) -> f32 {
    let paint = ui.painter();
    let b = Rect::from_min_size(at, Vec2::splat(16.0));
    paint.rect_filled(b, 4.0, if *on { p.accent_fill } else { p.surface });
    paint.rect_stroke(b, 4.0, Stroke::new(1.0, if *on { p.accent_fill } else { p.border }), StrokeKind::Inside);
    if *on {
        ux::icon_in(paint, b.shrink(3.0), &icons::TICK, p.on_accent);
    }
    let g = ux::text(paint, at + vec2(24.0, -1.0), label, types::SMALL, p.text, None);
    let hit = Rect::from_min_max(b.min, pos2(g.right(), b.bottom()));
    if control(shell, ui, id, "check", hit, true).clicked() {
        *on = !*on;
    }
    g.right() + 20.0
}

/// An icon button of 28 px: quiet, bordered or primary.
fn icon_button(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    id: &str,
    icon: &ux::Icon,
    ink: egui::Color32,
    fill: Option<egui::Color32>,
    hint: &str,
) -> bool {
    let paint = ui.painter();
    let b = Rect::from_min_size(at, Vec2::splat(FIELD));
    match fill {
        Some(f) => {
            paint.rect_filled(b, 6.0, f);
        }
        None => {
            paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
    }
    ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), icon, ink);
    control(shell, ui, id, "button", b, true).on_hover_text(tr(hint)).clicked()
}

// Properties.

/// Draws the Properties panel's body under the header, from `y`.
fn properties(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    y = section(shell, ui, p, r, y, &subject.word, None).unwrap_or(y + 50.0);
    if subject.class != Some(archforge_er::ATTRIBUTE) {
        shell.panels.selected_column = None;
    }
    match subject.class {
        None => {
            // The model root: its name, which the project's name is.
            ux::text(&paint, pos2(r.left() + PAD, y), &tr("Name"), types::CAPTION, p.text_secondary, Some(width.max(0.0)));
            let field = Rect::from_min_size(pos2(r.left() + PAD, y + 17.0), vec2(width.max(0.0), FIELD));
            paint.rect_filled(field, 6.0, p.field);
            ux::text(&paint, field.min + vec2(8.0, 5.0), &subject.name, types::SMALL, p.text, Some((width - 16.0).max(0.0)));
            control(shell, ui, "workspace.properties.name", "button", field, false);
        }
        Some(c) if c == archforge_er::SCHEMA => {
            name_field(shell, ui, p, pos2(r.left() + PAD, y), width, subject, "er.schema.rename", "schema");
        }
        Some(c) if c == archforge_er::ENTITY => entity_body(shell, ui, p, r, y, subject),
        Some(c) if c == archforge_er::RELATIONSHIP => {
            // A relationship: its name; the ends and the foreign key come with slice E3.
            name_field(shell, ui, p, pos2(r.left() + PAD, y), width, subject, "er.relationship.rename", "relationship");
        }
        Some(c) if c == archforge_diagram::DIAGRAM => {
            // A diagram: its name, and the schema it is of, which does not change.
            let half = (width - 16.0) / 2.0;
            name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, "diagram.rename", "diagram");
            let schema = shell
                .session
                .as_ref()
                .and_then(|s| {
                    let store = s.doc();
                    Some(archforge_er::rules::name_of(store, crate::diagrams::schema_of(store, subject.obj?)?))
                })
                .unwrap_or_default();
            let at = pos2(r.left() + PAD + half + 16.0, y);
            ux::text(&paint, at, &tr("Schema"), types::CAPTION, p.text_secondary, Some(half.max(0.0)));
            let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(half.max(0.0), FIELD));
            paint.rect_filled(field, 6.0, p.field);
            ux::text(&paint, field.min + vec2(8.0, 5.0), &schema, types::SMALL, p.text, Some((half - 36.0).max(0.0)));
            ux::icon(&paint, pos2(field.right() - 22.0, field.top() + 6.0), &icons::CHEVRON_DOWN, 16.0, p.text_muted);
            control(shell, ui, "workspace.properties.schema", "select", field, false);
        }
        Some(c) if c == archforge_er::ATTRIBUTE => {
            // A column: its table's panel, the column's editor open on it.
            let (entity, entity_id) = shell
                .session
                .as_ref()
                .and_then(|s| {
                    let store = s.doc();
                    let e = archforge_er::rules::entity_of(store, subject.obj?)?;
                    Some((e, store.uuid(e).ok()?))
                })
                .unzip();
            if let (Some(entity), Some(entity_id), Some(id)) = (entity, entity_id, subject.id) {
                if shell.panels.selected_column != Some(id) {
                    shell.panels.selected_column = Some(id);
                    open_column(shell, entity_id, Some(id));
                }
                let store = shell.session.as_ref().map(|s| s.doc());
                let name = store.map(|st| archforge_er::rules::name_of(st, entity)).unwrap_or_default();
                let table = Subject {
                    obj: Some(entity),
                    id: Some(entity_id),
                    class: Some(archforge_er::ENTITY),
                    name,
                    kind: tr("Table · model, in every diagram"),
                    this: tr("This table"),
                    on_this: tr("On this table · {n}"),
                    word: tr("Table"),
                };
                entity_body(shell, ui, p, r, y, &table);
            }
        }
        _ => {}
    }
}

/// The name field of a schema or table, committed through its rename command.
fn name_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject, cmd: &str, key: &str) {
    let Some(id) = subject.id else { return };
    let mut text = match &shell.panels.name {
        Some(n) if n.target == id => n.text.clone(),
        _ => subject.name.clone(),
    };
    let committed = text_field(shell, ui, p, at, width, "workspace.properties.name", &tr("Name"), &mut text, MAX_NAME, false, true);
    if committed {
        shell.panels.name = None;
        if text.trim() != subject.name {
            match run(shell, cmd, json!({ key: id.to_string(), "name": text.trim() })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    } else if text != subject.name {
        shell.panels.name = Some(NameEdit { target: id, text });
    } else if shell.panels.name.as_ref().is_some_and(|n| n.target == id) {
        shell.panels.name = None;
    }
}

/// One column as the panel lists it.
struct ColumnRow {
    id: Uuid,
    name: String,
    ty: String,
    primary: bool,
    unique: bool,
    foreign: bool,
}

/// One key as the panel lists it.
struct KeyRow {
    primary: bool,
    name: String,
    columns: String,
}

fn entity_rows(store: &Store, e: Obj) -> (Vec<ColumnRow>, Vec<KeyRow>, bool) {
    let columns = children(store, e, archforge_er::entity::ATTRIBUTE)
        .into_iter()
        .filter_map(|a| {
            Some(ColumnRow {
                id: store.uuid(a).ok()?,
                name: archforge_er::rules::name_of(store, a),
                ty: string_of(store, a, archforge_er::attribute::TYPE),
                primary: archforge_er::rules::in_primary_key(store, a),
                unique: archforge_er::rules::is_unique(store, a),
                foreign: store.referrers(a).iter().any(|(_, p)| *p == archforge_er::foreign_key::COLUMN),
            })
        })
        .collect();
    let key_row = |k: Obj, primary: bool| KeyRow {
        primary,
        name: archforge_er::rules::name_of(store, k),
        columns: archforge_er::rules::key_columns(store, k)
            .into_iter()
            .map(|a| archforge_er::rules::name_of(store, a))
            .collect::<Vec<_>>()
            .join(", "),
    };
    let mut keys: Vec<KeyRow> = archforge_er::rules::primary_key_of(store, e).map(|k| key_row(k, true)).into_iter().collect();
    keys.extend(archforge_er::rules::unique_constraints(store, e).into_iter().map(|k| key_row(k, false)));
    let associative = store.get(e, archforge_er::entity::KIND).ok().and_then(|v| match v {
        Value::Enum(i) => archforge_er::EntityKind::from_index(i),
        _ => None,
    }) == Some(archforge_er::EntityKind::Associative);
    (columns, keys, associative)
}

/// A table: name and kind, its columns with the editor, its keys.
fn entity_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (Some(entity), Some(entity_id)) = (subject.obj, subject.id) else { return };
    let (columns, keys, associative) = match shell.session.as_ref() {
        Some(s) => entity_rows(s.doc(), entity),
        None => return,
    };
    let half = ((width - 8.0) / 2.0).max(0.0);
    name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, "er.entity.rename", "entity");
    let kind_word = if associative { tr("Mapping table") } else { tr("Regular") };
    let kind_field =
        select_field(shell, ui, p, pos2(r.left() + PAD + half + 8.0, y), half, "workspace.properties.kind", &tr("Kind"), &kind_word, Menu::Kind);
    y += 17.0 + FIELD + 16.0;
    // Columns.
    let title = fmt(&tr("Columns · {n}"), &[("n", &columns.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.columns.add", "Add column"))) else {
        open_column(shell, entity_id, None);
        return;
    };
    y = next - 6.0;
    for col in &columns {
        if shell.panels.column.as_ref().is_some_and(|c| c.target == Some(col.id)) {
            y = column_editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), ROW));
        let resp = control(shell, ui, &format!("workspace.properties.column.{}", col.id), "row", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::text(&paint, row.min + vec2(2.0, 10.0), "⋮⋮", types::CAPTION, p.text_muted, None);
        let (badge, tint, ink) = if col.primary {
            (Some("PK"), p.amber_tint, p.amber)
        } else if col.foreign {
            (Some("FK"), p.amber_tint, p.amber)
        } else if col.unique {
            (Some("UQ"), p.accent_tint, p.accent)
        } else {
            (None, p.surface, p.text)
        };
        if let Some(b) = badge {
            let br = Rect::from_min_size(row.min + vec2(24.0, 10.0), vec2(26.0, 16.0));
            paint.rect_filled(br, 4.0, tint);
            let g = ux::galley(&paint, b, types::TINY, ink, None);
            paint.galley(br.center() - g.size() / 2.0, g, ink);
        }
        let name = ux::text(&paint, row.min + vec2(58.0, 8.5), &col.name, types::UI, p.text, Some((width - 58.0 - 40.0).max(0.0)));
        ux::text(
            &paint,
            pos2(name.right() + 8.0, row.top() + 10.5),
            &col.ty,
            types::CAPTION,
            p.text_secondary,
            Some((row.right() - name.right() - 40.0).max(0.0)),
        );
        if resp.hovered() {
            ux::icon_in(&paint, Rect::from_min_size(pos2(row.right() - 24.0, row.top() + 10.0), Vec2::splat(16.0)), &icons::EDIT, p.text_secondary);
        }
        if resp.clicked() {
            open_column(shell, entity_id, Some(col.id));
        }
        y += ROW;
    }
    if shell.panels.column.as_ref().is_some_and(|c| c.entity == entity_id && c.target.is_none()) {
        y = column_editor(shell, ui, p, r, y);
    }
    y += 10.0;
    // Keys.
    let title = fmt(&tr("Keys · {n}"), &[("n", &keys.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
    for key in &keys {
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 44.0));
        let (b, tint, ink) = if key.primary { ("PK", p.amber_tint, p.amber) } else { ("UQ", p.accent_tint, p.accent) };
        let br = Rect::from_min_size(row.min + vec2(2.0, 6.0), vec2(26.0, 16.0));
        paint.rect_filled(br, 4.0, tint);
        let g = ux::galley(&paint, b, types::TINY, ink, None);
        paint.galley(br.center() - g.size() / 2.0, g, ink);
        let name = if key.name.is_empty() { if key.primary { tr("Primary key") } else { tr("Unique") } } else { key.name.clone() };
        ux::text(&paint, row.min + vec2(38.0, 4.0), &name, types::UI, p.text, Some((width - 38.0).max(0.0)));
        ux::text(&paint, row.min + vec2(38.0, 23.0), &key.columns, types::CAPTION, p.text_secondary, Some((width - 38.0).max(0.0)));
        y += 44.0;
    }
    // The kind menu last, over the rows.
    if shell.panels.menu == Some(Menu::Kind) {
        let items = [tr("Regular"), tr("Mapping table")];
        if let Some(i) = menu(shell, ui, p, kind_field, "workspace.properties.kind", &items) {
            let kind = if i == 1 { "associative" } else { "regular" };
            match run(shell, "er.entity.set-kind", json!({ "entity": entity_id.to_string(), "kind": kind })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
}

/// Opens the column editor on `target`, or on a column to add to `entity`.
pub(crate) fn open_column(shell: &mut Shell, entity: Uuid, target: Option<Uuid>) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let edit = match target.and_then(|t| store.by_uuid(t)) {
        Some(a) => ColumnEdit {
            target,
            entity,
            name: archforge_er::rules::name_of(store, a),
            ty: string_of(store, a, archforge_er::attribute::TYPE),
            primary_key: archforge_er::rules::in_primary_key(store, a),
            not_null: !archforge_er::rules::is_nullable(store, a),
            unique: archforge_er::rules::is_unique(store, a),
            refused: None,
            opened: true,
        },
        None => {
            let taken = store
                .by_uuid(entity)
                .map(|e| {
                    children(store, e, archforge_er::entity::ATTRIBUTE)
                        .into_iter()
                        .map(|a| archforge_er::rules::name_of(store, a))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let name = crate::explorer::fresh_name("column", taken.into_iter(), true).unwrap_or_default();
            ColumnEdit {
                target: None,
                entity,
                name,
                ty: "TEXT".into(),
                primary_key: false,
                not_null: false,
                unique: false,
                refused: None,
                opened: true,
            }
        }
    };
    shell.panels.column = Some(edit);
    shell.panels.menu = None;
}

/// The column editor card, as the design's; gives the y under it.
fn column_editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let Some(mut edit) = shell.panels.column.clone() else { return y };
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 206.0 + if edit.refused.is_some() { 20.0 } else { 0.0 }));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = edit.opened;
    edit.opened = false;
    let name_done =
        text_field(shell, ui, p, at(0.0, 12.0), inner, "workspace.properties.column.name", &tr("Name"), &mut edit.name, MAX_NAME, take, false);
    let type_done =
        text_field(shell, ui, p, at(0.0, 68.0), inner, "workspace.properties.column.type", &tr("Type"), &mut edit.ty, MAX_TYPE, false, false);
    // The checks in a row: the second starts where the first's label ended.
    let next_x = check(shell, ui, p, at(0.0, 126.0), "workspace.properties.column.primary-key", &tr("Primary key"), &mut edit.primary_key);
    check(shell, ui, p, pos2(next_x - 12.0, card.top() + 126.0), "workspace.properties.column.not-null", &tr("Not null"), &mut edit.not_null);
    check(shell, ui, p, at(0.0, 154.0), "workspace.properties.column.unique", &tr("Unique"), &mut edit.unique);
    let actions_y = 166.0 + 14.0;
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, actions_y), "workspace.properties.column.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + actions_y),
        "workspace.properties.column.cancel",
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - FIELD, card.top() + actions_y),
        "workspace.properties.column.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, 206.0), why, types::CAPTION, p.amber, Some(inner));
    }
    let enter = name_done || type_done;
    shell.panels.column = Some(edit.clone());
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.column = None;
    } else if remove {
        if let Some(t) = edit.target {
            match run(shell, "er.attribute.remove", json!({ "attribute": t.to_string() })) {
                Ok(_) => shell.panels.column = None,
                Err(e) => set_column_refusal(shell, e),
            }
        }
    } else if apply || enter {
        commit_column(shell, &edit);
    }
    card.bottom() + 8.0
}

fn set_column_refusal(shell: &mut Shell, why: String) {
    if let Some(c) = shell.panels.column.as_mut() {
        c.refused = Some(why);
    }
}

/// Writes the column editor through the commands: add, or rename, type and flags.
fn commit_column(shell: &mut Shell, edit: &ColumnEdit) {
    let nullable = if edit.not_null { "no" } else { "yes" };
    let outcome = match edit.target {
        None => crate::editing::run_growing(
            shell,
            &tr("Add column"),
            "er.attribute.add",
            json!({ "entity": edit.entity.to_string(), "name": edit.name.trim(), "type": edit.ty.trim(), "primary_key": edit.primary_key, "nullable": nullable, "unique": edit.unique }),
            edit.entity,
        )
        .map(|_| ()),
        Some(t) => {
            let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
            let Some(a) = store.by_uuid(t) else { return };
            let (old_name, old_type) = (archforge_er::rules::name_of(store, a), string_of(store, a, archforge_er::attribute::TYPE));
            let id = t.to_string();
            let mut result = Ok(());
            if edit.name.trim() != old_name {
                result = run(shell, "er.attribute.rename", json!({ "attribute": id, "name": edit.name.trim() })).map(|_| ());
            }
            if result.is_ok() && edit.ty.trim() != old_type {
                result = run(shell, "er.attribute.set-type", json!({ "attribute": id, "type": edit.ty.trim() })).map(|_| ());
            }
            if result.is_ok() {
                result = run(
                    shell,
                    "er.attribute.set-flags",
                    json!({ "attribute": id, "primary_key": edit.primary_key, "nullable": nullable, "unique": edit.unique }),
                )
                .map(|_| ());
            }
            result
        }
    };
    match outcome {
        Ok(()) => shell.panels.column = None,
        Err(e) => set_column_refusal(shell, e),
    }
}

// Skills.

/// One skill as the panel lists it.
struct SkillRow {
    id: Uuid,
    name: String,
    /// What it aims at, or where it comes from.
    detail: String,
    trigger: Option<SkillTrigger>,
    own: bool,
}

fn target_word(store: &Store, s: Obj) -> String {
    let applies = string_of(store, s, archforge_core::skill::APPLIES_TO);
    let pattern = string_of(store, s, archforge_core::skill::TYPE_PATTERN);
    match archforge_core::skills::kind_by_key::<archforge_er::ErView>(store, &applies).map(|(_, w)| w) {
        Some("table") => tr("every table"),
        Some("relationship") => tr("every relationship"),
        Some(_) if !pattern.is_empty() => pattern,
        Some(_) => tr("every column"),
        None => String::new(),
    }
}

fn skill_rows(store: &Store, subject: &Subject) -> (Vec<SkillRow>, Vec<SkillRow>) {
    let this = subject.this.clone();
    let subject = subject.obj;
    let own: Vec<SkillRow> = archforge_core::skills::skills(store)
        .into_iter()
        .filter(|s| archforge_core::skills::subject_of(store, *s) == subject)
        .filter_map(|s| {
            let target = target_word(store, s);
            Some(SkillRow {
                id: store.uuid(s).ok()?,
                name: archforge_core::skills::name_of(store, s),
                detail: if target.is_empty() { this.clone() } else { target },
                trigger: archforge_core::skills::triggers_of(store, s).first().copied(),
                own: true,
            })
        })
        .collect();
    let inherited = match subject {
        Some(element) => archforge_core::skills::effective_skills::<archforge_er::ErView>(store, element, None)
            .into_iter()
            .filter(|e| e.origin != Some(element))
            .filter_map(|e| {
                let origin = match e.origin {
                    Some(o) => archforge_er::rules::name_of(store, o),
                    None => tr("project"),
                };
                Some(SkillRow {
                    id: store.uuid(e.skill).ok()?,
                    name: archforge_core::skills::name_of(store, e.skill),
                    detail: if e.overridden_by.is_some() { tr("overridden below") } else { origin },
                    trigger: archforge_core::skills::triggers_of(store, e.skill).first().copied(),
                    own: false,
                })
            })
            .collect(),
        None => Vec::new(),
    };
    (own, inherited)
}

fn trigger_icon(t: Option<SkillTrigger>) -> Option<&'static ux::Icon> {
    match t? {
        SkillTrigger::Modelling => Some(&icons::TABLE),
        SkillTrigger::Drawing => Some(&icons::BRUSH),
        SkillTrigger::Naming => Some(&icons::NAMING),
        SkillTrigger::Export => Some(&icons::EXPORT),
    }
}

/// Draws the Skills panel's body under the header, from `y`.
fn skills(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (own, inherited) = match shell.session.as_ref() {
        Some(s) => skill_rows(s.doc(), subject),
        None => return,
    };
    let title = fmt(&subject.on_this, &[("n", &own.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.skills.add", "Add skill"))) else {
        open_skill(shell, subject.id, None);
        return;
    };
    y = next - 6.0;
    let editing_new = shell.panels.skill.as_ref().is_some_and(|e| e.target.is_none());
    for row in &own {
        if shell.panels.skill.as_ref().is_some_and(|e| e.target == Some(row.id)) {
            y = skill_editor(shell, ui, p, r, y, subject);
            continue;
        }
        y = skill_row(shell, ui, &paint, p, r, y, row, width);
    }
    if editing_new {
        y = skill_editor(shell, ui, p, r, y, subject);
    }
    y += 10.0;
    let title = fmt(&tr("Inherited · {n}"), &[("n", &inherited.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
    for row in &inherited {
        y = skill_row(shell, ui, &paint, p, r, y, row, width);
    }
}

fn skill_row(shell: &mut Shell, ui: &egui::Ui, paint: &egui::Painter, p: &Palette, r: Rect, y: f32, row: &SkillRow, width: f32) -> f32 {
    let rect = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 30.0));
    let resp = control(shell, ui, &format!("workspace.skills.row.{}", row.id), "row", rect, row.own);
    if resp.hovered() && row.own {
        paint.rect_filled(rect, 5.0, p.page);
    }
    ux::icon_in(paint, Rect::from_min_size(rect.min + vec2(8.0, 6.0), Vec2::splat(18.0)), &icons::SKILL, p.text_secondary);
    let detail = ux::galley(paint, &row.detail, types::CAPTION, p.text_muted, Some((width / 2.0).max(0.0)));
    let trigger_w = if row.trigger.is_some() { 26.0 } else { 0.0 };
    let name_room = (width - 34.0 - detail.size().x - 16.0 - trigger_w).max(0.0);
    ux::text(paint, rect.min + vec2(34.0, 6.5), &row.name, types::SMALL, p.text, Some(name_room));
    paint.galley(pos2(rect.right() - 8.0 - trigger_w - detail.size().x, rect.top() + 8.0), detail, p.text_muted);
    if let Some(icon) = trigger_icon(row.trigger) {
        ux::icon_in(paint, Rect::from_min_size(pos2(rect.right() - 8.0 - 18.0, rect.top() + 6.0), Vec2::splat(18.0)), icon, p.text_secondary);
    }
    if resp.clicked() && row.own {
        let subject_id = shell
            .panels
            .skill
            .as_ref()
            .and_then(|e| e.subject)
            .or_else(|| shell.session.as_ref().and_then(|s| s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok())));
        open_skill(shell, subject_id, Some(row.id));
    }
    y + 30.0
}

/// Opens the skill editor on `target`, or on a skill to add on `subject`.
fn open_skill(shell: &mut Shell, subject: Option<Uuid>, target: Option<Uuid>) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let edit = match target.and_then(|t| store.by_uuid(t)) {
        Some(s) => {
            let triggers = archforge_core::skills::triggers_of(store, s);
            let applies = string_of(store, s, archforge_core::skill::APPLIES_TO);
            let word = archforge_core::skills::kind_by_key::<archforge_er::ErView>(store, &applies).map(|(_, w)| w);
            SkillEdit {
                target,
                subject: archforge_core::skills::subject_of(store, s).and_then(|o| store.uuid(o).ok()),
                name: archforge_core::skills::name_of(store, s),
                description: string_of(store, s, archforge_core::skill::DESCRIPTION),
                body: string_of(store, s, archforge_core::skill::BODY),
                triggers: [
                    triggers.contains(&SkillTrigger::Modelling),
                    triggers.contains(&SkillTrigger::Drawing),
                    triggers.contains(&SkillTrigger::Naming),
                    triggers.contains(&SkillTrigger::Export),
                ],
                applies: match word {
                    Some("table") => Applies::Tables,
                    Some("relationship") => Applies::Relationships,
                    Some(_) => Applies::Columns,
                    None => Applies::Subject,
                },
                type_pattern: string_of(store, s, archforge_core::skill::TYPE_PATTERN),
                dialect: string_of(store, s, archforge_core::skill::DIALECT),
                refused: None,
                opened: true,
            }
        }
        None => {
            let taken = archforge_core::skills::skills(store)
                .into_iter()
                .filter(|s| archforge_core::skills::subject_of(store, *s).and_then(|o| store.uuid(o).ok()) == subject)
                .map(|s| archforge_core::skills::name_of(store, s));
            SkillEdit {
                target: None,
                subject,
                name: crate::explorer::fresh_name("Skill", taken, false).unwrap_or_default(),
                description: String::new(),
                body: String::new(),
                triggers: [false; 4],
                applies: Applies::Subject,
                type_pattern: String::new(),
                dialect: String::new(),
                refused: None,
                opened: true,
            }
        }
    };
    shell.panels.skill = Some(edit);
    shell.panels.menu = None;
}

fn applies_word(a: Applies, this: &str) -> String {
    match a {
        Applies::Subject => this.to_string(),
        Applies::Tables => tr("Every table below"),
        Applies::Relationships => tr("Every relationship below"),
        Applies::Columns => tr("Columns of a type"),
    }
}

/// The skill editor card, as the design's; gives the y under it.
fn skill_editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) -> f32 {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let Some(mut edit) = shell.panels.skill.clone() else { return y };
    let inner = (width - 24.0).max(0.0);
    let export = edit.triggers[3];
    let columns = edit.applies == Applies::Columns;
    let mut height = 12.0 + 45.0 + 10.0 + 45.0 + 10.0 + 17.0 + 32.0 + 10.0 + 17.0 + 120.0 + 10.0 + FIELD + 12.0;
    if columns {
        height += 55.0;
    }
    if export {
        height += 55.0;
    }
    if edit.refused.is_some() {
        height += 20.0;
    }
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = edit.opened;
    edit.opened = false;
    let mut dy = 12.0;
    let name_done = text_field(shell, ui, p, at(0.0, dy), inner, "workspace.skills.name", &tr("Name"), &mut edit.name, MAX_NAME, take, false);
    dy += 55.0;
    let applies_field = select_field(
        shell,
        ui,
        p,
        at(0.0, dy),
        inner,
        "workspace.skills.applies",
        &tr("Applies to"),
        &applies_word(edit.applies, &subject.this),
        Menu::Applies,
    );
    dy += 55.0;
    if columns {
        text_field(
            shell,
            ui,
            p,
            at(0.0, dy),
            inner,
            "workspace.skills.type",
            &tr("Column type"),
            &mut edit.type_pattern,
            archforge_core::skills::MAX_TYPE_PATTERN,
            false,
            false,
        );
        dy += 55.0;
    }
    ux::text(&paint, at(0.0, dy), &tr("When"), types::CAPTION, p.text_secondary, Some(inner));
    dy += 17.0;
    for (i, (id, icon, hint)) in [
        ("modelling", &icons::TABLE, "Modelling"),
        ("drawing", &icons::BRUSH, "Drawing"),
        ("naming", &icons::NAMING, "Naming"),
        ("export", &icons::EXPORT, "Export"),
    ]
    .iter()
    .enumerate()
    {
        let b = Rect::from_min_size(at(i as f32 * 36.0, dy), Vec2::splat(32.0));
        let on = edit.triggers[i];
        paint.rect_filled(b, 8.0, if on { p.accent_tint } else { p.surface });
        if !on {
            paint.rect_stroke(b, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(18.0)), icon, if on { p.accent } else { p.text_secondary });
        if control(shell, ui, &format!("workspace.skills.when.{id}"), "toggle", b, true).on_hover_text(tr(hint)).clicked() {
            edit.triggers[i] = !on;
        }
    }
    dy += 42.0;
    if export {
        text_field(
            shell,
            ui,
            p,
            at(0.0, dy),
            inner,
            "workspace.skills.dialect",
            &tr("Dialect"),
            &mut edit.dialect,
            archforge_core::skills::MAX_DIALECT,
            false,
            false,
        );
        dy += 55.0;
    }
    ux::text(&paint, at(0.0, dy), &tr("Guidance"), types::CAPTION, p.text_secondary, Some(inner));
    dy += 17.0;
    let body = Rect::from_min_size(at(0.0, dy), vec2(inner, 120.0));
    paint.rect_filled(body, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, "workspace.skills.body", "field", body);
    let body_inner = body.shrink(8.0);
    let out = egui::TextEdit::multiline(&mut edit.body)
        .id(widgets::egui_id("workspace.skills.body"))
        .frame(egui::Frame::NONE)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(archforge_core::skills::MAX_BODY)
        .desired_width(body_inner.width())
        .desired_rows(5)
        .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(body_inner)));
    if out.response.has_focus() {
        paint.rect_stroke(body, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    dy += 130.0;
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, dy), "workspace.skills.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + dy),
        "workspace.skills.cancel",
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - FIELD, card.top() + dy),
        "workspace.skills.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, dy + FIELD + 6.0), why, types::CAPTION, p.amber, Some(inner));
    }
    shell.panels.skill = Some(edit.clone());
    if shell.panels.menu == Some(Menu::Applies) {
        let items = [Applies::Subject, Applies::Tables, Applies::Relationships, Applies::Columns].map(|a| applies_word(a, &subject.this));
        if let Some(i) = menu(shell, ui, p, applies_field, "workspace.skills.applies", &items)
            && let Some(e) = shell.panels.skill.as_mut()
        {
            e.applies = [Applies::Subject, Applies::Tables, Applies::Relationships, Applies::Columns][i.min(3)];
        }
    }
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape)) && shell.panels.menu.is_none();
    if cancel || escape {
        shell.panels.skill = None;
    } else if remove {
        if let Some(t) = edit.target {
            match run(shell, "skill.remove", json!({ "skill": t.to_string() })) {
                Ok(_) => shell.panels.skill = None,
                Err(e) => set_skill_refusal(shell, e),
            }
        }
    } else if apply || name_done {
        commit_skill(shell, &edit);
    }
    card.bottom() + 8.0
}

fn set_skill_refusal(shell: &mut Shell, why: String) {
    if let Some(s) = shell.panels.skill.as_mut() {
        s.refused = Some(why);
    }
}

/// Writes the skill editor through `skill.add` or `skill.set`.
fn commit_skill(shell: &mut Shell, edit: &SkillEdit) {
    let triggers: Vec<&str> = [SkillTrigger::Modelling, SkillTrigger::Drawing, SkillTrigger::Naming, SkillTrigger::Export]
        .iter()
        .zip(edit.triggers)
        .filter(|(_, on)| *on)
        .map(|(t, _)| t.literal())
        .collect();
    let applies_to = match edit.applies {
        Applies::Subject => "",
        Applies::Tables => "table",
        Applies::Relationships => "relationship",
        Applies::Columns => "column",
    };
    let type_pattern = if edit.applies == Applies::Columns { edit.type_pattern.trim() } else { "" };
    let dialect = if edit.triggers[3] { edit.dialect.trim() } else { "" };
    let fields = json!({
        "name": edit.name.trim(), "description": edit.description, "body": edit.body, "triggers": triggers,
        "dialect": dialect, "applies_to": applies_to, "type_pattern": type_pattern,
    });
    let outcome = match edit.target {
        None => {
            let mut params = fields;
            params["subject"] = json!(edit.subject.map(|s| s.to_string()).unwrap_or_default());
            run(shell, "skill.add", params)
        }
        Some(t) => {
            let mut params = fields;
            params["skill"] = json!(t.to_string());
            run(shell, "skill.set", params)
        }
    };
    match outcome {
        Ok(_) => {
            shell.panels.skill = None;
            // skill.add selects the new skill; the panel stays on the element it sits on.
            let ids: Vec<String> = edit.subject.into_iter().map(|s| s.to_string()).collect();
            let _ = run(shell, "view.select", json!({ "ids": ids }));
        }
        Err(e) => set_skill_refusal(shell, e),
    }
}

/// Every text that reaches `tr` through a variable: the hover hints of the icon buttons and the
/// trigger toggles, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add column", "Add skill", "Remove", "Cancel", "Apply", "Modelling", "Drawing", "Naming", "Export"].into_iter()
}

// The panel.

/// Draws the panel body for `panel` under its header, from `y`: Properties and Skills follow the
/// selection; Agent and Appearance say what is true.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, panel: Panel, project: &str) {
    let paint = ui.painter().clone();
    let s = subject(shell, project);
    match panel {
        Panel::Agent => {
            ux::text(&paint, r.min + vec2(PAD, y), &tr("No agent connected"), types::UI, p.text_muted, Some((r.width() - 2.0 * PAD).max(0.0)));
            return;
        }
        Panel::Appearance => {
            ux::text(&paint, r.min + vec2(PAD, y), &tr("Nothing selected"), types::UI, p.text_muted, Some((r.width() - 2.0 * PAD).max(0.0)));
            return;
        }
        Panel::Properties | Panel::Skills => {}
    }
    ux::text(&paint, r.min + vec2(PAD, y), &s.name, types::CARD_TITLE, p.text, Some((r.width() - 2.0 * PAD).max(0.0)));
    ux::text(&paint, r.min + vec2(PAD, y + 26.0), &s.kind, types::CAPTION, p.text_secondary, Some((r.width() - 2.0 * PAD).max(0.0)));
    let body_y = r.top() + y + 51.0;
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(r.left(), body_y), r.max)));
    clip.set_clip_rect(Rect::from_min_max(pos2(r.left(), body_y), r.max));
    match panel {
        Panel::Properties => properties(shell, &mut clip, p, r, body_y, &s),
        Panel::Skills => skills(shell, &mut clip, p, r, body_y, &s),
        _ => {}
    }
    if let Some(notice) = shell.panels.notice.clone() {
        ux::text(&paint, pos2(r.left() + PAD, r.bottom() - 36.0), &notice, types::CAPTION, p.amber, Some((r.width() - 2.0 * PAD).max(0.0)));
    }
}
