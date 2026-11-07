//! Deleting a conversation from the Chat panel's history (AR-217, page 07 of the design, frames
//! B7a to B7c): the bin on a row and the row's menu, the confirmation, and the toast with Undo.
//!
//! - **Asking.** The bin shows on the row the pointer is on or the keyboard is on; the row's menu
//!   (right-click, the menu key or ⇧F10) holds Open and Delete conversation; ⌫ or Delete on the
//!   focused row asks too. All three open the same confirmation, which names the conversation and
//!   says that it leaves this project's list and stays in the project's saved history. While chat
//!   writing is off (a re-read of the list failed) the bin and the item are disabled and say why;
//!   a row that cannot be read has a disabled bin of its own.
//! - **Deleting.** The project deletes its rows in one chat-only commit (`Chat N: deleted`); the
//!   model, its undo history and its saved state are not touched, and ⌘Z never deletes or restores
//!   a conversation. The open conversation gives way first: its running turn ends stopped, its
//!   reply so far kept, written before the deletion on the project's writer, and the next row
//!   opens (else the one above, else a new conversation). Focus goes to the next row, else the one
//!   above, else New conversation.
//! - **Undo.** The toast says the conversation is still in the project's saved history and offers
//!   Undo for [`TOAST_SECONDS`], longer while the pointer or the focus is on it. Undo writes the rows
//!   back from the commit just before the deletion (owner decision), in a chat commit of its own;
//!   pressed before the deletion has finished, it runs once it has. While a toast is up, a new
//!   conversation never takes the deleted one's label.
//! - **The private folder** of the conversation's program is removed only once its toast has
//!   closed without Undo, when an Undo failed, when its project closes or when Archforge quits
//!   ([`settle`]): the host ends the program first and waits a bounded time. A folder of a
//!   conversation a project open here still shows (a copy of the project) stays.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use archforge_mof::Uuid;
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::ai_agents::FolderGoes;
use crate::chat_panel::{Panel, TurnState, View, clock};
use crate::chat_store::{Answer, Listed, Refusal, Row, Write};
use crate::i18n::{fmt, tr, trn};
use crate::settings::{Look, button, control, focus_ring};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// How long the toast stays once neither the pointer nor the focus is on it, in seconds.
pub const TOAST_SECONDS: u64 = 8;
/// How many frames a focus request is made before it is given up.
pub const FOCUS_TRIES: u8 = 4;
/// The confirmation's width and the toast's height, as the design draws them.
const DIALOG_WIDTH: f32 = 440.0;
const TOAST_HEIGHT: f32 = 40.0;
/// The row menu's width and its rows' height.
const MENU_WIDTH: f32 = 220.0;
const MENU_ROW: f32 = 30.0;

/// Where a deletion stands with the project.
pub enum Stage {
    /// The deletion was asked.
    Deleting { rx: Receiver<Result<Answer, Refusal>> },
    /// Deleted: `before` is the commit just before the deletion, which still holds it whole.
    Deleted { before: String },
    /// Undo was asked; `deleting` is the deletion's answer while it has not come, for an Undo
    /// pressed before it finished (the restoration, asked after it on the project's writer, takes
    /// the commit the deletion names there).
    Restoring { rx: Receiver<Result<Answer, Refusal>>, deleting: Option<Receiver<Result<Answer, Refusal>>> },
}

/// A conversation being deleted, deleted while its toast shows, or being brought back.
pub struct Deletion {
    /// Its project, which every write names.
    pub project: PathBuf,
    pub chat: Uuid,
    /// Its id as the host takes it, which names its private folder.
    pub key: String,
    /// Its label, which a new conversation skips while the deletion can still be undone.
    pub origin: String,
    pub title: String,
    pub stage: Stage,
    /// Its toast shows: until it times out, another deletion's toast replaces it, Undo is pressed
    /// or its project closes.
    pub toast: bool,
    /// When the toast goes, in seconds since 1970; none until it is first drawn in its project.
    pub until: Option<u64>,
}

/// The confirmation open over the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub chat: Uuid,
    /// Its row in the history, where the focus goes back on Cancel.
    pub row: usize,
    /// The line that names it: its title, messages, agent and age.
    pub summary: String,
    /// Its reply is running: the confirmation says it stops first.
    pub running: bool,
    /// The focus was put on Cancel.
    pub focused: bool,
}

/// A row's menu: the row, where it opened, and the item the arrow keys reached.
#[derive(Debug, Clone, PartialEq)]
pub struct RowMenu {
    pub chat: Uuid,
    pub row: usize,
    pub at: Pos2,
    pub reached: Option<usize>,
}

