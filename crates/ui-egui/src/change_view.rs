//! A changed element opened in the centre (frames V1.1 and V3.3 of page 09): its name, what happened to it
//! and where. Its sides are built off the interface thread from rows already held, so a click reads nothing.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::{Uuid, rows::snapshot};
use archforge_notations::differences::{ChangeKind, Difference, ElementSides, element_sides};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::i18n::tr;
use crate::settings::{Look, button, control};
use crate::state::Explorer;
use crate::ux::{self, Palette, types};
use crate::version_details::{BAND, MARGIN, breadcrumb};
use crate::versions::{Compared, State, base_held, kind_look, last_known, last_version, working_key};

/// The changed element the centre shows: its row, which speaks for it until its sides are known, and what was last read of them.
pub(crate) struct OpenedElement {
    pub(crate) row: Difference,
    read_of: Option<Compared>,
    found: Option<Result<ElementSides, String>>,
}

/// The one read of an element's two sides under way, and what it is about.
pub(crate) struct ElementReading {
    pub(crate) id: Uuid,
    pub(crate) key: Compared,
    pub(crate) answer: Receiver<Result<Option<ElementSides>, String>>,
}

/// Opens the change `row` in place of a version's details and ends what was said of earlier work.
pub(crate) fn open(shell: &mut Shell, row: Difference) {
    (shell.versions.details, shell.versions.selected, shell.canvas.notice) = (None, None, None);
    let v = &mut shell.versions;
    if v.element.as_ref().is_none_or(|o| o.row.id != row.id) {
        v.element = Some(OpenedElement { row, read_of: None, found: None });
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
        Err(TryRecvError::Disconnected) => Err(tr("This element couldn't be compared.")),
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
fn land(v: &mut State, key: Compared, found: Result<ElementSides, String>) {
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
    let id = o.row.id;
    let (Some(before), Some(session)) = (base_held(v, &key.0), shell.session.as_ref()) else { return };
    let (tables, junctions) = archforge_notations::storage();
    let v = &mut shell.versions;
    let after = match snapshot(session.doc(), &tables, &junctions) {
        Ok(rows) => rows,
        Err(e) => return land(v, key, Err(e.to_string())),
    };
    let (tx, rx) = channel();
    let read = move || drop(tx.send(element_sides(&before, &after, id, crate::editing::fit_tables).map_err(|e| e.to_string())));
    match std::thread::Builder::new().name("archforge-element-sides".into()).spawn(read) {
        Ok(_) => v.element_reading = Some(ElementReading { id, key, answer: rx }),
        Err(e) => land(v, key, Err(e.to_string())),
    }
}

/// Draws the open element in `r` while Versions is the explorer; false leaves `r` to the canvas. Under
/// the header: why it could not be read, bars, or for an element on both sides the not-shown-yet note.
pub(crate) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> bool {
    let v = &shell.versions;
    let Some(o) = v.element.as_ref().filter(|_| shell.state.explorer == Explorer::SourceControl) else { return false };
    let row = v.changes.iter().filter(|d| d.id == o.row.id).max_by_key(|d| d.kind != ChangeKind::Removed).unwrap_or(&o.row).clone();
    let retry = matches!(o.found, Some(Err(_)));
    let found = o.found.clone().or_else(|| v.compare_failed.clone().map(Err));
    ui.painter().rect_filled(r, 0.0, p.page);
    if breadcrumb(shell, ui, p, r, "workspace.change-view.close", &tr("Working changes")) {
        shell.versions.element = None;
    }
    let (left, width) = (r.left() + MARGIN, (r.width() - 2.0 * MARGIN).max(0.0));
    let said = crate::canvas::notice(shell, ui, p, pos2(left, r.top() + BAND + 6.0), width);
    let sides = found.as_ref().and_then(|f| f.as_ref().ok());
    let card = sides.and_then(|s| s.after.as_ref().or(s.before.as_ref()));
    let name = card.map_or(row.name.as_ref(), |c| c.name.as_ref()).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| tr("Unnamed"));
    // From the two sides, as a change of table is one change, not the two rows the list has for it.
    let kind =
        sides.map(|s| s.before.as_ref().map_or(ChangeKind::Added, |_| if s.after.is_none() { ChangeKind::Removed } else { ChangeKind::Changed }));
    let y = title(shell, ui, p, (r, r.top() + BAND + 20.0 + (said - types::CAPTION.line).max(0.0)), (&name, &row.path.join(" / ")), kind);
    match found {
        Some(Err(why)) => {
            let (words, at) = (tr("This element couldn't be compared."), Rect::from_min_size(pos2(left, y), vec2((width - 80.0).max(0.0), 28.0)));
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
        Some(Ok(sides)) if sides.before.is_some() && sides.after.is_some() => {
            let words = tr("What changed inside this element isn't shown here yet.");
            let g = ux::galley_rows(ui.painter(), &words, types::UI, p.text, Some((width - 40.0).max(0.0)), 3);
            let at = Rect::from_min_size(pos2(left, y), vec2(width, g.size().y + 34.0));
            let _ = control(shell, ui, "workspace.change-view.note", "status", at, false).accessible_name(words);
            ui.painter().rect_filled(at, 8.0, p.surface);
            ui.painter().rect_stroke(at, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            ui.painter().galley(pos2(left + 20.0, y + 17.0), g, p.text);
        }
        Some(Ok(_)) => {}
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
