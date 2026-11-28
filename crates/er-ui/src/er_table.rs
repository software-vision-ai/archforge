//! A table in Properties, ER's own (from the design's pages 03 and 04): its name and kind (regular
//! or mapping table), the offer to collapse a pure mapping table back into its line, its columns
//! with the in-place column editor (type and size by the type's shape, default, generation,
//! collation, the column's flags, its domain or enum type, and the link to the column it follows),
//! then its keys, checks, indexes, subtypes and problems. A column's panel is its table's. Every
//! change goes through ER's commands, as one undo step; a refusal shows on the editor's card.

use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::er_state::menus;
use crate::er_state::{er, er_mut};
use crate::er_ui::children;
use crate::ux::{self, Palette, icons, types};
use archforge_shell_api::cx::Ctx;
use archforge_shell_api::i18n::{fmt, tr};
use archforge_shell_api::notation_ui::Subject;
use archforge_shell_api::panels::{
    FIELD, PAD, ROW, check_cx, control_cx, edit_button_cx, icon_button_cx, menu_cx, name_field_cx, refusal, reveal_cx, run_cx, section_cx,
    select_element_cx, select_field_cx, selected_id_cx, string_of, text_field_cx,
};

/// The longest type name a column takes, as ER's rules allow.
const MAX_TYPE: usize = archforge_er::rules::MAX_TYPE;

/// The column editor as typed: the column it edits (or one to add), its fields and switches, and
/// the refusal its card shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnEdit {
    /// The column, or none for one being added to `entity`.
    pub target: Option<Uuid>,
    pub entity: Uuid,
    pub name: String,
    pub ty: String,
    /// The size fields as typed: a length, or a precision and a scale, empty for none.
    pub length: String,
    pub precision: String,
    pub scale: String,
    pub default: String,
    pub generation: archforge_er::Generation,
    pub expression: String,
    pub collation: String,
    pub primary_key: bool,
    pub not_null: bool,
    pub unique: bool,
    /// Whether More (the collation) is open.
    pub more: bool,
    /// The column this one follows, as "Table.column" (slice S4), until unlinked.
    pub linked: Option<String>,
    /// Whether Unlink was clicked; the unlink goes out with Apply.
    pub unlink: bool,
    /// The domain or enum type the column takes instead of a type name (slice S5).
    pub domain: Option<Uuid>,
    pub enum_type: Option<Uuid>,
    pub refused: Option<String>,
    pub opened: bool,
}

/// The column `a` follows, as "Table.column".
fn linked_label(store: &Store, a: Obj) -> Option<String> {
    let t = archforge_er::rules::linked_to(store, a)?;
    let table = archforge_er::rules::entity_of(store, t).map(|e| archforge_core::name_of(store, e)).unwrap_or_default();
    Some(format!("{table}.{}", archforge_core::name_of(store, t)))
}

/// The base types the column type menu offers after the schema's own types: the names the size
/// rules know, and the common plain ones.
pub const BASE_TYPES: [&str; 12] =
    ["VARCHAR", "CHAR", "TEXT", "SMALLINT", "INTEGER", "BIGINT", "NUMERIC", "BOOLEAN", "DATE", "TIMESTAMP", "TIME", "UUID"];

/// One column as the panel lists it.
struct ColumnRow {
    id: Uuid,
    name: String,
    ty: String,
    primary: bool,
    unique: bool,
    foreign: bool,
    /// The column this one follows, as "Table.column".
    linked: Option<String>,
}

/// The columns of the table `e` as the panel lists them, and whether it is a mapping table.
fn entity_rows(store: &Store, e: Obj) -> (Vec<ColumnRow>, bool) {
    let columns = children(store, e, archforge_er::entity::ATTRIBUTE)
        .into_iter()
        .filter_map(|a| {
            Some(ColumnRow {
                id: store.uuid(a).ok()?,
                name: archforge_core::name_of(store, a),
                ty: archforge_er::rules::type_display(store, a),
                primary: archforge_er::rules::in_primary_key(store, a),
                unique: archforge_er::rules::is_unique(store, a),
                foreign: store.referrers(a).iter().any(|(_, p)| *p == archforge_er::foreign_key::COLUMN),
                linked: linked_label(store, a),
            })
        })
        .collect();
    let associative = store.get(e, archforge_er::entity::KIND).ok().and_then(|v| match v {
        Value::Enum(i) => archforge_er::EntityKind::from_index(i),
        _ => None,
    }) == Some(archforge_er::EntityKind::Associative);
    (columns, associative)
}