/// Where the keyboard goes once the history is drawn again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTo {
    /// The history's row at this place.
    Row(usize),
    /// The row of this conversation, once the list holds it.
    Chat(Uuid),
    /// New conversation in the panel's header, else the history button.
    New,
}

/// The key that deletes, as this system names it: ⌫ on macOS, Delete elsewhere.
pub fn delete_key(ctx: &egui::Context) -> &'static str {
    if ctx.os() == egui::os::OperatingSystem::Mac { "⌫" } else { "Delete" }
}

/// The key that opens, as this system names it.
fn open_key(ctx: &egui::Context) -> &'static str {
    if ctx.os() == egui::os::OperatingSystem::Mac { "↵" } else { "Enter" }
}

/// Why a history entry cannot be deleted now: chat writing is off, or its row cannot be read.
pub fn refusal(panel: &Panel, entry: &Result<Row, String>) -> Option<String> {
    if let Some(why) = &panel.writing_off {
        return Some(fmt(&tr("The conversations in this project cannot be changed: {why}"), &[("why", why)]));
    }
    entry.as_ref().err().map(|_| tr("This conversation cannot be read"))
}

/// When a conversation was last active, as the confirmation says it: "today", "yesterday", "3 Nov"
/// (with the year when it is not this one).
pub fn age(then: u64, clock: crate::chat_words::Clock) -> String {
    let local = |s: u64| {
        let s = i64::try_from(s).unwrap_or(i64::MAX);
        s.saturating_add(i64::from(clock.offset.unwrap_or(0).clamp(-14 * 3_600, 14 * 3_600))).div_euclid(86_400)
    };
    let (today, that) = (local(clock.now), local(then));
    if that == today {
        return tr("today");
    }
    if that.saturating_add(1) == today {
        return tr("yesterday");
    }
    let (y, m, d) = crate::chat_words::civil(that);
    let (this_year, _, _) = crate::chat_words::civil(today);
    if y == this_year {
        fmt(&tr("{day} {month}"), &[("day", &d.to_string()), ("month", &crate::chat_words::month(m))])
    } else {
        fmt(&tr("{day} {month} {year}"), &[("day", &d.to_string()), ("month", &crate::chat_words::month(m)), ("year", &y.to_string())])
    }
}

/// The rows of the history as last read: none while it was never read or could not be.
fn rows(panel: &Panel) -> &[Listed] {
    match &panel.history {
        Some(Ok(rows)) => rows,
        _ => &[],
    }
}

/// Opens the confirmation for the conversation `chat`, the history's row `row`: refused while
/// chat writing is off or its row cannot be read.
pub fn ask(shell: &mut Shell, chat: Uuid, row: usize) {
    shell.chat_panel.row_menu = None;
    let Some(listed) = rows(&shell.chat_panel).iter().find(|l| l.id == chat).cloned() else { return };
    if refusal(&shell.chat_panel, &listed.entry).is_some() {
        return;
    }
    let Ok(entry) = &listed.entry else { return };
    let clock = clock(shell);
    let summary = fmt(
        &tr("{title} · {messages} with {agent}, {when}"),
        &[
            ("title", &entry.title),
            ("messages", &trn("{n} message", "{n} messages", u64::from(entry.messages))),
            ("agent", entry.provider.name()),
            ("when", &age(entry.updated_at, clock)),
        ],
    );
    let running = shell.chat_panel.chat.as_ref().is_some_and(|c| c.id == chat && c.busy());
    shell.chat_panel.ask = Some(Ask { chat, row, summary, running, focused: false });
}

/// Cancel or Escape: the confirmation closes and the focus goes back to the row.
pub fn cancel(shell: &mut Shell) {
    if let Some(a) = shell.chat_panel.ask.take() {
        shell.chat_panel.focus = Some(FocusTo::Row(a.row));
        shell.chat_panel.focus_tries = FOCUS_TRIES;
    }
}

