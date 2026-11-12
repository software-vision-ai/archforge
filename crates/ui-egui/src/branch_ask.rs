//! Naming a branch (`docs/design/versioning.md`, sections 8 and 10, frames V5.2, V5.7 to V5.9):
//! New branch… from current work and the switch to it once the host has made it, or Rename… from a
//! branch row's menu, each with the name field and its refusal under it; and Delete… from that
//! menu, saying what goes with the branch, and why it stays when it does.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::close_ask::Ask;
use crate::i18n::{fmt, tr, trn};
use crate::settings::{Look, button, control};
use crate::ux::{self, Palette, icons, types};
use crate::{BranchCounts, BranchEntry, BranchRefused, ProjectHost, Shell, widgets};

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
    /// The branch being renamed, as stored; none while a new branch is named.
    renaming: Option<String>,
    /// A rename to this name, stored and as shown, whose outcome couldn't be told: only asking for
    /// it again settles it, so it is the one name offered, and a branch listed under it doesn't
    /// count as taking it.
    unsettled: Option<(String, String)>,
    /// The branch to delete, in place of naming one.
    deleting: Option<Doomed>,
}

/// A branch asked to be deleted: its name as stored and as shown, and what goes with it once read,
/// or why that couldn't be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Doomed {
    name: String,
    shown: String,
    counts: Option<Result<BranchCounts, String>>,
}

/// The host's count of what goes with the branch the dialog deletes, on its way. Opening Delete…
/// again replaces it, and a dialog gone takes no answer.
pub(crate) type Counting = Receiver<Result<BranchCounts, String>>;

/// A creation or rename asked for: the project and session it was asked over, the name as stored,
/// the branch renamed (none for a creation), and the host's answer on its way.
pub(crate) struct Making {
    about: (PathBuf, u64),
    name: String,
    renaming: Option<String>,
    /// It deletes the branch `name`.
    deleting: bool,
    answer: Receiver<Result<(), BranchRefused>>,
}

/// Whether New branch… and Rename… are offered: the host keeps branches for the project shown, it
/// is not held by another operation, and no creation or rename is on its way.
pub(crate) fn offered(shell: &Shell) -> bool {
    let host = shell.services.projects.as_ref().zip(shell.project.as_ref());
    shell.versions.making.is_none() && !shell.is_held() && host.is_some_and(|(h, o)| h.current_branch(&o.path).is_some())
}

/// Opens New branch… over the work shown, with the field empty.
pub(crate) fn open(shell: &mut Shell) {
    ask(shell, Naming::default());
}

/// Opens Rename… for the branch `b` over the work shown, the field holding its name.
pub(crate) fn open_rename(shell: &mut Shell, b: &BranchEntry) {
    ask(shell, Naming { text: b.shown.clone(), renaming: Some(b.name.clone()), ..Naming::default() });
}

/// Opens Delete… for the branch `b`, and reads what goes with it.
pub(crate) fn open_delete(shell: &mut Shell, b: &BranchEntry) {
    let doomed = Doomed { name: b.name.clone(), shown: b.shown.clone(), counts: None };
    if let (Some(path), Some(host)) = (ask(shell, Naming { deleting: Some(doomed), ..Naming::default() }), shell.services.projects.clone()) {
        shell.versions.counting = Some(host.branch_counts(&path, &b.name));
    }
}

/// Puts the dialog holding `naming` over the work shown, where it is offered; gives the project.
fn ask(shell: &mut Shell, naming: Naming) -> Option<PathBuf> {
    let path = shell.project.as_ref().filter(|_| offered(shell)).map(|o| o.path.clone())?;
    shell.ask = Some(Ask::Branch { path: path.clone(), generation: shell.session_generation(), naming });
    Some(path)
}

