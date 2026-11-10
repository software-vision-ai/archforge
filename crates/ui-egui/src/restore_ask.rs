//! Restoring a version as the person meets it (`docs/design/versioning.md`, section 7, frames V4.1
//! to V4.4): when it is offered, the question that says what it will do, and what is said when it
//! did not finish. The restore itself is [`Shell::restore_version`].

use egui::{Key, Modifiers, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::close_ask::Ask;
use crate::hold::HoldRefused;
use crate::i18n::{fmt, tr};
use crate::settings::{Look, button};
use crate::ux::{self, Palette, types};
use crate::versions::{last_known, last_version};
use crate::{Shell, version_chip, widgets};

const WIDTH: f32 = 480.0;

/// The version a question is about and, once tried, why restoring it did not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    commit: String,
    /// When it was saved, as the list says it: the new version is named after this.
    saved: String,
    /// How the question names it.
    named: String,
    failure: Option<String>,
}

/// Whether restoring `commit` is on offer: not a version that can't be read, nor the last version
/// with nothing unsaved, which is what current work holds. A version equal to the work in any other
/// way is only found when it runs (section 7).
pub(crate) fn offered(shell: &Shell, commit: &str) -> bool {
    let v = &shell.versions;
    let listed = last_version(v).filter(|_| last_known(v)).is_some_and(|last| last == commit);
    let stepped = shell.version_list.steps.history.first().is_some_and(|last| last.hash == commit);
    !v.unavailable.contains_key(commit) && (shell.shown_unsaved() || !(listed || stepped))
}

/// Asks whether to restore the version `commit`, saved at `saved` and called `named`, about the
/// project and session shown now; with `failure`, says that the last try did not finish.
pub(crate) fn ask(shell: &mut Shell, (commit, saved, named): (String, String, String), failure: Option<String>) {
    if let Some(path) = shell.project.as_ref().map(|o| o.path.clone()) {
        shell.ask = Some(Ask::Restore { path, generation: shell.session_generation(), confirm: Confirm { commit, saved, named, failure } });
    }
}

/// Starts the restore asked for, its failure to come back here. Chat's question keeps it waiting;
/// refused, the reason goes where a refused view's does.
fn go(shell: &mut Shell, confirm: Confirm) {
    match shell.restore_version(&confirm.commit, &confirm.saved) {
        Ok(()) | Err(HoldRefused::ChatRunning) => shell.versions.restoring = Some(Confirm { failure: None, ..confirm }),
        Err(why) => {
            let words = version_chip::refused_words(shell, why);
            shell.canvas.say_refused_step(words);
        }
    }
}

/// Once a frame, after the replacement's step: a restore that is neither running nor waiting for
/// Chat's question is over.
pub(crate) fn keep(shell: &mut Shell) {
    if shell.replacing.is_none() && !matches!(shell.ask, Some(Ask::StopChat { .. })) {
        shell.versions.restoring = None;
    }
}

/// A restore started here ended without restoring: the question comes back and says why, in the
/// words `save_error` holds, unless another question is already up.
pub(crate) fn failed(shell: &mut Shell) {
    let Some(c) = shell.versions.restoring.take().filter(|_| shell.ask.is_none()) else { return };
    let why = shell.save_error.clone().unwrap_or_else(|| tr("The change did not finish, so the project is as it was."));
    ask(shell, (c.commit, c.saved, c.named), Some(why));
}

/// Draws the question over the window and acts on its answer. Enter does nothing here: a Restore
/// is chosen with the pointer or by moving to its button.
pub(crate) fn show(shell: &mut Shell, ui: &mut egui::Ui, confirm: Confirm) {
    let p = *ux::palette(ui.visuals().dark_mode);
    let window = ui.ctx().content_rect();
    let (mut restore, mut close) = (false, false);
    egui::Area::new(widgets::egui_id("restore-ask")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        ui.painter().rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("restore-ask.veil"), Sense::click_and_drag());
        (restore, close) = card(shell, ui, &p, window, &confirm);
        close |= ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    });
    if restore {
        shell.ask = None;
        go(shell, confirm);
    } else if close {
        shell.ask = None;
    }
}

