//! The Diagrams explorer (slice W4 of the workspace chrome, from the design's "Diagram navigation"
//! frame and the card "New diagram (dialog)"): the tree of the open project's diagrams, in the
//! folders the model keeps them in, with the model behind the open diagram named under the tree.
//! A click opens the diagram in the centre and selects it, so Properties shows it. The `+`, like
//! the one in the tab strip, opens the New diagram dialog: a name, the kind and the schema the
//! diagram is of, or a new schema when the model has none yet (owner decision archforge-tec).
//! Every change goes through the engine, so undo and redo cover it.
//!
//! The geometry is the Models tree's: rows 34 px apart from y 119, a level 13 px deeper than its
//! parent, the chevron at x 13, the icon at x 31 and the name at x 53 of the first level; the
//! footer 128 px above the column's bottom.

use std::collections::BTreeSet;

use archforge_diagram::{DIAGRAM, diagram};
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::tr;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

const FIRST_ROW: f32 = 119.0;
const ROW: f32 = 34.0;
const ROW_HEIGHT: f32 = 32.0;
const INDENT: f32 = 13.0;
/// The most rows the tree draws and the deepest folder level it enters.
const MAX_ROWS: usize = 2000;
const MAX_DEPTH: u8 = 32;
/// The most characters a diagram's name may have: the diagram rule's.
const MAX_NAME: usize = archforge_diagram::rules::MAX_NAME;
/// The footer's lines, from the column's bottom.
const FOOTER_LINE: f32 = 128.0;
const FOOTER_TITLE: f32 = 105.0;
const FOOTER_TEXT: f32 = 80.0;

/// What the explorer keeps between frames: nothing the model holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// The folders folded shut; everything else is open.
    pub collapsed: BTreeSet<Uuid>,
    /// The filter field's text.
    pub filter: String,
    /// The New diagram dialog, while it is open.
    pub new: Option<NewDiagram>,
    /// The last refusal, shown under the tree until the next action.
    pub notice: Option<String>,
    /// How far the tree is scrolled, and the row it last brought into view.
    pub scroll: crate::explorer::TreeScroll,
}

/// The New diagram dialog's fields: the name, the schema the diagram is of (none for a new
/// schema named like the diagram), the select that is open, the engine's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDiagram {
    pub name: String,
    pub schema: Option<Uuid>,
    pub menu: Option<Field>,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once.
    pub opened: bool,
}

/// The dialog's selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Kind,
    Schema,
}

/// What a row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Folder,
    Diagram,
}

/// One row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: Uuid,
    pub obj: Obj,
    pub depth: u8,
    pub kind: Kind,
    pub label: String,
    pub expandable: bool,
    pub expanded: bool,
}

