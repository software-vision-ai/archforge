//! A table's indexes in Properties (slice S3 of archforge-gm2.34, from the design's panel state
//! "Editing an index"): the Indexes section lists them with an `IX` badge, the name (or the
//! generated one) and the columns; `+` starts one, a click on a row opens the editor in its place
//! (name, Unique, the columns in index order each with a grip, a direction button and a remove, an
//! expression typed in place, Add a column with "An expression" last, More for Where and Method;
//! remove, cancel and apply as icons). A right click on a column row offers its nulls order (owner
//! decision archforge-gm2.34.9). Apply is one command, `er.index.add` or `er.index.set`; the bin
//! runs `er.index.remove`.

use archforge_er::{NullsOrder, SortDirection};
use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::checks::badge;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, check, icon_button, menu, run, section, text_field};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// One column of an index as edited: a column of the table, or an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    pub attribute: Option<Uuid>,
    pub expression: String,
    pub direction: SortDirection,
    pub nulls: NullsOrder,
}

/// An index being edited: the index, or none for one about to be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEdit {
    pub target: Option<Uuid>,
    pub entity: Uuid,
    pub name: String,
    pub unique: bool,
    pub columns: Vec<ColumnSpec>,
    pub where_: String,
    pub method: String,
    /// Whether More (Where and Method) is open.
    pub more: bool,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once.
    pub opened: bool,
}

/// One index as the panel lists it.
pub struct IndexRow {
    pub id: Uuid,
    pub name: String,
    pub label: String,
    pub unique: bool,
    pub columns: Vec<ColumnSpec>,
    /// The columns as a sentence: names and expressions, in order.
    pub summary: String,
    pub where_: String,
    pub method: String,
}

fn name(store: &Store, o: Obj) -> String {
    archforge_er::rules::name_of(store, o)
}

/// The indexes of a table, in order.
pub fn rows(store: &Store, e: Obj) -> Vec<IndexRow> {
    archforge_er::rules::indexes_of(store, e)
        .into_iter()
        .filter_map(|ix| {
            let specs = archforge_er::rules::index_specs(store, ix);
            let summary =
                specs.iter().map(|s| s.attribute.map(|a| name(store, a)).unwrap_or_else(|| s.expression.clone())).collect::<Vec<_>>().join(", ");
            Some(IndexRow {
                id: store.uuid(ix).ok()?,
                name: name(store, ix),
                label: archforge_er::rules::display_name(store, ix),
                unique: archforge_er::rules::is_unique_index(store, ix),
                columns: specs
                    .into_iter()
                    .map(|s| ColumnSpec {
                        attribute: s.attribute.and_then(|a| store.uuid(a).ok()),
                        expression: s.expression,
                        direction: s.direction,
                        nulls: s.nulls,
                    })
                    .collect(),
                summary,
                where_: archforge_er::rules::text_of(store, ix, archforge_er::table_index::WHERE),
                method: archforge_er::rules::text_of(store, ix, archforge_er::table_index::METHOD),
            })
        })
        .collect()
}

/// Opens the editor on an index, or on one to add to `entity`.
pub fn open(shell: &mut Shell, entity: Uuid, target: Option<&IndexRow>) {
    shell.panels.index = Some(match target {
        Some(ix) => IndexEdit {
            target: Some(ix.id),
            entity,
            name: ix.name.clone(),
            unique: ix.unique,
            columns: ix.columns.clone(),
            where_: ix.where_.clone(),
            method: ix.method.clone(),
            more: !ix.where_.is_empty() || !ix.method.is_empty(),
            refused: None,
            opened: true,
        },
        None => IndexEdit {
            target: None,
            entity,
            name: String::new(),
            unique: false,
            columns: Vec::new(),
            where_: String::new(),
            method: String::new(),
            more: false,
            refused: None,
            opened: true,
        },
    });
    shell.panels.menu = None;
}