/// Delete in the confirmation: the open conversation gives way (its running turn ends stopped,
/// written first), the row leaves the list, the deletion is asked of the project, and the toast
/// shows with Undo. The earlier toast closes.
pub fn confirm(shell: &mut Shell) {
    let Some(ask) = shell.chat_panel.ask.take() else { return };
    let Some(project) = shell.project.as_ref().map(|p| p.path.clone()) else { return };
    let listed: Vec<Listed> = rows(&shell.chat_panel).to_vec();
    let Some(Ok(row)) = listed.iter().find(|l| l.id == ask.chat).map(|l| l.entry.clone()) else { return };
    let Some(store) = shell.services.chats.clone() else {
        shell.chat_panel.list_notice = Some(tr("This project keeps no conversations"));
        return;
    };
    let at = listed.iter().position(|l| l.id == ask.chat).unwrap_or(ask.row);
    if shell.chat_panel.chat.as_ref().is_some_and(|c| c.id == ask.chat) {
        // Its reply so far is kept and its turn ends stopped, written before the deletion on the
        // project's writer; a question on its way is dropped, and its program is let go of.
        crate::chat_panel::end_here(shell, TurnState::Stopped);
        if let Some(c) = shell.chat_panel.chat.as_mut() {
            c.step = crate::chat_panel::Step::Idle;
            c.queued = None;
        }
        crate::chat_panel::retire(shell);
        let readable = |l: &&Listed| l.id != ask.chat && l.entry.is_ok();
        let next = listed.iter().skip(at + 1).find(readable).or_else(|| listed.iter().take(at).rev().find(readable)).map(|l| l.id);
        match next {
            Some(id) => {
                shell.chat_panel.loading_stays = true;
                shell.chat_panel.loading = None;
                crate::chat_panel::open(shell, id);
            }
            None => crate::chat_panel::showing(shell),
        }
    }
    let rx = store.write(&project, Write::Delete { chat: ask.chat });
    close_toasts(shell, &project);
    shell.chats.deletions.push(Deletion {
        project,
        chat: ask.chat,
        key: ask.chat.to_string(),
        origin: row.origin.clone(),
        title: row.title.clone(),
        stage: Stage::Deleting { rx },
        toast: true,
        until: None,
    });
    if let Some(Ok(rows)) = shell.chat_panel.history.as_mut() {
        rows.retain(|l| l.id != ask.chat);
    }
    // The next readable row takes the keyboard, else the one above, else New conversation.
    let left = rows(&shell.chat_panel);
    let next = left.iter().skip(at).position(|l| l.entry.is_ok()).map(|i| at + i);
    let above = left.iter().take(at).rposition(|l| l.entry.is_ok());
    shell.chat_panel.focus = Some(next.or(above).map_or(FocusTo::New, FocusTo::Row));
    shell.chat_panel.focus_tries = FOCUS_TRIES;
    shell.chat_panel.list_notice = None;
}

/// The toasts of `project` close: a deletion that has finished lets its folder go. A parked
/// project's toast waits for its tab.
fn close_toasts(shell: &mut Shell, project: &Path) {
    for d in shell.chats.deletions.iter_mut().filter(|d| d.project == project) {
        d.toast = false;
    }
}

/// The labels a new conversation of `project` may not take: those of its deletions that can still
/// be undone.
pub fn held_labels(shell: &Shell, project: &Path) -> Vec<String> {
    shell.chats.deletions.iter().filter(|d| d.project == project).map(|d| d.origin.clone()).collect()
}

/// Undo on the toast: the conversation is written back from the commit before its deletion; pressed
/// before the deletion answered, the restoration goes to the project's writer after it and takes
/// the commit it names there. The toast closes. ⌘Z never comes here.
pub fn undo(shell: &mut Shell) {
    let Some(project) = shell.project.as_ref().map(|p| p.path.clone()) else { return };
    let Some(store) = shell.services.chats.clone() else { return };
    let Some(d) = shell.chats.deletions.iter_mut().rev().find(|d| d.toast && d.project == project) else { return };
    d.toast = false;
    let stage = std::mem::replace(&mut d.stage, Stage::Deleted { before: String::new() });
    d.stage = match stage {
        Stage::Deleting { rx } => Stage::Restoring { rx: store.write(&project, Write::Restore { chat: d.chat, from: None }), deleting: Some(rx) },
        Stage::Deleted { before } => {
            Stage::Restoring { rx: store.write(&project, Write::Restore { chat: d.chat, from: Some(before) }), deleting: None }
        }
        restoring @ Stage::Restoring { .. } => restoring,
    };
}

/// The panel of `project`: the one shown, or a parked tab's.
fn panel_of<'a>(shell: &'a mut Shell, project: &Path) -> Option<&'a mut Panel> {
    if shell.project.as_ref().is_some_and(|p| p.path == project) {
        return Some(&mut shell.chat_panel);
    }
    shell.tabs.iter_mut().find(|t| t.path == project).and_then(|t| t.parked.as_mut()).map(|p| &mut p.chat_panel)
}

