//! A version's details in the centre (frame V2.2 of page 09 of the design and sections 4 and 10 of
//! `docs/design/versioning.md`): what it is, when and by whom, the way to view it, what changed
//! since the version before and a link to that version. Compare opens its page in their place;
//! Restore joins the actions when it can be carried out from here.

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
use crate::versions::{change_row, dated, lines};
use crate::{Shell, VersionEntry};

pub(crate) mod compare;

pub(crate) const MARGIN: f32 = 32.0;
pub(crate) const BAND: f32 = 72.0;
/// A changed item's card, the distance between cards, and the room kept under them.
const CARD: f32 = 52.0;
const PITCH: f32 = 60.0;
const BELOW_CARDS: f32 = 64.0;
/// Where the section under the list starts, below its last card, and the link's line in it.
const BELOW_LIST: f32 = 16.0;
const LINK: f32 = 27.0;
/// The bars a read in progress shows in place of the cards.
const SKELETON_CARDS: usize = 3;

/// What a comparison of two versions found, or why it could not tell.
type Found = Result<Arc<Vec<Difference>>, String>;

/// What changed since the version before, and that version for the link: on their way, then known.
struct Comparison {
    /// The version compared with, which the answer has to be about.
    before: String,
    found: Option<Found>,
    /// The version before as the link shows it, or why it could not be read.
    earlier: Option<Result<VersionEntry, String>>,
}

/// A read the details have under way. A version's two models cannot be called back once asked for,
/// so one comparison runs at a time however quickly rows are clicked, and the link's read takes
/// turns with it, so the details never add to its two reads: the next starts for the details shown.
pub(crate) struct Running<T> {
    about: (PathBuf, u64),
    /// The version before and the one it comes before.
    pair: (String, String),
    answer: Receiver<Result<T, String>>,
}

impl<T> Running<T> {
    /// The answer, if it has come, with the two versions it is about; `stopped` says why when the
    /// read ended without answering.
    fn answered(&self, stopped: impl FnOnce() -> String) -> Option<((String, String), Result<T, String>)> {
        match self.answer.try_recv() {
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some((self.pair.clone(), Err(stopped()))),
            Ok(answer) => Some((self.pair.clone(), answer)),
        }
    }
}

/// The details shown: of one version, for the project and session they were read in.
pub struct Details {
    about: (PathBuf, u64),
    entry: VersionEntry,
    /// None for the first version.
    comparison: Option<Comparison>,
    /// The Compare page shown in place of these details, while it is.
    compare: Option<compare::Pair>,
    scroll: TreeScroll,
}

/// What the person is told when a comparison failed, and the reason when none is known.
fn not_compared() -> String {
    tr("The changes couldn't be compared.")
}

