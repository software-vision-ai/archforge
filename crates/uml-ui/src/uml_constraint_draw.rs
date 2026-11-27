//! UML's constraints on the canvas (AR-133), which UML's diagram kind draws through the kind's
//! hooks while the canvas draws only its own cards and lines: a constraint's braces beside its
//! element, or its constraint note (`DiagramKind::draw_card`); the note's dashed links to the
//! nodes of the elements it constrains (`DiagramKind::draw_links`); and a double click anywhere on
//! either opens its editor (`DiagramKind::opens_on_double_click`).

use archforge_diagram::figure::{Card, Tone};
use archforge_diagram::node;
use archforge_mof::{Obj, Store};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::ux::{self, types};
use archforge_shell_api::canvas::{DETAIL_ZOOM, Drawing, element_of, exit_point, eyebrow_case, ink_for, look_of, say};
use archforge_shell_api::kinds::CardDrawn;

/// Draws the card of a constraint, braces or a note, in place of the canvas's own figure for it;
/// false for any other card, which the canvas draws.
pub(crate) fn draw_card(paint: &egui::Painter, d: &Drawing<'_>, c: &CardDrawn<'_>) -> bool {
    if !matches!(c.card.tone, Tone::Constraint | Tone::Braces) {
        return false;
    }
    // Clipped to the card as drawn.
    let inner = paint.with_clip_rect(c.rect.intersect(paint.clip_rect()));
    draw_constraint(paint, &inner, d, c.node, c.card, c.rect);
    true
}

/// Whether a double click on the card of `element` opens its editor with no row: a constraint's,
/// its braces or its note, which have no header to rename in and no rows of their own.
pub(crate) fn opens_on_double_click(store: &Store, element: Obj) -> bool {
    archforge_uml::constraints::is_constraint(store, element)
}

/// A constraint as its diagram shows it (AR-133): braces, `{expression}` in purple beside its
/// element with no card around it; or a constraint note, the surface with a purple border and a
/// folded corner, `CONSTRAINT · NOT EVALUATED` in its purple eyebrow, its name, then its expression
/// between braces, as written. Its own look on the diagram wins, as a note's does.
fn draw_constraint(paint: &egui::Painter, inner: &egui::Painter, d: &Drawing<'_>, n: Obj, card: &Card, rect: Rect) {
    let (p, z) = (d.p, d.view.zoom);
    let look = look_of(d.store, n, node::STYLE);
    let text_ink = look.text.unwrap_or_else(|| look.fill.map_or(p.text, |f| ink_for(f).text));
    let rows: Vec<&str> = card.compartments.iter().flatten().map(|r| r.text.as_str()).collect();
    if card.tone == Tone::Braces {
        let text = rows.first().copied().unwrap_or_default();
        let ink = look.text.unwrap_or(p.purple);
        ux::text(paint, rect.min + vec2(2.0, 2.0) * z, text, types::CAPTION.scaled(z), ink, None);
        return;
    }
    let edge = look.stroke.unwrap_or(p.purple);
    let border = look.width.map_or(1.25 * z, |w| w * z).max(1.0);
    paint.rect_filled(rect, 0.0, look.fill.unwrap_or(p.surface));
    paint.rect_stroke(rect, 0.0, Stroke::new(border, edge), StrokeKind::Inside);
    // The folded corner at the top right.
    let fold = (14.0 * z).min(rect.width() / 2.0).min(rect.height() / 2.0);
    let (tr_, fl, fb) = (rect.right_top(), rect.right_top() - vec2(fold, 0.0), rect.right_top() + vec2(0.0, fold));
    paint.add(egui::Shape::convex_polygon(vec![fl, pos2(fl.x, fb.y), fb], p.purple_tint, Stroke::new(border, edge)));
    paint.line_segment([fl, tr_], Stroke::new(border, look.fill.unwrap_or(p.canvas)));
    let room = Some((rect.width() - 24.0 * z - fold).max(0.0));
    if z < DETAIL_ZOOM {
        let g = ux::galley(inner, &card.name, types::SECTION.scaled(z.max(DETAIL_ZOOM)), text_ink, room);
        inner.galley(rect.center() - g.size() / 2.0, g, text_ink.into());
        return;
    }
    let eyebrow = eyebrow_case(&say(&card.eyebrow));
    ux::text(inner, rect.min + vec2(12.0, 12.0) * z, &eyebrow, types::EYEBROW.scaled(z), look.stroke.unwrap_or(p.purple), room);
    let mut y = 30.0;
    if !card.name.is_empty() {
        ux::text(inner, rect.min + vec2(12.0, y) * z, &card.name, types::SMALL_STRONG.scaled(z), text_ink, room);
        y += 21.0;
    }
    for row in rows {
        if rect.top() + (y + 17.0) * z > rect.bottom() {
            break;
        }
        ux::text(inner, rect.min + vec2(12.0, y) * z, row, types::SMALL.scaled(z), text_ink, Some((rect.width() - 24.0 * z).max(0.0)));
        y += 18.0;
    }
}

/// The dashed links of the diagram's constraint notes to the nodes of the elements they constrain
/// (AR-133), as UML works them out from the model each frame (`constraints::note_links`): purple, the accent while
/// the note is selected.
pub(crate) fn draw_links(paint: &egui::Painter, d: &Drawing<'_>) {
    let (p, store, z) = (d.p, d.store, d.view.zoom);
    for (n, _) in &d.boxes {
        let Some(e) = element_of(store, *n, node::ELEMENT) else { continue };
        let targets = archforge_uml::constraints::note_links(store, e, d.diagram);
        if targets.is_empty() {
            continue;
        }
        let from_rect = d.node_rect(*n);
        let selected = d.selection.contains(n) || d.selection.contains(&e);
        let stroke = Stroke::new((1.25 * z).clamp(1.0, 2.5), if selected { p.accent } else { p.purple });
        for t in targets {
            let to_rect = d.node_rect(t);
            let (from, to) = (exit_point(from_rect, to_rect.center()), exit_point(to_rect, from_rect.center()));
            if from.is_finite() && to.is_finite() {
                paint.extend(egui::Shape::dashed_line(&[from, to], stroke, (3.0 * z).max(1.5), (3.0 * z).max(1.5)));
            }
        }
    }
}
