//! The related changes under the Before and After cards (frame V3.3 of page 09): the other elements
//! connected to the open one that differ in the same comparison, each with a link that opens it. A
//! link only moves along the list held, so the comparison stays the one shown and nothing is read.

use std::collections::HashMap;

use archforge_mof::{Universe, Uuid};
use archforge_notations::differences::{Area, ChangeKind, Difference};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::i18n::{fmt, tr, trn};
use crate::model_words::kind_label;
use crate::settings::{control, focus_ring};
use crate::ux::{self, Palette, types};
use crate::widgets::egui_id;

/// The room above the card, its inset, the middle of its heading and of its first row, and the distance between rows.
const ABOVE: f32 = 14.0;
const INSET: f32 = 20.0;
const HEAD_MID: f32 = 21.5;
const FIRST_MID: f32 = 44.0;
const ROW: f32 = 28.0;
/// The widest a link gets, and the room between it and its sentence.
const LINK: f32 = 260.0;
const BETWEEN: f32 = 12.0;

/// One related change: where its row is in the list of changes, and what happened to it.
pub(super) type Link = (usize, ChangeKind);

/// The rows of `changes` that `related` names, in that order. An element with rows in two tables is
/// one change, named from a row of the side it is still on; only a model's rows open, so a diagram's stay out.
pub(super) fn links(changes: &[Difference], related: &[Uuid]) -> Vec<Link> {
    if related.is_empty() {
        return Vec::new();
    }
    let mut rows: HashMap<Uuid, Link> = HashMap::new();
    for (i, d) in changes.iter().enumerate().filter(|(_, d)| d.area == Area::Model) {
        let (at, kind) = rows.entry(d.id).or_insert((i, d.kind));
        if d.kind != ChangeKind::Removed {
            *at = i;
        }
        if *kind != d.kind {
            *kind = ChangeKind::Changed;
        }
    }
    related.iter().filter_map(|id| rows.get(id).copied()).collect()
}

/// How much room `count` links take under the cards: none without any.
pub(super) fn height(count: usize) -> f32 {
    if count == 0 { 0.0 } else { ABOVE + FIRST_MID + count as f32 * ROW }
}

/// Draws the card of `links` in `area`, its room above included. Only rows in view, a row past each
/// end of it, the first, and the link that has the keys with its neighbours are worded and drawn, so a
/// long list costs a frame only what shows while Tab and Shift+Tab still find the next link, from
/// wherever the cards are scrolled, and bring it into view, and Enter still opens a link wheeled out of view.
pub(super) fn draw(shell: &mut Shell, clip: &egui::Ui, p: &Palette, universe: &Universe, area: Rect, links: &[Link]) {
    if links.is_empty() {
        return;
    }
    let (card, paint) = (area.with_min_y(area.top() + ABOVE), clip.painter().clone());
    paint.rect_filled(card, 10.0, p.surface);
    paint.rect_stroke(card, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let (left, right) = (card.left() + INSET, card.right() - INSET);
    let heading = trn("Related change", "Related changes", links.len() as u64);
    let head = ux::text_mid(&paint, left, card.top() + HEAD_MID, &heading, types::SMALL, p.text_secondary, Some((right - left).max(0.0)));
    let _ = control(shell, clip, "workspace.change-view.related", "status", head, false).accessible_name(&heading);
    let (room, view, keys) = ((right - left - BETWEEN).max(0.0), clip.clip_rect(), clip.ctx().memory(|m| m.focused()));
    // Ids are made only while something has the keys, to keep the link that has them and its neighbours.
    let keyed = keys.and_then(|k| links.iter().position(|(at, _)| shell.versions.changes.get(*at).is_some_and(|d| k == egui_id(&link_id(d.id)))));
    for (i, (at, kind)) in links.iter().enumerate() {
        let mid = card.top() + FIRST_MID + i as f32 * ROW;
        let line = Rect::from_min_max(pos2(left, mid - ROW / 2.0), pos2(right, mid + ROW / 2.0));
        if !(i == 0 || line.intersects(view.expand2(vec2(0.0, ROW))) || keyed.is_some_and(|k| i.abs_diff(k) <= 1)) {
            continue;
        }
        let Some(row) = shell.versions.changes.get(*at).cloned() else { continue };
        let name = row.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| kind_label(universe, row.class));
        let (said, go) = (sentence(*kind, &name), fmt(&tr("Go to {name}"), &[("name", &name)]));
        let g = ux::galley(&paint, &go, types::UI_STRONG, p.accent, Some(LINK.min(room / 2.0)));
        let link = Rect::from_min_size(pos2(right - g.size().x, mid - g.size().y / 2.0), g.size());
        let shown = ux::text_mid(&paint, left, mid, &said, types::UI, p.text, Some((link.left() - BETWEEN - left).max(0.0)));
        let id = link_id(row.id);
        let _ = control(shell, clip, &format!("{id}.said"), "status", shown, false).on_hover_text(&said).accessible_name(&said);
        let resp = control(shell, clip, &id, "link", link, true).on_hover_text(&go);
        let _ = resp.clone().accessible_name(&go);
        if resp.gained_focus() {
            into_view(shell, clip.ctx(), line, view);
        }
        focus_ring(&paint, p, link, 3.0, &resp);
        paint.galley(link.min, g, p.accent);
        if resp.clicked() {
            super::open(shell, row);
        }
    }
}

/// The automation id of the link to the element `id`.
fn link_id(id: Uuid) -> String {
    format!("workspace.change-view.related.{id}")
}

/// Scrolls the cards by what `line` stands out of `view`, so the link the keys just reached shows
/// whole; the wheel is free again after.
fn into_view(shell: &mut Shell, ctx: &egui::Context, line: Rect, view: Rect) {
    let by = (line.top() - view.top()).min(0.0) + (line.bottom() - view.bottom()).max(0.0);
    if let Some(o) = shell.versions.element.as_mut().filter(|_| by != 0.0) {
        o.scroll.offset += by;
        ctx.request_repaint();
    }
}

/// What happened to the element called `name`, as a whole sentence in the language shown.
fn sentence(kind: ChangeKind, name: &str) -> String {
    let template = match kind {
        ChangeKind::Added => tr("{name} was added in this comparison."),
        ChangeKind::Changed => tr("{name} was changed in this comparison."),
        ChangeKind::Removed => tr("{name} was removed in this comparison."),
    };
    fmt(&template, &[("name", name)])
}
