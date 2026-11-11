//! Switching branch as the person meets it (`docs/design/versioning.md`, section 8, frames V5.4 and
//! V5.9): unsaved work asks first, none switches at once, and a switch that ended without arriving
//! says why, where the project is and what became of the work. The switch is [`Shell::switch_branch`].

use std::path::PathBuf;

use egui::{Key, Modifiers, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::close_ask::Ask;
use crate::hold::HoldRefused;
use crate::i18n::{fmt, tr};
use crate::leaving::Leaving;
use crate::settings::{Look, button};
use crate::ux::{self, Palette, types};
use crate::{Shell, version_chip, widgets};

/// The switch a question is about and, once tried, how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    /// The branch asked for, as stored.
    branch: String,
    /// That branch's name and the current one's, as shown.
    target: String,
    from: String,
    /// Chat's question was answered on the way here, so the switch doesn't ask it again.
    after_chat: bool,
    /// How it ended without arriving: whether its work could not be written, and what is said.
    failure: Option<(bool, String)>,
}

/// A switch started here, kept until it arrives or says why not.
pub(crate) struct Switching {
    path: PathBuf,
    confirm: Confirm,
    /// It saves the work first.
    save_first: bool,
}

impl Shell {
    /// Goes to the branch stored as `branch` as a branch row asks to: refused by
    /// [`Self::clear_to_leave`] with nothing changed, asking first when the work shown is unsaved,
    /// and otherwise switching at once. A Chat turn running asks before all of that.
    pub fn ask_switch_branch(&mut self, branch: String) -> Result<(), HoldRefused> {
        let after_chat = self.chat_answered;
        self.clear_to_leave(Some(Leaving::AskSwitch(branch.clone())))?;
        let (Some(host), Some(path)) = (self.services.projects.clone(), self.project.as_ref().map(|p| p.path.clone())) else {
            return Err(HoldRefused::NoProject);
        };
        let from = host.current_branch(&path).ok_or(HoldRefused::NoProject)?;
        let confirm = Confirm { target: host.branch_shown(&branch), from: host.branch_shown(&from), branch, after_chat, failure: None };
        if self.shown_unsaved() {
            self.ask = Some(Ask::Switch { path, generation: self.session_generation(), confirm });
            return Ok(());
        }
        go(self, confirm, false)
    }
}

/// Starts the switch asked for, its ending to come back here. One that doesn't start says why in
/// its result; Chat's question for a turn begun after this question leaves it to the status bar.
fn go(shell: &mut Shell, confirm: Confirm, save_first: bool) -> Result<(), HoldRefused> {
    let Some(path) = shell.project.as_ref().map(|p| p.path.clone()) else { return Err(HoldRefused::NoProject) };
    shell.chat_answered = confirm.after_chat;
    shell.start_switch(confirm.branch.clone(), save_first)?;
    shell.versions.switching = Some(Switching { path, confirm, save_first });
    Ok(())
}

/// Once a frame, after the replacement's step: a switch that began and has ended without arriving
/// comes back saying how it stood when it ended, waiting while another question is up. One that
/// arrived is no longer kept, nor is one whose project no longer shows.
pub(crate) fn keep(shell: &mut Shell) {
    if shell.replacing.is_some() {
        return;
    }
    let Some(mut s) = shell.versions.switching.take() else { return };
    let Some(path) = shell.project.as_ref().map(|o| o.path.clone()).filter(|p| *p == s.path) else { return };
    if s.confirm.failure.is_none() {
        let refused = shell.refused.is_some() && shell.refused == shell.session.as_ref().map(|x| x.revision());
        let why = shell.save_error.clone().unwrap_or_else(|| tr("The switch did not finish, so the project stayed on its branch."));
        // A Save that answered marked the session saved; one that failed left it unsaved.
        let template = if refused {
            tr("{why} Your work could not be kept, so the switch was not made. You are still on {branch}.")
        } else if s.save_first && !shell.shown_unsaved() {
            tr("{why} You are still on {branch}. Your work is saved.")
        } else {
            tr("{why} You are still on {branch} and your work is unchanged.")
        };
        s.confirm.failure = Some((refused, fmt(&template, &[("why", &why), ("branch", &s.confirm.from)])));
    }
    if shell.ask.is_some() {
        shell.versions.switching = Some(s);
        return;
    }
    shell.ask = Some(Ask::Switch { path, generation: shell.session_generation(), confirm: s.confirm });
}

