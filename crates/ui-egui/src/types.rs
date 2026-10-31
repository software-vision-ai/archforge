//! A schema's domains and enum types in Properties (slice S5 of archforge-gm2.34, from the design's
//! panel states "Schema with types", "Domain editor" and "Enum editor"): the Types section lists
//! them with a `DM` or `EN` badge, the name and the base type with its size, not null and checks,
//! or the literals; `+` asks which kind to add and opens the editor, a click on a row opens the
//! editor in its place. The domain editor takes the name, the base type with the size fields its
//! shape allows, the default, Not null and the checks; the enum editor the name and the literals
//! in order, typed in place. Apply is one command, `er.domain.add`/`set` or `er.enum.add`/`set`;
//! the bin runs the remove. A selected domain or enum type shows its schema's section with its
//! editor open.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::checks::badge;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, check, ghost_button, icon_button, menu, number, run, section, size_value, text_field};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// Which kind of type an editor edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Domain,
    Enum,
}

/// A domain or enum type being edited: the type, or none for one about to be added to `schema`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeEdit {
    pub kind: TypeKind,
    pub target: Option<Uuid>,
    pub schema: Uuid,
    pub name: String,
    /// A domain's fields.
    pub base: String,
    pub length: String,
    pub precision: String,
    pub scale: String,
    pub default: String,
    pub not_null: bool,
    pub checks: Vec<String>,
    /// An enum type's literals, in order.
    pub literals: Vec<String>,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once.
    pub opened: bool,
}

/// One type as the panel lists it.
pub struct TypeRow {
    pub id: Uuid,
    pub kind: TypeKind,
    pub name: String,
    /// The base type with its size, not null and checks; or the literals.
    pub detail: String,
}

/// The domains, then the enum types, of a schema.
pub fn rows(store: &Store, schema: Obj) -> Vec<TypeRow> {
    use archforge_er::rules as r;
    let mut out = Vec::new();
    for d in r::domains_of(store, schema) {
        let Ok(id) = store.uuid(d) else { continue };
        let (l, p, s) = r::own_size_of(store, d);
        let base = r::text_of(store, d, archforge_er::domain::BASE_TYPE);
        let mut detail = match (l, p, s) {
            (Some(l), _, _) => format!("{base}({l})"),
            (_, Some(p), Some(s)) => format!("{base}({p}, {s})"),
            (_, Some(p), None) => format!("{base}({p})"),
            _ => base,
        };
        if store.get(d, archforge_er::domain::NOT_NULL).ok().and_then(|v| v.as_bool()).unwrap_or(false) {
            detail = format!("{detail} · {}", tr("not null"));
        }
        let checks = r::domain_checks(store, d).len();
        if checks > 0 {
            detail = format!("{detail} · {}", crate::i18n::trn("{n} check", "{n} checks", checks as u64));
        }
        out.push(TypeRow { id, kind: TypeKind::Domain, name: r::name_of(store, d), detail });
    }
    for e in r::enums_of(store, schema) {
        let Ok(id) = store.uuid(e) else { continue };
        out.push(TypeRow { id, kind: TypeKind::Enum, name: r::name_of(store, e), detail: r::literals_of(store, e).join(" · ") });
    }
    out
}

/// Opens the editor on a type, or on one of `kind` to add to `schema`.
pub fn open(shell: &mut Shell, schema: Uuid, kind: TypeKind, target: Option<Uuid>) {
    let mut edit = TypeEdit {
        kind,
        target,
        schema,
        name: String::new(),
        base: "TEXT".into(),
        length: String::new(),
        precision: String::new(),
        scale: String::new(),
        default: String::new(),
        not_null: false,
        checks: Vec::new(),
        literals: Vec::new(),
        refused: None,
        opened: true,
    };
    if let (Some(t), Some(store)) = (target, shell.session.as_ref().map(|s| s.doc()))
        && let Some(o) = store.by_uuid(t)
    {
        use archforge_er::rules as r;
        edit.name = r::name_of(store, o);
        match kind {
            TypeKind::Domain => {
                let (l, p, s) = r::own_size_of(store, o);
                edit.base = r::text_of(store, o, archforge_er::domain::BASE_TYPE);
                edit.length = number(l);
                edit.precision = number(p);
                edit.scale = number(s);
                edit.default = r::text_of(store, o, archforge_er::domain::DEFAULT);
                edit.not_null = store.get(o, archforge_er::domain::NOT_NULL).ok().and_then(|v| v.as_bool()).unwrap_or(false);
                edit.checks = r::domain_checks(store, o).into_iter().map(|c| r::text_of(store, c, archforge_er::domain_check::EXPRESSION)).collect();
            }
            TypeKind::Enum => edit.literals = r::literals_of(store, o),
        }
    }
    shell.panels.type_edit = Some(edit);
    shell.panels.menu = None;
}

