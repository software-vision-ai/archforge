//! A schema's sequences and views in Properties (slice S7 of archforge-gm2.34, from the design's
//! panel states "Schema with sequences and views", "Sequence editor" and "View editor"): the
//! Sequences section lists them with an `SQ` badge, the name, where they start, their step and
//! the column that owns them; the Views section with a `VW` badge, the name, whether the view is
//! materialized and the start of its query. `+` opens the editor for a new one, a click on a row
//! opens the editor in its place. The sequence editor takes the name, Start and Increment side by
//! side, Minimum and Maximum side by side, Cycle and Owned by (the schema's whole-number columns);
//! the view editor the name, Materialized and the query in a tall field. Apply is one command,
//! `er.sequence.add`/`set` or `er.view.add`/`set`; the bin runs the remove. A selected sequence or
//! view shows its schema's section with its editor open.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::checks::badge;
use crate::home::control;
use crate::i18n::{fmt, tr, tr_ctx};
use crate::panels::{FIELD, Menu, PAD, check, icon_button, menu, run, section, select_field, text_field};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The most columns the Owned by menu offers.
const MAX_OWNERS: usize = 30;
/// The height of the view editor's query field.
const QUERY_HEIGHT: f32 = 104.0;

/// Which kind of schema object an editor edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Sequence,
    View,
}

/// A sequence or view being edited: the element, or none for one about to be added to `schema`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectEdit {
    pub kind: ObjectKind,
    pub target: Option<Uuid>,
    pub schema: Uuid,
    pub name: String,
    /// A sequence's numbers as typed, its flag and the column that owns it.
    pub start: String,
    pub increment: String,
    pub minimum: String,
    pub maximum: String,
    pub cycle: bool,
    pub owned_by: Option<Uuid>,
    /// A view's query and flag.
    pub query: String,
    pub materialized: bool,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once.
    pub opened: bool,
}

/// One sequence or view as the panel lists it.
pub struct ObjectRow {
    pub id: Uuid,
    pub name: String,
    pub detail: String,
}

/// A column as the Owned by select names it: `Table.column`.
fn column_label(store: &Store, a: Obj) -> String {
    use archforge_er::rules as r;
    match r::entity_of(store, a) {
        Some(e) => format!("{}.{}", r::name_of(store, e), r::name_of(store, a)),
        None => r::name_of(store, a),
    }
}

/// The sequences of a schema: where each starts, its step, whether it cycles, and its owner.
pub fn sequence_rows(store: &Store, schema: Obj) -> Vec<ObjectRow> {
    use archforge_er::rules as r;
    r::sequences_of(store, schema)
        .into_iter()
        .filter_map(|q| {
            let n = r::sequence_numbers(store, q);
            let mut detail = fmt(&tr("from {start} by {increment}"), &[("start", &n.start.to_string()), ("increment", &n.increment.to_string())]);
            if n.cycle {
                detail = format!("{detail} · {}", tr("cycles"));
            }
            if let Some(a) = r::owner_of_sequence(store, q) {
                detail = format!("{detail} · {}", fmt(&tr("owned by {column}"), &[("column", &column_label(store, a))]));
            }
            Some(ObjectRow { id: store.uuid(q).ok()?, name: r::name_of(store, q), detail })
        })
        .collect()
}

/// The views of a schema: view or materialized, and the start of the query on one line.
pub fn view_rows(store: &Store, schema: Obj) -> Vec<ObjectRow> {
    use archforge_er::rules as r;
    r::views_of(store, schema)
        .into_iter()
        .filter_map(|v| {
            let materialized = store.get(v, archforge_er::view::MATERIALIZED).ok().and_then(|x| x.as_bool()).unwrap_or(false);
            // The query's first words on one line, read no further than the row shows.
            let held = store.get(v, archforge_er::view::QUERY).ok();
            let mut query = String::new();
            for word in held.as_ref().and_then(|x| x.as_str()).unwrap_or_default().split_whitespace() {
                if query.len() >= 80 {
                    break;
                }
                if !query.is_empty() {
                    query.push(' ');
                }
                query.push_str(word);
            }
            let query: String = query.chars().take(80).collect();
            let word = if materialized { tr("materialized") } else { tr("view") };
            Some(ObjectRow { id: store.uuid(v).ok()?, name: r::name_of(store, v), detail: format!("{word} · {query}") })
        })
        .collect()
}