/// Whether `project` is shown with its history open.
fn history_shown(shell: &Shell, project: &Path) -> bool {
    shell.project.as_ref().is_some_and(|p| p.path == project) && shell.chat_panel.view == View::History
}

/// Reads the history of `project` again when it is shown with its history open.
fn reread(shell: &mut Shell, project: &Path) {
    if history_shown(shell, project)
        && let Some(store) = shell.services.chats.clone()
    {
        shell.chat_panel.listing = Some(store.list(project));
    }
}

/// Says `words` under the history of `project`.
fn notice(shell: &mut Shell, project: &Path, words: String) {
    log::warn!("{words}");
    if let Some(panel) = panel_of(shell, project) {
        panel.list_notice = Some(words);
    }
}

/// Whether a panel of an open project holds the conversation `key`: a copy of the project open
/// beside it shares its private folder, which then stays.
fn held_elsewhere(shell: &Shell, key: &str) -> bool {
    let holds = |p: &Panel| p.chat.as_ref().is_some_and(|c| c.key == key);
    holds(&shell.chat_panel) || shell.tabs.iter().filter_map(|t| t.parked.as_ref()).any(|p| holds(&p.chat_panel))
}

/// Asks the host to remove the private folder of the conversation `key`, its program ended first;
/// with `after`, once the project's answer lets it go.
fn forget(shell: &mut Shell, key: &str, after: Option<(Receiver<Result<Answer, Refusal>>, FolderGoes)>) {
    if held_elsewhere(shell, key) {
        log::info!("the private folder of the conversation {key:?} is kept: a project open here shows that conversation");
        return;
    }
    let Some(host) = shell.services.agents.clone() else { return };
    let rx = match after {
        None => host.forget(key),
        Some((answer, goes)) => host.forget_after(key, answer, goes),
    };
    shell.chats.forgetting.push((key.to_string(), rx));
}

/// What became of a deletion this frame.
enum Next {
    Keep(Deletion),
    /// Done; its folder goes when `forget`.
    Done {
        forget: bool,
    },
}

/// Once per frame: takes the project's answers to deletions and restorations, closes a toast whose
/// time is up, and lets the folders of those that are settled go.
pub(crate) fn frame(shell: &mut Shell) {
    let now = clock(shell).now;
    let shown = shell.project.as_ref().map(|p| p.path.clone());
    for d in std::mem::take(&mut shell.chats.deletions) {
        let key = d.key.clone();
        let here = shown.as_deref() == Some(d.project.as_path());
        match step(shell, d, now, here) {
            Next::Keep(d) => shell.chats.deletions.push(d),
            Next::Done { forget: true } => forget(shell, &key, None),
            Next::Done { forget: false } => {}
        }
    }
    shell.chats.forgetting.retain(|(key, rx)| match rx.try_recv() {
        Ok(Ok(_)) => false,
        Ok(Err(why)) => {
            log::warn!("the private folder of the conversation {key:?} was not removed: {why}");
            false
        }
        Err(TryRecvError::Empty) => true,
        Err(TryRecvError::Disconnected) => false,
    });
}

/// The reason in a refusal, for the panel.
fn refused_why(r: &Refusal) -> String {
    match r {
        Refusal::Failed(why) => why.clone(),
        Refusal::Listed => tr("it is in the conversation list already"),
        Refusal::HeldKey { .. } => tr("the project did not answer"),
    }
}

