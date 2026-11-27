//! UML's adornments of the canvas's lines and cards, which UML's diagram kind draws through the
//! kind's hooks while the canvas draws only its lines and cards: an association end's qualifier
//! box against its class, the line starting from the box's outer side (AR-127,
//! `DiagramKind::line_ends`); a line's stereotypes above its name (AR-135,
//! `DiagramKind::line_caption`); and a template's signature over its card's top-right corner
//! (AR-130, `DiagramKind::card_extras`).

use archforge_diagram::edge;
use archforge_diagram::figure::Tone;
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::ux::{self, Palette, types};
use archforge_shell_api::canvas::{DETAIL_ZOOM, Drawing, along, dashed_rect, element_of};
use archforge_shell_api::kinds::{CardDrawn, LineDrawn, LineEnds};
use archforge_shell_api::widgets::{self, Registry};

/// Moves each end of `line` that holds qualifiers out to its qualifier box (AR-127) in `points`,
/// writing it in `Drawing::moved_ends`, and gives the boxes to draw over the line: a box sits
/// against the side of its node where the line leaves it, centred on the line, and the line's end
/// moves to the box's outer side, so the end's mark, role and text sit there. Records each box in
/// `registry`. A line from a node to itself has none.
pub(crate) fn qualifier_boxes(
    paint: &egui::Painter,
    d: &Drawing<'_>,
    line: &LineEnds<'_>,
    points: &mut [Pos2],
    registry: &mut Registry,
    ui: &egui::Ui,
) -> Vec<egui::Shape> {
    let (store, z, n) = (d.store, d.view.zoom, points.len());
    let nodes = [element_of(store, line.line, edge::SOURCE), element_of(store, line.line, edge::TARGET)];
    if n < 2 || nodes[0] == nodes[1] {
        return Vec::new();
    }
    let stroke = Stroke::new((1.0 * z).clamp(1.0, 2.0), line.stroke.color);
    let mut shapes = Vec::new();
    for (i, end) in line.figure.ends.iter().enumerate() {
        let (Some(node), false) = (nodes[i], end.qualifiers.is_empty()) else { continue };
        let k = if i == 0 { 0 } else { n - 1 };
        let rows: Vec<_> = end.qualifiers.iter().map(|t| ux::galley(paint, t, types::SMALL.scaled(z), d.p.text, None)).collect();
        let width = rows.iter().map(|g| g.size().x).fold(0.0_f32, f32::max) + 16.0 * z;
        let height = rows.iter().map(|g| g.size().y).sum::<f32>() + 8.0 * z;
        let (rect, outer) = qualifier_place(d.node_rect(node), points[k], vec2(width, height));
        d.moved_ends.borrow_mut().push((points[k], outer));
        points[k] = outer;
        qualifier_box(&mut shapes, d.p, rect, rows, stroke, z);
        if let Ok(id) = store.uuid(line.line) {
            widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}.qualifiers.{i}"), "label", rect);
        }
    }
    shapes
}

/// Where a qualifier box of `size` sits against `node` for a line leaving it at `tip`: on the side
/// `tip` is nearest, centred on it, and the point on its outer side the line starts from.
fn qualifier_place(node: Rect, tip: Pos2, size: Vec2) -> (Rect, Pos2) {
    let gaps = [(tip.x - node.left()).abs(), (tip.x - node.right()).abs(), (tip.y - node.top()).abs(), (tip.y - node.bottom()).abs()];
    let side = (0..4).min_by(|a, b| gaps[*a].total_cmp(&gaps[*b])).unwrap_or(1);
    let (w, h) = (size.x, size.y);
    match side {
        0 => (Rect::from_min_size(pos2(node.left() - w, tip.y - h / 2.0), size), pos2(node.left() - w, tip.y)),
        1 => (Rect::from_min_size(pos2(node.right(), tip.y - h / 2.0), size), pos2(node.right() + w, tip.y)),
        2 => (Rect::from_min_size(pos2(tip.x - w / 2.0, node.top() - h), size), pos2(tip.x, node.top() - h)),
        _ => (Rect::from_min_size(pos2(tip.x - w / 2.0, node.bottom()), size), pos2(tip.x, node.bottom() + h)),
    }
}