/// Opens the editor on a sequence or view, or on one of `kind` to add to `schema`.
pub fn open(shell: &mut Shell, schema: Uuid, kind: ObjectKind, target: Option<Uuid>) {
    let mut edit = ObjectEdit {
        kind,
        target,
        schema,
        name: String::new(),
        start: "1".into(),
        increment: "1".into(),
        minimum: String::new(),
        maximum: String::new(),
        cycle: false,
        owned_by: None,
        query: String::new(),
        materialized: false,
        refused: None,
        opened: true,
    };
    if let (Some(t), Some(store)) = (target, shell.session.as_ref().map(|s| s.doc()))
        && let Some(o) = store.by_uuid(t)
    {
        use archforge_er::rules as r;
        edit.name = r::name_of(store, o);
        match kind {
            ObjectKind::Sequence => {
                let n = r::sequence_numbers(store, o);
                edit.start = n.start.to_string();
                edit.increment = n.increment.to_string();
                edit.minimum = n.minimum;
                edit.maximum = n.maximum;
                edit.cycle = n.cycle;
                edit.owned_by = r::owner_of_sequence(store, o).and_then(|a| store.uuid(a).ok());
            }
            ObjectKind::View => {
                edit.query = r::text_of(store, o, archforge_er::view::QUERY);
                edit.materialized = store.get(o, archforge_er::view::MATERIALIZED).ok().and_then(|x| x.as_bool()).unwrap_or(false);
            }
        }
    }
    shell.panels.object_edit = Some(edit);
    shell.panels.menu = None;
}

/// The kind of a schema object, when it is one.
pub fn kind_of(store: &Store, o: Obj) -> Option<ObjectKind> {
    match store.class_of(o).ok() {
        Some(c) if c == archforge_er::SEQUENCE => Some(ObjectKind::Sequence),
        Some(c) if c == archforge_er::VIEW => Some(ObjectKind::View),
        _ => None,
    }
}