/// What the person is told when the version before could not be read, the reason when none is known.
fn not_read() -> String {
    tr("The previous version couldn't be read.")
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

/// Selects `entry` and opens its details; [`keep`] compares it with the version before when no
/// other comparison is running. Without a host it is only selected.
pub(crate) fn open(shell: &mut Shell, entry: VersionEntry) {
    shell.versions.element = None;
    shell.versions.selected = Some(entry.hash.clone());
    let Some(about) = about(shell).filter(|_| shell.services.projects.is_some()) else { return };
    let comparison = entry.previous.clone().map(|before| Comparison { before, found: None, earlier: None });
    shell.versions.details = Some(Details { about, entry, comparison, compare: None, scroll: TreeScroll::default() });
}

/// Once a frame: what belongs to another project or session is let go, the answers that have come
/// are taken, the version before comes from the list when it holds it, the details shown get their
/// next read when none runs, and an answer still awaited wakes the window. An answer lands only on
/// the two versions it is about, the link only for the version it names.
pub(crate) fn keep(shell: &mut Shell, ctx: &egui::Context) {
    compare::keep(shell);
    let about = about(shell);
    let host = shell.services.projects.clone();
    let v = &mut shell.versions;
    if v.details.as_ref().is_some_and(|d| Some(&d.about) != about.as_ref()) {
        v.details = None;
    }
    if v.details_comparing.as_ref().is_some_and(|c| Some(&c.about) != about.as_ref()) {
        v.details_comparing = None;
    }
    if v.details_reading.as_ref().is_some_and(|c| Some(&c.about) != about.as_ref()) {
        v.details_reading = None;
    }
    let answered = v.details_comparing.as_ref().and_then(|c| c.answered(not_compared));
    if answered.is_some() {
        v.details_comparing = None;
    }
    let read = v.details_reading.as_ref().and_then(|c| c.answered(not_read));
    if read.is_some() {
        v.details_reading = None;
    }
    let Some(d) = v.details.as_mut() else { return };
    let Some(c) = d.comparison.as_mut() else { return };
    let pair = (c.before.clone(), d.entry.hash.clone());
    if let Some((_, found)) = answered.filter(|(compared, _)| *compared == pair) {
        c.found = Some(found);
    }
    if let Some((_, page)) = read.filter(|(asked, _)| *asked == pair) {
        c.earlier = Some(page.and_then(|page| page.versions.into_iter().next().filter(|e| e.hash == pair.0).ok_or_else(not_read)));
    }
    if c.earlier.is_none()
        && let Some(listed) = v.page.iter().flat_map(|page| &page.versions).find(|e| e.hash == pair.0)
    {
        c.earlier = Some(Ok(listed.clone()));
    }
    if let (true, None, Some(host), Some(about)) = (v.details_comparing.is_none(), &v.details_reading, host, about) {
        if c.found.is_none() {
            let (old, new) = (host.read_version(&about.0, pair.0.clone()), host.read_version(&about.0, pair.1.clone()));
            v.details_comparing = Some(Running { about, pair, answer: compare(old, new) });
        } else if c.earlier.is_none() {
            let answer = host.versions(&about.0, Some(pair.1.clone()), 1);
            v.details_reading = Some(Running { about, pair, answer });
        }
    }
    if c.found.is_none() || c.earlier.is_none() {
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
    if shell.versions.details.as_ref().is_some_and(|d| d.compare.is_some()) {
        compare::draw(shell, ui, p, r);
        return true;
    }
    if breadcrumb(shell, ui, p, r, "workspace.version-details.close", &tr("History")).0 {
        shell.versions.details = None;
    }
    let clock = crate::chat_panel::clock(shell);
    let (title, meta) = lines(&entry, clock);
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    // Panels still report on the canvas, which this covers.
    let said = crate::canvas::notice(shell, ui, p, pos2(left, r.top() + BAND + 6.0), width);
    let mut y = r.top() + BAND + 24.0 + (said - types::CAPTION.line).max(0.0);
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
    let (view, pressed) = button(shell, ui, p, id, pos2(left, y), &tr("View this version"), Look::Primary, off.is_none(), off.as_deref());
    if pressed.clicked() {
        // Refused, these details stay and say why, above: an editor still open is one of the reasons.
        if shell.viewing() == Some(entry.hash.as_str()) {
            // Already showing, so nothing can refuse: a reason from an earlier try goes with these,
            // and so does a step on its way to another.
            shell.loads.call_off();
            shell.canvas.notice = None;
            shell.versions.details = None;
        } else if crate::version_chip::view(shell, entry.hash.clone(), crate::version_chip::when(&entry.date, clock)) {
            shell.versions.details = None;
        }
        return true;
    }
    let id = "workspace.version-details.compare";
    if button(shell, ui, p, id, pos2(view.right() + 8.0, y), &tr("Compare…"), Look::Secondary, true, None).1.clicked() {
        compare::open(shell);
        return true;
    }
    y += 45.0;
    ui.painter().rect_filled(Rect::from_min_size(pos2(left, y), vec2(width, 1.0)), 0.0, p.border);
    y += 22.0;
    let y = changes(shell, ui, p, r, y);
    previous(shell, ui, p, r, y, clock);
    true
}

/// The way back at the top of the centre `r`: the project's name, registered as `id`, and `place`,
/// where the centre is. Gives whether the name was clicked, for the caller to close what it shows,
/// and where the breadcrumb ends.
pub(crate) fn breadcrumb(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, id: &str, place: &str) -> (bool, f32) {
    let name = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
    let (mid, paint) = (r.top() + 34.0, ui.painter().clone());
    let g = ux::galley(&paint, &name, types::UI, p.text_secondary, Some((r.width() / 2.0).max(0.0)));
    let at = Rect::from_min_size(pos2(r.left() + MARGIN, mid - g.size().y / 2.0), g.size());
    let resp = control(shell, ui, id, "button", at, true).on_hover_text(tr("Close"));
    let _ = resp.clone().accessible_name(format!("{name}, {}", tr("Close")));
    focus_ring(&paint, p, at, 3.0, &resp);
    paint.galley(at.min, g, p.text_secondary);
    ux::icon_mid(&paint, pos2(at.right() + 18.0, mid), &icons::CHEVRON_RIGHT, p.text_secondary);
    let end = ux::text_mid(&paint, at.right() + 37.0, mid, place, types::UI, p.text, Some((r.width() / 3.0).max(0.0))).right();
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), r.top() + BAND - 1.0), vec2(r.width(), 1.0)), 0.0, p.border);
    (resp.clicked(), end)
}

