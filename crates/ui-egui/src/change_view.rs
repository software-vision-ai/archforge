//! A changed element or diagram opened in the centre (frames V1.1, V3.3 and V3.4 of page 09): its name, what happened
//! to it, where, and its two sides. Those are built off the interface thread from rows already held, so a click reads nothing.

mod cards;
mod diagram;
mod related;
mod summary;

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex, PoisonError};

use archforge_mof::{Uuid, rows::snapshot};
use archforge_notations::differences::{Area, ChangeKind, DiagramSides, Difference, ElementSides, diagram_sides, element_sides};
use egui::{Rect, pos2, vec2};

use crate::Shell;
use crate::explorer::TreeScroll;
use crate::i18n::{fmt, tr};
use crate::settings::{Look, button, control};
use crate::state::Explorer;
use crate::ux::{self, Palette, types};
use crate::version_details::{BAND, MARGIN, breadcrumb};
use crate::versions::{Compared, State, base_held, kind_look, last_known, last_version, path_words, when, working_key};

/// The two sides the centre shows: an element's cards, or a diagram's drawings.
#[derive(Clone)]
pub(crate) enum Sides {
    Element(ElementSides),
    /// Behind a lock as a store can move between threads but not be shared; only the interface thread takes it.
    Diagram(Arc<Mutex<DiagramSides>>),
}

/// The changed element the centre shows: its row, which speaks for it until its sides are known, and what was last read of them.
pub(crate) struct OpenedElement {
    pub(crate) row: Difference,
    read_of: Option<Compared>,
    found: Option<Result<Sides, String>>,
    scroll: TreeScroll,
    /// The zoom and pan a diagram's two sides are drawn with once the person has moved either; none while they fit.
    nav: Option<(f32, egui::Vec2)>,
}

/// The one read of an element's two sides under way, and what it is about.
pub(crate) struct ElementReading {
    pub(crate) id: Uuid,
    pub(crate) key: Compared,
    pub(crate) answer: Receiver<Result<Option<Sides>, String>>,
}

/// Opens the change `row` in place of a version's details and ends what was said of earlier work.
/// Opening the one shown again keeps what was read of it, and fits a diagram's two sides again.
pub(crate) fn open(shell: &mut Shell, row: Difference) {
    (shell.versions.details, shell.versions.selected, shell.canvas.notice) = (None, None, None);
    let v = &mut shell.versions;
    match v.element.as_mut() {
        Some(o) if o.row.id == row.id => o.nav = None,
        _ => v.element = Some(OpenedElement { row, read_of: None, found: None, scroll: TreeScroll::default(), nav: None }),
    }
}