/// A table: name and kind, its columns with the editor, its keys.
pub(crate) fn entity_body(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (Some(entity), Some(entity_id)) = (subject.obj, subject.id) else { return };
    let (columns, associative) = match cx.session.as_ref() {
        Some(s) => entity_rows(s.doc(), entity),
        None => return,
    };
    let half = ((width - 8.0) / 2.0).max(0.0);
    name_field_cx(cx, ui, p, pos2(r.left() + PAD, y), half, subject, "er.entity.rename", "entity");
    let kind_word = if associative { tr("Mapping table") } else { tr("Regular") };
    let kind_field =
        select_field_cx(cx, ui, p, pos2(r.left() + PAD + half + 8.0, y), half, "workspace.properties.kind", &tr("Kind"), &kind_word, menus::KIND);
    y += 17.0 + FIELD + 16.0;
    // A pure mapping table offers to collapse back into its line (slice S4).
    let collapsible = cx.session.as_ref().is_some_and(|s| archforge_er::rules::collapse_problem(s.doc(), entity).is_none());
    if collapsible {
        y = collapse_offer(cx, ui, p, r, y, entity, entity_id, &subject.name);
    }
    // Columns.
    let title = fmt(&tr("Columns · {n}"), &[("n", &columns.len().to_string())]);
    let Some(next) = section_cx(cx, ui, p, r, y, &title, Some(("workspace.properties.columns.add", "Add column"))) else {
        open_column(cx, entity_id, None);
        return;
    };
    y = next - 6.0;
    let chosen = selected_id_cx(&cx.cx_ref());
    for col in &columns {
        if er(cx.notation_states).and_then(|er| er.column.as_ref()).is_some_and(|c| c.target == Some(col.id)) {
            y = column_editor(cx, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), ROW));
        let resp = control_cx(cx, ui, &format!("workspace.properties.column.{}", col.id), "row", row, true);
        if Some(col.id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::icon_mid(&paint, pos2(row.left() + 8.0, row.center().y), &icons::GRIP, p.text_muted);
        let (badge, tint, ink) = if col.primary {
            (Some("PK"), p.amber_tint, p.amber)
        } else if col.foreign {
            (Some("FK"), p.amber_tint, p.amber)
        } else if col.unique {
            (Some("UQ"), p.accent_tint, p.accent)
        } else {
            (None, p.surface, p.text)
        };
        // Badge, name, link mark and type share the row's middle line.
        let mid = row.center().y;
        if let Some(b) = badge {
            let br = Rect::from_center_size(pos2(row.left() + 37.0, mid), vec2(26.0, 16.0));
            paint.rect_filled(br, 4.0, tint);
            let g = ux::galley(&paint, b, types::TINY, ink, None);
            paint.galley(br.center() - g.size() / 2.0, g, ink.into());
        }
        let name = ux::text_mid(&paint, row.left() + 58.0, mid, &col.name, types::UI, p.text, Some((width - 58.0 - 40.0).max(0.0)));
        let mut after = name.right() + 8.0;
        if let Some(target) = &col.linked {
            // The link mark of a key column that follows the column it points at.
            let mark = Rect::from_min_size(pos2(after, mid - 7.0), vec2(14.0, 14.0));
            ux::icon_in(&paint, mark, &icons::LINK, p.text_secondary);
            control_cx(cx, ui, &format!("workspace.properties.column.{}.link", col.id), "mark", mark.expand(3.0), true)
                .on_hover_text(fmt(&tr("Follows {target}"), &[("target", target)]));
            after += 20.0;
        }
        ux::text_mid(&paint, after, mid + 0.5, &col.ty, types::CAPTION, p.text_secondary, Some((row.right() - after - 32.0).max(0.0)));
        let id = format!("workspace.properties.column.{}", col.id);
        if edit_button_cx(cx, ui, p, row, &id, resp.contains_pointer()) || resp.double_clicked() {
            select_element_cx(cx, col.id);
            open_column(cx, entity_id, Some(col.id));
        } else if resp.clicked() {
            select_element_cx(cx, col.id);
        }
        y += ROW;
    }
    if er(cx.notation_states).and_then(|er| er.column.as_ref()).is_some_and(|c| c.entity == entity_id && c.target.is_none()) {
        y = column_editor(cx, ui, p, r, y);
    }
    y += 10.0;
    // Keys, checks and indexes, then the problems the rules find on the table.
    y = crate::er_keys::keys_section(cx, ui, p, r, y, entity, entity_id);
    y = crate::er_checks::checks_section(cx, ui, p, r, y + 4.0, entity, entity_id);
    y = crate::er_indexes::indexes_section(cx, ui, p, r, y + 4.0, entity, entity_id);
    y = crate::er_subtypes::subtypes_section(cx, ui, p, r, y + 4.0, entity, entity_id, entity_id);
    archforge_shell_api::panels::problems_section_cx(cx, ui, p, r, y + 4.0, entity);
    // The kind menu last, over the rows.
    if cx.panels.menu == Some(menus::KIND) {
        let items = [tr("Regular"), tr("Mapping table")];
        if let Some(i) = menu_cx(cx, ui, p, kind_field, "workspace.properties.kind", &items) {
            let kind = if i == 1 { "associative" } else { "regular" };
            match run_cx(cx, "er.entity.set-kind", json!({ "entity": entity_id.to_string(), "kind": kind })) {
                Ok(_) => cx.panels.notice = None,
                Err(e) => cx.panels.notice = Some(e),
            }
        }
    }
}

