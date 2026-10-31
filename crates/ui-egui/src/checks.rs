//! A table's check constraints in Properties (slice S3 of archforge-gm2.34, from the design's
//! panel state "Editing a check constraint"): the Checks section lists them with a `CK` badge, the
//! name (or the generated one) and the expression; `+` starts one, a click on a row opens the
//! editor in its place (name, the expression in a multiline field; remove, cancel and apply as
//! icons). Apply is one command, `er.check.add` or `er.check.set`; the bin runs `er.check.remove`.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, PAD, icon_button, run, section, text_field};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// A check being edited: the check, or none for one about to be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckEdit {
    pub target: Option<Uuid>,
    pub entity: Uuid,
    pub name: String,
    pub expression: String,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once.
    pub opened: bool,
}

/// One check as the panel lists it.
pub struct CheckRow {
    pub id: Uuid,
    pub name: String,
    pub label: String,
    pub expression: String,
}

/// The checks of a table, in order.
pub fn rows(store: &Store, e: Obj) -> Vec<CheckRow> {
    archforge_er::rules::checks_of(store, e)
        .into_iter()
        .filter_map(|c| {
            Some(CheckRow {
                id: store.uuid(c).ok()?,
                name: archforge_er::rules::name_of(store, c),
                label: archforge_er::rules::display_name(store, c),
                expression: archforge_er::rules::text_of(store, c, archforge_er::check_constraint::EXPRESSION),
            })
        })
        .collect()
}

/// Opens the editor on a check, or on one to add to `entity`.
pub fn open(shell: &mut Shell, entity: Uuid, target: Option<&CheckRow>) {
    shell.panels.check = Some(match target {
        Some(c) => CheckEdit { target: Some(c.id), entity, name: c.name.clone(), expression: c.expression.clone(), refused: None, opened: true },
        None => CheckEdit { target: None, entity, name: String::new(), expression: String::new(), refused: None, opened: true },
    });
    shell.panels.menu = None;
}

/// The Checks section with its rows and the editor; gives the y under it.
pub(crate) fn checks_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, entity: Obj, entity_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let checks = match shell.session.as_ref() {
        Some(s) => rows(s.doc(), entity),
        None => return y,
    };
    let title = fmt(&tr("Checks · {n}"), &[("n", &checks.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.checks.add", "Add check constraint"))) else {
        open(shell, entity_id, None);
        return y;
    };
    y = next - 6.0;
    let editing_new = shell.panels.check.as_ref().is_some_and(|c| c.target.is_none() && c.entity == entity_id);
    for check in &checks {
        if shell.panels.check.as_ref().is_some_and(|c| c.target == Some(check.id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let resp = control(shell, ui, &format!("workspace.properties.check.row.{}", check.id), "row", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 6.0, p.page);
            ux::icon_mid(&paint, pos2(row.right() - 16.0, row.center().y), &icons::EDIT, p.text_secondary);
        }
        badge(&paint, row.min + vec2(2.0, 6.0), "CK", p.green_tint, p.green);
        ux::text(&paint, row.min + vec2(38.0, 4.0), &check.label, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, row.min + vec2(38.0, 23.0), &check.expression, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        if resp.clicked() {
            open(shell, entity_id, Some(check));
        }
        y += 44.0;
    }
    if editing_new {
        y = editor(shell, ui, p, r, y);
    }
    y
}

/// A two-letter badge at `at`.
pub(crate) fn badge(paint: &egui::Painter, at: egui::Pos2, text: &str, tint: egui::Color32, ink: egui::Color32) {
    let br = Rect::from_min_size(at, vec2(26.0, 16.0));
    paint.rect_filled(br, 4.0, tint);
    let g = ux::galley(paint, text, types::TINY, ink, None);
    paint.galley(br.center() - g.size() / 2.0, g, ink);
}

/// A label over a filled multiline field of `height`; gives whether it has the keyboard.
pub(crate) fn text_area(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    height: f32,
    id: &str,
    label: &str,
    text: &mut String,
) {
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let body = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), height));
    paint.rect_filled(body, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, id, "field", body);
    let inner = body.shrink2(vec2(8.0, 6.0));
    let out = egui::TextEdit::multiline(text)
        .id(widgets::egui_id(id))
        .frame(egui::Frame::NONE)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(archforge_er::rules::MAX_TEXT)
        .desired_width(inner.width())
        .desired_rows(2)
        .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(label);
    if out.response.has_focus() {
        paint.rect_stroke(body, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
}

/// The check editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.check.clone() else { return y };
    let refused_h = if edit.refused.is_some() { 20.0 } else { 0.0 };
    let height = 12.0 + 17.0 + FIELD + 14.0 + 17.0 + 64.0 + 14.0 + FIELD + 12.0 + refused_h;
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
        "workspace.properties.check.name",
        &tr("Name"),
        &mut edit.name,
        archforge_er::rules::MAX_NAME,
        take,
        false,
    );
    let mut cy = 12.0 + 17.0 + FIELD + 14.0;
    text_area(shell, ui, p, at(0.0, cy), inner, 64.0, "workspace.properties.check.expression", &tr("Expression"), &mut edit.expression);
    cy += 17.0 + 64.0 + 14.0;
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.check.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.check.cancel",
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
        "workspace.properties.check.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, cy + FIELD + 8.0), why, types::CAPTION, p.amber, Some(inner));
    }
    shell.panels.check = Some(edit.clone());
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.check = None;
    } else if remove {
        match run(shell, "er.check.remove", json!({ "check": edit.target.map(|t| t.to_string()).unwrap_or_default() })) {
            Ok(_) => shell.panels.check = None,
            Err(e) => refuse(shell, e),
        }
    } else if apply || name_done {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

fn refuse(shell: &mut Shell, why: String) {
    if let Some(c) = shell.panels.check.as_mut() {
        c.refused = Some(why);
    }
}

/// Writes the check through its command; the table stays selected.
fn commit(shell: &mut Shell, edit: &CheckEdit) {
    let name = edit.name.trim().to_string();
    let expression = edit.expression.trim().to_string();
    let (cmd, params) = match edit.target {
        None => ("er.check.add", json!({ "entity": edit.entity.to_string(), "expression": expression, "name": name })),
        Some(t) => ("er.check.set", json!({ "check": t.to_string(), "expression": expression, "name": name })),
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.check = None;
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
    ["Add check constraint"].into_iter()
}