/// The kind of a type element, when it is one.
pub fn kind_of(store: &Store, o: Obj) -> Option<TypeKind> {
    match store.class_of(o).ok() {
        Some(c) if c == archforge_er::DOMAIN => Some(TypeKind::Domain),
        Some(c) if c == archforge_er::ENUM_TYPE => Some(TypeKind::Enum),
        _ => None,
    }
}

/// The Types section with its rows and the editor; gives the y under it.
pub(crate) fn types_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, schema: Obj, schema_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let list = match shell.session.as_ref() {
        Some(s) => rows(s.doc(), schema),
        None => return y,
    };
    let title = fmt(&tr("Types · {n}"), &[("n", &list.len().to_string())]);
    let add_at = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.types.add", "Add type"))) else {
        shell.panels.menu = if shell.panels.menu == Some(Menu::TypeKind) { None } else { Some(Menu::TypeKind) };
        return y + 50.0;
    };
    y = next - 6.0;
    let editing_new = shell.panels.type_edit.as_ref().is_some_and(|t| t.target.is_none() && t.schema == schema_id);
    let chosen = crate::panels::selected_id(shell);
    for row in &list {
        if shell.panels.type_edit.as_ref().is_some_and(|t| t.target == Some(row.id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let rect = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let id = format!("workspace.properties.type.row.{}", row.id);
        let resp = control(shell, ui, &id, "row", rect, true);
        if Some(row.id) == chosen {
            paint.rect_filled(rect, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(rect, 6.0, p.page);
        }
        match row.kind {
            TypeKind::Domain => badge(&paint, rect.min + vec2(2.0, 6.0), "DM", p.green_tint, p.green),
            TypeKind::Enum => badge(&paint, rect.min + vec2(2.0, 6.0), "EN", p.purple_tint, p.purple),
        }
        ux::text(&paint, rect.min + vec2(38.0, 4.0), &row.name, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, rect.min + vec2(38.0, 23.0), &row.detail, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        if crate::panels::edit_button(shell, ui, p, rect, &id, resp.contains_pointer()) || resp.double_clicked() {
            open(shell, schema_id, row.kind, Some(row.id));
        }
        y += 44.0;
    }
    if editing_new {
        y = editor(shell, ui, p, r, y);
    }
    // Which kind to add, under the +.
    if shell.panels.menu == Some(Menu::TypeKind) {
        let items = [tr("Domain"), tr("Enum type")];
        let anchor = Rect::from_min_size(pos2(r.right() - PAD - 160.0, add_at.top()), vec2(160.0, 28.0));
        if let Some(i) = menu(shell, ui, p, anchor, "workspace.properties.types.add", &items) {
            open(shell, schema_id, if i == 0 { TypeKind::Domain } else { TypeKind::Enum }, None);
        }
    }
    y
}

/// The domain or enum editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.type_edit.clone() else { return y };
    let row = 17.0 + FIELD + 10.0;
    let shape = archforge_er::rules::type_shape(&edit.base);
    let height = match edit.kind {
        TypeKind::Domain => 12.0 + row * 3.0 + 28.0 + 17.0 + edit.checks.len() as f32 * 32.0 + 24.0 + 14.0 + FIELD + 12.0,
        TypeKind::Enum => 12.0 + row + 17.0 + edit.literals.len() as f32 * 32.0 + 24.0 + 14.0 + FIELD + 12.0,
    } + if edit.refused.is_some() { 20.0 } else { 0.0 };
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    crate::panels::reveal(shell, ui, card);
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
        "workspace.properties.type.name",
        &tr("Name"),
        &mut edit.name,
        archforge_er::rules::MAX_NAME,
        take,
        false,
    );
    cy += row;
    let mut remove_at: Option<usize> = None;
    match edit.kind {
        TypeKind::Domain => {
            // The base type, with the size fields its shape takes beside it.
            let (type_w, size_w) = match shape {
                archforge_er::rules::TypeShape::Plain => (inner, 0.0),
                archforge_er::rules::TypeShape::PrecisionScale => ((inner - 2.0 * 64.0 - 16.0).max(60.0), 64.0),
                _ => ((inner - 80.0 - 8.0).max(60.0), 80.0),
            };
            enter |= text_field(
                shell,
                ui,
                p,
                at(0.0, cy),
                type_w,
                "workspace.properties.domain.base",
                &tr("Base type"),
                &mut edit.base,
                archforge_er::rules::MAX_TYPE,
                false,
                false,
            );
            match shape {
                archforge_er::rules::TypeShape::Length => {
                    enter |= text_field(
                        shell,
                        ui,
                        p,
                        at(type_w + 8.0, cy),
                        size_w,
                        "workspace.properties.domain.length",
                        &tr("Length"),
                        &mut edit.length,
                        5,
                        false,
                        false,
                    );
                }
                archforge_er::rules::TypeShape::Precision => {
                    enter |= text_field(
                        shell,
                        ui,
                        p,
                        at(type_w + 8.0, cy),
                        size_w,
                        "workspace.properties.domain.precision",
                        &tr("Precision"),
                        &mut edit.precision,
                        5,
                        false,
                        false,
                    );
                }
                archforge_er::rules::TypeShape::PrecisionScale => {
                    enter |= text_field(
                        shell,
                        ui,
                        p,
                        at(type_w + 8.0, cy),
                        size_w,
                        "workspace.properties.domain.precision",
                        &tr("Precision"),
                        &mut edit.precision,
                        5,
                        false,
                        false,
                    );
                    enter |= text_field(
                        shell,
                        ui,
                        p,
                        at(type_w + 8.0 + size_w + 8.0, cy),
                        size_w,
                        "workspace.properties.domain.scale",
                        &tr("Scale"),
                        &mut edit.scale,
                        5,
                        false,
                        false,
                    );
                }
                archforge_er::rules::TypeShape::Plain => {}
            }
            cy += row;
            enter |= text_field(
                shell,
                ui,
                p,
                at(0.0, cy),
                inner,
                "workspace.properties.domain.default",
                &tr("Default"),
                &mut edit.default,
                archforge_er::rules::MAX_TEXT,
                false,
                false,
            );
            cy += row;
            check(shell, ui, p, at(0.0, cy), "workspace.properties.domain.not-null", &tr("Not null"), &mut edit.not_null);
            cy += 28.0;
            ux::text(&paint, at(0.0, cy), &tr("Checks"), types::CAPTION, p.text_secondary, Some(inner));
            cy += 17.0;
            for i in 0..edit.checks.len() {
                let rect = Rect::from_min_size(at(0.0, cy), vec2(inner, FIELD));
                paint.rect_filled(rect, 6.0, p.field);
                let field =
                    crate::panels::field_inner(ui, Rect::from_min_max(pos2(rect.left() + 0.0, rect.top()), pos2(rect.right() - 22.0, rect.bottom())));
                let id = format!("workspace.properties.domain.check.{i}");
                widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
                let out = egui::TextEdit::singleline(&mut edit.checks[i])
                    .id(widgets::egui_id(&id))
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .font(types::SMALL.font())
                    .text_color(p.text)
                    .char_limit(archforge_er::rules::MAX_TEXT)
                    .desired_width(field.width())
                    .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(field)));
                let _ = (*out.response).clone().accessible_name(tr("Checks"));
                if out.response.has_focus() {
                    paint.rect_stroke(rect, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
                }
                let x = Rect::from_min_size(pos2(rect.right() - 26.0, rect.top() + 6.0), Vec2::splat(16.0));
                ux::icon_in(&paint, x, &icons::CLOSE, p.text_secondary);
                if control(shell, ui, &format!("{id}.remove"), "button", x.expand(4.0), true).on_hover_text(tr("Remove")).clicked() {
                    remove_at = Some(i);
                }
                cy += 32.0;
            }
            if ghost_button(shell, ui, p, at(0.0, cy), "workspace.properties.domain.check.add", &icons::PLUS, &tr("Add a check"))
                && edit.checks.len() < archforge_er::rules::MAX_KEY_COLUMNS
            {
                edit.checks.push(String::new());
            }
            cy += 24.0 + 14.0;
            if let Some(i) = remove_at
                && i < edit.checks.len()
            {
                edit.checks.remove(i);
            }
        }
        TypeKind::Enum => {
            ux::text(&paint, at(0.0, cy), &tr("Literals, in order"), types::CAPTION, p.text_secondary, Some(inner));
            cy += 17.0;
            for i in 0..edit.literals.len() {
                let rect = Rect::from_min_size(at(0.0, cy), vec2(inner, FIELD));
                paint.rect_filled(rect, 6.0, p.field);
                ux::icon_mid(&paint, pos2(rect.left() + 14.0, rect.center().y), &icons::GRIP, p.text_muted);
                let field = crate::panels::field_inner(
                    ui,
                    Rect::from_min_max(pos2(rect.left() + 20.0, rect.top()), pos2(rect.right() - 22.0, rect.bottom())),
                );
                let id = format!("workspace.properties.enum.literal.{i}");
                widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
                let out = egui::TextEdit::singleline(&mut edit.literals[i])
                    .id(widgets::egui_id(&id))
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .font(types::SMALL.font())
                    .text_color(p.text)
                    .char_limit(archforge_er::rules::MAX_NAME)
                    .desired_width(field.width())
                    .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(field)));
                let _ = (*out.response).clone().accessible_name(tr("Literals, in order"));
                if out.response.has_focus() {
                    paint.rect_stroke(rect, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
                }
                let x = Rect::from_min_size(pos2(rect.right() - 26.0, rect.top() + 6.0), Vec2::splat(16.0));
                ux::icon_in(&paint, x, &icons::CLOSE, p.text_secondary);
                if control(shell, ui, &format!("{id}.remove"), "button", x.expand(4.0), true).on_hover_text(tr("Remove")).clicked() {
                    remove_at = Some(i);
                }
                cy += 32.0;
            }
            if ghost_button(shell, ui, p, at(0.0, cy), "workspace.properties.enum.literal.add", &icons::PLUS, &tr("Add a literal"))
                && edit.literals.len() < archforge_er::rules::MAX_LITERALS
            {
                edit.literals.push(String::new());
            }
            cy += 24.0 + 14.0;
            if let Some(i) = remove_at
                && i < edit.literals.len()
            {
                edit.literals.remove(i);
            }
        }
    }
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.type.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.type.cancel",
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
        "workspace.properties.type.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, cy + FIELD + 8.0), why, types::CAPTION, p.amber, Some(inner));
    }
    shell.panels.type_edit = Some(edit.clone());
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.type_edit = None;
    } else if remove {
        let (cmd, key) = match edit.kind {
            TypeKind::Domain => ("er.domain.remove", "domain"),
            TypeKind::Enum => ("er.enum.remove", "enum"),
        };
        match run(shell, cmd, json!({ key: edit.target.map(|t| t.to_string()).unwrap_or_default() })) {
            Ok(_) => {
                shell.panels.type_edit = None;
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
    if let Some(t) = shell.panels.type_edit.as_mut() {
        t.refused = Some(why);
    }
}

/// The schema is what the panel shows after a type command selected the type.
fn reselect(shell: &mut Shell, schema: Uuid) {
    if let Err(e) = run(shell, "view.select", json!({ "ids": [schema.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// Writes the type through its command; the schema stays selected.
fn commit(shell: &mut Shell, edit: &TypeEdit) {
    let name = edit.name.trim().to_string();
    let (cmd, params) = match edit.kind {
        TypeKind::Domain => {
            let size = match (size_value(&edit.length), size_value(&edit.precision), size_value(&edit.scale)) {
                (Ok(l), Ok(p), Ok(s)) => (l, p, s),
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                    refuse(shell, e);
                    return;
                }
            };
            let checks: Vec<String> = edit.checks.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
            let mut params = json!({
                "name": name, "base_type": edit.base.trim(), "length": size.0, "precision": size.1, "scale": size.2,
                "default": edit.default.trim(), "not_null": edit.not_null, "checks": checks,
            });
            match edit.target {
                None => {
                    params["schema"] = json!(edit.schema.to_string());
                    ("er.domain.add", params)
                }
                Some(t) => {
                    params["domain"] = json!(t.to_string());
                    ("er.domain.set", params)
                }
            }
        }
        TypeKind::Enum => {
            let literals: Vec<String> = edit.literals.iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
            let mut params = json!({ "name": name, "literals": literals });
            match edit.target {
                None => {
                    params["schema"] = json!(edit.schema.to_string());
                    ("er.enum.add", params)
                }
                Some(t) => {
                    params["enum"] = json!(t.to_string());
                    ("er.enum.set", params)
                }
            }
        }
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.type_edit = None;
            shell.panels.notice = None;
            reselect(shell, edit.schema);
        }
        Err(e) => refuse(shell, e),
    }
}

/// Every hover text to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add type"].into_iter()
}