/// The column editor, while it is open, in what the notations keep between frames `states`: ER's
/// own ([`crate::er_state::ErState::column`]), for the app's journeys, which fill it in as a person
/// types.
#[cfg(feature = "snapshot")]
pub fn column_edit(states: &archforge_shell_api::notation_ui::NotationStates) -> Option<&ColumnEdit> {
    er(states).and_then(|er| er.column.as_ref())
}

/// The column editor, while it is open, to fill in ([`column_edit`]).
#[cfg(feature = "snapshot")]
pub fn column_edit_mut(states: &mut archforge_shell_api::notation_ui::NotationStates) -> Option<&mut ColumnEdit> {
    er_mut(states).and_then(|er| er.column.as_mut())
}

/// Opens the column editor on `target`, or on a column to add to `entity`.
pub fn open_column(cx: &mut Ctx, entity: Uuid, target: Option<Uuid>) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let edit = match target.and_then(|t| store.by_uuid(t)) {
        Some(a) => {
            let (length, precision, scale) = archforge_er::rules::own_size_of(store, a);
            ColumnEdit {
                target,
                entity,
                name: archforge_core::name_of(store, a),
                ty: if archforge_er::rules::domain_of(store, a).is_some() || archforge_er::rules::enum_of(store, a).is_some() {
                    archforge_er::rules::type_display(store, a)
                } else {
                    string_of(store, a, archforge_er::attribute::TYPE)
                },
                length: number(length),
                precision: number(precision),
                scale: number(scale),
                default: string_of(store, a, archforge_er::attribute::DEFAULT),
                generation: archforge_er::rules::generation_of(store, a),
                expression: string_of(store, a, archforge_er::attribute::EXPRESSION),
                collation: string_of(store, a, archforge_er::attribute::COLLATION),
                primary_key: archforge_er::rules::in_primary_key(store, a),
                not_null: !archforge_er::rules::is_nullable(store, a),
                unique: archforge_er::rules::is_unique(store, a),
                more: false,
                linked: linked_label(store, a),
                unlink: false,
                domain: archforge_er::rules::domain_of(store, a).and_then(|d| store.uuid(d).ok()),
                enum_type: archforge_er::rules::enum_of(store, a).and_then(|e| store.uuid(e).ok()),
                refused: None,
                opened: true,
            }
        }
        None => {
            let taken = store
                .by_uuid(entity)
                .map(|e| {
                    children(store, e, archforge_er::entity::ATTRIBUTE).into_iter().map(|a| archforge_core::name_of(store, a)).collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let name = archforge_shell_api::explorer::fresh_name("column", taken.into_iter(), true).unwrap_or_default();
            ColumnEdit {
                target: None,
                entity,
                name,
                ty: "TEXT".into(),
                length: String::new(),
                precision: String::new(),
                scale: String::new(),
                default: String::new(),
                generation: archforge_er::Generation::NoneKind,
                expression: String::new(),
                collation: String::new(),
                primary_key: false,
                not_null: false,
                unique: false,
                more: false,
                linked: None,
                unlink: false,
                domain: None,
                enum_type: None,
                refused: None,
                opened: true,
            }
        }
    };
    if let Some(er) = er_mut(cx.notation_states) {
        er.column = Some(edit);
    }
    cx.panels.menu = None;
}

/// The generation choices, in the engine's order, with their words for the select.
const GENERATIONS: [(archforge_er::Generation, &str); 5] = [
    (archforge_er::Generation::NoneKind, "None"),
    (archforge_er::Generation::IdentityAlways, "Identity, always"),
    (archforge_er::Generation::IdentityByDefault, "Identity, by default"),
    (archforge_er::Generation::ComputedStored, "Computed, stored"),
    (archforge_er::Generation::ComputedVirtual, "Computed, virtual"),
];

/// Whether the generation `g` computes the column from an expression, stored or virtual.
fn is_computed(g: archforge_er::Generation) -> bool {
    matches!(g, archforge_er::Generation::ComputedStored | archforge_er::Generation::ComputedVirtual)
}

/// The column editor card, as the design's (slice S2 adds the size, default, generation and,
/// under More, the collation); gives the y under it.
/// The offer on a pure mapping table to become its many-to-many line again (the design's card
/// "Mapping table that can collapse"); gives the y under it.
fn collapse_offer(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, entity: Obj, entity_id: Uuid, name: &str) -> f32 {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (from, to) = match cx.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let mut parents = archforge_er::rules::foreign_keys(store, entity).into_iter().filter_map(|fk| {
                let k = archforge_er::rules::key_parts(store, fk).key?;
                Some(archforge_core::name_of(store, archforge_er::rules::entity_of(store, k)?))
            });
            (parents.next().unwrap_or_default(), parents.next().unwrap_or_default())
        }
        None => return y,
    };
    let y = section_cx(cx, ui, p, r, y, &tr("Mapping table"), None).unwrap_or(y + 50.0) - 6.0;
    let message = fmt(
        &tr("{name} holds nothing but its two keys, so it can become the many-to-many line between {from} and {to} again."),
        &[("name", name), ("from", &from), ("to", &to)],
    );
    let inner = (width - 24.0).max(0.0);
    let g = ux::galley_rows(&paint, &message, types::SMALL, p.text, Some(inner), 4);
    let text_h = g.size().y;
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 12.0 + text_h + 12.0 + 24.0 + 12.0));
    paint.rect_filled(card, 8.0, p.accent_tint);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.galley(card.min + vec2(12.0, 12.0), g, p.text.into());
    let label = tr("Collapse into a relationship");
    let g = ux::galley(&paint, &label, types::UI, p.text, None);
    let button = Rect::from_min_size(card.min + vec2(12.0, 12.0 + text_h + 12.0), vec2(6.0 + 14.0 + 7.0 + g.size().x + 8.0, 24.0));
    ux::icon_in(&paint, Rect::from_min_size(button.min + vec2(6.0, 5.0), Vec2::splat(14.0)), &icons::MERGE_LINE, p.text);
    paint.galley(button.min + vec2(27.0, 2.5), g, p.text.into());
    let resp = control_cx(cx, ui, "workspace.properties.collapse", "button", button, true);
    if resp.hovered() {
        paint.rect_stroke(button, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    if resp.clicked() {
        // The command selects the relationship it makes.
        match run_cx(cx, "er.relationship.collapse", json!({ "entity": entity_id.to_string() })) {
            Ok(_) => cx.panels.notice = None,
            Err(e) => cx.panels.notice = Some(e),
        }
    }
    card.bottom() + 10.0
}

/// Draws the open column editor's card from `y` and acts on what is done in it; gives the y under
/// it, or `y` while no column editor is open.
fn column_editor(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let Some(mut edit) = er(cx.notation_states).and_then(|er| er.column.clone()) else { return y };
    let typed = edit.domain.is_some() || edit.enum_type.is_some();
    let shape = if typed { archforge_er::rules::TypeShape::Plain } else { archforge_er::rules::type_shape(&edit.ty) };
    let computed = is_computed(edit.generation);
    // The card's height from its rows.
    let row = 17.0 + FIELD + 10.0;
    let on_domain = edit.domain.is_some();
    let mut height = 12.0 + row * 4.0 + 20.0 + 8.0 + 20.0 + 14.0 + 24.0 + 10.0 + FIELD + 12.0;
    if on_domain {
        // The domain brings the default and the collation: neither row shows.
        height -= row + 24.0 + 10.0;
    }
    if computed {
        height += row;
    }
    if edit.more && !on_domain {
        height += row;
    }
    if edit.linked.is_some() {
        height += 28.0;
    }
    if edit.refused.is_some() {
        height += 20.0;
    }
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    reveal_cx(cx, ui, crate::er_state::COLUMN, card);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = std::mem::take(&mut edit.opened);
    let mut cy = 12.0;
    let mut enter = text_field_cx(
        cx,
        ui,
        p,
        at(0.0, cy),
        inner,
        "workspace.properties.column.name",
        &tr("Name"),
        &mut edit.name,
        archforge_er::rules::MAX_NAME,
        take,
        false,
    );
    cy += row;
    if let Some(target) = edit.linked.clone() {
        // What the column follows, with the way out of it.
        ux::icon_in(&paint, Rect::from_min_size(at(0.0, cy + 3.0), Vec2::splat(14.0)), &icons::LINK, p.text_secondary);
        ux::text(
            &paint,
            at(20.0, cy),
            &fmt(&tr("Follows {target}"), &[("target", &target)]),
            types::CAPTION,
            p.text_secondary,
            Some((inner - 20.0 - 32.0).max(0.0)),
        );
        let unlink = Rect::from_min_size(pos2(card.right() - 12.0 - 24.0, card.top() + cy - 4.0), Vec2::splat(24.0));
        ux::icon_in(&paint, Rect::from_center_size(unlink.center(), Vec2::splat(12.0)), &icons::CLOSE, p.text_secondary);
        if control_cx(cx, ui, "workspace.properties.column.unlink", "button", unlink, true).on_hover_text(tr("Unlink")).clicked() {
            // Pending until Apply, so the card stays one step and Cancel keeps the link.
            edit.linked = None;
            edit.unlink = true;
        }
        cy += 28.0;
    }
    // The type, with the size fields its shape takes beside it.
    let (type_w, size_w) = match shape {
        archforge_er::rules::TypeShape::Plain => (inner, 0.0),
        archforge_er::rules::TypeShape::PrecisionScale => ((inner - 2.0 * 64.0 - 16.0).max(60.0), 64.0),
        _ => ((inner - 80.0 - 8.0).max(60.0), 80.0),
    };
    let typed_name = edit.ty.clone();
    enter |= text_field_cx(cx, ui, p, at(0.0, cy), type_w, "workspace.properties.column.type", &tr("Type"), &mut edit.ty, MAX_TYPE, false, false);
    if typed && edit.ty != typed_name {
        // Typing a name over a domain or enum type leaves it.
        edit.domain = None;
        edit.enum_type = None;
    }
    // The chevron inside the type field opens the schema's types and the known base types.
    let type_field = Rect::from_min_size(at(0.0, cy + 17.0), vec2(type_w, FIELD));
    let chevron = Rect::from_min_size(pos2(type_field.right() - 24.0, type_field.top() + 2.0), Vec2::splat(24.0));
    ux::icon(&paint, pos2(chevron.left() + 4.0, chevron.top() + 10.0), &icons::CHEVRON_DOWN, 16.0, p.text_secondary);
    if control_cx(cx, ui, "workspace.properties.column.type.menu", "button", chevron, true).on_hover_text(tr("Pick a type")).clicked() {
        cx.panels.menu = if cx.panels.menu == Some(menus::COLUMN_TYPE) { None } else { Some(menus::COLUMN_TYPE) };
    }
    match shape {
        archforge_er::rules::TypeShape::Length => {
            enter |= text_field_cx(
                cx,
                ui,
                p,
                at(type_w + 8.0, cy),
                size_w,
                "workspace.properties.column.length",
                &tr("Length"),
                &mut edit.length,
                5,
                false,
                false,
            );
        }
        archforge_er::rules::TypeShape::Precision => {
            enter |= text_field_cx(
                cx,
                ui,
                p,
                at(type_w + 8.0, cy),
                size_w,
                "workspace.properties.column.precision",
                &tr("Precision"),
                &mut edit.precision,
                5,
                false,
                false,
            );
        }
        archforge_er::rules::TypeShape::PrecisionScale => {
            enter |= text_field_cx(
                cx,
                ui,
                p,
                at(type_w + 8.0, cy),
                size_w,
                "workspace.properties.column.precision",
                &tr("Precision"),
                &mut edit.precision,
                5,
                false,
                false,
            );
            enter |= text_field_cx(
                cx,
                ui,
                p,
                at(type_w + 8.0 + size_w + 8.0, cy),
                size_w,
                "workspace.properties.column.scale",
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
    if !on_domain {
        enter |= text_field_cx(
            cx,
            ui,
            p,
            at(0.0, cy),
            inner,
            "workspace.properties.column.default",
            &tr("Default"),
            &mut edit.default,
            archforge_er::rules::MAX_TEXT,
            false,
            false,
        );
        cy += row;
    }
    let generation_word = GENERATIONS.iter().find(|(g, _)| *g == edit.generation).map(|(_, w)| tr(w)).unwrap_or_default();
    let generation_field = select_field_cx(
        cx,
        ui,
        p,
        at(0.0, cy),
        inner,
        "workspace.properties.column.generation",
        &tr("Generation"),
        &generation_word,
        menus::GENERATION,
    );
    cy += row;
    if computed {
        enter |= text_field_cx(
            cx,
            ui,
            p,
            at(0.0, cy),
            inner,
            "workspace.properties.column.expression",
            &tr("Expression"),
            &mut edit.expression,
            archforge_er::rules::MAX_TEXT,
            false,
            false,
        );
        cy += row;
    }
    // The checks in a row: the second starts where the first's label ended.
    // A key column is never null: ticking Primary key ticks Not null, and unticking Not null
    // unticks Primary key.
    let was_key = edit.primary_key;
    let next_x = check_cx(cx, ui, p, at(0.0, cy), "workspace.properties.column.primary-key", &tr("Primary key"), &mut edit.primary_key);
    if edit.primary_key && !was_key {
        edit.not_null = true;
    }
    let was_not_null = edit.not_null;
    check_cx(cx, ui, p, pos2(next_x - 12.0, card.top() + cy), "workspace.properties.column.not-null", &tr("Not null"), &mut edit.not_null);
    if was_not_null && !edit.not_null {
        edit.primary_key = false;
    }
    cy += 28.0;
    check_cx(cx, ui, p, at(0.0, cy), "workspace.properties.column.unique", &tr("Unique"), &mut edit.unique);
    cy += 20.0 + 14.0;
    // More: the collation; a domain brings its own, so a column on one shows neither.
    if !on_domain {
        let more = Rect::from_min_size(at(0.0, cy), vec2(inner, 24.0));
        ux::icon_in(
            &paint,
            Rect::from_min_size(more.min + vec2(0.0, 4.0), Vec2::splat(16.0)),
            if edit.more { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT },
            p.text_secondary,
        );
        ux::text(&paint, more.min + vec2(22.0, 2.0), &tr("More"), types::UI, p.text, Some((inner - 22.0).max(0.0)));
        if control_cx(cx, ui, "workspace.properties.column.more", "disclosure", more, true).clicked() {
            edit.more = !edit.more;
        }
        cy += 24.0 + 10.0;
        if edit.more {
            enter |= text_field_cx(
                cx,
                ui,
                p,
                at(0.0, cy),
                inner,
                "workspace.properties.column.collation",
                &tr("Collation"),
                &mut edit.collation,
                archforge_er::rules::MAX_NAME,
                false,
                false,
            );
            cy += row;
        }
    }
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button_cx(cx, ui, p, at(0.0, cy), "workspace.properties.column.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button_cx(
        cx,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.column.cancel",
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
        "workspace.properties.column.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, cy + FIELD + 8.0), why, types::CAPTION, p.amber, Some(inner));
    }
    // The type menu, over the card: the schema's domains and enum types, then the base types.
    if cx.panels.menu == Some(menus::COLUMN_TYPE) {
        let types: Vec<(Option<(bool, Uuid)>, String)> = cx
            .session
            .as_ref()
            .and_then(|s| {
                let store = s.doc();
                let e = store.by_uuid(edit.entity)?;
                let schema = archforge_er::rules::schema_of(store, e)?;
                let mut out: Vec<(Option<(bool, Uuid)>, String)> = Vec::new();
                for d in archforge_er::rules::domains_of(store, schema) {
                    if let Ok(id) = store.uuid(d) {
                        out.push((Some((true, id)), format!("{} · {}", archforge_core::name_of(store, d), tr("domain"))));
                    }
                }
                for en in archforge_er::rules::enums_of(store, schema) {
                    if let Ok(id) = store.uuid(en) {
                        out.push((Some((false, id)), format!("{} · {}", archforge_core::name_of(store, en), tr("enum"))));
                    }
                }
                Some(out)
            })
            .unwrap_or_default();
        let mut items: Vec<(Option<(bool, Uuid)>, String)> = types;
        items.extend(BASE_TYPES.iter().map(|t| (None, (*t).to_string())));
        let labels: Vec<String> = items.iter().map(|(_, l)| l.clone()).collect();
        if let Some(i) = menu_cx(cx, ui, p, type_field, "workspace.properties.column.type.menu", &labels)
            && let Some((picked, label)) = items.get(i)
        {
            match picked {
                Some((true, id)) => {
                    edit.domain = Some(*id);
                    edit.enum_type = None;
                    edit.ty = label.split(" · ").next().unwrap_or_default().to_string();
                    // The domain brings the default and the collation.
                    edit.default.clear();
                    edit.collation.clear();
                    edit.more = false;
                }
                Some((false, id)) => {
                    edit.enum_type = Some(*id);
                    edit.domain = None;
                    edit.ty = label.split(" · ").next().unwrap_or_default().to_string();
                }
                None => {
                    edit.domain = None;
                    edit.enum_type = None;
                    edit.ty = label.clone();
                }
            }
            edit.length.clear();
            edit.precision.clear();
            edit.scale.clear();
        }
    }
    // The generation menu, over the card.
    if cx.panels.menu == Some(menus::GENERATION) {
        let items: Vec<String> = GENERATIONS.iter().map(|(_, w)| tr(w)).collect();
        if let Some(i) = menu_cx(cx, ui, p, generation_field, "workspace.properties.column.generation", &items)
            && let Some((g, _)) = GENERATIONS.get(i)
        {
            edit.generation = *g;
        }
    }
    if let Some(er) = er_mut(cx.notation_states) {
        er.column = Some(edit.clone());
    }
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if let Some(er) = er_mut(cx.notation_states) {
            er.column = None;
        }
    } else if remove {
        if let Some(t) = edit.target {
            // What goes with the column, named for the toast.
            let went: Vec<String> = cx
                .session
                .as_ref()
                .and_then(|s| {
                    let store = s.doc();
                    let a = store.by_uuid(t)?;
                    let (keys, fks) = archforge_er::commands::dependents(store, a).ok()?;
                    Some(keys.iter().chain(fks.iter()).filter(|k| **k != a).map(|k| archforge_er::rules::label(store, *k)).collect())
                })
                .unwrap_or_default();
            match run_cx(cx, "er.attribute.remove", json!({ "attribute": t.to_string() })) {
                Ok(_) => {
                    if let Some(er) = er_mut(cx.notation_states) {
                        er.column = None;
                    }
                    let text = if went.is_empty() {
                        fmt(&tr("{name} removed"), &[("name", &edit.name)])
                    } else {
                        fmt(&tr("{name} removed · {keys} went with it"), &[("name", &edit.name), ("keys", &went.join(", "))])
                    };
                    let undo_label = cx.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
                    cx.canvas.toast = Some(archforge_shell_api::canvas::Toast { text, until: cx.frames.saturating_add(300), undo_label });
                }
                Err(e) => set_column_refusal(cx, e),
            }
        }
    } else if apply || enter {
        commit_column(cx, &edit);
    }
    card.bottom() + 8.0
}

/// Shows `why` on the open column editor's card, which stays open with what was typed.
fn set_column_refusal(cx: &mut Ctx, why: String) {
    if let Some(c) = er_mut(cx.notation_states).and_then(|er| er.column.as_mut()) {
        c.refused = Some(why);
    }
}

/// A size field as typed: empty is none, else a number.
pub(crate) fn number(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

/// A size field's value as typed, spaces trimmed: empty is 0, and text that is no number is
/// refused in the user's words.
pub(crate) fn size_value(text: &str) -> Result<i64, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>().map_err(|_| fmt(&tr("{text} is not a number"), &[("text", t)]))
}

/// Writes the column editor through the commands: add, or rename, type with its size, default,
/// generation, collation and flags, each only when it changed. Every step keeps the column's
/// generation rules, so an edit that changes the generation gives the old one up first (as None)
/// and takes the new one last, after the type, default and flags it needs; the expression goes
/// only with a computed generation, the field being hidden otherwise.
fn commit_column(cx: &mut Ctx, edit: &ColumnEdit) {
    let nullable = if edit.not_null { "no" } else { "yes" };
    let size = match (size_value(&edit.length), size_value(&edit.precision), size_value(&edit.scale)) {
        (Ok(l), Ok(p), Ok(s)) => (l, p, s),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            set_column_refusal(cx, e);
            return;
        }
    };
    let generation = archforge_er::commands::GENERATION_WORDS.iter().find(|(g, _)| *g == edit.generation).map(|(_, w)| *w).unwrap_or("none");
    let expression = if is_computed(edit.generation) { edit.expression.trim() } else { "" };
    let outcome = match edit.target {
        None => crate::er_place::run_growing(
            cx,
            &tr("Add column"),
            "er.attribute.add",
            json!({
                "entity": edit.entity.to_string(), "name": edit.name.trim(),
                "type": if edit.domain.is_some() || edit.enum_type.is_some() { "" } else { edit.ty.trim() },
                "domain": edit.domain.map(|d| d.to_string()).unwrap_or_default(),
                "enum": edit.enum_type.map(|e| e.to_string()).unwrap_or_default(),
                "length": size.0, "precision": size.1, "scale": size.2,
                "default": if edit.domain.is_some() { "" } else { edit.default.trim() },
                "generation": generation, "expression": expression,
                "collation": if edit.domain.is_some() { "" } else { edit.collation.trim() },
                "primary_key": edit.primary_key, "nullable": nullable, "unique": edit.unique,
            }),
            edit.entity,
        )
        .map(|_| ()),
        Some(t) => {
            // Every changed field in one batch: a refusal rolls the whole card back, so what the
            // card shows and what landed never drift apart.
            let Some(session) = cx.session.as_mut() else { return };
            let store = session.doc();
            let Some(a) = store.by_uuid(t) else { return };
            let old_name = archforge_core::name_of(store, a);
            let old_type = string_of(store, a, archforge_er::attribute::TYPE);
            let old_domain = archforge_er::rules::domain_of(store, a).and_then(|d| store.uuid(d).ok());
            let old_enum = archforge_er::rules::enum_of(store, a).and_then(|e| store.uuid(e).ok());
            let (ol, op, os) = archforge_er::rules::own_size_of(store, a);
            let old_size = (ol.unwrap_or(0), op.unwrap_or(0), os.unwrap_or(0));
            let old_default = string_of(store, a, archforge_er::attribute::DEFAULT);
            let old_generation = archforge_er::rules::generation_of(store, a);
            let old_expression = string_of(store, a, archforge_er::attribute::EXPRESSION);
            let old_collation = string_of(store, a, archforge_er::attribute::COLLATION);
            let id = t.to_string();
            let mut steps: Vec<(&str, serde_json::Value)> = Vec::new();
            if edit.unlink {
                steps.push(("er.attribute.unlink", json!({ "attribute": id })));
            }
            if edit.name.trim() != old_name {
                steps.push(("er.attribute.rename", json!({ "attribute": id, "name": edit.name.trim() })));
            }
            // A generation left is given up before anything else changes: as None the column takes
            // any type, default and nullability, and the new generation is checked last.
            let leaves = old_generation != archforge_er::Generation::NoneKind && edit.generation != old_generation;
            if leaves {
                steps.push(("er.attribute.set-generation", json!({ "attribute": id, "generation": "none", "expression": "" })));
            }
            let typed = edit.domain.is_some() || edit.enum_type.is_some();
            if (!typed && (edit.ty.trim() != old_type || size != old_size)) || edit.domain != old_domain || edit.enum_type != old_enum {
                steps.push((
                    "er.attribute.set-type",
                    json!({
                        "attribute": id,
                        "type": if typed { "" } else { edit.ty.trim() },
                        "domain": edit.domain.map(|d| d.to_string()).unwrap_or_default(),
                        "enum": edit.enum_type.map(|e| e.to_string()).unwrap_or_default(),
                        "length": size.0, "precision": size.1, "scale": size.2,
                    }),
                ));
            }
            if edit.domain.is_none() && edit.default.trim() != old_default {
                steps.push(("er.attribute.set-default", json!({ "attribute": id, "default": edit.default.trim() })));
            }
            if edit.domain.is_none() && edit.collation.trim() != old_collation {
                steps.push(("er.attribute.set-collation", json!({ "attribute": id, "collation": edit.collation.trim() })));
            }
            steps.push((
                "er.attribute.set-flags",
                json!({ "attribute": id, "primary_key": edit.primary_key, "nullable": nullable, "unique": edit.unique }),
            ));
            // The new generation last, once the type, default and flags it needs are in place; a
            // generation given up for None is already where it ends.
            let taken = if leaves {
                edit.generation != archforge_er::Generation::NoneKind
            } else {
                edit.generation != old_generation || expression != old_expression
            };
            if taken {
                steps.push(("er.attribute.set-generation", json!({ "attribute": id, "generation": generation, "expression": expression })));
            }
            if steps.len() == 1 {
                session.execute(steps[0].0, &steps[0].1).map(|_| ()).map_err(|e| refusal(cx.notation_uis, &e))
            } else {
                session
                    .batch(&tr("Edit column"), |b| {
                        for (cmd, params) in &steps {
                            b.execute(cmd, params)?;
                        }
                        Ok(())
                    })
                    .map(|_| ())
                    .map_err(|e| refusal(cx.notation_uis, &e))
            }
        }
    };
    match outcome {
        Ok(()) => {
            if let Some(er) = er_mut(cx.notation_states) {
                er.column = None;
            }
        }
        Err(e) => set_column_refusal(cx, e),
    }
}

/// The words of the table's panel that reach `tr` through a variable: the hover hints of its icon
/// buttons and the generations' names, for the catalogue tests.
#[cfg(feature = "snapshot")]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add column", "Remove", "Cancel", "Apply", "Unlink", "Pick a type"].into_iter().chain(GENERATIONS.iter().map(|(_, w)| *w))
}
