//! The Save version dialog (AR-251, page 09 of the design): a review of what changed, a description
//! and a save of exactly the state reviewed. The review is tied to the session's revision: when
//! that moves the dialog says so, and Save waits until the summary is of the rows now shown.

use std::path::PathBuf;

use archforge_notations::differences::Area;
use egui::{Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::{fmt, tr, trn};
use crate::settings::{Look, button};
use crate::ux::{self, Palette, icons, types};
use crate::versions::{last_saved, settled};
use crate::{SaveState, Shell, widgets};

const WIDTH: f32 = 480.0;
/// The most characters of a description: the design's.
const MOST: usize = 200;

/// The dialog while it is open.
pub struct Dialog {
    /// The project and session it reviews.
    about: (PathBuf, u64),
    pub(crate) text: String,
    /// The session's revision the summary is of.
    reviewed: u64,
    /// The model changed after the dialog opened.
    moved: bool,
    /// The save asked for, while it runs.
    ticket: Option<u64>,
    /// Why the last try did not save; what was typed stays.
    error: Option<String>,
    /// The field has been given the keyboard; Enter gives it up, so it is given back.
    focused: bool,
}

/// What the dialog is about now: the project shown and its session.
fn about(shell: &Shell) -> Option<(PathBuf, u64)> {
    shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()))
}

/// Why Save version… is off, when it is: it needs current work with settled changes to save.
pub(crate) fn off_because(shell: &Shell) -> Option<String> {
    if shell.viewing.is_some() {
        Some(tr("Back to current work to save a version"))
    } else if shell.is_saving() {
        Some(tr("Saving…"))
    } else if shell.is_held() || !settled(shell) {
        Some(tr("Loading"))
    } else if shell.versions.compare_failed.is_some() {
        Some(tr("The changes couldn't be compared."))
    } else if shell.versions.working == 0 {
        Some(tr("No changes since the last version."))
    } else {
        None
    }
}

/// Opens the dialog on the state now shown, when Save version… is on.
pub(crate) fn open(shell: &mut Shell) {
    let (None, Some(about), Some(session)) = (off_because(shell), about(shell), shell.session.as_ref()) else { return };
    let dialog = Dialog { about, text: String::new(), reviewed: session.revision(), moved: false, ticket: None, error: None, focused: false };
    shell.versions.dialog = Some(dialog);
}

/// Closes the dialog; the button that opened it takes the keyboard back when next drawn.
fn close(shell: &mut Shell) {
    (shell.versions.dialog, shell.versions.focus_save) = (None, true);
}

/// Once per frame, whatever is drawn: the dialog goes with its project, session or a hold, follows
/// a model that moved once the summary is of it, and takes how its save went.
pub(crate) fn frame(shell: &mut Shell) {
    let now = about(shell);
    let (is_settled, revision) = (settled(shell) && shell.versions.compare_failed.is_none(), shell.session.as_ref().map(|s| s.revision()));
    let gone = shell.is_held() || shell.viewing.is_some();
    let Some(d) = shell.versions.dialog.as_mut() else { return };
    if Some(&d.about) != now.as_ref() || gone {
        shell.versions.dialog = None;
        return;
    }
    if let (None, true, Some(revision)) = (d.ticket, is_settled, revision)
        && d.reviewed != revision
    {
        (d.reviewed, d.moved, d.error) = (revision, true, None);
    }
    let Some(ticket) = d.ticket else { return };
    let failed = match shell.save_state(ticket) {
        SaveState::Running => return,
        SaveState::Saved(_) => return close(shell),
        SaveState::Failed(why) => why,
        SaveState::Unknown | SaveState::Forgotten => tr("Archforge could not tell whether a version was made. Look at History before trying again."),
    };
    if let Some(d) = shell.versions.dialog.as_mut() {
        (d.ticket, d.error) = (None, Some(failed));
    }
}

/// Whether Save can go: a description, the summary of the rows now shown, and nothing saving.
fn can_save(shell: &Shell, d: &Dialog) -> bool {
    !d.text.trim().is_empty() && settled(shell) && shell.versions.working > 0 && d.ticket.is_none() && !shell.is_saving()
}

/// Saves what was reviewed, under the description typed.
fn save(shell: &mut Shell) {
    let Some(d) = shell.versions.dialog.as_ref().filter(|d| can_save(shell, d)) else { return };
    let (text, reviewed, before) = (d.text.trim().to_string(), d.reviewed, shell.save_ticket);
    shell.save_reviewed(text, reviewed);
    let asked = (shell.save_ticket != before).then_some(shell.save_ticket);
    if let Some(d) = shell.versions.dialog.as_mut() {
        (d.ticket, d.error) = (asked, None);
    }
}

/// What the review says changed, and since when.
fn summary(shell: &Shell) -> String {
    let count = |area| shell.versions.changes.iter().filter(|d| d.area == area).count() as u64;
    let (model, diagrams) = (count(Area::Model), count(Area::Diagram));
    let models = trn("{n} model element", "{n} model elements", model);
    let shown = trn("{n} diagram", "{n} diagrams", diagrams);
    let what = match (model, diagrams) {
        (0, 0) if shell.versions.compare_failed.is_some() => return tr("The changes couldn't be compared."),
        (0, 0) => return tr("No changes since the last version."),
        (_, 0) => models,
        (0, _) => shown,
        _ => fmt(&tr("{first} and {second}"), &[("first", &models), ("second", &shown)]),
    };
    match last_saved(&shell.versions, crate::chat_panel::clock(shell)) {
        Some(when) => fmt(&tr("{what} changed since {when}."), &[("what", &what), ("when", &when)]),
        None => fmt(&tr("{what} changed."), &[("what", &what)]),
    }
}