/// One deletion's step this frame; `shown` when its project is the one shown.
fn step(shell: &mut Shell, mut d: Deletion, now: u64, shown: bool) -> Next {
    let project = d.project.clone();
    let not_deleted = |shell: &mut Shell, why: String| {
        notice(shell, &project, fmt(&tr("This conversation could not be deleted: {why}"), &[("why", &why)]));
        reread(shell, &project);
        Next::Done { forget: false }
    };
    match d.stage {
        Stage::Deleting { rx } => match rx.try_recv() {
            Err(TryRecvError::Empty) => {
                d.stage = Stage::Deleting { rx };
                Next::Keep(d)
            }
            Ok(Ok(Answer::Deleted { before })) => {
                reread(shell, &project);
                d.stage = Stage::Deleted { before };
                if d.toast { Next::Keep(d) } else { Next::Done { forget: true } }
            }
            Ok(Err(r)) => not_deleted(shell, refused_why(&r)),
            Ok(Ok(_)) => not_deleted(shell, tr("the project did not answer")),
            Err(TryRecvError::Disconnected) => not_deleted(shell, tr("the project did not answer")),
        },
        Stage::Deleted { before } => {
            d.stage = Stage::Deleted { before };
            if d.toast && shown {
                // Eight seconds of being seen: the time starts when it is first drawn in its
                // project, and again whenever the pointer or the focus is on it.
                let until = *d.until.get_or_insert(now.saturating_add(TOAST_SECONDS));
                if now > until {
                    d.toast = false;
                }
            }
            if d.toast { Next::Keep(d) } else { Next::Done { forget: true } }
        }
        Stage::Restoring { rx, deleting: Some(deleting) } => match deleting.try_recv() {
            Err(TryRecvError::Empty) => {
                d.stage = Stage::Restoring { rx, deleting: Some(deleting) };
                Next::Keep(d)
            }
            Ok(Ok(Answer::Deleted { .. })) => {
                d.stage = Stage::Restoring { rx, deleting: None };
                Next::Keep(d)
            }
            // Not deleted: the restoration after it is refused, and nothing is to say of it.
            Ok(Err(r)) => not_deleted(shell, refused_why(&r)),
            Ok(Ok(_)) | Err(TryRecvError::Disconnected) => not_deleted(shell, tr("the project did not answer")),
        },
        Stage::Restoring { rx, deleting: None } => match rx.try_recv() {
            Err(TryRecvError::Empty) => {
                d.stage = Stage::Restoring { rx, deleting: None };
                Next::Keep(d)
            }
            // Back, or in the list anyway: its folder stays.
            Ok(Ok(_)) | Ok(Err(Refusal::Listed)) => {
                if history_shown(shell, &project) {
                    shell.chat_panel.focus = Some(FocusTo::Chat(d.chat));
                    shell.chat_panel.focus_tries = FOCUS_TRIES;
                }
                reread(shell, &project);
                Next::Done { forget: false }
            }
            Ok(Err(r)) => {
                notice(shell, &project, fmt(&tr("This conversation could not be brought back: {why}"), &[("why", &refused_why(&r))]));
                reread(shell, &project);
                Next::Done { forget: true }
            }
            // Whether it came back is not known: its folder stays.
            Err(TryRecvError::Disconnected) => {
                notice(
                    shell,
                    &project,
                    fmt(&tr("This conversation could not be brought back: {why}"), &[("why", &tr("the project did not answer"))]),
                );
                reread(shell, &project);
                Next::Done { forget: false }
            }
        },
    }
}

/// Whether deletions wait for the project, the toast of the project shown counts down, or a folder
/// is being removed: the window looks again soon. A parked project's toast waits without it.
pub(crate) fn waiting(shell: &Shell) -> bool {
    let shown = shell.project.as_ref().map(|p| p.path.as_path());
    shell.chats.deletions.iter().any(|d| !matches!(d.stage, Stage::Deleted { .. }) || (d.toast && shown == Some(d.project.as_path())))
        || !shell.chats.forgetting.is_empty()
}

/// The project at `project` closes, or Archforge quits (`project` none): their toasts close at
/// once, and their folders are settled without waiting here. A deletion that has finished lets its
/// folder go; one still under way hands its answer to the host, which removes the folder once it
/// says deleted; an Undo still under way hands the restoration's, which removes the folder only
/// when the conversation stays deleted. The host waits a bounded time for each, and at quit is
/// waited for with its own threads; an answer that does not come in time keeps the folder, as a
/// crash does. An Undo pressed before its deletion answered is on the writer already, ahead of the
/// closing.
pub(crate) fn settle(shell: &mut Shell, project: Option<&Path>) {
    let (ours, others): (Vec<Deletion>, Vec<Deletion>) =
        std::mem::take(&mut shell.chats.deletions).into_iter().partition(|d| project.is_none_or(|p| d.project == p));
    shell.chats.deletions = others;
    for d in ours {
        let key = d.key.clone();
        match d.stage {
            Stage::Deleted { .. } => forget(shell, &key, None),
            Stage::Deleting { rx } => forget(shell, &key, Some((rx, FolderGoes::IfDeleted))),
            Stage::Restoring { rx, deleting: None } => forget(shell, &key, Some((rx, FolderGoes::IfNotRestored))),
            Stage::Restoring { rx, deleting: Some(deleting) } => match deleting.try_recv() {
                // Not deleted: nothing to remove.
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => log::info!("the conversation {key:?} was not deleted; its folder stays"),
                // Deleted, or not answered yet: the restoration after it decides.
                _ => forget(shell, &key, Some((rx, FolderGoes::IfNotRestored))),
            },
        }
    }
}

// Drawing.