/// A diagram's name as stored, or nothing.
pub fn diagram_name(store: &Store, d: Obj) -> String {
    store.get(d, diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The schema a diagram is of, when it has one.
pub fn schema_of(store: &Store, d: Obj) -> Option<Obj> {
    store.get(d, diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o))
}

fn members(store: &Store, owner: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(owner, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|o| store.is_alive(*o)).collect()
}

/// The tree's rows: the model's folders with what they hold, and its diagrams, in the model's
/// order; a filter shows the rows whose name holds it and their folders, all open. The walk stops
/// at `MAX_ROWS` rows and `MAX_DEPTH` levels, so a model past either gets the filter.
pub fn rows(store: &Store, state: &State) -> Vec<Row> {
    let filter = state.filter.trim().to_lowercase();
    let mut out = Vec::new();
    let Some(model) = archforge_core::skills::model_of(store) else { return out };
    walk(store, state, &filter, model, archforge_core::model::MEMBER, 0, &mut out);
    out
}

fn walk(store: &Store, state: &State, filter: &str, owner: Obj, prop: archforge_mof::PropId, depth: u8, out: &mut Vec<Row>) {
    if depth >= MAX_DEPTH {
        return;
    }
    for m in members(store, owner, prop) {
        if out.len() >= MAX_ROWS {
            return;
        }
        let Ok(id) = store.uuid(m) else { continue };
        match store.class_of(m).ok() {
            Some(c) if c == archforge_core::FOLDER => {
                let label = archforge_core::name_of(store, m);
                let expanded = !filter.is_empty() || !state.collapsed.contains(&id);
                let matches = filter.is_empty() || label.to_lowercase().contains(filter);
                let at = out.len();
                out.push(Row { id, obj: m, depth, kind: Kind::Folder, label, expandable: true, expanded });
                if expanded {
                    walk(store, state, filter, m, archforge_core::folder::MEMBER, depth.saturating_add(1), out);
                }
                // A folder neither named by the filter nor holding a match is left out.
                if !matches && out.len() == at + 1 {
                    out.pop();
                }
            }
            Some(c) if c == DIAGRAM => {
                let label = diagram_name(store, m);
                if filter.is_empty() || label.to_lowercase().contains(filter) {
                    out.push(Row { id, obj: m, depth, kind: Kind::Diagram, label, expandable: false, expanded: false });
                }
            }
            _ => {}
        }
    }
}

fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

/// The schemas of the model, in its order.
fn schemas(store: &Store) -> Vec<Obj> {
    let Some(model) = archforge_core::skills::model_of(store) else { return Vec::new() };
    members(store, model, archforge_core::model::MEMBER).into_iter().filter(|o| store.class_of(*o).ok() == Some(archforge_er::SCHEMA)).collect()
}

/// Opens the New diagram dialog: the name and the schema start from the selection's schema, else
/// the first schema, else a new schema named like the diagram.
pub fn open_new(shell: &mut Shell) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let selected = session.view().selection.first().copied().filter(|o| store.is_alive(*o));
    let schema = selected
        .and_then(|o| if store.class_of(o).ok() == Some(archforge_er::SCHEMA) { Some(o) } else { archforge_er::rules::schema_of(store, o) })
        .or_else(|| schemas(store).first().copied());
    let taken = store.alive().filter(|o| store.class_of(*o).ok() == Some(DIAGRAM)).map(|d| diagram_name(store, d));
    let name = match schema {
        Some(s) => archforge_core::name_of(store, s),
        None => crate::explorer::fresh_name(&tr("Diagram"), taken, false).unwrap_or_default(),
    };
    shell.diagrams.new = Some(NewDiagram { name, schema: schema.and_then(|s| store.uuid(s).ok()), menu: None, refused: None, opened: true });
    shell.diagrams.notice = None;
}

/// Creates the diagram the dialog describes: the schema first when a new one is asked for, then
/// the diagram of it, opened in the centre. A refusal stays in the dialog.
fn create(shell: &mut Shell) {
    let Some(edit) = shell.diagrams.new.clone() else { return };
    let name = edit.name.trim().to_string();
    if name.is_empty() {
        if let Some(e) = shell.diagrams.new.as_mut() {
            e.refused = Some(tr("Name the diagram"));
        }
        return;
    }
    let fresh_schema = edit.schema.is_none();
    let schema = match edit.schema {
        Some(s) => Ok(s.to_string()),
        None => run(shell, "er.schema.add", json!({ "name": name })).map(|v| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string()),
    };
    let made = schema.and_then(|schema| {
        let made = run(shell, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": &schema, "name": name }));
        // A schema made for a diagram that was then refused goes again, so nothing is left behind.
        if made.is_err() && fresh_schema {
            let _ = run(shell, "er.remove", json!({ "ids": [schema] }));
        }
        made
    });
    match made {
        Ok(v) => {
            shell.diagrams.new = None;
            if let Some(id) = v.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse) {
                crate::canvas::open(shell, id);
            }
        }
        Err(e) => {
            if let Some(edit) = shell.diagrams.new.as_mut() {
                edit.refused = Some(e);
            }
        }
    }
}

/// Drops what the store no longer has.
pub fn retain_valid(state: &mut State, store: &Store) {
    state.collapsed.retain(|id| store.by_uuid(*id).is_some());
    if let Some(n) = state.new.as_mut()
        && n.schema.is_some_and(|s| store.by_uuid(s).is_none())
    {
        n.schema = None;
    }
}

