//! Naming a new branch (`docs/design/versioning.md`, sections 8 and 10, frames V5.2 and V5.9):
//! New branch… from current work, the name field with its refusal under it, and the switch to the
//! branch once the host has made it.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::close_ask::Ask;
use crate::i18n::{fmt, tr};
use crate::settings::{Look, button, control};
use crate::ux::{self, Palette, types};
use crate::{BranchEntry, BranchRefused, ProjectHost, Shell, widgets};

/// The most characters the field takes: more than a name may hold, so a long paste shows as too
/// long rather than cut.
const MOST: usize = 200;

/// What the dialog holds between frames.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Naming {
    pub(crate) text: String,
    /// The name has been typed in, so an empty one is said to be.
    edited: bool,
    /// Why the host refused the last try, until the name changes.
    refused: Option<String>,
    /// A creation is on its way.
    waiting: bool,
    /// The field has been given the keyboard; Enter gives it up, so it is given back.
    focused: bool,
}

/// A creation asked for: the project and session it was asked over, the name as stored, and the
/// host's answer on its way.
pub(crate) struct Making {
    about: (PathBuf, u64),
    name: String,
    answer: Receiver<Result<(), BranchRefused>>,
}

/// Whether New branch… is offered: the host keeps branches for the project shown, it is not held
/// by another operation, and no creation is on its way.
pub(crate) fn offered(shell: &Shell) -> bool {
    let host = shell.services.projects.as_ref().zip(shell.project.as_ref());
    shell.versions.making.is_none() && !shell.is_held() && host.is_some_and(|(h, o)| h.current_branch(&o.path).is_some())
}

/// Opens New branch… over the work shown, with the field empty.
pub(crate) fn open(shell: &mut Shell) {
    if let Some(path) = shell.project.as_ref().filter(|_| offered(shell)).map(|o| o.path.clone()) {
        shell.ask = Some(Ask::Branch { path, generation: shell.session_generation(), naming: Naming::default() });
    }
}

/// The name to store for `text`, or why it can't be one: the host's own refusal, or a branch
/// listed with that name in any case, as the project compares names.
fn checked(host: &dyn ProjectHost, listed: &[BranchEntry], text: &str) -> Result<String, String> {
    let name = host.branch_name(text)?;
    if listed.iter().any(|b| b.name.eq_ignore_ascii_case(&name)) {
        return Err(words(&BranchRefused::Taken));
    }
    Ok(name)
}

/// What a person is told of a refusal.
fn words(refused: &BranchRefused) -> String {
    match refused {
        BranchRefused::Taken => tr("That name is already in use."),
        BranchRefused::NotCleanedUp => tr("The last new branch is not cleaned up yet. Opening the project again removes it."),
        BranchRefused::Moved => tr("The project changed before the branch was made, so none was made."),
        BranchRefused::Unclear(_) => tr("Archforge could not tell whether the branch was made. Look at Branches before trying again."),
        BranchRefused::Failed(why) => fmt(&tr("Couldn't make the branch: {why}"), &[("why", why)]),
    }
}

/// Once a frame: takes the host's answer to a creation, and reads the branches again. Made, the
/// dialog closes and the project goes to the new branch; refused, the dialog says why under the
/// field. Either only while the dialog is up over the session it was asked over.
pub(crate) fn frame(shell: &mut Shell, ctx: &egui::Context) {
    let Some(making) = shell.versions.making.as_ref() else { return };
    let answer = match making.answer.try_recv() {
        Ok(answer) => answer,
        Err(TryRecvError::Empty) => return ctx.request_repaint_after(std::time::Duration::from_millis(100)),
        // The host may have made it before it stopped.
        Err(TryRecvError::Disconnected) => Err(BranchRefused::Unclear("the host stopped without an answer".into())),
    };
    let Some(Making { about, name, .. }) = shell.versions.making.take() else { return };
    ctx.request_repaint();
    shell.versions.branches_asked = false;
    let shown = shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()));
    let up = matches!(&shell.ask, Some(Ask::Branch { path, generation, .. }) if (path, *generation) == (&about.0, about.1));
    if !up || shown.as_ref() != Some(&about) {
        return;
    }
    match answer {
        Ok(()) => {
            shell.ask = None;
            if let Err(why) = crate::switch_ask::to_new_branch(shell, name) {
                let words = crate::version_chip::refused_words(shell, why);
                shell.canvas.say_refused_step(words);
            }
        }
        Err(refused) => {
            if let Some(Ask::Branch { naming, .. }) = shell.ask.as_mut() {
                // The field takes the keyboard back, so the name can be changed at once.
                (naming.refused, naming.waiting, naming.focused) = (Some(words(&refused)), false, false);
            }
        }
    }
}

/// Asks the host to make the branch stored as `name` from current work.
fn create(shell: &mut Shell, host: &dyn ProjectHost, name: String) {
    let Some(Ask::Branch { path, generation, naming }) = shell.ask.as_mut() else { return };
    let answer = host.create_branch(path, &name);
    shell.versions.making = Some(Making { about: (path.clone(), *generation), name, answer });
    (naming.waiting, naming.refused) = (true, None);
}