/// The bin on a history row, at the row's right: live, or disabled saying `refused`. Gives whether
/// it was clicked.
#[allow(clippy::too_many_arguments, reason = "the bin is drawn for its row, its place and its reason")]
pub(crate) fn bin(shell: &mut Shell, ui: &egui::Ui, p: &Palette, paint: &egui::Painter, id: &str, r: Rect, refused: Option<&str>) -> bool {
    let words = tr("Delete conversation");
    let resp = match refused {
        None => {
            let hint = fmt(&tr("Delete conversation ({key})"), &[("key", delete_key(ui.ctx()))]);
            control(shell, ui, id, "button", r, true).on_hover_text(hint)
        }
        Some(why) => {
            let resp = control(shell, ui, id, "button", r, false);
            let why = why.to_string();
            ui.ctx().accesskit_node_builder(widgets::egui_id(id), |node| {
                node.set_role(egui::accesskit::Role::Button);
                node.set_disabled();
                node.set_description(why.clone());
            });
            resp.on_hover_text(why)
        }
    };
    let _ = resp.clone().accessible_name(&words);
    if refused.is_none() && resp.hovered() {
        paint.rect_filled(r, 6.0, p.border);
    }
    ux::icon_in(
        paint,
        Rect::from_center_size(r.center(), egui::Vec2::splat(14.0)),
        &icons::TRASH,
        if refused.is_none() { p.text_secondary } else { p.text_muted },
    );
    focus_ring(paint, p, r, 6.0, &resp);
    refused.is_none() && resp.clicked()
}

/// What the row's menu was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuPick {
    Open,
    Delete,
}

/// The row's menu, when open: Open, a line, Delete conversation (disabled saying why when it may not
/// be). The arrow keys walk its live items, Enter picks, Escape closes it and gives the row its
/// focus back; a press elsewhere closes it.
pub(crate) fn row_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette) -> Option<(Uuid, MenuPick)> {
    let menu = shell.chat_panel.row_menu.clone()?;
    let entry = rows(&shell.chat_panel).iter().find(|l| l.id == menu.chat).map(|l| l.entry.clone());
    let Some(entry) = entry else {
        shell.chat_panel.row_menu = None;
        return None;
    };
    let refused = refusal(&shell.chat_panel, &entry);
    let window = ui.ctx().content_rect();
    let height = 8.0 + MENU_ROW + 9.0 + MENU_ROW + 8.0;
    // At the pointer, or to its left near the window's edge.
    let x = if menu.at.x + MENU_WIDTH <= window.right() - 8.0 { menu.at.x } else { (menu.at.x - MENU_WIDTH).max(window.left() + 8.0) };
    let y = menu.at.y.min(window.bottom() - height - 8.0).max(window.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), vec2(MENU_WIDTH, height));
    let live: Vec<usize> = if refused.is_none() { vec![0, 1] } else { vec![0] };
    let (down, up, enter, escape) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if escape {
        shell.chat_panel.row_menu = None;
        shell.chat_panel.focus = Some(FocusTo::Row(menu.row));
        shell.chat_panel.focus_tries = FOCUS_TRIES;
        return None;
    }
    let mut reached = menu.reached;
    if down || up {
        let at = reached.and_then(|r| live.iter().position(|l| *l == r));
        let next = match at {
            None => 0,
            Some(i) if down => (i + 1).min(live.len().saturating_sub(1)),
            Some(i) => i.saturating_sub(1),
        };
        reached = live.get(next).copied();
    }
    let (open_key, delete_key) = (open_key(ui.ctx()), delete_key(ui.ctx()));
    let p2 = *p;
    let (clicked, hovered) = ux::overlay(ui, widgets::egui_id("workspace.chat.history.menu.layer"), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 10.0, p2.shadow);
        paint.rect_filled(m, 10.0, p2.surface);
        paint.rect_stroke(m, 10.0, Stroke::new(1.0, p2.border), StrokeKind::Inside);
        let items = [
            ("workspace.chat.history.menu.open", tr("Open"), open_key, None),
            ("workspace.chat.history.menu.delete", tr("Delete conversation"), delete_key, refused.clone()),
        ];
        let (mut clicked, mut hovered) = (None, None);
        let mut top = m.top() + 8.0;
        for (i, (id, label, key, why)) in items.into_iter().enumerate() {
            if i == 1 {
                paint.rect_filled(Rect::from_min_size(pos2(m.left() + 12.0, top + 4.0), vec2(m.width() - 24.0, 1.0)), 0.0, p2.border);
                top += 9.0;
            }
            let row = Rect::from_min_size(pos2(m.left() + 6.0, top), vec2(m.width() - 12.0, MENU_ROW));
            top += MENU_ROW;
            let resp = match &why {
                None => control(shell, ui, id, "menu-item", row, true),
                Some(why) => {
                    let resp = control(shell, ui, id, "menu-item", row, false);
                    let why = why.clone();
                    ui.ctx().accesskit_node_builder(widgets::egui_id(id), |node| {
                        node.set_role(egui::accesskit::Role::MenuItem);
                        node.set_disabled();
                        node.set_description(why.clone());
                    });
                    resp.on_hover_text(why)
                }
            }
            .accessible_name(label.as_str());
            if resp.hovered() {
                hovered = Some(i);
            }
            if why.is_none() && (resp.hovered() || reached == Some(i)) {
                paint.rect_filled(row, 5.0, p2.accent_tint);
            }
            let ink = if why.is_none() { p2.text } else { p2.text_muted };
            let detail = ux::galley(&paint, key, types::CAPTION, p2.text_secondary, None);
            let dw = detail.size().x;
            paint.galley(pos2(row.right() - 10.0 - dw, row.center().y - detail.size().y / 2.0), detail, p2.text_secondary);
            ux::text_mid(&paint, row.left() + 10.0, row.center().y, &label, types::SMALL, ink, Some((row.width() - 28.0 - dw).max(0.0)));
            if why.is_none() && resp.clicked() {
                clicked = Some(i);
            }
        }
        (clicked, hovered)
    });
    if let Some(i) = hovered.filter(|i| live.contains(i)) {
        reached = Some(i);
    }
    let picked = clicked.or_else(|| enter.then_some(reached).flatten());
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos)));
    shell.chat_panel.row_menu = (!outside && picked.is_none()).then_some(RowMenu { reached, ..menu.clone() });
    picked.map(|i| (menu.chat, if i == 0 { MenuPick::Open } else { MenuPick::Delete }))
}