/// Draws the Diagrams explorer into `r`: the title with `+`, the filter field, the tree and the
/// model behind the open diagram.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    ux::text(&paint, at(20.0, 25.0), &tr("Diagrams").to_uppercase(), types::EYEBROW, p.text, Some((r.width() - 80.0).max(0.0)));
    let add = Rect::from_min_size(pos2(r.right() - 40.0, r.top() + 20.0), Vec2::splat(24.0));
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
    let live = shell.session.is_some();
    if control(shell, ui, "workspace.diagrams.add", "button", add, live).on_hover_text(tr("New diagram")).clicked() {
        open_new(shell);
    }
    filter_field(shell, ui, p, Rect::from_min_max(at(16.0, 65.0), pos2(r.right() - 16.0, r.top() + 97.0)));
    let rows = match &shell.session {
        Some(s) => rows(s.doc(), &shell.diagrams),
        None => Vec::new(),
    };
    let active = shell.canvas.active;
    let footer = active.is_some();
    let tree = Rect::from_min_max(at(0.0, FIRST_ROW - 12.0), pos2(r.right(), if footer { r.bottom() - FOOTER_LINE } else { r.bottom() }));
    if rows.is_empty() {
        let empty = if shell.diagrams.filter.trim().is_empty() { tr("No diagrams yet") } else { tr("Nothing matches the filter") };
        ux::text(&paint, at(36.0, FIRST_ROW), &empty, types::SMALL, p.text_muted, Some((r.width() - 52.0).max(0.0)));
    }
    // The rows scroll under the filter; the diagram newly opened is brought into view.
    let follow = active.and_then(|a| rows.iter().position(|row| row.id == a).zip(Some(a)));
    let tail = if rows.len() >= MAX_ROWS { 40.0 } else { 12.0 };
    let wheel = shell.diagrams.new.is_none();
    let offset = crate::explorer::scroll_tree(ui, p, tree, r.top() + FIRST_ROW, rows.len(), ROW, tail, wheel, follow, &mut shell.diagrams.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, row) in rows.iter().enumerate() {
        let y = r.top() + FIRST_ROW + i as f32 * ROW - offset;
        if y + ROW_HEIGHT < tree.top() || y - 7.0 > tree.bottom() {
            continue;
        }
        tree_row(shell, &mut clip, p, r, y, row, active == Some(row.id));
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
    if footer {
        footer_lines(shell, &paint, p, r);
    }
    if let Some(notice) = shell.diagrams.notice.clone() {
        ux::text(&paint, pos2(r.left() + 16.0, r.bottom() - 40.0), &notice, types::CAPTION, p.amber, Some((r.width() - 32.0).max(0.0)));
    }
}

/// The model behind the open diagram: the project and the schema the diagram is of.
fn footer_lines(shell: &Shell, paint: &egui::Painter, p: &Palette, r: Rect) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let Some(d) = shell.canvas.active.and_then(|id| store.by_uuid(id)) else { return };
    let project = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
    let schema = schema_of(store, d).map(|s| archforge_core::name_of(store, s)).unwrap_or_default();
    paint.rect_filled(Rect::from_min_size(pos2(r.left() + 16.0, r.bottom() - FOOTER_LINE), vec2((r.width() - 32.0).max(0.0), 1.0)), 0.0, p.border);
    let room = Some((r.width() - 40.0).max(0.0));
    ux::text(
        paint,
        pos2(r.left() + 20.0, r.bottom() - FOOTER_TITLE),
        &tr("Model behind this diagram").to_uppercase(),
        types::EYEBROW,
        p.text_muted,
        room,
    );
    ux::text(paint, pos2(r.left() + 20.0, r.bottom() - FOOTER_TEXT), &format!("{project} / {schema}"), types::UI, p.text, room);
}

/// The filter field: a real text field narrowing the tree as it is typed.
fn filter_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 8.0, p.page);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon_mid(&paint, pos2(field.left() + 17.0, field.center().y), &icons::SEARCH, p.text_secondary);
    widgets::record_visible(ui, &mut shell.registry, "workspace.diagrams.filter", "field", field);
    let inner = crate::panels::field_inner(ui, field.with_min_x(field.left() + 22.0));
    let edit = egui::TextEdit::singleline(&mut shell.diagrams.filter)
        .id(widgets::egui_id("workspace.diagrams.filter"))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(tr("Filter diagrams…")).color(p.text_secondary).font(types::SMALL.font()))
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let _ = ui.put(inner, edit).accessible_name(tr("Filter diagrams…"));
}