/// Once a frame: takes the answer, closes an element the list no longer holds, starts the next read
/// and wakes the window while one is awaited. A failed list says nothing of the element: that closes nothing.
pub(crate) fn keep(shell: &mut Shell, ctx: &egui::Context) {
    take_answer(shell);
    let v = &mut shell.versions;
    if v.compare_failed.is_none() && v.element.as_ref().is_some_and(|o| !v.changes.iter().any(|d| d.id == o.row.id)) {
        v.element = None;
    }
    start_read(shell);
    if shell.versions.element_reading.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Lands the read's answer on its own element and last version. Rows that have only moved on still
/// show what they were, or edits would hide every answer; a failure or absence waits for the next read.
fn take_answer(shell: &mut Shell) {
    let Some(reading) = &shell.versions.element_reading else { return };
    let answer = match reading.answer.try_recv() {
        Err(TryRecvError::Empty) => return,
        Ok(answer) => answer,
        Err(TryRecvError::Disconnected) => Err(failed_words(shell.versions.element.as_ref().map_or(Area::Model, |o| o.row.area))),
    };
    let (id, key) = (reading.id, reading.key.clone());
    let moved = shell.session.as_ref().map(|s| s.revision()) != Some(key.1);
    let v = &mut shell.versions;
    v.element_reading = None;
    if !last_known(v) || key.0 != last_version(v) || v.element.as_ref().is_none_or(|o| o.row.id != id) {
        return;
    }
    match answer {
        Ok(Some(sides)) => land(v, key, Ok(sides)),
        Ok(None) if !moved => v.element = None,
        Err(why) if !moved => land(v, key, Err(why)),
        _ => {}
    }
}

/// Keeps what a read found, or why it could not tell, with what it was about.
fn land(v: &mut State, key: Compared, found: Result<Sides, String>) {
    if let Some(o) = v.element.as_mut() {
        (o.read_of, o.found) = (Some(key), Some(found));
    }
}

/// Builds the open element's sides off the interface thread when none runs and they are not of the
/// rows of now; the last version's rows are waited for, as the list asks for them.
fn start_read(shell: &mut Shell) {
    let Some(key) = working_key(shell) else { return };
    let v = &shell.versions;
    let Some(o) = v.element.as_ref().filter(|o| v.element_reading.is_none() && o.read_of.as_ref() != Some(&key)) else { return };
    let (id, area) = (o.row.id, o.row.area);
    let (Some(before), Some(session)) = (base_held(v, &key.0), shell.session.as_ref()) else { return };
    let (tables, junctions) = archforge_notations::storage();
    let v = &mut shell.versions;
    let after = match snapshot(session.doc(), &tables, &junctions) {
        Ok(rows) => rows,
        Err(e) => return land(v, key, Err(e.to_string())),
    };
    let (tx, rx) = channel();
    let read = move || {
        let repair = crate::editing::fit_tables;
        let sides = match area {
            Area::Model | Area::Skill => element_sides(&before, &after, id, repair).map(|s| s.map(Sides::Element)),
            Area::Diagram => diagram_sides(&before, &after, id, repair).map(|s| s.map(|s| Sides::Diagram(Arc::new(Mutex::new(s))))),
        };
        drop(tx.send(sides.map_err(|e| e.to_string())));
    };
    match std::thread::Builder::new().name("archforge-element-sides".into()).spawn(read) {
        Ok(_) => v.element_reading = Some(ElementReading { id, key, answer: rx }),
        Err(e) => land(v, key, Err(e.to_string())),
    }
}

/// Draws the open element in `r` while Versions is the explorer; false leaves `r` to the canvas. Under
/// the header: why it could not be read, bars, or its two cards.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> bool {
    let v = &shell.versions;
    let Some(o) = v.element.as_ref().filter(|_| shell.state.explorer == Explorer::SourceControl) else { return false };
    let row = v.changes.iter().filter(|d| d.id == o.row.id).max_by_key(|d| d.kind != ChangeKind::Removed).unwrap_or(&o.row).clone();
    let retry = matches!(o.found, Some(Err(_)));
    let found = o.found.clone().or_else(|| v.compare_failed.clone().map(Err));
    let clock = crate::chat_panel::clock(shell);
    let listed = |h: &String| v.page.iter().flat_map(|l| &l.versions).find(|e| e.hash == *h);
    // A read that failed compared nothing, so the header names no version.
    let against = o.read_of.as_ref().filter(|_| !retry).and_then(|key| match &key.0 {
        None => Some(tr("There is no version to compare with yet.")),
        Some(hash) => listed(hash).map(|e| fmt(&tr("Compared with the version of {when}"), &[("when", &when(&e.date, clock))])),
    });
    ui.painter().rect_filled(r, 0.0, p.page);
    let (closed, crumb_end) = breadcrumb(shell, ui, p, r, "workspace.change-view.close", &tr("Working changes"));
    if closed {
        shell.versions.element = None;
    }
    if let Some(words) = against {
        let g = ux::galley(ui.painter(), &words, types::SMALL, p.text_secondary, Some((r.right() - MARGIN - crumb_end - 16.0).max(0.0)));
        let at = Rect::from_min_size(pos2(r.right() - MARGIN - g.size().x, r.top() + 34.0 - g.size().y / 2.0), g.size());
        let _ = control(shell, ui, "workspace.change-view.compared", "status", at, false).on_hover_text(&words).accessible_name(&words);
        ui.painter().galley(at.min, g, p.text_secondary);
    }
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    let said = crate::canvas::notice(shell, ui, p, pos2(left, r.top() + BAND + 6.0), width);
    let (name, path, kind) = heading(&row, found.as_ref().and_then(|f| f.as_ref().ok()));
    let name = name.filter(|n| !n.is_empty()).unwrap_or_else(|| tr("Unnamed"));
    let y = title(shell, ui, p, (r, r.top() + BAND + 20.0 + (said - types::CAPTION.line).max(0.0)), (&name, &path), kind);
    match found {
        Some(Err(why)) => {
            let words = failed_words(row.area);
            let at = Rect::from_min_size(pos2(left, y), vec2((width - 80.0).max(0.0), 28.0));
            let _ = control(shell, ui, "workspace.change-view.failed", "status", at, false).on_hover_text(why).accessible_name(&words);
            ux::text_mid(ui.painter(), left, at.center().y, &words, types::SMALL, p.danger, Some(at.width()));
            let id = "workspace.change-view.retry";
            if retry
                && button(shell, ui, p, id, pos2(at.right() + 8.0, y), &tr("Retry"), Look::Secondary, true, None).1.clicked()
                && let Some(o) = shell.versions.element.as_mut()
            {
                (o.read_of, o.found) = (None, None);
            }
        }
        Some(Ok(Sides::Element(sides))) => cards::draw(shell, ui, p, r, y, &sides),
        Some(Ok(Sides::Diagram(sides))) => diagram::draw(shell, ui, p, r, y, &sides.lock().unwrap_or_else(PoisonError::into_inner)),
        None => {
            let _ = control(shell, ui, "workspace.change-view.loading", "status", Rect::from_min_size(pos2(left, y), vec2(width, 66.0)), false)
                .accessible_name(tr("Loading"));
            for (i, share) in [1.0, 0.78, 0.9].into_iter().enumerate() {
                ui.painter().rect_filled(Rect::from_min_size(pos2(left, y + i as f32 * 22.0), vec2(width * share, 11.0)), 4.0, p.border);
            }
        }
    }
    true
}

