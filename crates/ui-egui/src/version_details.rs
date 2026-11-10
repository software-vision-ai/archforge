//! A version's details in the centre (frame V2.2 of page 09 of the design and sections 4 and 10 of
//! `docs/design/versioning.md`): what it is, when and by whom, the way to view it and what
//! changed since the version before. Compare and Restore join the actions when they can be carried
//! out from here.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::rows::Rows;
use archforge_notations::differences::{Area, Difference, classified};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::{tr, trn};
use crate::settings::{Look, button, control, focus_ring};
use crate::state::Explorer;
use crate::ux::{self, Palette, icons, types};
use crate::versions::{change_row, lines};
use crate::{Shell, VersionEntry};

const MARGIN: f32 = 32.0;
const BAND: f32 = 72.0;
/// A changed item's card, the distance between cards, and the room kept under them.
const CARD: f32 = 52.0;
const PITCH: f32 = 60.0;
const BELOW_CARDS: f32 = 64.0;
/// The bars a read in progress shows in place of the cards.
const SKELETON_CARDS: usize = 3;

/// What a comparison of two versions found, or why it could not tell.
type Found = Result<Arc<Vec<Difference>>, String>;

/// What changed since the version before: on its way, then known.
struct Comparison {
    answer: Receiver<Found>,
    found: Option<Found>,
}

/// The details shown: of one version, for the project and session they were read in.
pub struct Details {
    about: (PathBuf, u64),
    entry: VersionEntry,
    /// None for the first version.
    comparison: Option<Comparison>,
    scroll: TreeScroll,
}

/// What the person is told when a comparison failed, and the reason when none is known.
fn not_compared() -> String {
    tr("The changes couldn't be compared.")
}

/// The project and session shown.
fn about(shell: &Shell) -> Option<(PathBuf, u64)> {
    shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()))
}

/// Compares the two versions' models off the interface thread once both have been read. Answers
/// why when either could not be read or compared; a dropped answer ends with nobody listening.
fn compare(before: Receiver<Result<Arc<Rows>, String>>, after: Receiver<Result<Arc<Rows>, String>>) -> Receiver<Found> {
    let (tx, rx) = channel();
    let work = move || {
        // A side read without an answer drops `tx`, which the taker reads as a failed comparison.
        let (Ok(old), Ok(new)) = (before.recv(), after.recv()) else { return };
        let found = old.and_then(|old| classified(&old, &*new?, crate::editing::fit_tables).map_err(|e| e.to_string()));
        // The model's root is no change of its own; model elements come before diagrams.
        let _ = tx.send(found.map(|mut items| {
            items.retain(|d| d.class != archforge_core::MODEL);
            items.sort_by_key(|d| d.area == Area::Diagram);
            Arc::new(items)
        }));
    };
    // A thread that cannot start drops `tx` too.
    if let Err(e) = std::thread::Builder::new().name("archforge-version-details".into()).spawn(work) {
        log::warn!("the version's changes could not be compared: {e}");
    }
    rx
}

/// Selects `entry` and opens its details, reading the version before it and comparing the two. What
/// was on its way for the details left is dropped with them. Without a host it is only selected.
pub(crate) fn open(shell: &mut Shell, entry: VersionEntry) {
    shell.versions.selected = Some(entry.hash.clone());
    let (Some(host), Some(about)) = (shell.services.projects.clone(), about(shell)) else { return };
    let comparison = entry.previous.clone().map(|before| {
        let (old, new) = (host.read_version(&about.0, before), host.read_version(&about.0, entry.hash.clone()));
        Comparison { answer: compare(old, new), found: None }
    });
    shell.versions.details = Some(Details { about, entry, comparison, scroll: TreeScroll::default() });
}

/// Once a frame: details of another project or session close, the comparison's answer is taken
/// when it has come, and one still on its way wakes the window. A comparison that ended without
/// an answer has failed.
pub(crate) fn keep(shell: &mut Shell, ctx: &egui::Context) {
    let about = about(shell);
    let v = &mut shell.versions;
    if v.details.as_ref().is_some_and(|d| Some(&d.about) != about.as_ref()) {
        v.details = None;
    }
    let Some(c) = v.details.as_mut().and_then(|d| d.comparison.as_mut()) else { return };
    if c.found.is_none() {
        c.found = match c.answer.try_recv() {
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(not_compared())),
            Ok(found) => Some(found),
        };
    }
    if c.found.is_none() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Draws the details in the centre `r` while Versions is the explorer; false leaves `r` to the canvas.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> bool {
    let Some(entry) = shell.versions.details.as_ref().map(|d| d.entry.clone()) else { return false };
    if shell.state.explorer != Explorer::SourceControl {
        return false;
    }
    ui.painter().rect_filled(r, 0.0, p.page);
    breadcrumb(shell, ui, p, r);
    let clock = crate::chat_panel::clock(shell);
    let (title, meta) = lines(&entry, clock);
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    // Panels still report on the canvas, which this covers.
    if let Some(notice) = shell.canvas.notice.clone() {
        ux::text(ui.painter(), pos2(left, r.top() + BAND + 6.0), &notice, types::CAPTION, p.amber, Some(width));
    }
    let mut y = r.top() + BAND + 24.0;
    let g = ux::galley_rows(ui.painter(), &title, types::PAGE_TITLE, p.text, Some(width), 3);
    let height = g.size().y;
    ui.painter().galley(pos2(left, y), g, p.text);
    y += height + 10.0;
    ux::text(ui.painter(), pos2(left, y), &meta, types::SMALL, p.text_secondary, Some(width));
    y += 32.0;
    let off = if shell.session.as_ref().is_some_and(|s| s.interacting()) {
        Some(tr("Finish or cancel your edit first."))
    } else {
        (!crate::version_chip::available(shell)).then(|| tr("Wait for the change under way to finish."))
    };
    let id = "workspace.version-details.view";
    if button(shell, ui, p, id, pos2(left, y), &tr("View this version"), Look::Primary, off.is_none(), off.as_deref()).1.clicked() {
        // Off above whatever would refuse it, so no refusal is left to say on the canvas this hides.
        shell.versions.details = None;
        if shell.viewing() != Some(entry.hash.as_str()) {
            crate::version_chip::view(shell, entry.hash.clone(), crate::version_chip::when(&entry.date, clock));
        }
        return true;
    }
    y += 45.0;
    ui.painter().rect_filled(Rect::from_min_size(pos2(left, y), vec2(width, 1.0)), 0.0, p.border);
    y += 22.0;
    changes(shell, ui, p, r, y);
    true
}