/// One row: its chevron, icon and name; a click on a diagram opens it, on a folder folds it.
fn tree_row(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, column: Rect, y: f32, row: &Row, is_active: bool) {
    let paint = ui.painter().clone();
    let indent = row.depth as f32 * INDENT;
    let hit = Rect::from_min_max(pos2(column.left() + 8.0, y - 7.0), pos2(column.right() - 8.0, y - 7.0 + ROW_HEIGHT));
    if is_active {
        paint.rect_filled(hit, 6.0, p.accent_tint);
    }
    let id = format!("workspace.diagrams.row.{}", row.id);
    let resp = control(shell, ui, &id, "row", hit, true);
    if row.expandable {
        let chevron = Rect::from_center_size(pos2(column.left() + 21.0 + indent, hit.center().y), Vec2::splat(16.0));
        ux::icon_in(&paint, chevron, if row.expanded { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, p.text_secondary);
    }
    let ink = if is_active { p.accent } else { p.text };
    let mid = hit.center().y;
    let icon_ink = if is_active { p.accent } else { p.text_secondary };
    let icon = match row.kind {
        Kind::Folder => &icons::FOLDER,
        Kind::Diagram => &icons::ER_DIAGRAM,
    };
    ux::icon_mid(&paint, pos2(column.left() + 38.5 + indent, mid), icon, icon_ink);
    let name_x = column.left() + 53.0 + indent;
    let label = if row.label.is_empty() { tr("Unnamed diagram") } else { row.label.clone() };
    ux::text_mid(&paint, name_x, mid, &label, types::UI, ink, Some((column.right() - 16.0 - name_x).max(0.0)));
    if resp.clicked() {
        match row.kind {
            Kind::Folder => {
                if row.expanded {
                    shell.diagrams.collapsed.insert(row.id);
                } else {
                    shell.diagrams.collapsed.remove(&row.id);
                }
            }
            Kind::Diagram => {
                shell.diagrams.notice = None;
                crate::canvas::open(shell, row.id);
            }
        }
    }
}

// The New diagram dialog.

/// Draws the New diagram dialog over the window while it is open.
pub(crate) fn new_dialog(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette) {
    if shell.diagrams.new.is_none() {
        return;
    }
    let window = ui.ctx().content_rect();
    let p = *p;
    egui::Area::new(widgets::egui_id("workspace.new-diagram")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        let paint = ui.painter().clone();
        paint.rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("workspace.new-diagram.veil"), Sense::click_and_drag());
        new_dialog_body(shell, ui, &p, &paint, window);
    });
}