/// The question's card, or its failure in place of what it would do; gives whether Restore or Retry
/// was clicked, and whether Cancel or Close was.
fn card(shell: &mut Shell, ui: &egui::Ui, p: &Palette, window: Rect, c: &Confirm) -> (bool, bool) {
    let paint = ui.painter().clone();
    let width = WIDTH.min(window.width() - 32.0).max(240.0);
    let inner = width - 40.0;
    let (inset, wrap) = if c.failure.is_some() { (10.0, inner - 20.0) } else { (0.0, inner) };
    let line = ux::galley_rows(&paint, &fmt(&tr("The version of {what}"), &[("what", &c.named)]), types::UI, p.text, Some(inner), 2);
    let small = |words: &str, ink| (ux::galley_rows(&paint, words, types::SMALL, ink, Some(wrap), 6), ink);
    let mut blocks = Vec::new();
    if let Some(why) = &c.failure {
        blocks.push(small(&tr("The restore did not finish"), p.danger));
        blocks.push(small(why, p.text));
    } else {
        // The name the host stores it under (`restored_message`), untranslated, as History shows it.
        let name = format!("Restored the version of {}", c.saved);
        let adds = fmt(&tr("A new version with that content is added on top of the history, called \"{name}\"."), &[("name", &name)]);
        blocks.push(small(&format!("{adds} {}", tr("Nothing is rewound or removed, and later versions stay in History.")), p.text_secondary));
        if shell.shown_unsaved() {
            let saves = tr("Your unsaved changes are saved first, as a version called \"Before restoring\" and the time.");
            blocks.push(small(&format!("{saves} {}", tr("You can reach them again from History.")), p.text));
        }
        blocks.push(small(&tr("Undo starts fresh after a Restore."), p.text_secondary));
    }
    let text: f32 = blocks.iter().map(|(g, _)| g.size().y + 6.0).sum::<f32>() + inset * 2.0;
    let frame = Rect::from_center_size(window.center(), vec2(width, 50.0 + line.size().y + 8.0 + text + 12.0 + 28.0 + 20.0));
    ux::shadow(&paint, frame, 10.0, p.shadow);
    paint.rect_filled(frame, 10.0, p.surface);
    paint.rect_stroke(frame, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "restore-ask", "dialog", frame);
    let left = frame.left() + 20.0;
    ux::text(&paint, pos2(left, frame.top() + 20.0), &tr("Restore this version?"), types::DIALOG_TITLE, p.text, Some(inner));
    let mut y = frame.top() + 50.0;
    let h = line.size().y;
    paint.galley(pos2(left, y), line, p.text);
    y += h + 8.0;
    if c.failure.is_some() {
        paint.rect_filled(Rect::from_min_size(pos2(left, y), vec2(inner, text)), 8.0, p.danger_tint);
        y += inset;
    }
    for (g, ink) in blocks {
        let h = g.size().y;
        paint.galley(pos2(left + inset, y), g, ink);
        y += h + 6.0;
    }
    // Trying again can't change that the version is the same.
    let same = c.failure.as_ref().is_some_and(|why| why.starts_with(&tr("This version is the same as your current work.")));
    let words = (!same).then(|| if c.failure.is_some() { tr("Retry") } else { tr("Restore") });
    let width_of = |text: &str| (ux::galley(&paint, text, types::UI, p.text, None).size().x + 20.0).ceil();
    let top = frame.bottom() - 48.0;
    let mut x = frame.right() - 20.0;
    let mut restore = false;
    if let Some(words) = &words {
        x -= width_of(words);
        restore = button(shell, ui, p, "restore-ask.restore", pos2(x, top), words, Look::Primary, true, None).1.clicked();
        x -= 8.0;
    }
    let close = if c.failure.is_some() { tr("Close") } else { tr("Cancel") };
    x -= width_of(&close);
    let look = if words.is_some() { Look::Secondary } else { Look::Primary };
    (restore, button(shell, ui, p, "restore-ask.close", pos2(x, top), &close, look, true, None).1.clicked())
}
