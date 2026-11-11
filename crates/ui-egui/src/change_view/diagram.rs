//! A changed diagram's Before and After (frame V3.4 of page 09): each side drawn from its own state
//! in one shared framing, so a box that stayed is in the same place on both.

use std::collections::BTreeMap;

use archforge_diagram::grid::{bounds_of, middle_of};
use archforge_diagram::{LabelPlace, diagram, edge, label, node_tree};
use archforge_mof::Value;
use archforge_notations::differences::{DiagramSide, DiagramSides};
use egui::{CornerRadius, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::cards::{self, BAND, EYEBROW, GUTTER};
use crate::Shell;
use crate::canvas::{Live, Scene, State, View, draw_scene, fit, real, refs};
use crate::i18n::tr;
use crate::settings::control;
use crate::ux::{self, Palette, types};
use crate::version_details::MARGIN;
use crate::widgets::Registry;

/// Draws the two sides side by side from `top` to the end of `r`. Everything comes from the sides
/// already held: nothing here asks the host or touches the session.
pub(super) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, top: f32, sides: &DiagramSides) {
    let area = Rect::from_min_max(pos2(r.left(), top), pos2(r.right(), r.bottom().max(top)));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    ui.set_clip_rect(area.intersect(ui.clip_rect()));
    let each = ((r.width() - 2.0 * MARGIN - GUTTER) / 2.0).max(0.0);
    let size = vec2(each, (r.bottom() - MARGIN - top - EYEBROW).max(0.0));
    let before = Rect::from_min_size(pos2(r.left() + MARGIN, top + EYEBROW), size);
    let framing = framing(sides, vec2(each, (size.y - BAND).max(0.0)));
    let head = cards::head_before(shell);
    pane(shell, &mut ui, p, (before, true), (sides, framing), &head);
    let after = before.translate(vec2(each + GUTTER, 0.0));
    pane(shell, &mut ui, p, (after, false), (sides, framing), &tr("Current work"));
}

/// The one zoom and pan both sides are drawn with: what fits all either side draws into `size`.
/// Neither comes from the project or the canvas, so how someone last looked at it is never a difference.
fn framing(sides: &DiagramSides, size: Vec2) -> (f32, Vec2) {
    let mut fitted = State::default();
    // Below the canvas's least zoom too, as nothing here pans or zooms to what it would leave out.
    fit(&mut fitted, size, [&sides.before, &sides.after].into_iter().flatten().filter_map(extent).reduce(Rect::union), f32::MIN_POSITIVE);
    (fitted.zoom, fitted.pan)
}

/// The rectangle around what `side` draws: every box, nested ones too, as nothing keeps them inside
/// their parent, every line, whose bends can reach past the boxes, and where each line's labels were
/// put, however far from it. A label's own size is left to the margin.
fn extent(side: &DiagramSide) -> Option<Rect> {
    let (store, point) = (&side.store, |(x, y): (f64, f64)| Rect::from_min_size(pos2(x as f32, y as f32), Vec2::ZERO));
    let tree = refs(store, side.diagram, diagram::NODE).into_iter().flat_map(|top| node_tree(store, top));
    let boxes = tree.map(|n| bounds_of(store, n)).map(|b| Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32)));
    let bends = side.grid.lines().flat_map(|(_, points)| points.iter().copied());
    let labels = side.grid.lines().flat_map(|(e, points)| {
        refs(store, e, edge::LABEL).into_iter().filter_map(move |l| {
            let from = match store.get(l, label::AT) {
                Ok(Value::Enum(i)) if LabelPlace::from_index(i) == Some(LabelPlace::Source) => points.first().copied(),
                Ok(Value::Enum(i)) if LabelPlace::from_index(i) == Some(LabelPlace::Target) => points.last().copied(),
                _ => Some(middle_of(points)),
            };
            from.map(|(x, y)| (x + real(store, l, label::DX), y + real(store, l, label::DY)))
        })
    });
    boxes.chain(bends.chain(labels).map(point)).filter(Rect::is_finite).reduce(Rect::union)
}

/// One side's pane in `card` under its eyebrow; `before` says which. A side the diagram is not on is
/// a tinted pane saying so: nothing yet on an added diagram's Before, Removed on a removed one's After.
fn pane(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, (card, before): (Rect, bool), (sides, framing): (&DiagramSides, (f32, Vec2)), head: &str) {
    let (side, words, eyebrow, on_it) =
        if before { ("before", tr("Nothing here yet"), tr("Before"), &sides.before) } else { ("after", tr("Removed"), tr("After"), &sides.after) };
    let paint = ui.painter().clone();
    ux::text(&paint, card.min - vec2(0.0, EYEBROW), &eyebrow.to_uppercase(), types::EYEBROW, p.text_secondary, None);
    let said = format!("{eyebrow}, {}", if on_it.is_some() { head } else { &words });
    let _ = control(shell, ui, &format!("workspace.change-view.{side}"), "status", card, false).accessible_name(said);
    paint.rect_filled(card, 8.0, if on_it.is_some() { p.surface } else { p.field });
    let Some(on_it) = on_it.as_ref() else {
        paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let g = ux::galley(&paint, &words, types::UI, p.text_muted, Some((card.width() - 48.0).max(0.0)));
        paint.galley(card.center() - g.size() / 2.0, g, p.text_muted);
        return;
    };
    let body = Rect::from_min_max(pos2(card.left(), (card.top() + BAND).min(card.bottom())), card.max);
    paint.rect_filled(Rect::from_min_max(card.min, body.right_top()), CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, p.page);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.hline(card.x_range(), body.top(), Stroke::new(1.0, p.border));
    ux::text_mid(&paint, card.left() + 20.0, card.top() + BAND / 2.0, head, types::UI, p.text, Some((card.width() - 40.0).max(0.0)));
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(body));
    clip.set_clip_rect(body.intersect(ui.clip_rect()));
    let view = View { origin: body.min, pan: framing.1, zoom: framing.0 };
    scene(shell, &clip, p, (side, body, &view), on_it);
}

/// Draws the side's diagram read-only: no selection, gesture or problem. It records under the side's
/// own ids, as both sides hold the same boxes and lines.
fn scene(shell: &mut Shell, clip: &egui::Ui, p: &Palette, (side, area, view): (&str, Rect, &View), on_it: &DiagramSide) {
    let (notations, none, clean) = (archforge_notations::view(), BTreeMap::new(), BTreeMap::new());
    let scene = Scene { p, store: &on_it.store, notations: &notations, diagram: on_it.diagram, view, grid: &on_it.grid, area };
    let live = Live {
        selection: &[],
        overrides: &none,
        key_status: false,
        renaming: None,
        problems: &clean,
        hovered: None,
        end_drag: None,
        segment_drag: None,
        draft: None,
        rename_text: None,
        set_drag: None,
        looking_only: true,
    };
    let mut drawn = Registry::default();
    let _ = draw_scene(clip, clip.painter(), &scene, live, &mut drawn);
    let prefix = format!("workspace.change-view.{side}");
    for e in drawn.entries() {
        let at = Rect::from_min_max(pos2(e.rect[0], e.rect[1]), pos2(e.rect[2], e.rect[3]));
        shell.registry.record(&e.id.replacen("workspace.canvas", &prefix, 1), e.kind, at);
    }
}