fn new_dialog_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, paint: &egui::Painter, window: Rect) {
    let size = vec2(440.0_f32.min((window.width() - 32.0).max(200.0)), 228.0);
    let dialog = Rect::from_center_size(window.center(), size);
    ux::shadow(paint, dialog, 10.0, p.shadow);
    paint.rect_filled(dialog, 10.0, p.surface);
    paint.rect_stroke(dialog, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    let at = |x: f32, y: f32| dialog.min + vec2(x, y);
    let width = size.x - 40.0;
    ux::text(paint, at(20.0, 20.0), &tr("New diagram"), types::CARD_TITLE, p.text, Some(width));
    // The schemas the diagram may be of, read once.
    let (schemas, taken_focus) = match (shell.session.as_ref(), shell.diagrams.new.as_ref()) {
        (Some(s), Some(n)) => {
            let store = s.doc();
            (schemas(store).into_iter().filter_map(|o| Some((store.uuid(o).ok()?, archforge_core::name_of(store, o)))).collect::<Vec<_>>(), n.opened)
        }
        _ => return,
    };
    // The name.
    ux::text(paint, at(20.0, 56.0), &tr("Name"), types::CAPTION, p.text_secondary, Some(width));
    let field = Rect::from_min_size(at(20.0, 73.0), vec2(width, 28.0));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, "workspace.new-diagram.name", "field", field);
    let inner = crate::panels::field_inner(ui, Rect::from_min_max(pos2(field.left(), field.top()), pos2(field.right(), field.bottom())));
    let Some(edit) = shell.diagrams.new.as_mut() else { return };
    let text_edit = egui::TextEdit::singleline(&mut edit.name)
        .id(widgets::egui_id("workspace.new-diagram.name"))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let out = text_edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(tr("Name"));
    if taken_focus {
        out.response.request_focus();
        edit.opened = false;
    }
    if out.response.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    let enter = out.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    // The kind and the schema, side by side.
    let half = (width - 16.0) / 2.0;
    let schema_word = match edit.schema {
        Some(id) => schemas.iter().find(|(s, _)| *s == id).map(|(_, n)| n.clone()).unwrap_or_else(|| tr("New schema")),
        None => tr("New schema"),
    };
    let kind_field = select(shell, ui, p, at(20.0, 121.0), half, "workspace.new-diagram.kind", &tr("Kind"), &tr("ER diagram"), Field::Kind);
    let schema_field =
        select(shell, ui, p, at(20.0 + half + 16.0, 121.0), half, "workspace.new-diagram.schema", &tr("Schema"), &schema_word, Field::Schema);
    if let Some(refused) = shell.diagrams.new.as_ref().and_then(|e| e.refused.clone()) {
        ux::text(paint, at(20.0, dialog.height() - 44.0), &refused, types::CAPTION, p.amber, Some((width - 200.0).max(0.0)));
    }
    // Cancel and Create.
    let mut x = dialog.right() - 20.0;
    let mut action: Option<bool> = None;
    for (id, label, primary) in [("workspace.new-diagram.create", tr("Create"), true), ("workspace.new-diagram.cancel", tr("Cancel"), false)] {
        let g = ux::galley(paint, &label, types::SMALL, p.text, None);
        let b = Rect::from_min_max(pos2(x - g.size().x - 24.0, dialog.bottom() - 48.0), pos2(x, dialog.bottom() - 20.0));
        paint.rect_filled(b, 6.0, if primary { p.accent_fill } else { p.field });
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, if primary { p.on_accent } else { p.text });
        if control(shell, ui, id, "button", b, true).clicked() {
            action = Some(primary);
        }
        x = b.left() - 8.0;
    }
    // The open select's menu, over the rest.
    let open = shell.diagrams.new.as_ref().and_then(|e| e.menu);
    match open {
        Some(Field::Kind) => {
            menu(shell, ui, p, kind_field, "workspace.new-diagram.kind", &[tr("ER diagram")]);
        }
        Some(Field::Schema) => {
            let mut items: Vec<String> = schemas.iter().map(|(_, n)| n.clone()).collect();
            items.push(tr("New schema"));
            if let Some(i) = menu(shell, ui, p, schema_field, "workspace.new-diagram.schema", &items)
                && let Some(edit) = shell.diagrams.new.as_mut()
            {
                edit.schema = schemas.get(i).map(|(id, _)| *id);
                edit.refused = None;
            }
        }
        None => {}
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        action = Some(false);
    }
    match action {
        Some(true) => create(shell),
        Some(false) => shell.diagrams.new = None,
        None if enter && open.is_none() => create(shell),
        None => {}
    }
}

/// A filled select showing `value`; a click opens or closes its menu.
fn select(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, width: f32, id: &str, label: &str, value: &str, which: Field) -> Rect {
    let paint = ui.painter();
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), 28.0));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(paint, field.left() + 8.0, field.center().y, value, types::SMALL, p.text, Some((width - 36.0).max(0.0)));
    ux::icon_mid(paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    if control(shell, ui, id, "select", field, true).clicked()
        && let Some(edit) = shell.diagrams.new.as_mut()
    {
        edit.menu = if edit.menu == Some(which) { None } else { Some(which) };
    }
    field
}

/// A menu under `anchor`; gives the index picked, closing on a pick or a click elsewhere.
fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String]) -> Option<usize> {
    let height = 8.0 + items.len() as f32 * 30.0;
    let r = ux::drop_rect(ui, anchor, anchor.left(), vec2(anchor.width(), height));
    // Over the dialog's fields below it, on a layer of its own.
    ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), r, |ui| menu_body(shell, ui, p, anchor, r, id, items))
}

fn menu_body(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, r: Rect, id: &str, items: &[String]) -> Option<usize> {
    let paint = ui.painter();
    ux::shadow(paint, r, 8.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    for (i, item) in items.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * 30.0), vec2((r.width() - 8.0).max(0.0), 30.0));
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
    if (picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)))
        && let Some(edit) = shell.diagrams.new.as_mut()
    {
        edit.menu = None;
    }
    picked
}
