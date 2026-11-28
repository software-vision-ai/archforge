//! A table's check constraints in Properties (from the design's
//! panel state "Editing a check constraint"): the Checks section lists them with a `CK` badge, the
//! name (or the generated one) and the expression; `+` starts one, a click on a row opens the
//! editor in its place (name, the expression in a multiline field; remove, cancel and apply as
//! icons). Apply is one command, `er.check.add` or `er.check.set`; the bin runs `er.check.remove`.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::er_state::{er, er_mut};
use crate::ux::{self, Palette, icons, types};
use archforge_shell_api::cx::Ctx;
use archforge_shell_api::i18n::{fmt, tr};
use archforge_shell_api::panels::{FIELD, PAD, control_cx, icon_button_cx, run_cx, section_cx, text_field_cx};
use archforge_shell_api::widgets;

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
                name: archforge_core::name_of(store, c),
                label: archforge_er::rules::display_name(store, c),
                expression: archforge_er::rules::text_of(store, c, archforge_er::check_constraint::EXPRESSION),
            })
        })
        .collect()
}

/// Opens the editor on a check, or on one to add to `entity`.
pub fn open(cx: &mut Ctx, entity: Uuid, target: Option<&CheckRow>) {
    if let Some(er) = er_mut(cx.notation_states) {
        er.check = Some(match target {
            Some(c) => CheckEdit { target: Some(c.id), entity, name: c.name.clone(), expression: c.expression.clone(), refused: None, opened: true },
            None => CheckEdit { target: None, entity, name: String::new(), expression: String::new(), refused: None, opened: true },
        });
    }
    cx.panels.menu = None;
}

/// The Checks section with its rows and the editor; gives the y under it.
pub(crate) fn checks_section(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, entity: Obj, entity_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let checks = match cx.session.as_ref() {
        Some(s) => rows(s.doc(), entity),
        None => return y,
    };
    let title = fmt(&tr("Checks · {n}"), &[("n", &checks.len().to_string())]);
    let Some(next) = section_cx(cx, ui, p, r, y, &title, Some(("workspace.properties.checks.add", "Add check constraint"))) else {
        open(cx, entity_id, None);
        return y;
    };
    y = next - 6.0;
    let editing_new = er(cx.notation_states).and_then(|er| er.check.as_ref()).is_some_and(|c| c.target.is_none() && c.entity == entity_id);
    for check in &checks {
        if er(cx.notation_states).and_then(|er| er.check.as_ref()).is_some_and(|c| c.target == Some(check.id)) {
            y = editor(cx, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let id = format!("workspace.properties.check.row.{}", check.id);
        let resp = control_cx(cx, ui, &id, "row", row, true);
        if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        badge(&paint, row.min + vec2(2.0, 6.0), "CK", p.green_tint, p.green);
        ux::text(&paint, row.min + vec2(38.0, 4.0), &check.label, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, row.min + vec2(38.0, 23.0), &check.expression, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        if archforge_shell_api::panels::edit_button_cx(cx, ui, p, row, &id, resp.contains_pointer()) || resp.double_clicked() {
            open(cx, entity_id, Some(check));
        }
        y += 44.0;
    }
    if editing_new {
        y = editor(cx, ui, p, r, y);
    }
    y
}

/// A two-letter badge at `at`.
pub(crate) fn badge(paint: &egui::Painter, at: egui::Pos2, text: &str, tint: ux::Colour, ink: ux::Colour) {
    let br = Rect::from_min_size(at, vec2(26.0, 16.0));
    paint.rect_filled(br, 4.0, tint);
    let g = ux::galley(paint, text, types::TINY, ink, None);
    paint.galley(br.center() - g.size() / 2.0, g, ink.into());
}

/// A label over a filled multiline field of `height`; gives whether it has the keyboard.
pub(crate) fn text_area(
    cx: &mut Ctx,
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
    widgets::record_visible(ui, cx.registry, id, "field", body);
    let inner = body.shrink2(vec2(8.0, 6.0));
    let out = egui::TextEdit::multiline(text)
        .id(widgets::egui_id(id))
        .frame(egui::Frame::NONE)
        .font(types::SMALL.font())
        .text_color(p.text.into())
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
fn editor(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = er(cx.notation_states).and_then(|er| er.check.clone()) else { return y };
    let refused_h = if edit.refused.is_some() { 20.0 } else { 0.0 };
    let height = 12.0 + 17.0 + FIELD + 14.0 + 17.0 + 64.0 + 14.0 + FIELD + 12.0 + refused_h;
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    archforge_shell_api::panels::reveal_cx(cx, ui, crate::er_state::CHECK, card);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = std::mem::take(&mut edit.opened);
    let name_done = text_field_cx(
        cx,
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
    text_area(cx, ui, p, at(0.0, cy), inner, 64.0, "workspace.properties.check.expression", &tr("Expression"), &mut edit.expression);
    cy += 17.0 + 64.0 + 14.0;
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button_cx(cx, ui, p, at(0.0, cy), "workspace.properties.check.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button_cx(
        cx,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.check.cancel",
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = icon_button_cx(
        cx,
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
    if let Some(er) = er_mut(cx.notation_states) {
        er.check = Some(edit.clone());
    }
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if let Some(er) = er_mut(cx.notation_states) {
            er.check = None;
        }
    } else if remove {
        match run_cx(cx, "er.check.remove", json!({ "check": edit.target.map(|t| t.to_string()).unwrap_or_default() })) {
            Ok(_) => {
                if let Some(er) = er_mut(cx.notation_states) {
                    er.check = None;
                }
            }
            Err(e) => refuse(cx, e),
        }
    } else if apply || name_done {
        commit(cx, &edit);
    }
    card.bottom() + 8.0
}

/// Shows `why` on the open check editor's card, which stays open with what was typed.
fn refuse(cx: &mut Ctx, why: String) {
    if let Some(c) = er_mut(cx.notation_states).and_then(|er| er.check.as_mut()) {
        c.refused = Some(why);
    }
}

/// Writes the check through its command; the table stays selected.
fn commit(cx: &mut Ctx, edit: &CheckEdit) {
    let name = edit.name.trim().to_string();
    let expression = edit.expression.trim().to_string();
    let (cmd, params) = match edit.target {
        None => ("er.check.add", json!({ "entity": edit.entity.to_string(), "expression": expression, "name": name })),
        Some(t) => ("er.check.set", json!({ "check": t.to_string(), "expression": expression, "name": name })),
    };
    match run_cx(cx, cmd, params) {
        Ok(_) => {
            if let Some(er) = er_mut(cx.notation_states) {
                er.check = None;
            }
            cx.panels.notice = None;
            let entity = edit.entity.to_string();
            if let Err(e) = run_cx(cx, "view.select", json!({ "ids": [entity] })) {
                cx.panels.notice = Some(e);
            }
        }
        Err(e) => refuse(cx, e),
    }
}

/// Every hover text to translate, for the catalogue tests.
#[cfg(feature = "snapshot")]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add check constraint"].into_iter()
}
