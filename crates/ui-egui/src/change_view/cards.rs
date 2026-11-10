//! The Before and After cards of a changed element (frames V1.1, V3.3 and V6.2 of page 09): what it
//! is and is called on each side and its values, worded while drawing, so the language shown is the
//! one used. What differs is marked with a sign as well as a tint.

use archforge_mof::{PropId, TypeRef, Universe, Value};
use archforge_notations::differences::ElementSides;
use egui::{CornerRadius, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::explorer::scroll_tree;
use crate::i18n::{fmt, tr};
use crate::model_words::{kind_label, literal_label, property_label};
use crate::settings::control;
use crate::ux::{self, Palette, types};
use crate::version_details::MARGIN;
use crate::versions::when;

/// The room between the cards, and from a side's eyebrow to its card.
const GUTTER: f32 = 24.0;
const EYEBROW: f32 = 24.0;
/// A card's head band, the top of its identity, where its rows start and the room under them.
const BAND: f32 = 46.0;
const IDENTITY: f32 = 52.0;
const ROWS_AT: f32 = 116.0;
const BELOW: f32 = 16.0;
/// The distance between rows, a card's inset, and how much further in a marked line starts.
const ROW: f32 = 28.0;
const INSET: f32 = 24.0;
const MARK: f32 = 16.0;

/// Draws the cards of `sides` from `top` to the end of `r`, scrolling as one, so a value stays level
/// with itself across them. Everything comes from the sides already held: nothing here asks the host.
pub(super) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, top: f32, sides: &ElementSides) {
    let Some(universe) = shell.session.as_ref().map(|s| s.doc().universe().clone()) else { return };
    let head = head_before(shell);
    let tree = Rect::from_min_max(pos2(r.left(), top), pos2(r.right() - 1.0, r.bottom().max(top)));
    let Some(o) = shell.versions.element.as_mut() else { return };
    let offset = scroll_tree(ui, p, tree, top, sides.fields.len(), ROW, EYEBROW + ROWS_AT + BELOW, true, None, &mut o.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    let each = ((r.width() - 2.0 * MARGIN - GUTTER) / 2.0).max(0.0);
    let size = vec2(each, ROWS_AT + sides.fields.len() as f32 * ROW + BELOW);
    let before = Rect::from_min_size(pos2(r.left() + MARGIN, top - offset + EYEBROW), size);
    card(shell, &clip, p, &universe, (before, true), sides, &head);
    card(shell, &clip, p, &universe, (before.translate(vec2(each + GUTTER, 0.0)), false), sides, &tr("Current work"));
}

/// What the Before card's head calls its side: the version compared with, by its date while the
/// history holds it, as a version further down the list is not read for this.
fn head_before(shell: &Shell) -> String {
    let v = &shell.versions;
    let hash = v.element.as_ref().and_then(|o| o.read_of.as_ref()).and_then(|key| key.0.as_ref());
    let listed = hash.and_then(|hash| v.page.iter().flat_map(|page| &page.versions).find(|e| e.hash == *hash));
    let clock = crate::chat_panel::clock(shell);
    listed.map_or_else(|| tr("Last version"), |e| fmt(&tr("Version of {when}"), &[("when", &when(&e.date, clock))]))
}

/// One side's card in `body` under its eyebrow; `before` says which. A side the element is not on
/// is a tinted card saying so: nothing yet on an added element's Before, Removed on a removed one's After.
fn card(shell: &mut Shell, clip: &egui::Ui, p: &Palette, universe: &Universe, (body, before): (Rect, bool), sides: &ElementSides, head: &str) {
    let (side, words, on_it, eyebrow) =
        if before { ("before", tr("Nothing here yet"), &sides.before, tr("Before")) } else { ("after", tr("Removed"), &sides.after, tr("After")) };
    let paint = clip.painter().clone();
    ux::text(&paint, body.min - vec2(0.0, EYEBROW), &eyebrow.to_uppercase(), types::EYEBROW, p.text_secondary, None);
    let Some(card) = on_it else {
        let id = format!("workspace.change-view.{side}");
        let _ = control(shell, clip, &id, "status", body, false).accessible_name(format!("{eyebrow}, {words}"));
        paint.rect_filled(body, 8.0, p.field);
        paint.rect_stroke(body, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let g = ux::galley(&paint, &words, types::UI, p.text_muted, Some((body.width() - 2.0 * INSET).max(0.0)));
        paint.galley(body.center() - g.size() / 2.0, g, p.text_muted);
        return;
    };
    let _ = control(shell, clip, &format!("workspace.change-view.{side}"), "status", body, false).accessible_name(format!("{eyebrow}, {head}"));
    paint.rect_filled(body, 8.0, p.surface);
    paint.rect_filled(Rect::from_min_size(body.min, vec2(body.width(), BAND)), CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, p.page);
    paint.rect_stroke(body, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.hline(body.x_range(), body.top() + BAND, Stroke::new(1.0, p.border));
    ux::text_mid(&paint, body.left() + 20.0, body.top() + BAND / 2.0, head, types::UI, p.text, Some((body.width() - 40.0).max(0.0)));
    let line = Rect::from_min_size(body.min + vec2(MARK, IDENTITY), vec2((body.width() - MARK - INSET).max(0.0), ROWS_AT - IDENTITY - 14.0));
    let marked = sides.before.as_ref().zip(sides.after.as_ref()).is_some_and(|(a, b)| a.name != b.name || a.class != b.class);
    if marked {
        mark(&paint, p, line, before);
    }
    let x = line.left() + INSET - MARK + if marked { MARK } else { 0.0 };
    let name = card.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| tr("Unnamed"));
    let kind = kind_label(universe, card.class);
    let room = Some((line.right() - 8.0 - x).max(0.0));
    ux::text(&paint, pos2(x, line.top() + 4.0), &kind.to_uppercase(), types::EYEBROW_SMALL, p.text_secondary, room);
    ux::text(&paint, pos2(x, line.top() + 19.0), &name, types::SECTION, p.text, room);
    let said = if marked { format!("{kind}, {name}, {}", tr("Changed")) } else { format!("{kind}, {name}") };
    let _ = control(shell, clip, &format!("workspace.change-view.{side}.name"), "status", line, false).on_hover_text(&said).accessible_name(said);
    paint.hline(body.left() + INSET..=body.right() - INSET, body.top() + ROWS_AT - 10.0, Stroke::new(1.0, p.border));
    rows(shell, clip, p, universe, (body, before), sides);
}

/// The values of one side, one row a field; a field this side has no such property for leaves its row empty.
fn rows(shell: &mut Shell, clip: &egui::Ui, p: &Palette, universe: &Universe, (body, before): (Rect, bool), sides: &ElementSides) {
    let side = if before { "before" } else { "after" };
    let value_at = body.left() + INSET + (body.width() - 2.0 * INSET) * 0.45;
    let paint = clip.painter().clone();
    for (i, field) in sides.fields.iter().enumerate() {
        let Some(value) = (if before { &field.before } else { &field.after }) else { continue };
        let at = pos2(body.left() + MARK, body.top() + ROWS_AT + i as f32 * ROW);
        let row = Rect::from_min_size(at, vec2((body.width() - MARK - INSET).max(0.0), ROW - 2.0));
        if !row.intersects(clip.clip_rect()) {
            continue;
        }
        if field.changed {
            mark(&paint, p, row, before);
        }
        let label = property_label(universe, field.prop).unwrap_or_else(|| tr("Unknown"));
        let value = worded(universe, field.prop, value);
        let x = body.left() + INSET + if field.changed { MARK } else { 0.0 };
        ux::text_mid(&paint, x, row.center().y, &label, types::UI, p.text_secondary, Some((value_at - x - 8.0).max(0.0)));
        ux::text_mid(&paint, value_at, row.center().y, &value, types::UI, p.text, Some((row.right() - 8.0 - value_at).max(0.0)));
        let said = if field.changed { format!("{label}, {value}, {}", tr("Changed")) } else { format!("{label}, {value}") };
        let id = format!("workspace.change-view.{side}.field.{i}");
        let _ = control(shell, clip, &id, "status", row, false).on_hover_text(&said).accessible_name(said);
    }
}

/// Tints `row` and writes its sign at its start: minus on the Before card, plus on the After card.
fn mark(paint: &egui::Painter, p: &Palette, row: Rect, before: bool) {
    let (sign, ink, tint) = if before { ("−", p.danger, p.danger_tint) } else { ("+", p.green, p.green_tint) };
    paint.rect_filled(row, 4.0, tint);
    ux::text_mid(paint, row.left() + 8.0, row.center().y, sign, types::UI, ink, None);
}

/// `value` of `prop` for a person to read in the language shown; a literal the metamodel does not know is Unknown.
fn worded(universe: &Universe, prop: PropId, value: &Value) -> String {
    match value {
        Value::Null => tr("Not set"),
        Value::Bool(true) => tr("Yes"),
        Value::Bool(false) => tr("No"),
        Value::Int(n) => n.to_string(),
        Value::Real(x) => x.to_string(),
        Value::Str(s) if s.is_empty() => tr("Not set"),
        Value::Str(s) => s.to_string(),
        Value::Unlimited(None) => "*".to_owned(),
        Value::Unlimited(Some(n)) => n.to_string(),
        Value::Enum(i) => match universe.prop(prop).map(|p| &p.ty) {
            Some(TypeRef::Data(ty)) => literal_label(universe, *ty, *i).unwrap_or_else(|| tr("Unknown")),
            _ => tr("Unknown"),
        },
        Value::Ref(_) => tr("Unknown"),
    }
}
