//! The Compare page in the centre, in place of a version's details (frames V3.1 and V3.2 of page 09
//! of the design and section 6 of `docs/design/versioning.md`): Before and After in a band, in the
//! direction Swap reverses. Each select opens a list of the versions already read, under the band;
//! AR-280 draws what the pair compares, so nothing is under the band yet.

use egui::accesskit::Role;
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use super::{BAND, MARGIN, breadcrumb};
use crate::chat_words::Clock;
use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::tr;
use crate::settings::{Look, button, control, focus_ring};
use crate::state::Explorer;
use crate::ux::{self, Palette, icons, types};
use crate::versions::{dated, described, read_more, when};
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
/// A dropdown's width, its group's heading, a row, and how many rows it shows before scrolling.
const LIST: f32 = 340.0;
const HEADING: f32 = 26.0;
const ROW: f32 = 30.0;
const MOST_ROWS: usize = 10;
const LIST_ID: &str = "workspace.version-compare.list";

/// One side of a comparison.
#[derive(Clone, PartialEq)]
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
    dropdown: Option<Dropdown>,
}

/// A select's list while it is open: which side it is for, the row the keys are on, and how far it
/// has scrolled.
#[derive(Clone)]
struct Dropdown {
    after: bool,
    highlight: usize,
    scroll: TreeScroll,
}

