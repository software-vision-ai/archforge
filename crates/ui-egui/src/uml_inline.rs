//! A class's attribute or operation typed in place on its card (AR-62): a row in its compartment,
//! ringed in the accent while it is a draft, holding the name and the type (an attribute's type,
//! an operation's return type) with the type menu beside it. Tab moves between the two; Enter
//! writes the member as one undo step through the same command as Properties' editor, a new one
//! growing its cards to show it (AR-57), and opens the next draft in the same compartment;
//! Escape drops the draft; a click away writes a draft that has a name and drops an empty one,
//! as the multiplicity field writes on leaving it. A refusal stays under the row with the text
//! as typed, the model untouched. The row's ⋯ takes it to Properties' editor for the rest
//! (visibility, multiplicity, static, abstract, parameters). A selected class shows a + at the
//! right of each compartment; ⌘↩ adds an attribute, ⇧⌘↩ an operation. Below the zoom at which a
//! card shows its rows, or without the class's card on the open diagram, Properties' editor opens
//! instead.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::canvas::{DETAIL_ZOOM, ENTITY_ROW, View};
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::uml_panel::{MemberEdit, MemberKind};
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// Which field of the row has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineField {
    Name,
    Type,
}

/// A member being typed on its class's card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineMember {
    /// The member as Properties' editor holds it: what the row does not show is kept as it is.
    pub edit: MemberEdit,
    /// The type as typed: a type's name, a built-in one or the model's own, or empty for none.
    pub ty: String,
    pub field: InlineField,
    /// The field takes the keyboard on the next frame.
    pub focus: bool,
    pub refused: Option<String>,
}

const NAME_ID: &str = "workspace.canvas.member.name";
const TYPE_ID: &str = "workspace.canvas.member.type";
/// The row's height over a row's pitch, and its details button and type chevron, in units.
const ROW_HEIGHT: f32 = ENTITY_ROW + 4.0;
const BUTTON: f32 = 18.0;

fn compartment(kind: MemberKind) -> usize {
    match kind {
        MemberKind::Attribute => 0,
        MemberKind::Operation => 1,
    }
}

/// The node of class `class` on the open diagram, when it has one.
fn card_node(shell: &Shell, store: &Store, class: Obj) -> Option<Obj> {
    let d = store.by_uuid(shell.canvas.active?)?;
    archforge_diagram::views_of(store, class).2.into_iter().find(|n| archforge_diagram::diagram_of(store, *n) == Some(d))
}

/// A type as the row shows it: its name, or nothing for none.
fn type_text(store: &Store, ty: &str) -> String {
    if ty.is_empty() { String::new() } else { crate::uml_panel::type_label(store, ty) }
}

/// The row's type as the commands take it. The type it holds (the member's, or the one picked
/// from the menu) is kept while the text still shows its name, so two types of one name in
/// different packages stay apart; text typed over it names a type: none for empty, a built-in
/// type or one of the model's; a name no type has, or that several have, is refused, naming it.
fn resolve(store: &Store, inline: &InlineMember) -> Result<String, String> {
    let text = inline.ty.trim();
    if text == type_text(store, &inline.edit.ty) {
        return Ok(inline.edit.ty.clone());
    }
    if text.is_empty() {
        return Ok(String::new());
    }
    match crate::uml_panel::types_named(store, text).as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(fmt(&tr("There is no type named {name}."), &[("name", text)])),
        _ => Err(fmt(&tr("Several types are named {name}; pick one from the menu."), &[("name", text)])),
    }
}

