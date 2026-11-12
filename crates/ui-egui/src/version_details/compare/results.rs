//! What the Compare page finds under its band (frame V3.2 of page 09 of the design and section 6 of
//! `docs/design/versioning.md`): a version read at its revision against a snapshot of current work,
//! off the interface thread. Comparing reads rows only, so the session is never replaced.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::rows::{Rows, snapshot};
use archforge_notations::differences::{Area, Difference, classified};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use super::Side;
use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::{tr, trn};
use crate::settings::{Look, button, control};
use crate::ux::{self, Palette, types};
use crate::version_details::{Found, MARGIN, about, not_compared};
use crate::version_load::{MOST_KEPT_BYTES, bytes_of};
use crate::{Shell, VersionRead};

/// A difference's card, the distance between cards, and what a group adds to its cards: its heading
/// and the room under it.
const CARD: f32 = 50.0;
const PITCH: f32 = 56.0;
const GROUP: f32 = 34.0;
/// A group's heading down to its first card, the cards' top under the page's heading, and a card
/// that says why there is no list.
const HEADING: f32 = 20.0;
const LIST_TOP: f32 = 64.0;
const SAID: f32 = 96.0;
/// The bars a comparison in progress shows in place of the cards.
const SKELETON_CARDS: usize = 3;

/// The version compared with current work, and whether current work is the Before side.
pub(crate) type Sides = (String, bool);

/// What a comparison is of: its sides, and the revision of current work it took.
#[derive(Clone, PartialEq)]
pub(crate) struct Key {
    pub(crate) sides: Sides,
    pub(crate) revision: u64,
}

/// A version's rows; `keep` is false for rows too large to hold past the comparison they start.
pub(crate) struct Held {
    pub(crate) version: String,
    pub(crate) rows: Arc<Rows>,
    pub(crate) keep: bool,
}

/// The page's one read and one comparison, and what the last comparison found.
#[derive(Default)]
pub(crate) struct Results {
    about: Option<(PathBuf, u64)>,
    pub(crate) held: Option<Held>,
    pub(crate) reading: Option<(String, Receiver<VersionRead>)>,
    pub(crate) comparing: Option<(Key, Receiver<Found>)>,
    pub(crate) found: Option<(Key, Found)>,
    /// How far the list has scrolled; a list of other sides starts at its top.
    pub(crate) scroll: TreeScroll,
}

/// The sides the page compares now; none without the page, or for a pair that is not a version and
/// current work.
fn sides(shell: &Shell) -> Option<Sides> {
    let pair = shell.versions.details.as_ref()?.compare.as_ref()?;
    match (&pair.before, &pair.after) {
        (Side::Version(v), Side::CurrentWork) => Some((v.hash.clone(), false)),
        (Side::CurrentWork, Side::Version(v)) => Some((v.hash.clone(), true)),
        _ => None,
    }
}

/// Once a frame, after the details have had their turn: lets go of what belonged to another project
/// or session and of the rows of a page gone, takes the answers, starts what is due, and wakes the
/// window while anything is awaited, its turn at a read included.
pub(crate) fn keep(shell: &mut Shell, ctx: &egui::Context) {
    let about = about(shell);
    if shell.versions.compare_results.about != about {
        shell.versions.compare_results = Results { about, ..Results::default() };
    }
    take_answers(shell);
    let now = sides(shell);
    if now.is_none() {
        let r = &mut shell.versions.compare_results;
        (r.held, r.found) = (None, None);
    }
    let waits = now.is_some() && (shell.versions.details_comparing.is_some() || shell.versions.details_reading.is_some());
    if let Some(now) = now {
        next(shell, now);
    }
    let r = &shell.versions.compare_results;
    if waits || r.reading.is_some() || r.comparing.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// The answer on `rx`, if it has come; a read that ended without one is a failure.
fn answered<T, E: From<String>>(rx: &Receiver<Result<T, E>>) -> Option<Result<T, E>> {
    match rx.try_recv() {
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err(not_compared().into())),
        Ok(answer) => Some(answer),
    }
}

/// Lands what has come, only on the sides it is about. Rows that have only moved on still show what
/// they were, or a steady stream of edits would hide every answer; a failure for them says nothing
/// of the newest rows, which the next comparison takes.
fn take_answers(shell: &mut Shell) {
    let (now, revision) = (sides(shell), shell.current_work().map(|s| s.revision()));
    let r = &mut shell.versions.compare_results;
    if let Some(answer) = r.reading.as_ref().and_then(|(_, rx)| answered(rx))
        && let Some((version, _)) = r.reading.take()
        && let Some(sides) = now.clone().filter(|s| s.0 == version)
    {
        match answer {
            Ok(rows) => r.held = Some(Held { version, keep: bytes_of(&rows) <= MOST_KEPT_BYTES, rows }),
            Err(why) => r.found = revision.map(|revision| (Key { sides, revision }, Err(why.to_string()))),
        }
    }
    if let Some(answer) = r.comparing.as_ref().and_then(|(_, rx)| answered(rx))
        && let Some((key, _)) = r.comparing.take()
        && now.as_ref() == Some(&key.sides)
        && (answer.is_ok() || revision == Some(key.revision))
    {
        if r.found.as_ref().is_none_or(|(shown, _)| shown.sides != key.sides) {
            r.scroll = TreeScroll::default();
        }
        r.found = Some((key, answer));
    }
}