/// Puts the page in place of the details open, with their version before current work, always the
/// default pair however it was left. What was said of those details ends with them.
pub(super) fn open(shell: &mut Shell) {
    (shell.canvas.notice, shell.versions.selected) = (None, None);
    if let Some(d) = shell.versions.details.as_mut() {
        d.compare = Some(Pair { before: Side::Version(d.entry.clone()), after: Side::CurrentWork, dropdown: None });
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
    let first = select(
        shell,
        ui,
        p,
        (left, mid),
        ("workspace.version-compare.before", &before, false),
        (&pair.before.words(clock), select_width),
        BEFORE_GAP,
    );
    let swap_at = Rect::from_min_size(pos2(first.right() + AROUND_SWAP, mid - SELECT_HEIGHT / 2.0), vec2(SELECT_HEIGHT, SELECT_HEIGHT));
    if swap(shell, ui, p, swap_at)
        && let Some(c) = shell.versions.details.as_mut().and_then(|d| d.compare.as_mut())
    {
        std::mem::swap(&mut c.before, &mut c.after);
    }
    let (x, side) = (swap_at.right() + AROUND_SWAP, ("workspace.version-compare.after", after.as_str(), true));
    let second = select(shell, ui, p, (x, mid), side, (&pair.after.words(clock), select_width), AFTER_GAP);
    let end = second.right();
    // Left out where the window has no room: the labels already say which side is which.
    if end + BEFORE_DIRECTION + width(&direction) <= right {
        let shown = ux::text_mid(&paint, right - width(&direction), mid, &direction, types::SMALL, p.text_muted, None);
        let _ = control(shell, ui, "workspace.version-compare.direction", "status", shown, false).accessible_name(&direction);
    }
    crate::canvas::notice(shell, ui, p, pos2(left, band.bottom() + 6.0), (right - left).max(0.0));
    if let Some(d) = dropdown_of(shell).cloned() {
        let (field, held) = if d.after { (second, &pair.after) } else { (first, &pair.before) };
        list(shell, ui, p, field, held, d);
    }
}

/// The dropdown open on the page, if any.
fn dropdown_of(shell: &Shell) -> Option<&Dropdown> {
    shell.versions.details.as_ref()?.compare.as_ref()?.dropdown.as_ref()
}

/// The page's pair, to change.
fn pair_mut(shell: &mut Shell) -> Option<&mut Pair> {
    shell.versions.details.as_mut()?.compare.as_mut()
}

/// The versions already read, newest first: the list reads none of its own.
fn listed(shell: &Shell) -> &[VersionEntry] {
    shell.versions.page.as_ref().map_or(&[], |page| &page.versions)
}

/// The row the keys are on in a select's open list; any list open holds the window's keys.
pub(crate) fn highlight(shell: &Shell) -> Option<usize> {
    Some(dropdown_of(shell)?.highlight)
}

/// Once a frame: a list closes when the page leaves the centre or a dialog covers it, as one
/// nobody can see must not go on taking the keys.
pub(super) fn keep(shell: &mut Shell) {
    if (shell.state.explorer != Explorer::SourceControl || crate::version_chip::dialog_open(shell))
        && let Some(pair) = pair_mut(shell)
    {
        pair.dropdown = None;
    }
}

/// Opens the list of a side on the row it holds, or closes the list already open on that side.
fn toggle(shell: &mut Shell, after: bool) {
    let versions = listed(shell).to_vec();
    let Some(pair) = pair_mut(shell) else { return };
    let highlight = match if after { &pair.after } else { &pair.before } {
        Side::CurrentWork => 0,
        Side::Version(held) => versions.iter().position(|v| v.hash == held.hash).map_or(0, |at| at + 1),
    };
    let same = pair.dropdown.as_ref().is_some_and(|d| d.after == after);
    pair.dropdown = (!same).then(|| Dropdown { after, highlight, scroll: TreeScroll::default() });
}

/// Row `n` of the list: current work first, then the versions read.
fn side_at(versions: &[VersionEntry], n: usize) -> Option<Side> {
    if n == 0 { Some(Side::CurrentWork) } else { versions.get(n - 1).cloned().map(Side::Version) }
}

/// Puts row `n` on the side the list was opened for and closes it.
fn pick(shell: &mut Shell, n: usize) {
    let side = side_at(listed(shell), n);
    if let (Some(pair), Some(side)) = (pair_mut(shell), side)
        && let Some(d) = pair.dropdown.take()
    {
        *(if d.after { &mut pair.after } else { &mut pair.before }) = side;
    }
}

/// The keys of an open list, before the window behind can read any of them: the arrows move, Enter
/// or Space picks, and Esc or Tab closes it. Gives whether a list was open, keys or none.
pub(crate) fn keys(shell: &mut Shell, ctx: &egui::Context) -> bool {
    use egui::Key::{ArrowDown, ArrowUp, Enter, Escape, Space, Tab};
    let last = listed(shell).len();
    let press = |key, repeats| crate::menu_bar::press(ctx, key, repeats);
    let Some(pair) = pair_mut(shell).filter(|pair| pair.dropdown.is_some()) else { return false };
    if press(Escape, false) | press(Tab, false) {
        pair.dropdown = None;
        return true;
    }
    let Some(d) = pair.dropdown.as_mut() else { return false };
    let (down, up) = (press(ArrowDown, true), press(ArrowUp, true));
    d.highlight = (d.highlight + usize::from(down)).min(last).saturating_sub(usize::from(up));
    let at = d.highlight;
    if press(Enter, false) | press(Space, false) {
        pick(shell, at);
    }
    true
}

/// The open list under the select `field`: This branch, current work and the versions read,
/// reading on as its end comes into view, and what could not be read. `held` is the row marked.
fn list(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, held: &Side, mut d: Dropdown) {
    let (versions, clock) = (listed(shell).to_vec(), crate::chat_panel::clock(shell));
    let (rows, failed) = (versions.len() + 1, shell.versions.more_failed.clone());
    let footer = if failed.is_some() { ROW } else { 0.0 };
    let (window, top) = (ui.ctx().content_rect(), field.bottom() + 8.0);
    let wanted = HEADING + rows.min(MOST_ROWS) as f32 * ROW + footer + 4.0;
    let room = (window.bottom() - 8.0 - top).max(HEADING + ROW + footer + 4.0);
    let x = field.left().min(window.right() - LIST - 8.0).max(window.left() + 8.0);
    let r = Rect::from_min_size(pos2(x, top), vec2(LIST, wanted.min(room)));
    let picked = ux::overlay(ui, widgets::egui_id(LIST_ID), r, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, r, 10.0, p.shadow);
        paint.rect_filled(r, 10.0, p.surface);
        paint.rect_stroke(r, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let group = tr("This branch").to_uppercase();
        ux::text_mid(&paint, r.left() + 14.0, r.top() + 15.0, &group, types::EYEBROW_SMALL, p.text_secondary, None);
        let (first, bottom) = (r.top() + HEADING, r.bottom() - 4.0 - footer);
        let tree = Rect::from_min_max(pos2(r.left(), first), pos2(r.right() - 1.0, bottom + 8.0));
        // The highlight's place stands for its row: scroll_tree follows a row when its id changes.
        let follow = Some((d.highlight, archforge_mof::Uuid((d.highlight as u128).to_be_bytes())));
        let offset = scroll_tree(ui, p, tree, first, rows, ROW, 0.0, true, follow, &mut d.scroll);
        let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
        clip.set_clip_rect(Rect::from_min_max(pos2(r.left(), first), pos2(r.right(), bottom)));
        let mut picked = None;
        for n in 0..rows {
            let y = first + n as f32 * ROW - offset;
            let Some(side) = side_at(&versions, n).filter(|_| y + ROW > first && y < bottom) else { continue };
            let at = Rect::from_min_size(pos2(r.left() + 4.0, y), vec2(LIST - 8.0, ROW));
            let (name, beside) = match &side {
                Side::CurrentWork => (tr("Current work"), if shell.shown_unsaved() { tr("Unsaved changes") } else { tr("No unsaved changes") }),
                Side::Version(v) => (
                    when(&v.date, clock),
                    described(v).map(str::to_string).or_else(|| v.by_agent.then(|| tr("Saved by an agent"))).unwrap_or_default(),
                ),
            };
            let id = format!("{LIST_ID}.row.{n}");
            let resp = control(shell, &clip, &id, "option", at, true);
            ui.ctx().accesskit_node_builder(widgets::egui_id(&id), |node| {
                node.set_role(Role::ListBoxOption);
                node.set_label(format!("{name}, {beside}"));
                node.set_selected(side == *held);
            });
            if n == d.highlight || resp.hovered() {
                clip.painter().rect_filled(at, 6.0, p.accent_tint);
            }
            let shown = ux::text_mid(clip.painter(), at.left() + 10.0, at.center().y, &name, types::SMALL, p.text, Some(LIST * 0.45));
            let g = ux::galley(clip.painter(), &beside, types::CAPTION, p.text_secondary, Some((at.right() - 26.0 - shown.right()).max(0.0)));
            clip.painter().galley(pos2(at.right() - 14.0 - g.size().x, at.center().y - g.size().y / 2.0), g, p.text_secondary);
            if resp.clicked() {
                picked = Some(n);
            }
        }
        let v = &shell.versions;
        let more = v.page.as_ref().is_some_and(|page| page.more) && v.more_reading.is_none() && v.reading.is_none();
        if let Some(why) = failed {
            failure_row(shell, ui, p, Rect::from_min_size(pos2(r.left() + 4.0, bottom), vec2(LIST - 8.0, ROW)), &why);
        } else if more && offset + (bottom - first) + ROW >= rows as f32 * ROW {
            read_more(shell);
            ui.ctx().request_repaint();
        }
        picked
    });
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|at| r.contains(at) || field.contains(at)));
    let Some(pair) = pair_mut(shell) else { return };
    pair.dropdown = (!outside).then_some(d);
    if let Some(n) = picked {
        pick(shell, n);
    }
}

