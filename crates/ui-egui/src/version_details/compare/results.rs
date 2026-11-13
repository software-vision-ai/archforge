//! What the Compare page finds under its band (frame V3.2 of page 09 of the design and section 6 of
//! `docs/design/versioning.md`): a version read at its revision against current work or another
//! version read at its own, off the interface thread; another branch's side is its latest version,
//! read on that branch's line. The frame only copies current work's objects;
//! their rows are built with the comparison. Comparing reads rows only, so the session is never
//! replaced. While a version shows, current work is the session set aside.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::Frozen;
use archforge_mof::compare::CompareError;
use archforge_mof::rows::Rows;
use archforge_notations::differences::{Area, ClassifyError, Difference, classified};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use super::Side;
use crate::explorer::{TreeScroll, scroll_tree};
use crate::i18n::{tr, trn};
use crate::settings::{Look, button, control};
use crate::ux::{self, Palette, types};
use crate::version_details::{Found, MARGIN, about, not_compared, scroll_keys};
use crate::version_load::{MOST_KEPT_BYTES, bytes_of};
use crate::{Shell, VersionRead, VersionReadError};

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
/// Stop's automation id, and the room it takes above an earlier answer while a comparison runs.
const STOP: &str = "workspace.version-compare.stop";
const STOP_ROW: f32 = 36.0;

/// The Before and the After side compared: a version, or current work as none. Never both current
/// work, nor one version on both sides.
pub(crate) type Sides = (Option<String>, Option<String>);

/// What a comparison is of: its sides, and the revision of current work it took; 0 for two
/// versions, which never move.
#[derive(Clone, PartialEq)]
pub(crate) struct Key {
    pub(crate) sides: Sides,
    pub(crate) revision: u64,
}

/// A version's rows; `keep` is false for rows too large to hold, beside the others held, past the
/// comparison they start.
pub(crate) struct Held {
    pub(crate) version: String,
    pub(crate) rows: Arc<Rows>,
    pub(crate) keep: bool,
}

/// Which step gave what is found: differences always come from a comparison, a failure from either.
/// A version that cannot be shown, a version or comparison past its budget and a comparison the
/// person stopped each say so in their own words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Origin {
    Read,
    Comparison,
    Unavailable,
    TooLarge,
    Stopped,
}

impl Origin {
    /// Whether what it found stays for its sides, whatever current work's revision, until Retry or
    /// another pair: a read again, or a comparison started again, would only say the same, spend
    /// the budget again or undo the person's stop. Only a failed comparison gives way to an edit.
    fn settled(self) -> bool {
        matches!(self, Origin::Read | Origin::Unavailable | Origin::TooLarge | Origin::Stopped)
    }
}

/// Why a comparison has no differences to give, and which kind of failure that is.
pub(crate) struct Failure {
    pub(crate) why: String,
    pub(crate) origin: Origin,
}

impl From<String> for Failure {
    /// Words alone are a comparison that failed, which Retry may mend.
    fn from(why: String) -> Self {
        Self { why, origin: Origin::Comparison }
    }
}

/// What a comparison's thread answers.
pub(crate) type Answer = Result<Arc<Vec<Difference>>, Failure>;

/// The page's one read and one comparison, and what the last of them found, with which one it was.
/// The read, of a version in a project and session, is never let go before it answers: a host read
/// cannot be called back and counts against the host's limit until it ends, so Compare never has
/// more than this one under way, whatever was opened and closed meanwhile.
#[derive(Default)]
pub(crate) struct Results {
    about: Option<(PathBuf, u64)>,
    pub(crate) held: Vec<Held>,
    pub(crate) reading: Option<((PathBuf, u64), String, Receiver<VersionRead>)>,
    pub(crate) comparing: Option<(Key, Receiver<Answer>)>,
    pub(crate) found: Option<(Key, Found, Origin)>,
    /// How far the list has scrolled; a list of other sides starts at its top.
    pub(crate) scroll: TreeScroll,
}

/// The sides the page compares now; none without the page, or for a pair with nothing to compare:
/// current work or one version on both sides.
fn sides(shell: &Shell) -> Option<Sides> {
    let pair = shell.versions.details.as_ref()?.compare.as_ref()?;
    Some((pair.before.version().cloned(), pair.after.version().cloned())).filter(|(before, after)| before != after)
}