/// Opens a row on `class`'s card on the open diagram: a new member of `kind`, or `target`. Gives
/// false when the card cannot hold one (no card of the class on the diagram, or the diagram drawn
/// below the zoom at which cards show their rows); the caller opens Properties' editor then.
pub(crate) fn open(shell: &mut Shell, class: Uuid, kind: MemberKind, target: Option<Uuid>) -> bool {
    if shell.canvas.zoom < DETAIL_ZOOM {
        return false;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return false };
    let Some(c) = store.by_uuid(class) else { return false };
    if card_node(shell, store, c).is_none() {
        return false;
    }
    let edit = crate::uml_panel::edit_of(Some(store), class, kind, target);
    let ty = type_text(store, &edit.ty);
    shell.canvas.inline = Some(InlineMember { edit, ty, field: InlineField::Name, focus: true, refused: None });
    // One draft at a time: Properties' editor closes.
    shell.panels.member = None;
    shell.panels.menu = None;
    if let Err(e) = crate::editing::run(shell, "view.select", json!({ "ids": [class.to_string()] })) {
        shell.canvas.notice = Some(e);
    }
    true
}

/// The row typed on a card, as the canvas draws it: the class, its compartment, and the row
/// counted across the compartments (none for a new one).
pub(crate) fn draft(shell: &Shell, store: &Store) -> Option<(Obj, usize, Option<usize>)> {
    let inline = shell.canvas.inline.as_ref()?;
    let c = store.by_uuid(inline.edit.class)?;
    let row = match inline.edit.target {
        None => None,
        Some(t) => {
            let o = store.by_uuid(t)?;
            let attributes = archforge_uml::members::attributes(store, c);
            match inline.edit.kind {
                MemberKind::Attribute => Some(attributes.iter().position(|a| *a == o)?),
                MemberKind::Operation => Some(attributes.len() + archforge_uml::members::operations(store, c).iter().position(|op| *op == o)?),
            }
        }
    };
    Some((c, compartment(inline.edit.kind), row))
}

/// Drops the row whose class or member is gone.
pub(crate) fn retain_valid(inline: &mut Option<InlineMember>, store: &Store) {
    if inline.as_ref().is_some_and(|i| store.by_uuid(i.edit.class).is_none() || i.edit.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        *inline = None;
    }
}

/// The card of class `c` on the open diagram, as drawn with the row typed: its screen rectangle
/// and the top of the row in units below the card's.
fn row_place(shell: &Shell, store: &Store, view: View, c: Obj) -> Option<(Rect, f32)> {
    let n = card_node(shell, store, c)?;
    let d = archforge_diagram::diagram_of(store, n)?;
    let mut card = shell.session.as_ref()?.view().of_element(store, c)?.card(store, c, d)?;
    let (_, compartment, row) = draft(shell, store)?;
    crate::canvas::with_draft(&mut card, compartment, row);
    let index = match row {
        Some(i) => i,
        None => card.compartments.iter().take(compartment + 1).map(Vec::len).sum::<usize>().checked_sub(1)?,
    };
    let mut rect = view.rect(&archforge_diagram::grid::bounds_of(store, n));
    if row.is_none() {
        rect.max.y = rect.max.y.max(rect.top() + crate::canvas::card_height(&card) * view.zoom);
    }
    Some((rect, crate::canvas::row_top(&card, index)?))
}