/// Draws the dialog over the window and acts on it. While a creation is on its way nothing closes
/// it and nothing more is asked.
pub(crate) fn show(shell: &mut Shell, ui: &mut egui::Ui, mut naming: Naming) {
    let (Some(host), Some(path)) = (shell.services.projects.clone(), shell.project.as_ref().map(|o| o.path.clone())) else {
        shell.ask = None;
        return;
    };
    let p = *ux::palette(ui.visuals().dark_mode);
    let window = ui.ctx().content_rect();
    let from = host.current_branch(&path).map(|b| host.branch_shown(&b)).unwrap_or_default();
    let listed = shell.versions.branches.clone().unwrap_or_default();
    let before = naming.text.clone();
    let mut asked = None;
    let mut close = false;
    egui::Area::new(widgets::egui_id("branch-ask")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        ui.ctx().memory_mut(|m| m.set_modal_layer(ui.layer_id()));
        ui.painter().rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("branch-ask.veil"), Sense::click_and_drag());
        let name = |text: &str| checked(host.as_ref(), &listed, text);
        (asked, close) = card(shell, ui, &p, (window, &from), &mut naming, name);
        close |= !naming.waiting && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    });
    if naming.text != before {
        (naming.edited, naming.refused) = (true, None);
    }
    if let Some(Ask::Branch { naming: kept, .. }) = shell.ask.as_mut() {
        *kept = naming;
    }
    if close {
        shell.ask = None;
    } else if let Some(name) = asked {
        create(shell, host.as_ref(), name);
    }
}

/// The dialog's card: what a branch is, where it starts, the name field and what is wrong with the
/// name. Gives the name to make, when Create or Enter asks for it, and whether Cancel was clicked.
fn card(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    (window, from): (Rect, &str),
    n: &mut Naming,
    name: impl Fn(&str) -> Result<String, String>,
) -> (Option<String>, bool) {
    let paint = ui.painter().clone();
    let width = 480.0_f32.min(window.width() - 32.0).max(240.0);
    let inner = width - 40.0;
    let start = if shell.shown_unsaved() {
        tr("It starts from current work on {branch}. Your unsaved changes come along, and {branch} keeps them too.")
    } else {
        tr("It starts from current work on {branch}.")
    };
    let what = tr("A branch keeps its own versions, unsaved work and conversations.");
    let lead = ux::galley_rows(&paint, &what, types::SMALL, p.text_secondary, Some(inner), 3);
    let starts = ux::galley_rows(&paint, &fmt(&start, &[("branch", from)]), types::SMALL, p.text, Some(inner), 4);
    let checked = name(&n.text);
    // The host's refusal of the name typed, else what is wrong with it once it has been typed in.
    let problem = n.refused.clone().or_else(|| checked.clone().err().filter(|_| n.edited));
    let (said, ink) = problem.map_or_else(|| (fmt(&tr("At most {n} characters."), &[("n", "60")]), p.text_muted), |why| (why, p.danger));
    let hint = ux::galley_rows(&paint, &said, types::CAPTION, ink, Some(inner), 3);
    let height = 46.0 + lead.size().y + 8.0 + starts.size().y + 14.0 + 49.0 + hint.size().y + 16.0 + 28.0 + 20.0;
    let frame = Rect::from_center_size(window.center(), vec2(width, height));
    ux::shadow(&paint, frame, 10.0, p.shadow);
    paint.rect_filled(frame, 10.0, p.surface);
    paint.rect_stroke(frame, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "branch-ask", "dialog", frame);
    let left = frame.left() + 20.0;
    ux::text(&paint, pos2(left, frame.top() + 20.0), &tr("New branch"), types::DIALOG_TITLE, p.text, Some(inner));
    let mut y = frame.top() + 46.0;
    for (g, ink, gap) in [(lead, p.text_secondary, 8.0), (starts, p.text, 14.0)] {
        let h = g.size().y;
        paint.galley(pos2(left, y), g, ink);
        y += h + gap;
    }
    let field = "branch-ask.name";
    let enter = crate::panels::text_field(shell, ui, p, pos2(left, y), inner, field, &tr("Name"), &mut n.text, MOST, !n.focused, false);
    n.focused = !enter;
    y += 49.0;
    let at = Rect::from_min_size(pos2(left, y), hint.size());
    let _ = control(shell, ui, "branch-ask.hint", "status", at, false).accessible_name(said);
    paint.galley(at.min, hint, ink);
    // Of the text as typed this frame, so Create never makes a name the field no longer shows.
    let live = (!n.waiting).then(|| name(&n.text).ok()).flatten();
    let top = frame.bottom() - 48.0;
    let width_of = |text: &str| (ux::galley(&paint, text, types::UI, p.text, None).size().x + 20.0).ceil();
    let at = pos2(frame.right() - 20.0 - width_of(&tr("Create")), top);
    let (_, create) = button(shell, ui, p, "branch-ask.create", at, &tr("Create"), Look::Primary, live.is_some(), None);
    let at = pos2(at.x - 8.0 - width_of(&tr("Cancel")), top);
    let (_, cancel) = button(shell, ui, p, "branch-ask.cancel", at, &tr("Cancel"), Look::Secondary, !n.waiting, None);
    (live.filter(|_| create.clicked() || enter), cancel.clicked())
}