/// The name to store for `text`, or why it can't be one: the host's own refusal, or a branch
/// listed with that name in any case, as the project compares names, or a rename to settle first.
/// Neither the branch `renaming` nor the name of a rename to settle is counted as taken.
fn checked(
    host: &dyn ProjectHost,
    listed: &[BranchEntry],
    (renaming, unsettled): (Option<&String>, Option<&(String, String)>),
    text: &str,
) -> Result<String, String> {
    let name = host.branch_name(text)?;
    if let Some((_, shown)) = unsettled.filter(|(to, _)| *to != name) {
        return Err(pending(shown));
    }
    let mine = renaming == Some(&name) || unsettled.is_some();
    if !mine && listed.iter().any(|b| b.name.eq_ignore_ascii_case(&name)) {
        return Err(words(&BranchRefused::Taken, None));
    }
    Ok(name)
}

/// What a person is told of a rename to `shown` whose outcome couldn't be told.
fn pending(shown: &str) -> String {
    fmt(&tr("Archforge could not tell whether the branch was renamed to {name}. Rename it to {name} again to find out."), &[("name", shown)])
}

/// What a person is told of a refusal; of a rename when `renaming` names the name, as shown, whose
/// outcome is unsettled or would be.
fn words(refused: &BranchRefused, renaming: Option<&str>) -> String {
    match (refused, renaming.is_some()) {
        (BranchRefused::Taken, _) => tr("That name is already in use."),
        (BranchRefused::NotCleanedUp, _) => tr("The last new branch is not cleaned up yet. Opening the project again removes it."),
        (BranchRefused::Moved, false) => tr("The project changed before the branch was made, so none was made."),
        (BranchRefused::Moved, true) => tr("The project changed before the branch was renamed, so its name stayed."),
        (BranchRefused::Unclear(_), false) => tr("Archforge could not tell whether the branch was made. Look at Branches before trying again."),
        (BranchRefused::Unclear(_), true) => pending(renaming.unwrap_or_default()),
        (BranchRefused::Failed(why), false) => fmt(&tr("Couldn't make the branch: {why}"), &[("why", why)]),
        (BranchRefused::Failed(why), true) => fmt(&tr("Couldn't rename the branch: {why}"), &[("why", why)]),
    }
}

/// What a person is told of a delete refused, or whose outcome couldn't be told.
fn undeleted(refused: &BranchRefused) -> String {
    match refused {
        BranchRefused::Moved => tr("The project changed before the branch was deleted, so it stays."),
        BranchRefused::Unclear(_) => tr("Archforge could not tell whether the branch was deleted. Delete it again to find out."),
        other => fmt(&tr("The branch stays: {why}"), &[("why", &other.to_string())]),
    }
}

/// Takes the host's count of what goes with a branch asked to be deleted, for its dialog still up.
fn counted(shell: &mut Shell, ctx: &egui::Context) {
    let Some(read) = shell.versions.counting.as_ref() else { return };
    let counts = match read.try_recv() {
        Ok(counts) => counts,
        Err(TryRecvError::Empty) => return ctx.request_repaint_after(std::time::Duration::from_millis(100)),
        Err(TryRecvError::Disconnected) => Err(tr("the host stopped without an answer")),
    };
    shell.versions.counting = None;
    ctx.request_repaint();
    if let Some(Ask::Branch { naming: Naming { deleting: Some(doomed), .. }, .. }) = shell.ask.as_mut() {
        doomed.counts = Some(counts);
    }
}