/// The row over its card, when one is typed: its fields, the type menu, the details button, the
/// refusal under it, and the keys.
pub(crate) fn row(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, view: View) {
    let Some(mut inline) = shell.canvas.inline.clone() else { return };
    let place = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        let c = store.by_uuid(inline.edit.class)?;
        row_place(shell, store, view, c)
    });
    let Some((card, top)) = place else {
        shell.canvas.inline = None;
        return;
    };
    let z = view.zoom;
    let paint = ui.painter().clone();
    let row =
        Rect::from_min_size(pos2(card.left() + 8.0 * z, card.top() + (top - 3.0) * z), vec2((card.width() - 16.0 * z).max(0.0), ROW_HEIGHT * z));
    paint.rect_filled(row, 4.0 * z, p.accent_tint);
    paint.rect_stroke(row, 4.0 * z, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let font = types::UI.scaled(z);
    let inner_top = row.top() + 3.0 * z;
    let field_h = (ROW_HEIGHT - 6.0) * z;
    let details = Rect::from_min_size(pos2(row.right() - (BUTTON + 3.0) * z, row.top() + 3.0 * z), vec2(BUTTON * z, field_h));
    // The name where a row's text starts, so the draft reads as the row it becomes.
    let left = card.left() + crate::canvas::ENTITY_TEXT * z;
    let usable = (details.left() - 4.0 * z - left).max(0.0);
    let operation = inline.edit.kind == MemberKind::Operation;
    let suffix = if operation { "()" } else { "" };
    let suffix_w = if operation { ux::galley(&paint, suffix, font, p.text_muted, None).size().x } else { 0.0 };
    let type_w = (usable * 0.45).max(0.0);
    let name_w = (usable - type_w - suffix_w - 14.0 * z).max(0.0);
    let name_rect = Rect::from_min_size(pos2(left, inner_top), vec2(name_w, field_h));
    let colon_x = name_rect.right() + suffix_w + 3.0 * z;
    let type_rect = Rect::from_min_size(pos2(colon_x + 10.0 * z, inner_top), vec2((details.left() - 4.0 * z - colon_x - 10.0 * z).max(0.0), field_h));
    let chevron = Rect::from_min_size(pos2(type_rect.right() - BUTTON * z, inner_top), vec2(BUTTON * z, field_h));
    if operation {
        ux::text(&paint, pos2(name_rect.right(), inner_top), suffix, font, p.text_muted, None);
    }
    ux::text(&paint, pos2(colon_x, inner_top), ":", font, p.text_muted, None);
    paint.rect_filled(type_rect, 3.0 * z, p.surface);
    // Tab moves between the two fields, the keys taken before the fields see them.
    let (name_eid, type_eid) = (widgets::egui_id(NAME_ID), widgets::egui_id(TYPE_ID));
    let focused = ui.memory(|m| m.focused());
    let mut switched = false;
    if focused == Some(name_eid) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
        inline.field = InlineField::Type;
        inline.focus = true;
        switched = true;
    } else if focused == Some(type_eid)
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
    {
        inline.field = InlineField::Name;
        inline.focus = true;
        switched = true;
    }
    widgets::record_visible(ui, &mut shell.registry, NAME_ID, "field", name_rect);
    widgets::record_visible(ui, &mut shell.registry, TYPE_ID, "field", type_rect);
    let field = |ui: &mut egui::Ui, text: &mut String, id: egui::Id, rect: Rect, hint: String, limit: usize| {
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .font(font.font())
            .text_color(p.text)
            .hint_text(egui::RichText::new(hint.clone()).color(p.text_muted))
            .char_limit(limit)
            // Tab stays in the row: it moves between its two fields, read before they are shown.
            .lock_focus(true)
            .desired_width(rect.width());
        let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(rect)));
        let _ = (*out.response).clone().accessible_name(hint);
        out.response
    };
    let type_hint = if operation { tr("Return type") } else { tr("Type") };
    let name_resp = field(ui, &mut inline.edit.name, name_eid, name_rect, tr("Name"), archforge_uml::commands::MAX_NAME);
    let type_text_rect = Rect::from_min_max(pos2(type_rect.left() + 4.0 * z, type_rect.top()), pos2(chevron.left(), type_rect.bottom()));
    let type_resp = field(ui, &mut inline.ty, type_eid, type_text_rect, type_hint, archforge_uml::commands::MAX_NAME);
    if inline.focus {
        match inline.field {
            InlineField::Name => name_resp.request_focus(),
            InlineField::Type => type_resp.request_focus(),
        }
        inline.focus = false;
    }
    ux::icon_in(&paint, chevron.shrink(3.0 * z), &icons::CHEVRON_SMALL, p.text_secondary);
    let menu_open = shell.panels.menu == Some(crate::panels::Menu::InlineType);
    if control(shell, ui, "workspace.canvas.member.type.menu", "button", chevron, true).on_hover_text(type_hint_of(operation)).clicked() {
        shell.panels.menu = if menu_open { None } else { Some(crate::panels::Menu::InlineType) };
    }
    ux::icon_in(&paint, details.shrink(2.0 * z), &icons::MORE, p.text_secondary);
    let to_details =
        control(shell, ui, "workspace.canvas.member.details", "button", details, true).on_hover_text(tr("More details in Properties")).clicked();
    if let Some(refused) = inline.refused.clone() {
        let g = ux::galley(&paint, &refused, types::CAPTION, p.danger, Some(row.width().max(0.0) - 8.0));
        let back = Rect::from_min_size(pos2(row.left(), row.bottom() + 2.0), g.size() + vec2(8.0, 4.0));
        paint.rect_filled(back, 4.0, p.surface);
        paint.galley(back.min + vec2(4.0, 2.0), g, p.danger);
    }
    // The type menu, under the type field: a pick sets the type and gives the field the keyboard.
    let mut picked = false;
    if shell.panels.menu == Some(crate::panels::Menu::InlineType) {
        let choices = shell.session.as_ref().map(|s| crate::uml_panel::type_choices(s.doc())).unwrap_or_default();
        let labels: Vec<String> = choices.iter().map(|(label, _)| label.clone()).collect();
        if let Some(i) = crate::panels::menu(shell, ui, p, type_rect, TYPE_ID, &labels) {
            // The pick is kept by what it is, its name shown.
            let value = choices.get(i).map(|(_, v)| v.clone()).unwrap_or_default();
            inline.ty = shell.session.as_ref().map(|s| type_text(s.doc(), &value)).unwrap_or_default();
            inline.edit.ty = value;
            inline.field = InlineField::Type;
            inline.focus = true;
            picked = true;
        }
    }
    let menu_open = shell.panels.menu == Some(crate::panels::Menu::InlineType);
    // A press on the row itself (its chevron, its details) is no click away.
    let pressed = ui.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten());
    let pressed_on_row = pressed.is_some_and(|pos| row.contains(pos));
    // A press elsewhere is a click away whether or not a field had the keyboard: the chevron takes
    // it from them, and the menu's closing on that press gives it back to neither.
    let pressed_away = pressed.is_some() && !pressed_on_row && !picked;
    let has_focus = name_resp.has_focus() || type_resp.has_focus();
    let lost = name_resp.lost_focus() || type_resp.lost_focus();
    let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    shell.canvas.inline = Some(inline);
    if to_details {
        to_properties(shell);
    } else if escape && !menu_open && (has_focus || lost || ui.memory(|m| m.focused().is_none())) {
        cancel(shell);
    } else if enter && (has_focus || lost) && !menu_open {
        commit(shell, true);
    } else if ((lost && !switched && !pressed_on_row) || pressed_away) && !has_focus && !menu_open && !enter {
        commit(shell, false);
    }
}