/// Compares the newest rows when what is shown is not of them. One comparison runs at a time, so
/// revisions that move while it runs fold into the next. A version that could not be read waits
/// for Retry, or each edit would read it again; nothing is compared while an edit is under way.
fn next(shell: &mut Shell, sides: Sides) {
    let Some(session) = shell.current_work().filter(|s| !s.interacting()) else { return };
    let key = Key { sides, revision: session.revision() };
    let r = &shell.versions.compare_results;
    let held = r.held.as_ref().filter(|h| h.version == key.sides.0);
    let known = r.found.as_ref().is_some_and(|(shown, found)| *shown == key || (shown.sides == key.sides && found.is_err() && held.is_none()));
    if known || r.comparing.is_some() {
        return;
    }
    let Some(before) = held.map(|h| h.rows.clone()) else { return read_before(shell, key.sides.0) };
    let (tables, junctions) = archforge_notations::storage();
    let started = snapshot(session.doc(), &tables, &junctions)
        .map_err(|e| e.to_string())
        .and_then(|live| compare(before, live, key.sides.1).map_err(|e| e.to_string()));
    let r = &mut shell.versions.compare_results;
    match started {
        Ok(answer) => r.comparing = Some((key, answer)),
        Err(why) => r.found = Some((key, Err(why))),
    }
    r.held = r.held.take().filter(|h| h.keep);
}

/// Asks for the rows of `version`, taking turns with the details' reads: the host refuses reads past
/// its limit, and that would read as a failed comparison.
fn read_before(shell: &mut Shell, version: String) {
    let v = &shell.versions;
    if v.compare_results.reading.is_some() || v.details_comparing.is_some() || v.details_reading.is_some() {
        return;
    }
    let (Some(host), Some((path, _))) = (shell.services.projects.clone(), v.compare_results.about.clone()) else { return };
    let answer = host.read_version(&path, version.clone());
    shell.versions.compare_results.reading = Some((version, answer));
}

/// Compares `version` and `live` on a thread of its own, `live` first when `swapped`. The model's
/// root is no change of its own.
fn compare(version: Arc<Rows>, live: Rows, swapped: bool) -> std::io::Result<Receiver<Found>> {
    let (tx, rx) = channel();
    let work = move || {
        let (before, after) = if swapped { (&live, &*version) } else { (&*version, &live) };
        let found = classified(before, after, crate::editing::fit_tables).map_err(|e| e.to_string());
        let _ = tx.send(found.map(|mut items| {
            items.retain(|d| d.class != archforge_core::MODEL);
            Arc::new(items)
        }));
    };
    std::thread::Builder::new().name("archforge-version-compare".into()).spawn(work).map(|_| rx)
}

/// Draws what the pair compares in `r`: the differences, or bars, or why there are none to list. A
/// failure is said before any count, so it never reads as no differences.
pub(super) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let Some(pair) = shell.versions.details.as_ref().and_then(|d| d.compare.clone()) else { return };
    let Some(now) = sides(shell) else {
        let both_current = pair.before == Side::CurrentWork && pair.after == Side::CurrentWork;
        let (head, body) = if both_current {
            (tr("Both sides are your current work."), tr("Choose another pair above."))
        } else {
            (tr("Not available yet"), String::new())
        };
        return say(shell, ui, p, r, &head, &body);
    };
    let found = shell.versions.compare_results.found.as_ref().filter(|(key, _)| key.sides == now).map(|(_, found)| found.clone());
    match found {
        None => loading(shell, ui, p, r),
        Some(Err(why)) => failed(shell, ui, p, r, &why),
        Some(Ok(items)) if items.is_empty() => {
            say(shell, ui, p, r, &tr("This version is the same as your current work."), &tr("Choose another pair above."))
        }
        Some(Ok(items)) => list(shell, ui, p, r, &items),
    }
}

/// The left edge and width of what is drawn in `r`.
fn column(r: Rect) -> (f32, f32) {
    (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0))
}

/// A card saying `head`, and under it `body`, in place of a list.
fn say(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, head: &str, body: &str) {
    let (left, width) = column(r);
    let card = Rect::from_min_size(pos2(left, r.top()), vec2(width, SAID));
    let paint = ui.painter();
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let room = Some((width - 40.0).max(0.0));
    ux::text_mid(paint, left + 20.0, card.center().y - if body.is_empty() { 0.0 } else { 11.0 }, head, types::BODY_STRONG, p.text, room);
    ux::text_mid(paint, left + 20.0, card.center().y + 12.0, body, types::SMALL, p.text_secondary, room);
    let _ = control(shell, ui, "workspace.version-compare.said", "status", card, false).accessible_name(format!("{head} {body}").trim_end());
}