/// The dialog over the window, when it is open: drawn after the window behind it, as the keys held
/// back from that window are given back for it.
pub(crate) fn dialog(shell: &mut Shell, ui: &mut egui::Ui) {
    if shell.versions.dialog.is_none() {
        return;
    }
    let p = *ux::palette(shell.state.theme.is_dark());
    let window = ui.ctx().content_rect();
    let shown =
        egui::Area::new(widgets::egui_id("workspace.versions.dialog")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
            ui.set_clip_rect(window);
            ui.ctx().memory_mut(|m| m.set_modal_layer(ui.layer_id()));
            body(shell, ui, &p, window)
        });
    if shown.inner {
        close(shell);
    }
}

/// What the dialog holds; true when it should close, which waits for a save under way to answer.
fn body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, window: Rect) -> bool {
    let Some(d) = shell.versions.dialog.as_ref() else { return true };
    let (moved, error, focused, idle, mut text) = (d.moved, d.error.clone(), d.focused, d.ticket.is_none(), d.text.clone());
    let paint = ui.painter().clone();
    paint.rect_filled(window, 0.0, p.scrim);
    ui.interact(window, widgets::egui_id("workspace.versions.dialog.veil"), Sense::click_and_drag());
    let width = WIDTH.min(window.width() - 32.0).max(240.0);
    let inner = width - 40.0;
    let name = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
    let lead = fmt(&tr("These changes become a new version of {name}."), &[("name", &name)]);
    let intro = ux::galley_rows(&paint, &lead, types::SMALL, p.text_secondary, Some(inner), 2);
    let said = ux::galley_rows(&paint, &summary(shell), types::UI, p.text, Some(inner), 3);
    let notice = moved.then(|| tr("The model changed while this was open. The summary is up to date again."));
    let warned = error.clone().map(|e| (e, p.danger, p.danger_tint)).or(notice.map(|n| (n, p.text, p.amber_tint)));
    let warning = warned.as_ref().map(|(w, ink, _)| ux::galley_rows(&paint, w, types::SMALL, *ink, Some(inner - 44.0), 4));
    // The design gives the intro way to a warning, and the hint to a failure.
    let (hint, hint_gap) =
        if error.is_some() { (String::new(), 0.0) } else { (tr("One line, at most 200 characters. It stays in the history."), 20.0) };
    let extra = warning.as_ref().map_or(intro.size().y + 8.0, |g| g.size().y + 12.0 + hint_gap);
    let height = 20.0 + 26.0 + said.size().y + 14.0 + 45.0 + 22.0 + 12.0 + extra + 28.0 + 20.0;
    let frame = Rect::from_center_size(window.center(), vec2(width, height));
    ux::shadow(&paint, frame, 10.0, p.shadow);
    paint.rect_filled(frame, 10.0, p.surface);
    paint.rect_stroke(frame, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "workspace.versions.dialog", "dialog", frame);
    let left = frame.left() + 20.0;
    ux::text(&paint, pos2(left, frame.top() + 20.0), &tr("Save version"), types::DIALOG_TITLE, p.text, Some(inner));
    let mut y = frame.top() + 46.0;
    for (g, ink, gap) in [(intro, p.text_secondary, 8.0), (said, p.text, 14.0)].into_iter().skip(usize::from(warned.is_some())) {
        let h = g.size().y;
        paint.galley(pos2(left, y), g, ink);
        y += h + gap;
    }
    let field = "workspace.versions.dialog.description";
    let enter = crate::panels::text_field(shell, ui, p, pos2(left, y), inner, field, &tr("Description"), &mut text, MOST, !focused, false);
    y += 49.0;
    ux::text(&paint, pos2(left, y), &hint, types::CAPTION, p.text_muted, Some(inner));
    y += 8.0 + hint_gap;
    if let (Some(g), Some((_, ink, tint))) = (warning, warned) {
        let area = Rect::from_min_size(pos2(left, y), vec2(inner, g.size().y + 20.0));
        widgets::record_visible(ui, &mut shell.registry, "workspace.versions.dialog.warning", "status", area);
        paint.rect_filled(area, 8.0, tint);
        if error.is_none() {
            ux::icon_in(&paint, Rect::from_center_size(pos2(area.left() + 17.0, area.center().y), Vec2::splat(14.0)), &icons::WARNING, ink);
        }
        paint.galley(pos2(area.left() + if error.is_some() { 12.0 } else { 32.0 }, area.top() + 10.0), g, ink);
    }
    if let Some(d) = shell.versions.dialog.as_mut() {
        (d.text, d.focused) = (text, !enter);
    }
    let live = shell.versions.dialog.as_ref().is_some_and(|d| can_save(shell, d));
    let label = if error.is_some() { tr("Retry") } else { tr("Save version") };
    let top = frame.bottom() - 48.0;
    let width_of = |text: &str| (ux::galley(&paint, text, types::UI, p.text, None).size().x + 20.0).ceil();
    let at = pos2(frame.right() - 20.0 - width_of(&label), top);
    let (_, saved) = button(shell, ui, p, "workspace.versions.dialog.save", at, &label, Look::Primary, live, None);
    let at = pos2(at.x - 8.0 - width_of(&tr("Cancel")), top);
    let (_, cancel) = button(shell, ui, p, "workspace.versions.dialog.cancel", at, &tr("Cancel"), Look::Secondary, idle, None);
    if live && (saved.clicked() || enter) {
        save(shell);
    }
    idle && (cancel.clicked() || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)))
}