/// Whether both of `sides` are versions, which a later revision of current work cannot change.
fn two_versions(sides: &Sides) -> bool {
    sides.0.is_some() && sides.1.is_some()
}

/// The revision a comparison of `sides` stands at: current work's, or 0 for two versions.
fn revision_of(sides: &Sides, work: Option<u64>) -> Option<u64> {
    if two_versions(sides) { Some(0) } else { work }
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
        (r.held, r.found) = (Vec::new(), None);
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
    let (now, work) = (sides(shell), shell.current_work().map(|s| s.revision()));
    let r = &mut shell.versions.compare_results;
    if let Some(answer) = r.reading.as_ref().and_then(|(.., rx)| answered(rx))
        && let Some((of, version, _)) = r.reading.take()
        && let Some(sides) = now.clone().filter(|s| s.0.as_ref() == Some(&version) || s.1.as_ref() == Some(&version))
        && r.about.as_ref() == Some(&of)
    {
        match answer {
            Ok(rows) => {
                // Only rows of the sides shown stay, and those kept past a comparison fit the bytes allowed together.
                r.held.retain(|h| [&sides.0, &sides.1].contains(&&Some(h.version.clone())));
                let kept = r.held.iter().filter(|h| h.keep).map(|h| bytes_of(&h.rows)).fold(bytes_of(&rows), usize::saturating_add);
                r.held.push(Held { version, keep: kept <= MOST_KEPT_BYTES, rows });
            }
            // The person stopped these sides: a failure says nothing until Retry, which reads again.
            Err(_) if stopped(r, &sides) => {}
            Err(why) => {
                // Asking again cannot make a version of another branch, a newer Archforge or past the read budget readable.
                let origin = match why {
                    VersionReadError::Failed(_) => Origin::Read,
                    VersionReadError::TooLarge => Origin::TooLarge,
                    VersionReadError::Unavailable | VersionReadError::Shape => Origin::Unavailable,
                };
                r.found = revision_of(&sides, work).map(|revision| (Key { sides, revision }, Err(why.to_string()), origin));
            }
        }
    }
    if let Some(answer) = r.comparing.as_ref().and_then(|(_, rx)| answered(rx))
        && let Some((key, _)) = r.comparing.take()
        && now.as_ref() == Some(&key.sides)
        && !stopped(r, &key.sides)
        && (answer.is_ok() || revision_of(&key.sides, work) == Some(key.revision))
    {
        if r.found.as_ref().is_none_or(|(shown, ..)| shown.sides != key.sides) {
            r.scroll = TreeScroll::default();
        }
        r.found = Some(match answer {
            Ok(items) => (key, Ok(items), Origin::Comparison),
            Err(Failure { why, origin }) => (key, Err(why), origin),
        });
    }
}

/// Whether the person stopped comparing `sides`: until Retry or another pair, nothing that comes
/// for them is shown.
fn stopped(r: &Results, sides: &Sides) -> bool {
    r.found.as_ref().is_some_and(|(shown, _, origin)| shown.sides == *sides && *origin == Origin::Stopped)
}

/// Compares the newest rows when what is shown is not of them. One comparison runs at a time, so
/// revisions that move while it runs fold into the next; nothing is compared with current work while
/// an edit is under way. What is shown for these sides is known at its own revision, whatever it is.
/// At a later one, differences and a failed comparison give way: the rows held are compared again,
/// or, if they went as too large, read again first, Before first. Two versions never move, so what
/// is known of them stays. A failed read, a version that cannot be shown and a comparison stopped stay
/// known until Retry clears them, or each edit would ask the host for the version again.
fn next(shell: &mut Shell, sides: Sides) {
    let work = shell.current_work().filter(|s| !s.interacting()).map(|s| s.revision());
    let Some(revision) = revision_of(&sides, work) else { return };
    let key = Key { sides, revision };
    let r = &shell.versions.compare_results;
    let known = r.found.as_ref().is_some_and(|(shown, _, origin)| *shown == key || (shown.sides == key.sides && origin.settled()));
    if known || r.comparing.is_some() {
        return;
    }
    let held = |v: &String| r.held.iter().find(|h| h.version == *v).map(|h| Got::Rows(h.rows.clone()));
    if let Some(unread) = [&key.sides.0, &key.sides.1].into_iter().flatten().find(|v| held(v).is_none()) {
        return read_side(shell, unread.clone());
    }
    // The frame only copies current work's objects; their rows are built with the comparison.
    let got = |side: &Option<String>| side.as_ref().map_or_else(|| shell.current_work().map(|s| Got::Live(Box::new(s.doc().freeze()))), held);
    let (Some(before), Some(after)) = (got(&key.sides.0), got(&key.sides.1)) else { return };
    let started = compare(before, after);
    let r = &mut shell.versions.compare_results;
    match started {
        Ok(answer) => r.comparing = Some((key, answer)),
        Err(why) => r.found = Some((key, Err(why.to_string()), Origin::Comparison)),
    }
    r.held.retain(|h| h.keep);
}