/// What the centre says when a read of a change in `area` fails.
fn failed_words(area: Area) -> String {
    if area == Area::Diagram { tr("This diagram couldn't be compared.") } else { tr("This element couldn't be compared.") }
}

/// The name, path words and change the header says: of `sides` once known, of `row` until then. The change
/// comes from the sides, as a change of table is one change, not the two rows the list has for it.
fn heading(row: &Difference, sides: Option<&Sides>) -> (Option<String>, String, Option<ChangeKind>) {
    let kind = |before: bool, after: bool| match (before, after) {
        (false, _) => ChangeKind::Added,
        (true, true) => ChangeKind::Changed,
        (true, false) => ChangeKind::Removed,
    };
    match sides {
        Some(Sides::Element(s)) => {
            let card = s.after.as_ref().or(s.before.as_ref());
            (card.map_or(row.name.clone(), |c| c.name.clone()), path_words(row), Some(kind(s.before.is_some(), s.after.is_some())))
        }
        Some(Sides::Diagram(d)) => {
            let d = d.lock().unwrap_or_else(PoisonError::into_inner);
            (d.name.clone(), d.path.join(" / "), Some(kind(d.before.is_some(), d.after.is_some())))
        }
        None => (row.name.clone(), path_words(row), None),
    }
}

/// The title from `y` with the pill beside it and the path under it, read whole on hover; gives where the next part starts.
fn title(shell: &mut Shell, ui: &egui::Ui, p: &Palette, (r, y): (Rect, f32), (name, path): (&str, &str), kind: Option<ChangeKind>) -> f32 {
    let (left, width, paint) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0), ui.painter().clone());
    let look = kind.map(|k| kind_look(k, p));
    let pill = look.as_ref().map(|(what, ink, _)| ux::galley(&paint, what, types::CAPTION, *ink, None));
    let g = ux::galley_rows(&paint, name, types::PAGE_TITLE, p.text, Some((width - pill.as_ref().map_or(0.0, |g| g.size().x + 52.0)).max(0.0)), 3);
    let (end, height) = (left + g.size().x, g.size().y);
    paint.galley(pos2(left, y), g, p.text);
    if let (Some((_, ink, tint)), Some(g)) = (&look, pill) {
        let area = Rect::from_min_size(pos2(end + 30.0, y + 3.0), vec2(g.size().x + 22.0, 24.0));
        paint.rect_filled(area, 12.0, *tint);
        paint.galley(area.center() - g.size() / 2.0, g, *ink);
    }
    let shown = ux::text(&paint, pos2(left, y + height + 13.0), path, types::SMALL, p.text_secondary, Some(width));
    let whole = look.map_or_else(|| format!("{name}, {path}"), |(what, _, _)| format!("{name}, {path}, {what}"));
    let area = Rect::from_min_max(pos2(left, y), pos2(left + width, shown.bottom()));
    let _ = control(shell, ui, "workspace.change-view.title", "status", area, false).on_hover_text(&whole).accessible_name(whole);
    shown.bottom() + 18.0
}