/// The changed items from `y`, or why they are not there: the first version has none, a comparison
/// under way shows bars, and one that failed says so beside Retry, never as no changes. Gives where
/// the next section starts.
fn changes(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
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
        return card.bottom() + BELOW_LIST;
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
        return at.bottom() + BELOW_LIST;
    }
    let Some(Ok(items)) = found else {
        let area = Rect::from_min_size(pos2(left, top), vec2(width, SKELETON_CARDS as f32 * PITCH));
        let _ = control(shell, ui, "workspace.version-details.loading", "status", area, false).accessible_name(tr("Loading"));
        for i in 0..SKELETON_CARDS {
            ui.painter().rect_filled(Rect::from_min_size(pos2(left, top + i as f32 * PITCH), vec2(width, CARD)), 8.0, p.border);
        }
        return area.bottom() - (PITCH - CARD) + BELOW_LIST;
    };
    // A little more than the cards: `scroll_tree` keeps that much of the area clear of a list that fits.
    let height = (items.len() as f32 * PITCH + 8.0).min(r.bottom() - BELOW_CARDS - top).max(PITCH);
    let tree = Rect::from_min_size(pos2(r.left(), top), vec2(r.width() - 1.0, height));
    let Some(d) = shell.versions.details.as_mut() else { return tree.bottom() };
    let offset = scroll_tree(ui, p, tree, top, items.len(), PITCH, 0.0, true, None, &mut d.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, item) in items.iter().enumerate() {
        let card = Rect::from_min_size(pos2(left, top + i as f32 * PITCH - offset), vec2(width, CARD));
        if card.intersects(tree) {
            clip.painter().rect_filled(card, 8.0, p.surface);
            clip.painter().rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            change_row(shell, &clip, p, card, &format!("workspace.version-details.change.{}", item.id), item, false);
        }
    }
    tree.bottom()
}

/// The version before, from `y`, as a link that opens its details; bars while it is on its way and
/// a reason for one that could not be read, with Retry unless the changes above offer theirs, which
/// reads it again too. Nothing for the first version, nor where the window leaves no room for it.
fn previous(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, clock: crate::chat_words::Clock) {
    let Some(c) = shell.versions.details.as_ref().and_then(|d| d.comparison.as_ref()) else { return };
    let (earlier, changes_failed) = (c.earlier.clone(), matches!(c.found, Some(Err(_))));
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    let line = y + LINK;
    // The tallest row, the one with Retry, ends 23 below its line.
    if line + 23.0 > r.bottom() {
        return;
    }
    ux::text(ui.painter(), pos2(left, y + 11.0), &tr("Previous version").to_uppercase(), types::EYEBROW_SMALL, p.text_secondary, None);
    match earlier {
        None => {
            let bar = Rect::from_min_size(pos2(left, line + 2.0), vec2(width.min(220.0), 12.0));
            let _ = control(shell, ui, "workspace.version-details.previous.loading", "status", bar, false).accessible_name(tr("Loading"));
            ui.painter().rect_filled(bar, 4.0, p.border);
        }
        Some(Err(why)) => {
            let words = not_read();
            let (id, at) =
                ("workspace.version-details.previous.retry", Rect::from_min_size(pos2(left, line - 5.0), vec2((width - 80.0).max(0.0), 28.0)));
            let _ = control(shell, ui, "workspace.version-details.previous.failed", "status", at, false).on_hover_text(why).accessible_name(&words);
            ux::text_mid(ui.painter(), left, at.center().y, &words, types::SMALL, p.danger, Some(at.width()));
            if !changes_failed
                && button(shell, ui, p, id, pos2(at.right() + 8.0, at.top()), &tr("Retry"), Look::Secondary, true, None).1.clicked()
                && let Some(c) = shell.versions.details.as_mut().and_then(|d| d.comparison.as_mut())
            {
                c.earlier = None;
            }
        }
        Some(Ok(before)) => {
            let words = dated(&before, clock);
            let g = ux::galley(ui.painter(), &words, types::SMALL, p.accent, Some(width));
            let at = Rect::from_min_size(pos2(left, line), g.size());
            let resp = control(shell, ui, "workspace.version-details.previous", "link", at, true);
            let _ = resp.clone().accessible_name(&words);
            focus_ring(ui.painter(), p, at, 3.0, &resp);
            ui.painter().galley(at.min, g, p.accent);
            if resp.clicked() {
                open(shell, before);
            }
        }
    }
}
