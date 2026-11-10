//! The Compare page in the centre, in place of a version's details (frames V3.1 and V3.2 of page 09
//! of the design and section 6 of `docs/design/versioning.md`): Before and After in a band, in the
//! direction Swap reverses. The selects only show the pair until the list to choose from lands.

use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use super::{BAND, MARGIN, breadcrumb};
use crate::chat_words::Clock;
use crate::i18n::tr;
use crate::settings::{control, focus_ring};
use crate::ux::{self, Palette, icons, types};
use crate::versions::dated;
use crate::{Shell, VersionEntry, widgets};

/// The band's height, a select's size at its widest and its narrowest.
const STRIP: f32 = 56.0;
const SELECT: f32 = 250.0;
const SELECT_NARROW: f32 = 60.0;
const SELECT_HEIGHT: f32 = 28.0;
/// The room between a label and its select, on the Before and the After side.
const BEFORE_GAP: f32 = 14.0;
const AFTER_GAP: f32 = 12.0;
/// The room on each side of Swap, and before the direction at the end of the band.
const AROUND_SWAP: f32 = 12.0;
const BEFORE_DIRECTION: f32 = 16.0;

/// One side of a comparison.
#[derive(Clone)]
enum Side {
    Version(VersionEntry),
    CurrentWork,
}

impl Side {
    /// What the select shows for this side.
    fn words(&self, clock: Clock) -> String {
        match self {
            Side::Version(entry) => dated(entry, clock),
            Side::CurrentWork => tr("Current work"),
        }
    }
}

/// The two sides on the page, Before first.
#[derive(Clone)]
pub(super) struct Pair {
    before: Side,
    after: Side,
}

/// Puts the page in place of the details open, with their version before current work, always the
/// default pair however it was left. What was said of those details ends with them.
pub(super) fn open(shell: &mut Shell) {
    (shell.canvas.notice, shell.versions.selected) = (None, None);
    if let Some(d) = shell.versions.details.as_mut() {
        d.compare = Some(Pair { before: Side::Version(d.entry.clone()), after: Side::CurrentWork });
    }
}

/// Draws the page in the centre `r`: the way back, the band and what was said since it opened, as
/// this covers the canvas that says it. The details go with the page, so whatever ends them ends it.
pub(super) fn draw(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let Some(pair) = shell.versions.details.as_ref().and_then(|d| d.compare.clone()) else { return };
    if breadcrumb(shell, ui, p, r, "workspace.version-compare.close", &tr("Compare")).0 {
        shell.versions.details = None;
    }
    let clock = crate::chat_panel::clock(shell);
    let band = Rect::from_min_size(pos2(r.left(), r.top() + BAND), vec2(r.width(), STRIP));
    let (paint, mid) = (ui.painter().clone(), band.center().y);
    paint.rect_filled(band, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_size(pos2(band.left(), band.bottom() - 1.0), vec2(band.width(), 1.0)), 0.0, p.border);
    let (left, right) = (r.left() + MARGIN, r.right() - MARGIN);
    let (before, after, direction) = (tr("Before"), tr("After"), tr("Before → After"));
    let width = |words: &str| ux::galley(&paint, words, types::SMALL, p.text, None).size().x;
    let (labels, ends) =
        (width(&before) + width(&after), BEFORE_GAP + AFTER_GAP + 2.0 * AROUND_SWAP + SELECT_HEIGHT + BEFORE_DIRECTION + width(&direction));
    let select_width = ((right - left - labels - ends) / 2.0).clamp(SELECT_NARROW, SELECT);
    let end = select(shell, ui, p, (left, mid), ("workspace.version-compare.before", &before), (&pair.before.words(clock), select_width), BEFORE_GAP);
    let swap_at = Rect::from_min_size(pos2(end + AROUND_SWAP, mid - SELECT_HEIGHT / 2.0), vec2(SELECT_HEIGHT, SELECT_HEIGHT));
    if swap(shell, ui, p, swap_at)
        && let Some(c) = shell.versions.details.as_mut().and_then(|d| d.compare.as_mut())
    {
        std::mem::swap(&mut c.before, &mut c.after);
    }
    let (x, side) = (swap_at.right() + AROUND_SWAP, ("workspace.version-compare.after", after.as_str()));
    let end = select(shell, ui, p, (x, mid), side, (&pair.after.words(clock), select_width), AFTER_GAP);
    // Left out where the window has no room: the labels already say which side is which.
    if end + BEFORE_DIRECTION + width(&direction) <= right {
        let shown = ux::text_mid(&paint, right - width(&direction), mid, &direction, types::SMALL, p.text_muted, None);
        let _ = control(shell, ui, "workspace.version-compare.direction", "status", shown, false).accessible_name(&direction);
    }
    crate::canvas::notice(shell, ui, p, pos2(left, band.bottom() + 6.0), (right - left).max(0.0));
}

/// A select at `x` on the line `mid`: its label, then what it holds in a field `width` wide, the
/// gap apart. It says it cannot be used yet, on hover and when read out. Gives where it ends.
fn select(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    (x, mid): (f32, f32),
    (id, label): (&str, &str),
    (value, width): (&str, f32),
    gap: f32,
) -> f32 {
    let paint = ui.painter().clone();
    let shown = ux::text_mid(&paint, x, mid, label, types::SMALL, p.text_secondary, None);
    let field = Rect::from_min_size(pos2(shown.right() + gap, mid - SELECT_HEIGHT / 2.0), vec2(width, SELECT_HEIGHT));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(&paint, field.left() + 8.0, mid, value, types::SMALL, p.text, Some((width - 38.0).max(0.0)));
    ux::icon_mid(&paint, pos2(field.right() - 16.0, mid), &icons::CHEVRON_SMALL, p.text_secondary);
    let off = tr("Not available yet");
    let _ = control(shell, ui, id, "select", field, false).on_hover_text(&off);
    let name = format!("{label}, {value}, {off}");
    ui.ctx().accesskit_node_builder(widgets::egui_id(id), |node| {
        node.set_role(egui::accesskit::Role::ComboBox);
        node.set_label(name);
        node.set_disabled();
    });
    field.right()
}

/// The Swap button in `at`; gives whether it was pressed.
fn swap(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Rect) -> bool {
    let words = tr("Swap Before and After");
    let resp = control(shell, ui, "workspace.version-compare.swap", "button", at, true).on_hover_text(&words);
    let _ = resp.clone().accessible_name(&words);
    let paint = ui.painter();
    paint.rect_filled(at, 6.0, if resp.hovered() { p.page } else { p.surface });
    paint.rect_stroke(at, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    focus_ring(paint, p, at, 6.0, &resp);
    ux::icon_mid(paint, at.center(), &icons::SWAP, p.text_secondary);
    resp.clicked()
}