/// Asks for the rows of `version`, taking turns with the details' reads: the host refuses reads past
/// its limit, and that would read as a failed comparison. A read still under way, whichever version
/// or project it is of, is waited for: one of this version lands as it answers, and the details'
/// two and this one leave the host room for the list's read or a view. Another branch's version is
/// read on that branch's line, as the current line does not hold it: by its own name while the
/// branches last read still list it, else by a listed branch's that has that version, as one
/// renamed meanwhile does.
fn read_side(shell: &mut Shell, version: String) {
    let v = &shell.versions;
    if v.compare_results.reading.is_some() || v.details_comparing.is_some() || v.details_reading.is_some() {
        return;
    }
    let (Some(host), Some(of)) = (shell.services.projects.clone(), v.compare_results.about.clone()) else { return };
    let pair = v.details.as_ref().and_then(|d| d.compare.as_ref());
    let branch = pair.into_iter().flat_map(|p| [&p.before, &p.after]).find_map(|side| match side {
        Side::Branch(b) if b.latest_version.as_ref() == Some(&version) => Some(b.name.clone()),
        _ => None,
    });
    let listed = v.branches.iter().flatten();
    let gone = |held: &String| !listed.clone().any(|b| b.name == *held);
    let renamed = listed.clone().find(|b| !b.current && b.latest_version.as_ref() == Some(&version));
    let branch = branch.map(|held| if gone(&held) { renamed.map_or(held, |b| b.name.clone()) } else { held });
    let answer = match branch {
        Some(branch) => host.read_branch_version(&of.0, &branch, version.clone()),
        None => host.read_version(&of.0, version.clone()),
    };
    shell.versions.compare_results.reading = Some((of, version, answer));
}

/// One side's rows for a comparison: a version's as read, or current work's objects, whose rows are
/// built on the comparison's thread.
enum Got {
    Rows(Arc<Rows>),
    Live(Box<Frozen>),
}

/// Compares `before` with `after` on a thread of its own, building current work's rows there; rows
/// that cannot be built fail the comparison with their reason, and one past the comparison's budget
/// is too large, with no partial list. The model's root is no change of its own.
fn compare(before: Got, after: Got) -> std::io::Result<Receiver<Answer>> {
    let (tx, rx) = channel();
    let (tables, junctions) = archforge_notations::storage();
    let work = move || {
        let rows = |side| match side {
            Got::Rows(rows) => Ok(rows),
            Got::Live(live) => live.rows(&tables, &junctions).map(Arc::new).map_err(|e| Failure::from(e.to_string())),
        };
        let found = rows(before)
            .and_then(|before| Ok((before, rows(after)?)))
            .and_then(|(before, after)| classified(&before, &after, crate::editing::fit_tables).map_err(failure));
        // Model elements come first, then diagrams, then skills, each in the order found.
        let _ = tx.send(found.map(|mut items| {
            items.retain(|d| d.class != archforge_core::MODEL);
            items.sort_by_key(|d| d.area);
            Arc::new(items)
        }));
    };
    std::thread::Builder::new().name("archforge-version-compare".into()).spawn(work).map(|_| rx)
}

/// What a comparison that could not classify says: too large past its budget, a failure otherwise.
pub(crate) fn failure(e: ClassifyError) -> Failure {
    let origin = if matches!(e, ClassifyError::Compare(CompareError::TooLarge { .. })) { Origin::TooLarge } else { Origin::Comparison };
    Failure { why: e.to_string(), origin }
}