/// The confirmation, when open, over the whole window on a modal layer of its own: the title, the
/// conversation named, what deleting it does, then Cancel and Delete. The focus starts on Cancel,
/// Tab reaches Delete, Enter or Space presses the focused button, Escape cancels.
pub(crate) fn dialog(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette) {
    let Some(ask) = shell.chat_panel.ask.clone() else { return };
    let window = ui.ctx().content_rect();
    let p = *p;
    let mut answer: Option<bool> = None;
    egui::Area::new(widgets::egui_id("workspace.chat.delete")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        // A modal layer: Tab and clicks stay in the dialog.
        ui.ctx().memory_mut(|m| m.set_modal_layer(ui.layer_id()));
        let paint = ui.painter().clone();
        paint.rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("workspace.chat.delete.veil"), Sense::click_and_drag());
        let width = DIALOG_WIDTH.min(window.width() - 32.0).max(240.0);
        let inner = width - 40.0;
        let title = ux::galley(&paint, &tr("Delete this conversation?"), types::DIALOG_TITLE, p.text, Some(inner));
        let mut lines = vec![ux::galley_rows(&paint, &ask.summary, types::SMALL, p.text, Some(inner), 3)];
        let mut said = vec![tr("It is removed from this project's conversation list.")];
        said.push(tr("It stays in the project's saved history: anyone with a copy of the project can still find it there."));
        if ask.running {
            said.push(tr("Its reply in progress stops first."));
        }
        lines.extend(said.iter().map(|l| ux::galley_rows(&paint, l, types::SMALL, p.text_secondary, Some(inner), 4)));
        let body: f32 = lines.iter().map(|g| g.size().y).sum::<f32>() + 6.0 * (lines.len().saturating_sub(1)) as f32;
        let height = 20.0 + title.size().y + 14.0 + body + 14.0 + 28.0 + 20.0;
        let dialog = Rect::from_center_size(window.center(), vec2(width, height));
        ux::shadow(&paint, dialog, 10.0, p.shadow);
        paint.rect_filled(dialog, 10.0, p.surface);
        paint.rect_stroke(dialog, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
        let _ = ui.interact(dialog, widgets::egui_id("workspace.chat.delete.frame"), Sense::click());
        widgets::record_visible(ui, &mut shell.registry, "workspace.chat.delete", "dialog", dialog);
        let mut y = dialog.top() + 20.0;
        let th = title.size().y;
        paint.galley(pos2(dialog.left() + 20.0, y), title, p.text);
        y += th + 14.0;
        for (i, g) in lines.into_iter().enumerate() {
            let h = g.size().y;
            let ink = if i == 0 { p.text } else { p.text_secondary };
            paint.galley(pos2(dialog.left() + 20.0, y), g, ink);
            y += h + 6.0;
        }
        // Delete at the right, Cancel before it.
        let top = dialog.bottom() - 20.0 - 28.0;
        let delete_w = ux::galley(&paint, &tr("Delete"), types::UI, p.text, None).size().x + 20.0;
        let delete_at = pos2(dialog.right() - 20.0 - delete_w.ceil(), top);
        let cancel_w = ux::galley(&paint, &tr("Cancel"), types::UI, p.text, None).size().x + 20.0;
        let cancel_at = pos2(delete_at.x - 8.0 - cancel_w.ceil(), top);
        let (_, cancel) = button(shell, ui, &p, "workspace.chat.delete.cancel", cancel_at, &tr("Cancel"), Look::Secondary, true, None);
        let (_, delete) = button(shell, ui, &p, "workspace.chat.delete.confirm", delete_at, &tr("Delete"), Look::DangerFill, true, None);
        if !ask.focused {
            cancel.request_focus();
        }
        if cancel.clicked() {
            answer = Some(false);
        } else if delete.clicked() {
            answer = Some(true);
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            answer = Some(false);
        }
    });
    match answer {
        Some(true) => confirm(shell),
        Some(false) => cancel(shell),
        None => {
            if let Some(a) = shell.chat_panel.ask.as_mut() {
                a.focused = true;
            }
        }
    }
}

