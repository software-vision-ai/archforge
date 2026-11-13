//! What the Compare page finds under its band (frame V3.2 of page 09 of the design and section 6 of
//! `docs/design/versioning.md`): a version read at its revision against current work, off the
//! interface thread. The frame only copies current work's objects; their rows are built with the
//! comparison. Comparing reads rows only, so the session is never replaced. While a version shows,
//! current work is the session set aside.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::Frozen;
use archforge_mof::rows::Rows;
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

/// Which step gave what is found: differences always come from a comparison, a failure from either.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Origin {
    Read,
    Comparison,
}

/// The page's one read and one comparison, and what the last of them found, with which one it was.
/// The read, of a version in a project and session, is never let go before it answers: a host read
/// cannot be called back and counts against the host's limit until it ends, so Compare never has
/// more than this one under way, whatever was opened and closed meanwhile.
#[derive(Default)]
pub(crate) struct Results {
    about: Option<(PathBuf, u64)>,
    pub(crate) held: Option<Held>,
    pub(crate) reading: Option<((PathBuf, u64), String, Receiver<VersionRead>)>,
    pub(crate) comparing: Option<(Key, Receiver<Found>)>,
    pub(crate) found: Option<(Key, Found, Origin)>,
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

/// Lets go of all but the read once the project or session shown is another, or none is, and takes
/// the answers that have come. Also once a frame before drawing, as Home and a project closed draw
/// no rail to call [`keep`]: an answer nobody wants is dropped there too, its rows with it.
pub(crate) fn let_go(shell: &mut Shell) {
    let about = about(shell);
    let r = &mut shell.versions.compare_results;
    if r.about != about {
        *r = Results { about, reading: r.reading.take(), ..Results::default() };
    }
    take_answers(shell);
}

/// Once a frame, after the details have had their turn: lets go of what belonged to another project
/// or session, takes the answers, starts what is due, and wakes the window while anything is
/// awaited, its turn at a read included. A page gone, or on a pair it does not compare, wants
/// nothing and wakes nothing: its rows go, and its read and comparison, which cannot be called
/// back, run on unwatched, their answers dropped as they come unless a page is back on their sides.
/// The details do not wait for that read.
pub(crate) fn keep(shell: &mut Shell, ctx: &egui::Context) {
    let_go(shell);
    let Some(now) = sides(shell) else {
        let r = &mut shell.versions.compare_results;
        (r.held, r.found) = (None, None);
        return;
    };
    let waits = shell.versions.details_comparing.is_some() || shell.versions.details_reading.is_some();
    next(shell, now);
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

/// Lands what has come, only on the sides it is about, a read only in the project and session it is
/// of, however alike another opened at the same path; what nobody wants is dropped. Rows that have only moved on still show what they were, or a steady
/// stream of edits would hide every answer; a failure for them says nothing of the newest rows,
/// which the next comparison takes.
fn take_answers(shell: &mut Shell) {
    let (now, revision) = (sides(shell), shell.current_work().map(|s| s.revision()));
    let r = &mut shell.versions.compare_results;
    if let Some(answer) = r.reading.as_ref().and_then(|(.., rx)| answered(rx))
        && let Some((of, version, _)) = r.reading.take()
        && let Some(sides) = now.clone().filter(|s| s.0 == version && r.about.as_ref() == Some(&of))
    {
        match answer {
            Ok(rows) => r.held = Some(Held { version, keep: bytes_of(&rows) <= MOST_KEPT_BYTES, rows }),
            Err(why) => r.found = revision.map(|revision| (Key { sides, revision }, Err(why.to_string()), Origin::Read)),
        }
    }
    if let Some(answer) = r.comparing.as_ref().and_then(|(_, rx)| answered(rx))
        && let Some((key, _)) = r.comparing.take()
        && now.as_ref() == Some(&key.sides)
        && (answer.is_ok() || revision == Some(key.revision))
    {
        if r.found.as_ref().is_none_or(|(shown, ..)| shown.sides != key.sides) {
            r.scroll = TreeScroll::default();
        }
        r.found = Some((key, answer, Origin::Comparison));
    }
}

/// Compares the newest rows when what is shown is not of them. One comparison runs at a time, so
/// revisions that move while it runs fold into the next; nothing is compared while an edit is under
/// way. What is shown for these sides is known at its own revision, whatever it is. At a later one,
/// differences and a failed comparison give way: the rows held are compared again, or, if they went
/// as too large, read again first. A failed read stays known until Retry clears it, or each edit
/// would ask the host for the version again.
fn next(shell: &mut Shell, sides: Sides) {
    let Some(session) = shell.current_work().filter(|s| !s.interacting()) else { return };
    let key = Key { sides, revision: session.revision() };
    let r = &shell.versions.compare_results;
    let held = r.held.as_ref().filter(|h| h.version == key.sides.0);
    let known = r.found.as_ref().is_some_and(|(shown, _, origin)| *shown == key || (shown.sides == key.sides && *origin == Origin::Read));
    if known || r.comparing.is_some() {
        return;
    }
    let Some(before) = held.map(|h| h.rows.clone()) else { return read_before(shell, key.sides.0) };
    // The frame only copies the objects; their rows are built with the comparison.
    let started = compare(before, session.doc().freeze(), key.sides.1);
    let r = &mut shell.versions.compare_results;
    match started {
        Ok(answer) => r.comparing = Some((key, answer)),
        Err(why) => r.found = Some((key, Err(why.to_string()), Origin::Comparison)),
    }
    r.held = r.held.take().filter(|h| h.keep);
}

/// Asks for the rows of `version`, taking turns with the details' reads: the host refuses reads past
/// its limit, and that would read as a failed comparison. A read still under way, whichever version
/// or project it is of, is waited for: one of this version lands as it answers, and the details'
/// two and this one leave the host room for the list's read or a view.
fn read_before(shell: &mut Shell, version: String) {
    let v = &shell.versions;
    if v.compare_results.reading.is_some() || v.details_comparing.is_some() || v.details_reading.is_some() {
        return;
    }
    let (Some(host), Some(of)) = (shell.services.projects.clone(), v.compare_results.about.clone()) else { return };
    let answer = host.read_version(&of.0, version.clone());
    shell.versions.compare_results.reading = Some((of, version, answer));
}

/// Builds the rows of `live` and compares them with `version` on a thread of its own, `live` first
/// when `swapped`; rows that cannot be built fail the comparison with their reason. The model's root
/// is no change of its own.
fn compare(version: Arc<Rows>, live: Frozen, swapped: bool) -> std::io::Result<Receiver<Found>> {
    let (tx, rx) = channel();
    let (tables, junctions) = archforge_notations::storage();
    let work = move || {
        let found = live.rows(&tables, &junctions).map_err(|e| e.to_string()).and_then(|live| {
            let (before, after) = if swapped { (&live, &*version) } else { (&*version, &live) };
            classified(before, after, crate::editing::fit_tables).map_err(|e| e.to_string())
        });
        // Model elements come first, then diagrams, then skills, each in the order found.
        let _ = tx.send(found.map(|mut items| {
            items.retain(|d| d.class != archforge_core::MODEL);
            items.sort_by_key(|d| d.area);
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
    let found = shell.versions.compare_results.found.as_ref().filter(|(key, ..)| key.sides == now).map(|(_, found, _)| found.clone());
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

/// How many differences there are, by area, and the cards in their groups (model elements, diagrams,
/// then skills), scrolling under that.
fn list(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, items: &[Difference]) {
    let (left, width) = column(r);
    let count = |area| items.iter().filter(|d| d.area == area).count();
    let (model, diagrams, skills) = (count(Area::Model), count(Area::Diagram), count(Area::Skill));
    let head = trn("{n} difference", "{n} differences", items.len() as u64);
    let shown = ux::text(ui.painter(), r.min + vec2(MARGIN, 0.0), &head, types::SECTION, p.text, Some(width));
    let _ = control(shell, ui, "workspace.version-compare.count", "status", shown, false).accessible_name(&head);
    let parts = [
        (model > 0).then(|| trn("{n} model element", "{n} model elements", model as u64)),
        (diagrams > 0).then(|| trn("{n} diagram", "{n} diagrams", diagrams as u64)),
        (skills > 0).then(|| trn("{n} skill", "{n} skills", skills as u64)),
    ];
    let breakdown = parts.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    ux::text(ui.painter(), pos2(left, r.top() + 32.0), &breakdown, types::SMALL, p.text_secondary, Some(width));
    let groups = [(tr("Model elements"), Area::Model, model), (tr("Diagrams"), Area::Diagram, diagrams), (tr("Skills"), Area::Skill, skills)];
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