/// The Indexes section with its rows and the editor; gives the y under it.
pub(crate) fn indexes_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, entity: Obj, entity_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let indexes = match shell.session.as_ref() {
        Some(s) => rows(s.doc(), entity),
        None => return y,
    };
    let title = fmt(&tr("Indexes · {n}"), &[("n", &indexes.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.indexes.add", "Add index"))) else {
        open(shell, entity_id, None);
        return y;
    };
    y = next - 6.0;
    let editing_new = shell.panels.index.as_ref().is_some_and(|i| i.target.is_none() && i.entity == entity_id);
    for index in &indexes {
        if shell.panels.index.as_ref().is_some_and(|i| i.target == Some(index.id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let resp = control(shell, ui, &format!("workspace.properties.index.row.{}", index.id), "row", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 6.0, p.page);
            ux::icon_mid(&paint, pos2(row.right() - 16.0, row.center().y), &icons::EDIT, p.text_secondary);
        }
        badge(&paint, row.min + vec2(2.0, 6.0), "IX", p.purple_tint, p.purple);
        ux::text(&paint, row.min + vec2(38.0, 4.0), &index.label, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, row.min + vec2(38.0, 23.0), &index.summary, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        if resp.clicked() {
            open(shell, entity_id, Some(index));
        }
        y += 44.0;
    }
    if editing_new {
        y = editor(shell, ui, p, r, y);
    }
    y
}

/// The index editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.index.clone() else { return y };
    // The table's columns, named, for the rows and the menu.
    let all_columns: Vec<(Uuid, String, String)> = match shell.session.as_ref().and_then(|s| s.doc().by_uuid(edit.entity).map(|e| (s.doc(), e))) {
        Some((store, e)) => store
            .list(e, archforge_er::entity::ATTRIBUTE)
            .unwrap_or_default()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .filter(|a| store.is_alive(*a))
            .filter_map(|a| Some((store.uuid(a).ok()?, name(store, a), archforge_er::rules::type_display(store, a))))
            .collect(),
        None => return y,
    };
    let rows = edit.columns.len() as f32;
    let more_h = if edit.more { 2.0 * (17.0 + FIELD + 14.0) } else { 0.0 };
    let refused_h = if edit.refused.is_some() { 20.0 } else { 0.0 };
    let height = 12.0 + 17.0 + FIELD + 14.0 + 28.0 + 17.0 + rows * 32.0 + FIELD + 12.0 + 34.0 + more_h + FIELD + 12.0 + refused_h;
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    crate::panels::reveal(shell, ui, card);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = std::mem::take(&mut edit.opened);
    let name_done = text_field(
        shell,
        ui,
        p,
        at(0.0, 12.0),
        inner,
        "workspace.properties.index.name",
        &tr("Name"),
        &mut edit.name,
        archforge_er::rules::MAX_NAME,
        take,
        false,
    );
    let mut cy = 12.0 + 17.0 + FIELD + 14.0;
    check(shell, ui, p, at(0.0, cy), "workspace.properties.index.unique", &tr("Unique"), &mut edit.unique);
    cy += 28.0;
    ux::text(&paint, at(0.0, cy), &tr("Columns, in index order"), types::CAPTION, p.text_secondary, Some(inner));
    cy += 17.0;
    let mut remove_at: Option<usize> = None;
    let mut nulls_anchor: Option<(usize, Rect)> = None;
    for i in 0..edit.columns.len() {
        let row = Rect::from_min_size(at(0.0, cy), vec2(inner, FIELD));
        paint.rect_filled(row, 6.0, p.field);
        ux::icon_mid(&paint, pos2(row.left() + 14.0, row.center().y), &icons::GRIP, p.text_muted);
        let spec = &mut edit.columns[i];
        let text_right = row.right() - 24.0 - 24.0 - 8.0;
        match spec.attribute {
            Some(id) => {
                let (n, ty) = all_columns.iter().find(|(c, _, _)| *c == id).map(|(_, n, t)| (n.clone(), t.clone())).unwrap_or_default();
                let name_rect =
                    ux::text(&paint, row.min + vec2(28.0, 5.0), &n, types::SMALL, p.text, Some((text_right - row.left() - 28.0).max(0.0)));
                ux::text(
                    &paint,
                    pos2(name_rect.right() + 8.0, row.top() + 6.0),
                    &ty,
                    types::CAPTION,
                    p.text_muted,
                    Some((text_right - name_rect.right() - 8.0).max(0.0)),
                );
            }
            None => {
                // An expression, typed in place.
                let field = Rect::from_min_max(pos2(row.left() + 24.0, row.top() + 2.0), pos2(text_right, row.bottom() - 2.0));
                let id = format!("workspace.properties.index.column.{i}.expression");
                widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
                let out = egui::TextEdit::singleline(&mut spec.expression)
                    .id(widgets::egui_id(&id))
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .font(types::SMALL.font())
                    .text_color(p.text)
                    .hint_text(tr("An expression"))
                    .char_limit(archforge_er::rules::MAX_TEXT)
                    .desired_width(field.width())
                    .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(crate::panels::field_inner(ui, field))));
                let _ = (*out.response).clone().accessible_name(tr("An expression"));
                if out.response.has_focus() {
                    paint.rect_stroke(row, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
                }
            }
        }
        // The direction, toggled by its button; the nulls order from the row's menu.
        let dir = Rect::from_min_size(pos2(row.right() - 24.0 - 24.0 - 4.0, row.top() + 2.0), Vec2::splat(24.0));
        let (icon, hint) = match spec.direction {
            SortDirection::Ascending => (&icons::SORT_ASCENDING, "Ascending"),
            SortDirection::Descending => (&icons::SORT_DESCENDING, "Descending"),
        };
        ux::icon_in(&paint, Rect::from_center_size(dir.center(), Vec2::splat(14.0)), icon, p.text_secondary);
        if control(shell, ui, &format!("workspace.properties.index.column.{i}.direction"), "button", dir, true).on_hover_text(tr(hint)).clicked() {
            spec.direction = match spec.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        }
        let x = Rect::from_min_size(pos2(row.right() - 26.0, row.top() + 6.0), Vec2::splat(16.0));
        ux::icon_in(&paint, x, &icons::CLOSE, p.text_secondary);
        if control(shell, ui, &format!("workspace.properties.index.column.{i}.remove"), "button", x.expand(4.0), true)
            .on_hover_text(tr("Remove"))
            .clicked()
        {
            remove_at = Some(i);
        }
        let hit = Rect::from_min_max(row.min, pos2(dir.left(), row.bottom()));
        if control(shell, ui, &format!("workspace.properties.index.column.{i}"), "row", hit, true).secondary_clicked() {
            shell.panels.menu = Some(Menu::IndexNulls(i));
        }
        if shell.panels.menu == Some(Menu::IndexNulls(i)) {
            nulls_anchor = Some((i, row));
        }
        cy += 32.0;
    }
    // Add a column: the table's columns not in the index yet, then an expression; none past the
    // rules' limit.
    let full = edit.columns.len() >= archforge_er::rules::MAX_KEY_COLUMNS;
    let add = Rect::from_min_size(at(0.0, cy), vec2(inner, FIELD));
    paint.rect_filled(add, 6.0, p.field);
    let ink = if full { p.text_muted } else { p.text_secondary };
    ux::text(&paint, add.min + vec2(8.0, 5.0), &tr("Add a column"), types::SMALL, ink, Some((inner - 36.0).max(0.0)));
    ux::icon_mid(&paint, pos2(add.right() - 14.0, add.center().y), &icons::CHEVRON_DOWN, ink);
    if control(shell, ui, "workspace.properties.index.column.add", "select", add, true).clicked() && !full {
        shell.panels.menu = if shell.panels.menu == Some(Menu::IndexColumn) { None } else { Some(Menu::IndexColumn) };
    }
    cy += FIELD + 12.0;
    // More: Where and Method.
    let more = Rect::from_min_size(at(0.0, cy), vec2(inner, 24.0));
    ux::icon_in(
        &paint,
        Rect::from_min_size(more.min + vec2(0.0, 4.0), Vec2::splat(16.0)),
        if edit.more { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT },
        p.text_secondary,
    );
    ux::text(&paint, more.min + vec2(22.0, 2.0), &tr("More"), types::UI, p.text, Some((inner - 22.0).max(0.0)));
    if control(shell, ui, "workspace.properties.index.more", "disclosure", more, true).clicked() {
        edit.more = !edit.more;
    }
    cy += 34.0;
    if edit.more {
        text_field(
            shell,
            ui,
            p,
            at(0.0, cy),
            inner,
            "workspace.properties.index.where",
            &tr("Where"),
            &mut edit.where_,
            archforge_er::rules::MAX_TEXT,
            false,
            false,
        );
        cy += 17.0 + FIELD + 14.0;
        text_field(
            shell,
            ui,
            p,
            at(0.0, cy),
            inner,
            "workspace.properties.index.method",
            &tr("Method"),
            &mut edit.method,
            archforge_er::rules::MAX_NAME,
            false,
            false,
        );
        cy += 17.0 + FIELD + 14.0;
    }
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.index.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.index.cancel",
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
        "workspace.properties.index.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, cy + FIELD + 8.0), why, types::CAPTION, p.amber, Some(inner));
    }
    if let Some(i) = remove_at
        && i < edit.columns.len()
    {
        edit.columns.remove(i);
    }
    // The menus, over the card.
    if shell.panels.menu == Some(Menu::IndexColumn) {
        let used: Vec<Uuid> = edit.columns.iter().filter_map(|c| c.attribute).collect();
        let free: Vec<&(Uuid, String, String)> = all_columns.iter().filter(|(c, _, _)| !used.contains(c)).collect();
        let mut items: Vec<String> = free.iter().map(|(_, n, t)| if t.is_empty() { n.clone() } else { format!("{n} · {t}") }).collect();
        items.push(tr("An expression"));
        if let Some(i) = menu(shell, ui, p, add, "workspace.properties.index.column.add", &items) {
            let attribute = free.get(i).map(|(id, _, _)| *id);
            edit.columns.push(ColumnSpec { attribute, expression: String::new(), direction: SortDirection::Ascending, nulls: NullsOrder::Default });
        }
    }
    if let Some((i, row)) = nulls_anchor {
        let items = [tr("Nulls default"), tr("Nulls first"), tr("Nulls last")];
        if let Some(pick) = menu(shell, ui, p, row, &format!("workspace.properties.index.column.{i}.nulls"), &items)
            && let Some(spec) = edit.columns.get_mut(i)
        {
            spec.nulls = match pick {
                1 => NullsOrder::First,
                2 => NullsOrder::Last,
                _ => NullsOrder::Default,
            };
        }
    }
    shell.panels.index = Some(edit.clone());
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.index = None;
    } else if remove {
        match run(shell, "er.index.remove", json!({ "index": edit.target.map(|t| t.to_string()).unwrap_or_default() })) {
            Ok(_) => shell.panels.index = None,
            Err(e) => refuse(shell, e),
        }
    } else if apply || name_done {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

fn refuse(shell: &mut Shell, why: String) {
    if let Some(i) = shell.panels.index.as_mut() {
        i.refused = Some(why);
    }
}

/// Writes the index through its command; the table stays selected.
fn commit(shell: &mut Shell, edit: &IndexEdit) {
    let columns: Vec<serde_json::Value> = edit
        .columns
        .iter()
        .map(|c| {
            json!({
                "attribute": c.attribute.map(|a| a.to_string()),
                "expression": c.expression.trim(),
                "direction": archforge_er::commands::direction_name(c.direction),
                "nulls": archforge_er::commands::nulls_name(c.nulls),
            })
        })
        .collect();
    let mut params = json!({
        "columns": columns,
        "name": edit.name.trim(),
        "unique": edit.unique,
        "where": edit.where_.trim(),
        "method": edit.method.trim(),
    });
    let cmd = match edit.target {
        None => {
            params["entity"] = json!(edit.entity.to_string());
            "er.index.add"
        }
        Some(t) => {
            params["index"] = json!(t.to_string());
            "er.index.set"
        }
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.index = None;
            shell.panels.notice = None;
            let entity = edit.entity.to_string();
            if let Err(e) = run(shell, "view.select", json!({ "ids": [entity] })) {
                shell.panels.notice = Some(e);
            }
        }
        Err(e) => refuse(shell, e),
    }
}

/// Every hover text to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add index", "Ascending", "Descending"].into_iter()
}