/// Once a frame: takes the host's count for a delete, and its answer to a creation, a rename or a
/// delete, and reads the branches again. Done, the dialog closes and a new branch is gone to;
/// refused, the dialog says why. Either only while the dialog is up over the session it was asked
/// over.
pub(crate) fn frame(shell: &mut Shell, ctx: &egui::Context) {
    counted(shell, ctx);
    let Some(making) = shell.versions.making.as_ref() else { return };
    let answer = match making.answer.try_recv() {
        Ok(answer) => answer,
        Err(TryRecvError::Empty) => return ctx.request_repaint_after(std::time::Duration::from_millis(100)),
        // The host may have made it before it stopped.
        Err(TryRecvError::Disconnected) => Err(BranchRefused::Unclear("the host stopped without an answer".into())),
    };
    let Some(Making { about, name, renaming, deleting, .. }) = shell.versions.making.take() else { return };
    ctx.request_repaint();
    shell.versions.branches_asked = false;
    let shown = shell.project.as_ref().map(|o| (o.path.clone(), shell.session_generation()));
    let up = matches!(&shell.ask, Some(Ask::Branch { path, generation, .. }) if (path, *generation) == (&about.0, about.1));
    if !up || shown.as_ref() != Some(&about) {
        return;
    }
    match answer {
        Ok(()) if deleting => shell.ask = None,
        Ok(()) => {
            shell.ask = None;
            // A rename of the branch in use is where the project reopens.
            if let Some(from) = renaming {
                return shell.branch_renamed(&about.0, &from, &name);
            }
            if let Err(why) = crate::switch_ask::to_new_branch(shell, name) {
                let words = crate::version_chip::refused_words(shell, why);
                shell.canvas.say_refused_step(words);
            }
        }
        Err(refused) => {
            if let Some(Ask::Branch { naming, .. }) = shell.ask.as_mut() {
                // The field takes the keyboard back, so the name can be changed at once.
                // The host settles an unclear rename only when the same one is asked for again, so
                // the first one stays the rename to ask for.
                if matches!(refused, BranchRefused::Unclear(_)) && renaming.is_some() {
                    let shown = shell.services.projects.as_ref().map_or_else(|| name.clone(), |h| h.branch_shown(&name));
                    naming.unsettled.get_or_insert((name, shown));
                }
                let to = renaming.as_ref().map(|_| naming.unsettled.as_ref().map_or("", |u| u.1.as_str()));
                let said = if deleting { undeleted(&refused) } else { words(&refused, to) };
                (naming.refused, naming.waiting, naming.focused) = (Some(said), false, false);
            }
        }
    }
}

/// Asks the host to make the branch stored as `name` from current work, to rename the branch the
/// dialog renames to it, or to delete the branch `name`. Before a creation or a rename of the
/// branch in use, as before a switch and a save, the skill text and description being typed are
/// kept and handed over first; one that can't be kept stops it, its reason under the field.
fn create(shell: &mut Shell, host: &dyn ProjectHost, name: String) {
    let Some(Ask::Branch { path, naming, .. }) = shell.ask.as_ref() else { return };
    let (renaming, deleting) = (naming.renaming.clone(), naming.deleting.is_some());
    let in_use = renaming.as_ref().is_none_or(|from| host.current_branch(path).is_some_and(|now| from.eq_ignore_ascii_case(&now)));
    // A delete leaves current work alone: the branch in use is never deleted.
    if deleting {
        let path = path.clone();
        let answer = crate::chat_delete::delete_branch(shell, &path, &name);
        return made(shell, name, None, answer);
    }
    if shell.viewing.is_none() && in_use {
        crate::skill_tab::sync(shell, 0.0, true, None);
        if crate::panels::commit_description(shell) || !crate::skill_tab::unkept(&shell.canvas).is_empty() {
            let why = crate::version_chip::refused_words(shell, crate::hold::HoldRefused::Description);
            if let Some(Ask::Branch { naming, .. }) = shell.ask.as_mut() {
                naming.refused = why;
            }
            return;
        }
        shell.write_changes();
    }
    let Some(Ask::Branch { path, .. }) = shell.ask.as_ref() else { return };
    let answer = match &renaming {
        Some(from) => host.rename_branch(path, from, &name),
        None => host.create_branch(path, &name),
    };
    made(shell, name, renaming, answer);
}

/// Keeps the host's `answer` to what the dialog asked for the branch `name`: made, renamed from
/// `renaming`, or deleted. The dialog waits for it.
fn made(shell: &mut Shell, name: String, renaming: Option<String>, answer: Receiver<Result<(), BranchRefused>>) {
    let Some(Ask::Branch { path, generation, naming }) = shell.ask.as_mut() else { return };
    let deleting = naming.deleting.is_some();
    shell.versions.making = Some(Making { about: (path.clone(), *generation), name, renaming, deleting, answer });
    (naming.waiting, naming.refused) = (true, None);
}

/// Draws the dialog over the window and acts on it. While a creation, rename or delete is on its
/// way nothing closes it and nothing more is asked.
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
        let (renaming, unsettled) = (naming.renaming.clone(), naming.unsettled.clone());
        let name = |text: &str| checked(host.as_ref(), &listed, (renaming.as_ref(), unsettled.as_ref()), text);
        (asked, close) = match naming.deleting.clone() {
            Some(doomed) => doomed_card(shell, ui, &p, window, &naming, &doomed),
            None => card(shell, ui, &p, (window, &from), &mut naming, name),
        };
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