/// The way back at the top: the project's name, which closes the details, and where they are.
fn breadcrumb(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let name = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
    let (mid, paint) = (r.top() + 34.0, ui.painter().clone());
    let g = ux::galley(&paint, &name, types::UI, p.text_secondary, Some((r.width() / 2.0).max(0.0)));
    let at = Rect::from_min_size(pos2(r.left() + MARGIN, mid - g.size().y / 2.0), g.size());
    let resp = control(shell, ui, "workspace.version-details.close", "button", at, true).on_hover_text(tr("Close"));
    let _ = resp.clone().accessible_name(format!("{name}, {}", tr("Close")));
    focus_ring(&paint, p, at, 3.0, &resp);
    paint.galley(at.min, g, p.text_secondary);
    ux::icon_mid(&paint, pos2(at.right() + 18.0, mid), &icons::CHEVRON_RIGHT, p.text_secondary);
    ux::text_mid(&paint, at.right() + 37.0, mid, &tr("History"), types::UI, p.text, Some((r.width() / 3.0).max(0.0)));
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), r.top() + BAND - 1.0), vec2(r.width(), 1.0)), 0.0, p.border);
    if resp.clicked() {
        shell.versions.details = None;
    }
}

/// The changed items from `y`, or why they are not there: the first version has none, a comparison
/// under way shows bars, and one that failed says so beside Retry, never as no changes.
fn changes(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) {
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    let Some(found) = shell.versions.details.as_ref().and_then(|d| d.comparison.as_ref()).map(|c| c.found.clone()) else {
        let card = Rect::from_min_size(pos2(left, y), vec2(width, CARD));
        ui.painter().rect_filled(card, 8.0, p.surface);
        ux::text_mid(
            ui.painter(),
            left + 12.0,
            card.center().y,
            &tr("First version: there is nothing earlier to compare it with."),
            types::UI,
            p.text,
            Some((width - 24.0).max(0.0)),
        );
        return;
    };
    ux::text(ui.painter(), pos2(left, y), &tr("Changed items").to_uppercase(), types::EYEBROW, p.text_secondary, None);
    if let Some(Ok(items)) = &found {
        let g = ux::galley(ui.painter(), &trn("{n} changed item", "{n} changed items", items.len() as u64), types::CAPTION, p.text_secondary, None);
        ui.painter().galley(pos2(r.right() - MARGIN - g.size().x, y), g, p.text_secondary);
    }
    let top = y + 25.0;
    if let Some(Err(why)) = &found {
        let words = not_compared();
        let at = Rect::from_min_size(pos2(left, top), vec2((width - 80.0).max(0.0), 28.0));
        let _ = control(shell, ui, "workspace.version-details.failed", "status", at, false).on_hover_text(why.as_str()).accessible_name(&words);
        ux::text_mid(ui.painter(), left, at.center().y, &words, types::SMALL, p.danger, Some(at.width()));
        if button(shell, ui, p, "workspace.version-details.retry", pos2(at.right() + 8.0, top), &tr("Retry"), Look::Secondary, true, None).1.clicked()
            && let Some(entry) = shell.versions.details.as_ref().map(|d| d.entry.clone())
        {
            open(shell, entry);
        }
        return;
    }
    let Some(Ok(items)) = found else {
        let area = Rect::from_min_size(pos2(left, top), vec2(width, SKELETON_CARDS as f32 * PITCH));
        let _ = control(shell, ui, "workspace.version-details.loading", "status", area, false).accessible_name(tr("Loading"));
        for i in 0..SKELETON_CARDS {
            ui.painter().rect_filled(Rect::from_min_size(pos2(left, top + i as f32 * PITCH), vec2(width, CARD)), 8.0, p.border);
        }
        return;
    };
    // A little more than the cards: `scroll_tree` keeps that much of the area clear of a list that fits.
    let height = (items.len() as f32 * PITCH + 8.0).min(r.bottom() - BELOW_CARDS - top).max(PITCH);
    let tree = Rect::from_min_size(pos2(r.left(), top), vec2(r.width() - 1.0, height));
    let Some(d) = shell.versions.details.as_mut() else { return };
    let offset = scroll_tree(ui, p, tree, top, items.len(), PITCH, 0.0, true, None, &mut d.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, item) in items.iter().enumerate() {
        let card = Rect::from_min_size(pos2(left, top + i as f32 * PITCH - offset), vec2(width, CARD));
        if card.intersects(tree) {
            clip.painter().rect_filled(card, 8.0, p.surface);
            clip.painter().rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            change_row(shell, &clip, p, card, &format!("workspace.version-details.change.{}", item.id), item);
        }
    }
}