/// Under the rows: that the next versions could not be read, why on hover, and Retry.
fn failure_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, why: &str) {
    let (words, label) = (tr("The next versions couldn't be read."), tr("Retry"));
    let width = (ux::galley(ui.painter(), &label, types::UI, p.text, None).size().x + 20.0).ceil();
    let area = Rect::from_min_max(r.min, pos2(r.right() - width - 6.0, r.bottom()));
    let _ = control(shell, ui, &format!("{LIST_ID}.failed"), "status", area, false).on_hover_text(why).accessible_name(words.as_str());
    ux::text_mid(ui.painter(), r.left() + 10.0, r.center().y, &words, types::SMALL, p.danger, Some((area.width() - 16.0).max(0.0)));
    if button(shell, ui, p, &format!("{LIST_ID}.retry"), pos2(r.right() - width, r.top() + 1.0), &label, Look::Secondary, true, None).1.clicked() {
        read_more(shell);
    }
}

/// A select at `x` on the line `mid`, a press toggling its list: its label, then what it holds in a
/// field `width` wide, the gap apart; `after` says which side. While open the select keeps the
/// keyboard, asked for each frame, and Esc is kept from dropping it. Gives its field.
fn select(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    (x, mid): (f32, f32),
    (id, label, after): (&str, &str, bool),
    (value, width): (&str, f32),
    gap: f32,
) -> Rect {
    let open = dropdown_of(shell).is_some_and(|d| d.after == after);
    let paint = ui.painter().clone();
    let shown = ux::text_mid(&paint, x, mid, label, types::SMALL, p.text_secondary, None);
    let field = Rect::from_min_size(pos2(shown.right() + gap, mid - SELECT_HEIGHT / 2.0), vec2(width, SELECT_HEIGHT));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(&paint, field.left() + 8.0, mid, value, types::SMALL, p.text, Some((width - 38.0).max(0.0)));
    ux::icon_mid(&paint, pos2(field.right() - 16.0, mid), &icons::CHEVRON_SMALL, p.text_secondary);
    let resp = control(shell, ui, id, "select", field, true);
    focus_ring(&paint, p, field, 6.0, &resp);
    let name = format!("{label}, {value}");
    let (egui_id, filter) = (widgets::egui_id(id), egui::EventFilter { escape: open, ..Default::default() });
    ui.ctx().accesskit_node_builder(egui_id, |node| {
        node.set_role(Role::ComboBox);
        node.set_label(name);
        node.set_expanded(open);
    });
    ui.memory_mut(|m| {
        if open {
            m.request_focus(egui_id);
        }
        m.set_focus_lock_filter(egui_id, filter);
    });
    if resp.clicked() {
        toggle(shell, after);
    }
    field
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