/// The dialog's card: what a branch is and where it starts, or for a rename what it leaves alone,
/// the name field and what is wrong with the name. Gives the name to make or rename to, when the
/// main button or Enter asks for it, and whether Cancel was clicked.
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
    let (title, what, act, act_id) = match n.renaming {
        Some(_) => (tr("Rename branch"), tr("The conversations and versions on this branch are not affected."), tr("Rename"), "branch-ask.rename"),
        None => (tr("New branch"), tr("A branch keeps its own versions, unsaved work and conversations."), tr("Create"), "branch-ask.create"),
    };
    let mut lines = vec![(ux::galley_rows(&paint, &what, types::SMALL, p.text_secondary, Some(inner), 3), p.text_secondary, 14.0)];
    // A rename starts nowhere: only a new branch says where.
    if n.renaming.is_none() {
        lines[0].2 = 8.0;
        lines.push((ux::galley_rows(&paint, &fmt(&start, &[("branch", from)]), types::SMALL, p.text, Some(inner), 4), p.text, 14.0));
    }
    let checked = name(&n.text);
    // The host's refusal of the name typed, else what is wrong with it once it has been typed in.
    let problem = n.refused.clone().or_else(|| checked.clone().err().filter(|_| n.edited));
    let (said, ink) = problem.map_or_else(|| (fmt(&tr("At most {n} characters."), &[("n", "60")]), p.text_muted), |why| (why, p.danger));
    let hint = ux::galley_rows(&paint, &said, types::CAPTION, ink, Some(inner), 3);
    let height = 46.0 + lines.iter().map(|(g, _, gap)| g.size().y + gap).sum::<f32>() + 49.0 + hint.size().y + 16.0 + 28.0 + 20.0;
    let frame = Rect::from_center_size(window.center(), vec2(width, height));
    ux::shadow(&paint, frame, 10.0, p.shadow);
    paint.rect_filled(frame, 10.0, p.surface);
    paint.rect_stroke(frame, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "branch-ask", "dialog", frame);
    let left = frame.left() + 20.0;
    ux::text(&paint, pos2(left, frame.top() + 20.0), &title, types::DIALOG_TITLE, p.text, Some(inner));
    let mut y = frame.top() + 46.0;
    for (g, ink, gap) in lines {
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
    // Of the text as typed this frame, so Create never makes a name the field no longer shows; a
    // rename to the name the branch has is no rename.
    let live = (!n.waiting).then(|| name(&n.text).ok()).flatten().filter(|to| n.renaming.as_ref() != Some(to));
    let top = frame.bottom() - 48.0;
    let width_of = |text: &str| (ux::galley(&paint, text, types::UI, p.text, None).size().x + 20.0).ceil();
    let at = pos2(frame.right() - 20.0 - width_of(&act), top);
    let (_, create) = button(shell, ui, p, act_id, at, &act, Look::Primary, live.is_some(), None);
    let at = pos2(at.x - 8.0 - width_of(&tr("Cancel")), top);
    let (_, cancel) = button(shell, ui, p, "branch-ask.cancel", at, &tr("Cancel"), Look::Secondary, !n.waiting, None);
    (live.filter(|_| create.clicked() || enter), cancel.clicked())
}

/// What goes with the branch `d`, as the delete's card says it.
fn goes_with(d: &Doomed) -> String {
    match &d.counts {
        None => tr("Counting what goes with it…"),
        Some(Err(why)) => fmt(&tr("What goes with it couldn't be read: {why}"), &[("why", why)]),
        Some(Ok(c)) => {
            let versions = trn("{n} version only on this branch", "{n} versions only on this branch", c.versions_only_here);
            let chats = trn("{n} conversation", "{n} conversations", c.conversations);
            let line = if c.unsaved { tr("{versions} · {conversations} · unsaved changes") } else { tr("{versions} · {conversations}") };
            fmt(&line, &[("versions", &versions), ("conversations", &chats)])
        }
    }
}