/// The Sequences or Views section with its rows and the editor; gives the y under it.
pub(crate) fn section_of(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    r: Rect,
    mut y: f32,
    schema: Obj,
    schema_id: Uuid,
    kind: ObjectKind,
) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let list = match shell.session.as_ref() {
        Some(s) if kind == ObjectKind::Sequence => sequence_rows(s.doc(), schema),
        Some(s) => view_rows(s.doc(), schema),
        None => return y,
    };
    let (title, add, hint, row_id, label) = match kind {
        ObjectKind::Sequence => {
            (tr("Sequences · {n}"), "workspace.properties.sequences.add", "Add sequence", "workspace.properties.sequence.row", "SQ")
        }
        ObjectKind::View => (tr("Views · {n}"), "workspace.properties.views.add", "Add view", "workspace.properties.view.row", "VW"),
    };
    let title = fmt(&title, &[("n", &list.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some((add, hint))) else {
        open(shell, schema_id, kind, None);
        return y + 50.0;
    };
    y = next - 6.0;
    let editing = |shell: &Shell| shell.panels.object_edit.as_ref().filter(|e| e.kind == kind).cloned();
    let editing_new = editing(shell).is_some_and(|e| e.target.is_none() && e.schema == schema_id);
    for row in &list {
        if editing(shell).is_some_and(|e| e.target == Some(row.id)) {
            y = editor(shell, ui, p, r, y, schema);
            continue;
        }
        let rect = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let resp = control(shell, ui, &format!("{row_id}.{}", row.id), "row", rect, true);
        if resp.hovered() {
            paint.rect_filled(rect, 6.0, p.page);
        }
        ux::icon_in(&paint, Rect::from_min_size(pos2(rect.right() - 24.0, rect.top() + 10.0), Vec2::splat(16.0)), &icons::EDIT, p.text_secondary);
        badge(&paint, rect.min + vec2(2.0, 6.0), label, p.field, p.text_secondary);
        ux::text(&paint, rect.min + vec2(38.0, 4.0), &row.name, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, rect.min + vec2(38.0, 23.0), &row.detail, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        if resp.clicked() {
            open(shell, schema_id, kind, Some(row.id));
        }
        y += 44.0;
    }
    if editing_new {
        y = editor(shell, ui, p, r, y, schema);
    }
    y
}

/// The whole-number columns of a schema that may own a sequence, by label, the chosen one kept,
/// at most `MAX_OWNERS` of them; and how many more there are, which the menu says.
fn owners(store: &Store, schema: Obj, chosen: Option<Uuid>) -> (Vec<(Uuid, String)>, usize) {
    use archforge_er::rules as r;
    let mut out: Vec<(Uuid, String)> = Vec::new();
    let mut more = 0;
    for e in store.list(schema, archforge_er::schema::ENTITY).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref) {
        for a in store.list(e, archforge_er::entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref) {
            let Ok(id) = store.uuid(a) else { continue };
            if Some(id) == chosen {
                out.push((id, column_label(store, a)));
            } else if r::is_integer_type(&r::type_of(store, a)) {
                if out.len() < MAX_OWNERS {
                    out.push((id, column_label(store, a)));
                } else {
                    more += 1;
                }
            }
        }
    }
    (out, more)
}

/// The sequence or view editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, schema: Obj) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.object_edit.clone() else { return y };
    let row = 17.0 + FIELD + 10.0;
    let height = match edit.kind {
        ObjectKind::Sequence => 12.0 + row * 3.0 + 28.0 + row + 4.0 + FIELD + 12.0,
        ObjectKind::View => 12.0 + row + 28.0 + 17.0 + QUERY_HEIGHT + 14.0 + FIELD + 12.0,
    } + if edit.refused.is_some() { 20.0 } else { 0.0 };
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = std::mem::take(&mut edit.opened);
    let mut cy = 12.0;
    let mut enter = text_field(
        shell,
        ui,
        p,
        at(0.0, cy),
        inner,
        "workspace.properties.object.name",
        &tr("Name"),
        &mut edit.name,
        archforge_er::rules::MAX_NAME,
        take,
        false,
    );
    cy += row;
    match edit.kind {
        ObjectKind::Sequence => {
            // Start wide and Increment narrow; Minimum and Maximum in halves.
            let narrow = 80.0_f32.min(inner / 2.0);
            let wide = (inner - narrow - 8.0).max(0.0);
            let half = ((inner - 8.0) / 2.0).max(0.0);
            for (x, w, id, label, text, dy) in [
                (0.0, wide, "workspace.properties.sequence.start", tr_ctx("sequence", "Start"), &mut edit.start, 0.0),
                (wide + 8.0, narrow, "workspace.properties.sequence.increment", tr("Increment"), &mut edit.increment, 0.0),
                (0.0, half, "workspace.properties.sequence.minimum", tr("Minimum"), &mut edit.minimum, row),
                (half + 8.0, half, "workspace.properties.sequence.maximum", tr("Maximum"), &mut edit.maximum, row),
            ] {
                enter |= text_field(shell, ui, p, at(x, cy + dy), w, id, &label, text, 20, false, false);
            }
            cy += 2.0 * row;
            check(shell, ui, p, at(0.0, cy), "workspace.properties.sequence.cycle", &tr("Cycle"), &mut edit.cycle);
            cy += 28.0;
            let (list, more) = shell.session.as_ref().map(|s| owners(s.doc(), schema, edit.owned_by)).unwrap_or_default();
            let value = edit.owned_by.and_then(|o| list.iter().find(|(id, _)| *id == o).map(|(_, l)| l.clone())).unwrap_or_else(|| tr("None"));
            let field =
                select_field(shell, ui, p, at(0.0, cy), inner, "workspace.properties.sequence.owned-by", &tr("Owned by"), &value, Menu::OwnedBy);
            cy += row + 4.0;
            if shell.panels.menu == Some(Menu::OwnedBy) {
                // Past the most the menu lists, it says how many it leaves out; the command takes any.
                let tail = (more > 0).then(|| crate::i18n::trn("{n} more column, not listed", "{n} more columns, not listed", more as u64));
                let items: Vec<String> = std::iter::once(tr("None")).chain(list.iter().map(|(_, l)| l.clone())).chain(tail).collect();
                if let Some(i) = menu(shell, ui, p, field, "workspace.properties.sequence.owned-by", &items) {
                    if i == 0 {
                        edit.owned_by = None;
                    } else if let Some((id, _)) = list.get(i - 1) {
                        edit.owned_by = Some(*id);
                    }
                }
            }
        }
        ObjectKind::View => {
            check(shell, ui, p, at(0.0, cy), "workspace.properties.view.materialized", &tr("Materialized"), &mut edit.materialized);
            cy += 28.0;
            ux::text(&paint, at(0.0, cy), &tr("Query"), types::CAPTION, p.text_secondary, Some(inner));
            let body = Rect::from_min_size(at(0.0, cy + 17.0), vec2(inner, QUERY_HEIGHT));
            paint.rect_filled(body, 6.0, p.field);
            let id = "workspace.properties.view.query";
            widgets::record_visible(ui, &mut shell.registry, id, "field", body);
            let field = body.shrink2(vec2(8.0, 6.0));
            let out = egui::ScrollArea::vertical().id_salt(widgets::egui_id(&format!("{id}.scroll"))).max_height(field.height()).show(
                &mut ui.new_child(egui::UiBuilder::new().max_rect(field)),
                |ui| {
                    egui::TextEdit::multiline(&mut edit.query)
                        .id(widgets::egui_id(id))
                        .frame(egui::Frame::NONE)
                        .font(types::SMALL.font())
                        .text_color(p.text)
                        .char_limit(archforge_er::rules::MAX_QUERY)
                        .desired_width(field.width())
                        .desired_rows(5)
                        .show(ui)
                },
            );
            if out.inner.response.has_focus() {
                paint.rect_stroke(body, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
            }
            cy += 17.0 + QUERY_HEIGHT + 14.0;
        }
    }
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.object.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.object.cancel",
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - FIELD, card.top() + cy),
        "workspace.properties.object.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, cy + FIELD + 8.0), why, types::CAPTION, p.amber, Some(inner));
    }
    shell.panels.object_edit = Some(edit.clone());
    if cancel || (shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.object_edit = None;
    } else if remove {
        let (cmd, key) = match edit.kind {
            ObjectKind::Sequence => ("er.sequence.remove", "sequence"),
            ObjectKind::View => ("er.view.remove", "view"),
        };
        match run(shell, cmd, json!({ key: edit.target.map(|t| t.to_string()).unwrap_or_default() })) {
            Ok(_) => {
                shell.panels.object_edit = None;
                reselect(shell, edit.schema);
            }
            Err(e) => refuse(shell, e),
        }
    } else if apply || enter {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

fn refuse(shell: &mut Shell, why: String) {
    if let Some(t) = shell.panels.object_edit.as_mut() {
        t.refused = Some(why);
    }
}

/// The schema is what the panel shows after a command selected the sequence or view.
fn reselect(shell: &mut Shell, schema: Uuid) {
    if let Err(e) = run(shell, "view.select", json!({ "ids": [schema.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// A whole number as typed, or the refusal naming the field by its label.
fn whole(text: &str, field: &str) -> Result<i64, String> {
    text.trim().parse::<i64>().map_err(|_| fmt(&tr("{field} takes a whole number"), &[("field", field)]))
}

/// Writes the sequence or view through its command; the schema stays selected.
fn commit(shell: &mut Shell, edit: &ObjectEdit) {
    let name = edit.name.trim().to_string();
    let (cmd, params) = match edit.kind {
        ObjectKind::Sequence => {
            let numbers = whole(&edit.start, &tr_ctx("sequence", "Start")).and_then(|s| whole(&edit.increment, &tr("Increment")).map(|i| (s, i)));
            let (start, increment) = match numbers {
                Ok(n) => n,
                Err(e) => {
                    refuse(shell, e);
                    return;
                }
            };
            let mut params = json!({
                "name": name, "start": start, "increment": increment, "minimum": edit.minimum.trim(), "maximum": edit.maximum.trim(),
                "cycle": edit.cycle, "owned_by": edit.owned_by.map(|o| o.to_string()).unwrap_or_default(),
            });
            match edit.target {
                None => {
                    params["schema"] = json!(edit.schema.to_string());
                    ("er.sequence.add", params)
                }
                Some(t) => {
                    params["sequence"] = json!(t.to_string());
                    ("er.sequence.set", params)
                }
            }
        }
        ObjectKind::View => {
            let mut params = json!({ "name": name, "query": edit.query.trim(), "materialized": edit.materialized });
            match edit.target {
                None => {
                    params["schema"] = json!(edit.schema.to_string());
                    ("er.view.add", params)
                }
                Some(t) => {
                    params["view"] = json!(t.to_string());
                    ("er.view.set", params)
                }
            }
        }
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.object_edit = None;
            shell.panels.notice = None;
            reselect(shell, edit.schema);
        }
        Err(e) => refuse(shell, e),
    }
}

/// Every hover text and field label to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add sequence", "Add view"].into_iter()
}
