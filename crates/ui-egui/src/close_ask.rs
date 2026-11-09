//! Closing and quitting with unsaved changes: the question a project's closing asks.
//! Unsaved changes ask Save, Don't save or Cancel (owner decision); Don't save puts the project
//! back to its last save. A project whose changes could not be written offers Close without
//! writing, which asks once more before the changes not yet written are lost (owner decision);
//! what was written stays, unsaved, for the next opening. Quitting asks about each unsaved
//! project in turn; Cancel stops quitting.

use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::ux::{self, Palette, types};
use crate::{Shell, widgets};

/// The question open over the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// The project holds changes not yet saved.
    Unsaved { path: std::path::PathBuf },
    /// The project's changes could not be written, for `why`.
    CannotWrite { path: std::path::PathBuf, why: String },
    /// Close without writing, asked once more.
    ConfirmLoss { path: std::path::PathBuf },
    /// A close or a quit waited for a hold that failed, with nothing unsaved: the person looks first.
    HoldFailed { path: std::path::PathBuf, held: String },
}

impl Ask {
    /// The project the question is about.
    fn path(&self) -> &std::path::Path {
        match self {
            Ask::Unsaved { path } | Ask::CannotWrite { path, .. } | Ask::ConfirmLoss { path } | Ask::HoldFailed { path, .. } => path,
        }
    }
}

/// What a button of the question does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Save,
    DontSave,
    CloseWithoutWriting,
    LoseChanges,
    GoOn,
    Cancel,
}

/// Draws the question, when one is open, over everything, and acts on its answer.
pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    let Some(ask) = shell.ask.clone() else { return };
    // The question belongs to a project still open; one gone takes it along.
    let Some(i) = shell.tab_of(ask.path()) else {
        shell.ask = None;
        return;
    };
    let name = shell.tabs[i].name.clone();
    let quitting = shell.quitting();
    let (title, lines, buttons): (String, Vec<String>, Vec<(&str, String, Answer, bool)>) = match &ask {
        Ask::Unsaved { .. } => (
            fmt(&tr("Save the changes to {name}?"), &[("name", &name)]),
            vec![tr("If you don't save, the changes since the last save are lost.")],
            vec![
                ("close-ask.save", tr("Save"), Answer::Save, true),
                ("close-ask.cancel", tr("Cancel"), Answer::Cancel, false),
                ("close-ask.discard", tr("Don't save"), Answer::DontSave, false),
            ],
        ),
        Ask::CannotWrite { why, .. } => (
            fmt(&tr("The changes to {name} cannot be written"), &[("name", &name)]),
            vec![why.clone(), tr("It stays open until its changes are written, or you close it without them.")],
            vec![
                ("close-ask.keep", tr("Keep open"), Answer::Cancel, false),
                ("close-ask.close-anyway", tr("Close without writing"), Answer::CloseWithoutWriting, false),
            ],
        ),
        Ask::ConfirmLoss { .. } => (
            fmt(&tr("Close {name} without writing?"), &[("name", &name)]),
            vec![tr("The changes since the last write are lost.")],
            vec![
                ("close-ask.confirm", tr("Close and lose changes"), Answer::LoseChanges, true),
                ("close-ask.cancel", tr("Cancel"), Answer::Cancel, false),
            ],
        ),
        Ask::HoldFailed { held, .. } => (
            if quitting { tr("Quit Archforge?") } else { fmt(&tr("Close {name}?"), &[("name", &name)]) },
            vec![fmt(&tr("The last change to {name} did not finish. Check it before you go on."), &[("name", held)])],
            vec![
                ("close-ask.go-on", if quitting { tr("Quit Archforge") } else { tr("Close") }, Answer::GoOn, true),
                ("close-ask.keep", tr("Keep open"), Answer::Cancel, false),
            ],
        ),
    };
    let p = *crate::ux::palette(ui.visuals().dark_mode);
    let window = ui.ctx().content_rect();
    let mut answer = None;
    egui::Area::new(widgets::egui_id("close-ask")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        let paint = ui.painter().clone();
        paint.rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("close-ask.veil"), Sense::click_and_drag());
        answer = body(shell, ui, &p, &paint, window, &title, &lines, &buttons);
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            answer = Some(Answer::Cancel);
        }
    });
    let Some(answer) = answer else { return };
    shell.ask = None;
    match answer {
        Answer::Save => shell.save_and_close(i),
        Answer::DontSave => shell.discard_tab(i, true),
        Answer::LoseChanges => shell.discard_tab(i, false),
        Answer::CloseWithoutWriting => shell.ask = Some(Ask::ConfirmLoss { path: ask.path().to_path_buf() }),
        // Quitting goes on by itself, with nothing left to ask about.
        Answer::GoOn if quitting => {}
        Answer::GoOn => shell.close_tab_now(i),
        Answer::Cancel => {
            shell.holding.forget_quit();
            if quitting {
                shell.cancel_quit();
            }
        }
    }
}

/// The question's card: its title, its lines and its buttons, the first primary; gives the answer
/// clicked.
#[allow(clippy::too_many_arguments, reason = "the card is drawn from its texts and buttons over the window")]
fn body(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    paint: &egui::Painter,
    window: Rect,
    title: &str,
    lines: &[String],
    buttons: &[(&str, String, Answer, bool)],
) -> Option<Answer> {
    let width = 460.0_f32.min(window.width() - 32.0).max(240.0);
    let wrapped: Vec<_> = lines.iter().map(|l| ux::galley(paint, l, types::UI, p.text_secondary, Some(width - 40.0))).collect();
    let text_height: f32 = wrapped.iter().map(|g| g.size().y + 6.0).sum();
    let size = vec2(width, 120.0 + text_height);
    let dialog = Rect::from_center_size(window.center(), size);
    ux::shadow(paint, dialog, 10.0, p.shadow);
    paint.rect_filled(dialog, 10.0, p.surface);
    paint.rect_stroke(dialog, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    ux::text(paint, dialog.min + vec2(20.0, 20.0), title, types::CARD_TITLE, p.text, Some(width - 40.0));
    let mut y = dialog.top() + 58.0;
    for g in wrapped {
        let h = g.size().y;
        paint.galley(pos2(dialog.left() + 20.0, y), g, p.text_secondary);
        y += h + 6.0;
    }
    // Buttons from the right: the primary last on the right, Don't save apart on the left.
    let mut x = dialog.right() - 20.0;
    let mut picked = None;
    for (id, label, answer, primary) in buttons {
        let danger = matches!(answer, Answer::LoseChanges);
        // Laid out in the ink it is drawn in: a galley keeps the colour it was laid out with, so
        // the primary's label is the white on its fill.
        let ink = if danger || *primary { p.on_accent } else { p.text };
        let g = ux::galley(paint, label, types::SMALL, ink, None);
        let w = g.size().x + 24.0;
        let left_aligned = *answer == Answer::DontSave;
        let b = if left_aligned {
            Rect::from_min_size(pos2(dialog.left() + 20.0, dialog.bottom() - 48.0), vec2(w, 28.0))
        } else {
            let r = Rect::from_min_max(pos2(x - w, dialog.bottom() - 48.0), pos2(x, dialog.bottom() - 20.0));
            x = r.left() - 8.0;
            r
        };
        let fill = if danger {
            p.danger
        } else if *primary {
            p.accent_fill
        } else {
            p.field
        };
        paint.rect_filled(b, 6.0, fill);
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, ink);
        if control(shell, ui, id, "button", b, true).clicked() {
            picked = Some(*answer);
        }
    }
    picked
}