/// The delete's card (frame V5.8): what goes with the branch once read, that it leaves the list
/// and may remain in the project folder, and that there is no Undo; refused, why it stays, in that
/// place (V5.9). Gives the branch to delete when Delete branch is clicked, once what goes with it
/// is read or can't be, and whether Cancel was.
fn doomed_card(shell: &mut Shell, ui: &egui::Ui, p: &Palette, window: Rect, n: &Naming, d: &Doomed) -> (Option<String>, bool) {
    let paint = ui.painter().clone();
    let width = 480.0_f32.min(window.width() - 32.0).max(240.0);
    let inner = width - 40.0;
    let named = [("branch", d.shown.as_str())];
    let (title, ink) = match n.refused {
        Some(_) => (fmt(&tr("Couldn't delete {branch}"), &named), p.danger),
        None => (fmt(&tr("Delete {branch}?"), &named), p.text),
    };
    let counts = goes_with(d);
    let remains = tr("Its contents may remain in the project folder's storage, as a deleted conversation does in the project's saved history.");
    let stays = format!("{} {remains}", tr("It leaves the project's list."));
    let small = |words: &str, ink, wrap: f32| ux::galley_rows(&paint, words, types::SMALL, ink, Some(wrap), 6);
    let lines = [
        (small(&tr("Goes with the branch:"), p.text_secondary, inner), p.text_secondary, 10.0),
        (small(&counts, p.text, inner), p.text, 12.0),
        (small(&stays, p.text_secondary, inner), p.text_secondary, 12.0),
    ];
    let notice = n.refused.clone().unwrap_or_else(|| tr("There is no Undo for deleting a branch."));
    let warn = small(&notice, p.danger, inner - 44.0);
    let tint = warn.size().y + 20.0;
    let height = 46.0 + lines.iter().map(|(g, _, gap)| g.size().y + gap).sum::<f32>() + tint + 16.0 + 28.0 + 20.0;
    let frame = Rect::from_center_size(window.center(), vec2(width, height));
    ux::shadow(&paint, frame, 10.0, p.shadow);
    paint.rect_filled(frame, 10.0, p.surface);
    paint.rect_stroke(frame, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    widgets::record_visible(ui, &mut shell.registry, "branch-ask", "dialog", frame);
    let left = frame.left() + 20.0;
    ux::text(&paint, pos2(left, frame.top() + 20.0), &title, types::DIALOG_TITLE, ink, Some(inner));
    let mut y = frame.top() + 46.0;
    for (i, (g, ink, gap)) in lines.into_iter().enumerate() {
        let at = Rect::from_min_size(pos2(left, y), g.size());
        if i == 1 {
            let _ = control(shell, ui, "branch-ask.counts", "status", at, false).accessible_name(counts.as_str());
        }
        paint.galley(at.min, g, ink);
        y = at.bottom() + gap;
    }
    let block = Rect::from_min_size(pos2(left, y), vec2(inner, tint));
    paint.rect_filled(block, 8.0, p.danger_tint);
    ux::icon_in(&paint, Rect::from_center_size(pos2(left + 20.0, y + 18.0), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
    let _ = control(shell, ui, "branch-ask.notice", "status", block, false).accessible_name(notice.as_str());
    paint.galley(pos2(left + 34.0, y + 10.0), warn, p.danger);
    let top = frame.bottom() - 48.0;
    let width_of = |text: &str| (ux::galley(&paint, text, types::UI, p.text, None).size().x + 20.0).ceil();
    let act = tr("Delete branch");
    let at = pos2(frame.right() - 20.0 - width_of(&act), top);
    let live = !n.waiting && d.counts.is_some();
    let (_, delete) = button(shell, ui, p, "branch-ask.delete", at, &act, Look::DangerFill, live, None);
    let at = pos2(at.x - 8.0 - width_of(&tr("Cancel")), top);
    let (_, cancel) = button(shell, ui, p, "branch-ask.cancel", at, &tr("Cancel"), Look::Secondary, !n.waiting, None);
    ((live && delete.clicked()).then(|| d.name.clone()), cancel.clicked())
}