/// Draws the question or the failure over the window and acts on the answer. Enter does nothing
/// here: an answer is chosen with the pointer or by moving to its button.
pub(crate) fn show(shell: &mut Shell, ui: &mut egui::Ui, confirm: Confirm) {
    let p = *ux::palette(ui.visuals().dark_mode);
    let window = ui.ctx().content_rect();
    let mut answer = None;
    egui::Area::new(widgets::egui_id("switch-ask")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        ui.painter().rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("switch-ask.veil"), Sense::click_and_drag());
        answer = card(shell, ui, &p, window, &confirm);
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            answer = Some(None);
        }
    });
    let Some(answer) = answer else { return };
    shell.ask = None;
    if let Some(save_first) = answer
        && let Err(why) = go(shell, confirm, save_first)
    {
        let words = version_chip::refused_words(shell, why);
        shell.canvas.say_refused_step(words);
    }
}

/// The question's card, or the failure's; gives the answer clicked: whether to save first, or none
/// for Cancel or Close.
fn card(shell: &mut Shell, ui: &egui::Ui, p: &Palette, window: Rect, c: &Confirm) -> Option<Option<bool>> {
    let paint = ui.painter().clone();
    let width = 480.0_f32.min(window.width() - 32.0).max(240.0);
    let inner = width - 40.0;
    let target = ("branch", c.target.as_str());
    let unsaved =
        tr("{branch} has unsaved changes. They stay on {branch} and are there when you come back. You won't be able to undo earlier steps.");
    let (title, ink, tint, body, body_ink) = match &c.failure {
        None => (fmt(&tr("Switch to {branch}?"), &[target]), p.text, None, fmt(&unsaved, &[("branch", &c.from)]), p.text_secondary),
        Some((true, why)) => (tr("Switching is refused"), p.amber, Some(p.amber_tint), why.clone(), p.text),
        Some((_, why)) => (fmt(&tr("Couldn't switch to {branch}"), &[target]), p.danger, Some(p.danger_tint), why.clone(), p.text),
    };
    let inset = if tint.is_some() { 14.0 } else { 0.0 };
    let wrap = inner - inset * 2.0;
    let head = ux::galley_rows(&paint, &title, types::DIALOG_TITLE, ink, Some(wrap), 2);
    let text = ux::galley_rows(&paint, &body, types::SMALL, body_ink, Some(wrap), 8);
    let block = head.size().y + 10.0 + text.size().y + inset * 2.0;
    let frame = Rect::from_center_size(window.center(), vec2(width, 20.0 + block + 18.0 + 28.0 + 20.0));
    ux::shadow(&paint, frame, 10.0, p.shadow);
    paint.rect_filled(frame, 10.0, p.surface);
    paint.rect_stroke(frame, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "switch-ask", "dialog", frame);
    let (left, top) = (frame.left() + 20.0, frame.top() + 20.0);
    if let Some(tint) = tint {
        paint.rect_filled(Rect::from_min_size(pos2(left, top), vec2(inner, block)), 8.0, tint);
    }
    let text_top = top + inset + head.size().y + 10.0;
    paint.galley(pos2(left + inset, top + inset), head, ink);
    paint.galley(pos2(left + inset, text_top), text, body_ink);
    let buttons: Vec<(&str, String, Option<bool>, Look)> = if c.failure.is_some() {
        vec![("switch-ask.close", tr("Close"), None, Look::Primary)]
    } else {
        vec![
            ("switch-ask.save", tr("Save version and switch"), Some(true), Look::Primary),
            ("switch-ask.switch", tr("Switch"), Some(false), Look::Secondary),
            ("switch-ask.cancel", tr("Cancel"), None, Look::Secondary),
        ]
    };
    let mut x = frame.right() - 20.0;
    let mut picked = None;
    for (id, words, answer, look) in buttons {
        x -= (ux::galley(&paint, &words, types::UI, p.text, None).size().x + 20.0).ceil();
        if button(shell, ui, p, id, pos2(x, frame.bottom() - 48.0), &words, look, true, None).1.clicked() {
            picked = Some(answer);
        }
        x -= 8.0;
    }
    picked
}