fn type_hint_of(operation: bool) -> String {
    if operation { tr("Return type") } else { tr("Type") }
}

/// Drops the row; the class stays selected and the canvas takes the keys again.
fn cancel(shell: &mut Shell) {
    shell.canvas.inline = None;
    shell.panels.menu = None;
    shell.canvas.focused = true;
}

/// Writes the row's member. With `next`, a new member's row opens again empty in the same
/// compartment; a row without a name or a type is only dropped, and a refusal keeps the row with
/// what was typed.
fn commit(shell: &mut Shell, next: bool) {
    let Some(mut inline) = shell.canvas.inline.clone() else { return };
    if inline.edit.target.is_none() && inline.edit.name.trim().is_empty() && inline.ty.trim().is_empty() {
        cancel(shell);
        return;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    match resolve(store, &inline) {
        Ok(ty) => inline.edit.ty = ty,
        Err(e) => {
            inline.refused = Some(e);
            inline.field = InlineField::Type;
            inline.focus = true;
            shell.canvas.inline = Some(inline);
            return;
        }
    }
    let (cmd, params) = crate::uml_panel::command_of(&inline.edit);
    match crate::uml_panel::write_in_sight(shell, &inline.edit, cmd, params) {
        Ok(_) => {
            shell.canvas.notice = None;
            if next && inline.edit.target.is_none() {
                let (class, kind) = (inline.edit.class, inline.edit.kind);
                if !open(shell, class, kind, None) {
                    cancel(shell);
                }
            } else {
                cancel(shell);
            }
        }
        Err(e) => {
            inline.refused = Some(e);
            inline.field = InlineField::Name;
            inline.focus = true;
            shell.canvas.inline = Some(inline);
        }
    }
}

/// Takes the row to Properties' editor, with what was typed, for what the row does not show; a
/// type that does not resolve keeps the row, its reason under it, rather than going over as
/// another type.
fn to_properties(shell: &mut Shell) {
    let Some(mut inline) = shell.canvas.inline.clone() else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    match resolve(store, &inline) {
        Ok(ty) => inline.edit.ty = ty,
        Err(e) => {
            inline.refused = Some(e);
            inline.field = InlineField::Type;
            inline.focus = true;
            shell.canvas.inline = Some(inline);
            return;
        }
    }
    shell.canvas.inline = None;
    let mut edit = inline.edit;
    edit.refused = None;
    edit.opened = true;
    shell.panels.menu = None;
    shell.panels.member = Some(edit);
    shell.state.show_panel(crate::state::Panel::Properties);
}

/// The + at the right of each compartment of a selected class's card, which opens a new row there.
pub(crate) fn add_buttons(shell: &mut Shell, ui: &egui::Ui, p: &Palette, clip: Rect, rect: Rect, class: Uuid) {
    let z = shell.canvas.zoom;
    // While a row is typed on the card, its own keys add the next.
    if z < DETAIL_ZOOM || shell.canvas.inline.as_ref().is_some_and(|i| i.edit.class == class) {
        return;
    }
    let card = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        let c = store.by_uuid(class)?;
        let n = card_node(shell, store, c)?;
        let card = s.view().of_element(store, c)?.card(store, c, archforge_diagram::diagram_of(store, n)?)?;
        Some(card)
    });
    let Some(card) = card else { return };
    let paint = ui.painter().with_clip_rect(clip);
    for (kind, key, hint) in [(MemberKind::Attribute, "add-attribute", "Add attribute"), (MemberKind::Operation, "add-operation", "Add operation")] {
        let Some(top) = crate::canvas::compartment_top(&card, compartment(kind)) else { continue };
        // A compartment past the card's foot (a card made short) takes its member from the keys.
        if (top + ENTITY_ROW) * z > rect.height() + z {
            continue;
        }
        // As tall as a row at the card's zoom, never too small to hit.
        let size = (ENTITY_ROW * z).max(14.0);
        let b = Rect::from_min_size(
            pos2(rect.right() - size - 6.0 * z, rect.top() + (top - 3.0) * z + (ENTITY_ROW * z - size) / 2.0),
            egui::Vec2::splat(size),
        );
        if !clip.intersects(b) {
            continue;
        }
        let resp = control(shell, ui, &format!("workspace.canvas.class.{key}"), "button", b, true).on_hover_text(tr(hint));
        if resp.hovered() {
            paint.rect_filled(b, 4.0, p.field);
        }
        ux::icon_in(&paint, b.shrink(3.0), &icons::PLUS, p.text_secondary);
        if resp.clicked() && !open(shell, class, kind, None) {
            crate::uml_panel::open(shell, class, kind, None);
            shell.state.show_panel(crate::state::Panel::Properties);
        }
    }
}
