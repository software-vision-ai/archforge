//! A table's keys in Properties (slice E4 of archforge-gm2.10, from the design's panel state
//! "Editing a unique constraint"): the Keys section lists the primary key and the unique
//! constraints; `+` starts a unique constraint, a click on a row opens the key editor in its place
//! (name, the columns in key order with a grip and a remove, Add a column, Nulls distinct for a
//! unique constraint; remove, cancel and apply as icons). Apply is the one step that goes to undo,
//! through `er.primary-key.set`, `er.unique.add` or `er.unique.set`; the bin runs
//! `er.primary-key.remove` or `er.unique.remove`. Below the keys, Problems lists what the rules
//! find on the selection; a click selects the element.

use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, check, icon_button, menu, run, section, text_field};
use crate::ux::{self, Palette, icons, types};

/// A key being edited: the key, or none for a unique constraint about to be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEdit {
    pub target: Option<Uuid>,
    pub entity: Uuid,
    pub primary: bool,
    pub name: String,
    pub columns: Vec<Uuid>,
    pub nulls_distinct: bool,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once.
    pub opened: bool,
}

/// One key as the panel lists it.
pub struct KeyRow {
    pub id: Uuid,
    pub primary: bool,
    pub name: String,
    pub columns: Vec<(Uuid, String, String)>,
    pub nulls_distinct: bool,
}

fn name(store: &Store, o: Obj) -> String {
    archforge_er::rules::name_of(store, o)
}

/// The keys of a table: the primary key first, then the unique constraints.
pub fn rows(store: &Store, e: Obj) -> Vec<KeyRow> {
    let row = |k: Obj, primary: bool| {
        Some(KeyRow {
            id: store.uuid(k).ok()?,
            primary,
            name: name(store, k),
            columns: archforge_er::rules::key_columns(store, k)
                .into_iter()
                .filter_map(|a| Some((store.uuid(a).ok()?, name(store, a), archforge_er::rules::type_of(store, a))))
                .collect(),
            nulls_distinct: !primary && archforge_er::rules::nulls_distinct(store, k),
        })
    };
    let mut out: Vec<KeyRow> = archforge_er::rules::primary_key_of(store, e).and_then(|k| row(k, true)).into_iter().collect();
    out.extend(archforge_er::rules::unique_constraints(store, e).into_iter().filter_map(|k| row(k, false)));
    out
}

/// Opens the editor on a key, or on a unique constraint to add to `entity`.
pub fn open(shell: &mut Shell, entity: Uuid, target: Option<&KeyRow>) {
    shell.panels.key = Some(match target {
        Some(k) => KeyEdit {
            target: Some(k.id),
            entity,
            primary: k.primary,
            name: k.name.clone(),
            columns: k.columns.iter().map(|(id, _, _)| *id).collect(),
            nulls_distinct: k.nulls_distinct,
            refused: None,
            opened: true,
        },
        None => KeyEdit {
            target: None,
            entity,
            primary: false,
            name: String::new(),
            columns: Vec::new(),
            nulls_distinct: true,
            refused: None,
            opened: true,
        },
    });
    shell.panels.menu = None;
}