/// Bars in place of the cards while the comparison is under way.
fn loading(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let (left, width) = column(r);
    let area = Rect::from_min_size(pos2(left, r.top()), vec2(width, SKELETON_CARDS as f32 * PITCH));
    let _ = control(shell, ui, "workspace.version-compare.loading", "status", area, false).accessible_name(tr("Loading"));
    for i in 0..SKELETON_CARDS {
        ui.painter().rect_filled(Rect::from_min_size(pos2(left, r.top() + i as f32 * PITCH), vec2(width, CARD)), 8.0, p.border);
    }
}

/// That the pair could not be compared, why on hover, and Retry.
fn failed(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, why: &str) {
    let (left, width) = column(r);
    let (words, at) = (not_compared(), Rect::from_min_size(pos2(left, r.top()), vec2((width - 80.0).max(0.0), 28.0)));
    let _ = control(shell, ui, "workspace.version-compare.failed", "status", at, false).on_hover_text(why).accessible_name(&words);
    ux::text_mid(ui.painter(), left, at.center().y, &words, types::SMALL, p.danger, Some(at.width()));
    if button(shell, ui, p, "workspace.version-compare.retry", pos2(at.right() + 8.0, r.top()), &tr("Retry"), Look::Secondary, true, None).1.clicked()
    {
        shell.versions.compare_results.found = None;
    }
}

/// How many differences there are, by area, and the cards in their groups, scrolling under that.
fn list(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, items: &[Difference]) {
    let (left, width) = column(r);
    let count = |area| items.iter().filter(|d| d.area == area).count();
    let (model, diagrams) = (count(Area::Model), count(Area::Diagram));
    let head = trn("{n} difference", "{n} differences", items.len() as u64);
    let shown = ux::text(ui.painter(), r.min + vec2(MARGIN, 0.0), &head, types::SECTION, p.text, Some(width));
    let _ = control(shell, ui, "workspace.version-compare.count", "status", shown, false).accessible_name(&head);
    let parts = [
        (model > 0).then(|| trn("{n} model element", "{n} model elements", model as u64)),
        (diagrams > 0).then(|| trn("{n} diagram", "{n} diagrams", diagrams as u64)),
    ];
    let breakdown = parts.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    ux::text(ui.painter(), pos2(left, r.top() + 32.0), &breakdown, types::SMALL, p.text_secondary, Some(width));
    let groups = [(tr("Model elements"), Area::Model, model), (tr("Diagrams"), Area::Diagram, diagrams)];
    let content: f32 = groups.iter().filter(|g| g.2 > 0).map(|g| GROUP + g.2 as f32 * PITCH).sum();
    let top = r.top() + LIST_TOP;
    let tree = Rect::from_min_max(pos2(r.left(), top), pos2(r.right() - 1.0, r.bottom().max(top)));
    // What shows of the list, as scroll_tree measures it.
    scroll_keys(shell, ui, (tree.bottom() - top - 8.0).max(1.0));
    let offset = scroll_tree(ui, p, tree, top, content as usize, 1.0, 0.0, true, None, &mut shell.versions.compare_results.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    let mut y = top - offset;
    for (label, area, n) in groups.iter().filter(|g| g.2 > 0) {
        ux::text(clip.painter(), pos2(left, y), &label.to_uppercase(), types::EYEBROW_SMALL, p.text_secondary, None);
        let g = ux::galley(clip.painter(), &n.to_string(), types::EYEBROW_SMALL, p.text_muted, None);
        clip.painter().galley(pos2(r.right() - MARGIN - g.size().x, y), g, p.text_muted);
        y += HEADING;
        for d in items.iter().filter(|d| d.area == *area) {
            let card = Rect::from_min_size(pos2(left, y), vec2(width, CARD));
            if card.intersects(tree) {
                clip.painter().rect_filled(card, 8.0, p.surface);
                clip.painter().rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
                // By class too: an element that changes table has two rows.
                crate::versions::change_row(shell, &clip, p, card, &format!("workspace.version-compare.change.{}.{}", d.id, d.class.0), d, false);
            }
            y += PITCH;
        }
        y += GROUP - HEADING;
    }
}

/// The keys scroll the cards while no control and not the explorer has them, as the explorer's
/// keys work only while it has them: the arrows by a card, Page Up and Page Down by the `page` that
/// shows, Home and End to either end. scroll_tree keeps the offset within the list.
fn scroll_keys(shell: &mut Shell, ui: &egui::Ui, page: f32) {
    use egui::Key::{ArrowDown, ArrowUp, End, Home, PageDown, PageUp};
    if shell.explorer.focused || ui.memory(|m| m.focused().is_some()) {
        return;
    }
    let press = |key| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
    let scroll = &mut shell.versions.compare_results.scroll;
    for (key, by) in [(ArrowDown, PITCH), (ArrowUp, -PITCH), (PageDown, page), (PageUp, -page)] {
        if press(key) {
            scroll.offset += by;
        }
    }
    if press(Home) {
        scroll.offset = 0.0;
    }
    if press(End) {
        scroll.offset = f32::MAX;
    }
}