/// The toast of the newest deletion of the project shown, at the foot of the centre `r`, clear of
/// the minimap: "<title> deleted · still in the project's saved history", with Undo. It stays
/// [`TOAST_SECONDS`] after the pointer and the focus last left it.
pub(crate) fn toast(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let Some(project) = shell.project.as_ref().map(|o| o.path.clone()) else { return };
    let Some(title) = shell.chats.deletions.iter().rev().find(|d| d.toast && d.project == project).map(|d| d.title.clone()) else { return };
    let now = clock(shell).now;
    let paint = ui.painter().clone();
    let edge = 16.0;
    let minimap = crate::canvas::minimap_box(shell, r);
    let right = minimap.map_or(r.right() - edge, |m| m.left() - edge);
    let room = (right - r.left() - edge).max(0.0);
    let words = fmt(&tr("{title} deleted · still in the project's saved history"), &[("title", &title)]);
    let label = ux::galley(&paint, &tr("Undo"), types::SMALL_STRONG, p.border_hover, None);
    let undo_w = label.size().x;
    let text = ux::galley_rows(&paint, &words, types::SMALL, p.surface, Some((room - 14.0 - 24.0 - undo_w - 14.0).max(0.0)), 3);
    let width = (14.0 + text.size().x + 24.0 + undo_w + 14.0).min(room.max(0.0));
    let height = (text.size().y + 22.0).max(TOAST_HEIGHT);
    let bottom = r.bottom() - 76.0;
    let left = (r.center().x - width / 2.0).min(right - width).max(r.left() + edge);
    let m = Rect::from_min_size(pos2(left, bottom - height), vec2(width, height));
    let p2 = *p;
    let over = ux::overlay(ui, widgets::egui_id("workspace.chat.toast.layer"), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p2.shadow);
        paint.rect_filled(m, 8.0, p2.text);
        widgets::record_visible(ui, &mut shell.registry, "workspace.chat.toast", "toast", m);
        let th = text.size().y;
        paint.galley(pos2(m.left() + 14.0, m.center().y - th / 2.0), text, p2.surface);
        let b = Rect::from_min_size(pos2(m.right() - 14.0 - undo_w - 6.0, m.center().y - 12.0), vec2(undo_w + 12.0, 24.0));
        let resp = control(shell, ui, "workspace.chat.toast.undo", "button", b, true);
        let _ = resp.clone().accessible_name(tr("Undo"));
        let uh = label.size().y;
        paint.galley(pos2(b.left() + 6.0, b.center().y - uh / 2.0), label, p2.border_hover);
        focus_ring(&paint, &p2, b, 6.0, &resp);
        let held = resp.has_focus() || ui.rect_contains_pointer(m);
        (resp.clicked(), held)
    });
    let (clicked, held) = over;
    if clicked {
        undo(shell);
        return;
    }
    if held && let Some(d) = shell.chats.deletions.iter_mut().rev().find(|d| d.toast && d.project == project) {
        d.until = Some(now.saturating_add(TOAST_SECONDS));
    }
}