/// Draws what the pair compares in `r`: the differences, or bars, or why there are none to list. A
/// failure is said before any count, so it never reads as no differences. While a comparison of the
/// pair runs with an earlier answer shown, as after an edit, Stop is above that answer.
pub(super) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let Some(pair) = shell.versions.details.as_ref().and_then(|d| d.compare.clone()) else { return };
    let Some(now) = sides(shell) else {
        let head = if pair.before == Side::CurrentWork { tr("Both sides are your current work.") } else { tr("These two versions are the same.") };
        return say(shell, ui, p, r, &head, &tr("Choose another pair above."));
    };
    let results = &shell.versions.compare_results;
    let found = results.found.as_ref().filter(|(key, ..)| key.sides == now).map(|(_, found, origin)| (found.clone(), *origin));
    let running = results.comparing.as_ref().is_some_and(|(key, _)| key.sides == now) && !stopped(results, &now);
    let r = if running && found.is_some() {
        if button(shell, ui, p, STOP, pos2(column(r).0, r.top()), &tr("Stop"), Look::Secondary, true, None).1.clicked() {
            stop(shell, now.clone());
        }
        Rect::from_min_max(pos2(r.left(), r.top() + STOP_ROW), r.max)
    } else {
        r
    };
    match found {
        None => loading(shell, ui, p, r, now),
        Some((Err(why), Origin::Unavailable)) => say(shell, ui, p, r, &tr("This version can't be shown"), &why),
        Some((Err(_), Origin::TooLarge)) => say(shell, ui, p, r, &tr("Too large to compare"), &tr("Choose another pair above.")),
        Some((Err(why), Origin::Stopped)) => failed(shell, ui, p, r, (&tr("The comparison was stopped."), &why)),
        Some((Err(why), _)) => failed(shell, ui, p, r, (&not_compared(), &why)),
        Some((Ok(items), _)) if items.is_empty() => {
            let head = if two_versions(&now) { tr("These two versions are the same.") } else { tr("This version is the same as your current work.") };
            say(shell, ui, p, r, &head, &tr("Choose another pair above."))
        }
        Some((Ok(items), _)) => list(shell, ui, p, r, &items),
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

/// Bars in place of the cards while the comparison of `sides` is under way, and Stop under them.
fn loading(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, sides: Sides) {
    let (left, width) = column(r);
    let area = Rect::from_min_size(pos2(left, r.top()), vec2(width, SKELETON_CARDS as f32 * PITCH));
    let _ = control(shell, ui, "workspace.version-compare.loading", "status", area, false).accessible_name(tr("Loading"));
    for i in 0..SKELETON_CARDS {
        ui.painter().rect_filled(Rect::from_min_size(pos2(left, r.top() + i as f32 * PITCH), vec2(width, CARD)), 8.0, p.border);
    }
    if button(shell, ui, p, STOP, pos2(left, area.bottom() + 2.0), &tr("Stop"), Look::Secondary, true, None).1.clicked() {
        stop(shell, sides);
    }
}

/// Stops comparing `sides`: the page says it stopped until Retry. A comparison under way, whose work
/// its budget bounds, runs on with its answer dropped, so none other starts before it ends; one
/// still running at Retry lands then. A read under way cannot be called back, so its rows are held.
fn stop(shell: &mut Shell, sides: Sides) {
    let Some(revision) = revision_of(&sides, shell.current_work().map(|s| s.revision())) else { return };
    shell.versions.compare_results.found = Some((Key { sides, revision }, Err(String::new()), Origin::Stopped));
}

/// That the pair was not compared, in `words`, why on hover when there is a why, and Retry.
fn failed(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, (words, why): (&str, &str)) {
    let (left, width) = column(r);
    let at = Rect::from_min_size(pos2(left, r.top()), vec2((width - 80.0).max(0.0), 28.0));
    let said = control(shell, ui, "workspace.version-compare.failed", "status", at, false);
    let _ = if why.is_empty() { said } else { said.on_hover_text(why) }.accessible_name(words);
    ux::text_mid(ui.painter(), left, at.center().y, words, types::SMALL, p.danger, Some(at.width()));
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
    let explorer = shell.explorer.focused;
    scroll_keys(ui, explorer, &mut shell.versions.compare_results.scroll, PITCH, (tree.bottom() - top - 8.0).max(1.0));
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