/// A qualifier box at `rect`, added to `shapes`: the surface, framed in the line's ink, its rows in
/// model order.
fn qualifier_box(shapes: &mut Vec<egui::Shape>, p: &Palette, rect: Rect, rows: Vec<std::sync::Arc<egui::Galley>>, stroke: Stroke, z: f32) {
    shapes.push(egui::Shape::rect_filled(rect, 1.0, p.surface));
    shapes.push(egui::Shape::rect_stroke(rect, 1.0, stroke, StrokeKind::Inside));
    let mut y = rect.top() + 4.0 * z;
    for g in rows {
        let h = g.size().y;
        if !g.is_empty() {
            shapes.push(egui::Shape::galley(pos2(rect.left() + 8.0 * z, y), g, p.text.into()));
        }
        y += h;
    }
}

/// Draws a line's stereotypes, `«Reference»` (AR-135, F3), in the caption style: just above its
/// name `name` where it shows one, else at its middle where a name would sit. Records them in
/// `registry`; nothing for a line with none.
pub(crate) fn stereotypes(paint: &egui::Painter, d: &Drawing<'_>, line: &LineDrawn<'_>, name: Option<Rect>, registry: &mut Registry, ui: &egui::Ui) {
    let Some(keyword) = line.figure.keyword.as_deref() else { return };
    let at = keyword_text(paint, d, keyword, line.points, name);
    if let Ok(id) = d.store.uuid(line.line) {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.edge.{id}.stereotypes"), "label", at);
    }
}

/// A line's stereotypes `keyword` as [`stereotypes`] draws them; gives where they are.
fn keyword_text(paint: &egui::Painter, d: &Drawing<'_>, keyword: &str, points: &[Pos2], name: Option<Rect>) -> Rect {
    let (p, z) = (d.p, d.view.zoom);
    let g = ux::galley(paint, keyword, types::CAPTION.scaled(z), p.text_secondary, None);
    let at = match name {
        Some(r) => pos2(r.center().x - g.size().x / 2.0, r.top() - g.size().y - 1.0 * z),
        None => {
            let upright = {
                let v = along(points, 0.51) - along(points, 0.49);
                v.y.abs() > v.x.abs()
            };
            let offset = if upright { vec2(8.0 * z, -g.size().y / 2.0) } else { vec2(-g.size().x / 2.0, -g.size().y - 6.0 * z) };
            along(points, 0.5) + offset
        }
    };
    let rect = Rect::from_min_size(at, g.size());
    paint.galley(at, g, p.text_secondary.into());
    rect
}

/// How far a template's signature box reaches past its card's right side and above its top, in
/// diagram units, as the design draws it (page 08, C1).
const SIGNATURE_OUT: Vec2 = vec2(28.0, 24.0);

/// Draws a template's signature over its card `c` (AR-130) from [`DETAIL_ZOOM`] on, recording it in
/// `registry`; nothing for a card that is no template, nor for a constraint's.
pub(crate) fn signature(paint: &egui::Painter, d: &Drawing<'_>, c: &CardDrawn<'_>, registry: &mut Registry, ui: &egui::Ui) {
    let (card, z) = (c.card, d.view.zoom);
    if matches!(card.tone, Tone::Constraint | Tone::Braces) || card.template.is_empty() || z < DETAIL_ZOOM {
        return;
    }
    let sig = signature_box(paint, d.p, &card.template, c.rect, z);
    if let Ok(id) = d.store.uuid(c.node) {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.signature"), "label", sig);
    }
}

/// A template's signature `rows`: a dashed box on the surface over the card's top-right corner, one
/// parameter a row in their order; gives its rectangle.
fn signature_box(paint: &egui::Painter, p: &Palette, rows: &[String], card: Rect, z: f32) -> Rect {
    let galleys: Vec<_> = rows.iter().map(|t| ux::galley(paint, t, types::CAPTION.scaled(z), p.text, None)).collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max) + 16.0 * z;
    let height = galleys.iter().map(|g| g.size().y).sum::<f32>() + 8.0 * z;
    let rect = signature_place(card, vec2(width, height), z);
    paint.rect_filled(rect, 0.0, p.surface);
    dashed_rect(paint, rect, Stroke::new(1.0, p.text_secondary), 4.0 * z, 3.0 * z);
    let mut y = rect.top() + 4.0 * z;
    for g in galleys {
        let h = g.size().y;
        paint.galley(pos2(rect.left() + 8.0 * z, y), g, p.text.into());
        y += h;
    }
    rect
}

/// Where a template signature box of `size` sits on `card`: over its top-right corner, reaching
/// [`SIGNATURE_OUT`] past it, scaled by `z`.
fn signature_place(card: Rect, size: Vec2, z: f32) -> Rect {
    Rect::from_min_size(pos2(card.right() + SIGNATURE_OUT.x * z - size.x, card.top() - SIGNATURE_OUT.y * z), size)
}