/// The Keys section with its rows and the editor; gives the y under it.
pub(crate) fn keys_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, entity: Obj, entity_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let keys = match shell.session.as_ref() {
        Some(s) => rows(s.doc(), entity),
        None => return y,
    };
    let title = fmt(&tr("Keys · {n}"), &[("n", &keys.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.keys.add", "Add unique constraint"))) else {
        open(shell, entity_id, None);
        return y;
    };
    y = next - 6.0;
    let editing_new = shell.panels.key.as_ref().is_some_and(|k| k.target.is_none() && k.entity == entity_id);
    for key in &keys {
        if shell.panels.key.as_ref().is_some_and(|k| k.target == Some(key.id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let resp = control(shell, ui, &format!("workspace.properties.key.row.{}", key.id), "row", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 6.0, p.page);
            ux::icon_in(&paint, Rect::from_min_size(pos2(row.right() - 24.0, row.top() + 10.0), Vec2::splat(16.0)), &icons::EDIT, p.text_secondary);
        }
        let (b, tint, ink) = if key.primary { ("PK", p.amber_tint, p.amber) } else { ("UQ", p.accent_tint, p.accent) };
        let br = Rect::from_min_size(row.min + vec2(2.0, 6.0), vec2(26.0, 16.0));
        paint.rect_filled(br, 4.0, tint);
        let g = ux::galley(&paint, b, types::TINY, ink, None);
        paint.galley(br.center() - g.size() / 2.0, g, ink);
        let label = if key.name.is_empty() { if key.primary { tr("Primary key") } else { tr("Unique") } } else { key.name.clone() };
        let columns = key.columns.iter().map(|(_, n, _)| n.as_str()).collect::<Vec<_>>().join(", ");
        ux::text(&paint, row.min + vec2(38.0, 4.0), &label, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, row.min + vec2(38.0, 23.0), &columns, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        if resp.clicked() {
            open(shell, entity_id, Some(key));
        }
        y += 44.0;
    }
    if editing_new {
        y = editor(shell, ui, p, r, y);
    }
    y
}

/// The key editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.key.clone() else { return y };
    // The table's columns, named, for the rows and the menu.
    let all_columns: Vec<(Uuid, String, String)> = match shell.session.as_ref().and_then(|s| s.doc().by_uuid(edit.entity).map(|e| (s.doc(), e))) {
        Some((store, e)) => store
            .list(e, archforge_er::entity::ATTRIBUTE)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_ref)
            .filter(|a| store.is_alive(*a))
            .filter_map(|a| Some((store.uuid(a).ok()?, name(store, a), archforge_er::rules::type_of(store, a))))
            .collect(),
        None => return y,
    };
    let rows = edit.columns.len() as f32;
    let nulls_row = if edit.primary { 0.0 } else { 28.0 };
    let refused_h = if edit.refused.is_some() { 20.0 } else { 0.0 };
    let height = 12.0 + 17.0 + FIELD + 14.0 + 17.0 + rows * 32.0 + FIELD + 12.0 + nulls_row + 14.0 + FIELD + 12.0 + refused_h;
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = std::mem::take(&mut edit.opened);
    let name_done = text_field(
        shell,
        ui,
        p,
        at(0.0, 12.0),
        inner,
        "workspace.properties.key.name",
        &tr("Name"),
        &mut edit.name,
        archforge_er::rules::MAX_NAME,
        take,
        false,
    );
    let mut cy = 12.0 + 17.0 + FIELD + 14.0;
    ux::text(&paint, at(0.0, cy), &tr("Columns, in key order"), types::CAPTION, p.text_secondary, Some(inner));
    cy += 17.0;
    let mut remove_at: Option<usize> = None;
    for (i, id) in edit.columns.iter().enumerate() {
        let row = Rect::from_min_size(at(0.0, cy), vec2(inner, FIELD));
        paint.rect_filled(row, 6.0, p.field);
        ux::text(&paint, row.min + vec2(8.0, 8.0), "⋮⋮", types::CAPTION, p.text_muted, None);
        let (n, ty) = all_columns.iter().find(|(c, _, _)| c == id).map(|(_, n, t)| (n.clone(), t.clone())).unwrap_or_default();
        let name_rect = ux::text(&paint, row.min + vec2(28.0, 5.0), &n, types::SMALL, p.text, Some((inner - 80.0).max(0.0)));
        ux::text(
            &paint,
            pos2(name_rect.right() + 8.0, row.top() + 6.0),
            &ty,
            types::CAPTION,
            p.text_muted,
            Some((row.right() - 36.0 - name_rect.right() - 8.0).max(0.0)),
        );
        let x = Rect::from_min_size(pos2(row.right() - 26.0, row.top() + 6.0), Vec2::splat(16.0));
        ux::icon_in(&paint, x, &icons::CLOSE, p.text_secondary);
        if control(shell, ui, &format!("workspace.properties.key.column.{i}.remove"), "button", x.expand(4.0), true)
            .on_hover_text(tr("Remove"))
            .clicked()
        {
            remove_at = Some(i);
        }
        cy += 32.0;
    }
    // Add a column: the table's columns not in the key yet.
    let add = Rect::from_min_size(at(0.0, cy), vec2(inner, FIELD));
    paint.rect_filled(add, 6.0, p.field);
    ux::text(&paint, add.min + vec2(8.0, 5.0), &tr("Add a column"), types::SMALL, p.text_secondary, Some((inner - 36.0).max(0.0)));
    ux::icon(&paint, pos2(add.right() - 22.0, add.top() + 6.0), &icons::CHEVRON_DOWN, 16.0, p.text_secondary);
    if control(shell, ui, "workspace.properties.key.column.add", "select", add, true).clicked() {
        shell.panels.menu = if shell.panels.menu == Some(Menu::KeyColumn) { None } else { Some(Menu::KeyColumn) };
    }
    cy += FIELD + 12.0;
    if !edit.primary {
        check(shell, ui, p, at(0.0, cy), "workspace.properties.key.nulls-distinct", &tr("Nulls distinct"), &mut edit.nulls_distinct);
        cy += 28.0;
    }
    cy += 14.0;
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.key.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.key.cancel",
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
        "workspace.properties.key.apply",
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
    // The column menu, over the card.
    if shell.panels.menu == Some(Menu::KeyColumn) {
        let free: Vec<&(Uuid, String, String)> = all_columns.iter().filter(|(c, _, _)| !edit.columns.contains(c)).collect();
        let items: Vec<String> = free.iter().map(|(_, n, t)| if t.is_empty() { n.clone() } else { format!("{n} · {t}") }).collect();
        if items.is_empty() {
            shell.panels.menu = None;
        } else if let Some(i) = menu(shell, ui, p, add, "workspace.properties.key.column.add", &items)
            && let Some((id, _, _)) = free.get(i)
        {
            edit.columns.push(*id);
        }
    }
    shell.panels.key = Some(edit.clone());
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.key = None;
    } else if remove {
        let (cmd, params) = if edit.primary {
            ("er.primary-key.remove", json!({ "entity": edit.entity.to_string() }))
        } else {
            ("er.unique.remove", json!({ "unique": edit.target.map(|t| t.to_string()).unwrap_or_default() }))
        };
        match run(shell, cmd, params) {
            Ok(_) => shell.panels.key = None,
            Err(e) => refuse(shell, e),
        }
    } else if apply || name_done {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

fn refuse(shell: &mut Shell, why: String) {
    if let Some(k) = shell.panels.key.as_mut() {
        k.refused = Some(why);
    }
}

/// Writes the key through the commands.
fn commit(shell: &mut Shell, edit: &KeyEdit) {
    let columns: Vec<String> = edit.columns.iter().map(Uuid::to_string).collect();
    let name = edit.name.trim().to_string();
    let (cmd, params) = match (edit.primary, edit.target) {
        (true, _) => ("er.primary-key.set", json!({ "entity": edit.entity.to_string(), "columns": columns, "name": name })),
        (false, None) => {
            ("er.unique.add", json!({ "entity": edit.entity.to_string(), "columns": columns, "name": name, "nulls_distinct": edit.nulls_distinct }))
        }
        (false, Some(t)) => {
            ("er.unique.set", json!({ "unique": t.to_string(), "columns": columns, "name": name, "nulls_distinct": edit.nulls_distinct }))
        }
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.key = None;
            shell.panels.notice = None;
            // The command selected the key; the table is what the panel shows.
            let entity = edit.entity.to_string();
            if let Err(e) = run(shell, "view.select", json!({ "ids": [entity] })) {
                shell.panels.notice = Some(e);
            }
        }
        Err(e) => refuse(shell, e),
    }
}

/// The Problems section for a selection, when the rules find any; gives the y under it.
pub(crate) fn problems_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: Obj) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let problems: Vec<(Option<Uuid>, String)> = match shell.session.as_ref() {
        Some(s) => crate::problems::of(s.doc(), &shell.problems, subject).into_iter().map(|pr| (s.doc().uuid(pr.element).ok(), pr.text)).collect(),
        None => return y,
    };
    if problems.is_empty() {
        return y;
    }
    let title = fmt(&tr("Problems · {n}"), &[("n", &problems.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
    let own = shell.session.as_ref().and_then(|s| s.doc().uuid(subject).ok());
    for (i, (element, text)) in problems.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 28.0));
        let live = element.is_some() && *element != own;
        let resp = control(shell, ui, &format!("workspace.properties.problem.{i}"), "row", row, live);
        if resp.hovered() && live {
            paint.rect_filled(row, 6.0, p.page);
        }
        paint.circle_stroke(pos2(row.left() + 10.0, row.center().y), 6.0, Stroke::new(1.2, p.key_foreign));
        ux::text(&paint, pos2(row.left() + 8.0, row.top() + 6.0), "!", types::TINY, p.key_foreign, None);
        ux::text(&paint, row.min + vec2(26.0, 6.0), text, types::SMALL, p.text, Some((width - 30.0).max(0.0)));
        if resp.clicked()
            && live
            && let Some(id) = element
            && let Err(e) = run(shell, "view.select", json!({ "ids": [id.to_string()] }))
        {
            shell.panels.notice = Some(e);
        }
        y += 28.0;
    }
    y + 8.0
}

/// Every hover text to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add unique constraint", "Remove", "Cancel", "Apply"].into_iter()
}
