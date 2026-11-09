//! The version chip in the project bar and its list (`docs/design/versioning.md`, section 2): what
//! the project shows, the versions to view and the steps between them. AR-341 brings the filter
//! and paging past the newest [`PAGE`]; AR-342 the narrowing order.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::chat_words::{Clock, civil, clock_time, local, month};
use crate::hold::HoldRefused;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::ux::{self, Palette, icons, types};
use crate::widgets;
use crate::{Shell, VersionEntry, VersionsPage};

/// The versions read: with Current work, eight rows, all in reach until AR-341 pages; a row; the list.
const PAGE: usize = 7;
/// The versions a step reads, apart from the list's until AR-341 folds them together.
const STEP_PAGE: usize = 50;
const ROW: f32 = 30.0;
const WIDTH: f32 = 360.0;
const ID: &str = "workspace.header.version";

/// The list's state between frames.
#[derive(Default)]
pub struct List {
    project: Option<PathBuf>,
    highlight: usize,
    versions: Vec<VersionEntry>,
    pub(crate) reading: Option<Receiver<Result<VersionsPage, String>>>,
    /// The version last picked, and how the chip says it.
    shown: Option<(String, String)>,
    pub(crate) steps: Steps,
}

/// What the steps between versions have read, for which project, and a step waiting for it.
#[derive(Default)]
pub struct Steps {
    project: Option<PathBuf>,
    history: Vec<VersionEntry>,
    pub(crate) reading: Option<Receiver<Result<VersionsPage, String>>>,
    /// The step to take when the read arrives: older when true.
    waiting: Option<bool>,
}

/// A row of the list: the version it views, none for current work; its name; the words beside it.
type Row = (Option<String>, String, String);

/// Whether the list can be asked for: a model shows, and no hold or dialog is up.
pub(crate) fn available(shell: &Shell) -> bool {
    let dialog = shell.ask.is_some() || shell.about.is_some() || shell.generate.is_some() || shell.new_project.is_some() || shell.settings.is_some();
    let panel_dialog = shell.explorer.delete.is_some() || shell.chat_panel.ask.is_some() || shell.diagrams.new.is_some();
    shell.session.is_some() && shell.project.is_some() && !shell.is_held() && !dialog && !panel_dialog
}

/// Opens the list and reads its versions again, when it can be asked for.
pub(crate) fn open(shell: &mut Shell) {
    if available(shell) {
        shell.version_list.project = shell.project.as_ref().map(|p| p.path.clone());
        if let (Some(host), Some(path)) = (shell.services.projects.clone(), shell.version_list.project.clone()) {
            shell.version_list.reading = Some(host.versions(&path, None, PAGE));
        }
    }
}

/// Once a frame: a list whose project is no longer shown, or that a hold or dialog now bars, closes.
pub(crate) fn keep_valid(shell: &mut Shell) {
    let path = shell.project.as_ref().map(|o| o.path.clone());
    let steps = &shell.version_list.steps;
    if steps.project.is_some() && (steps.project != path || (steps.reading.is_some() && shell.is_held())) {
        shell.version_list.steps = Steps::default();
    }
    if shell.viewing().is_some() && !shell.is_held() && shell.version_list.steps.project.is_none() {
        read_steps(shell, None);
    }
    take_steps(shell);
    if shell.version_list.project.is_some() && (shell.version_list.project != shell.project.as_ref().map(|o| o.path.clone()) || !available(shell)) {
        close(shell);
    }
}

/// Closes the list; what it read goes with it.
pub(crate) fn close(shell: &mut Shell) {
    let steps = std::mem::take(&mut shell.version_list.steps);
    shell.version_list = List { shown: shell.version_list.shown.take(), steps, ..List::default() };
}

/// A save finished: a version may have been made, so what the list and the steps read is old.
pub(crate) fn saved(shell: &mut Shell) {
    close(shell);
    shell.version_list.steps = Steps::default();
}

/// ⇧⌘H: back to current work while viewing, otherwise the list.
pub(crate) fn toggle(shell: &mut Shell) {
    if !shell.back_to_current_work() {
        open(shell);
    }
}

/// ⌘[: the next older version.
pub(crate) fn older(shell: &mut Shell) {
    ask_step(shell, true);
}

/// ⌘]: the next newer version.
pub(crate) fn newer(shell: &mut Shell) {
    ask_step(shell, false);
}

/// Takes the step now if the versions are read, otherwise when they arrive; a press while they
/// are on their way is ignored.
fn ask_step(shell: &mut Shell, older: bool) {
    let steps = &shell.version_list.steps;
    if steps.reading.is_some() {
        return;
    }
    if steps.history.is_empty() {
        read_steps(shell, Some(older));
    } else {
        step(shell, older);
    }
}

/// Asks the host for the versions the steps go through.
fn read_steps(shell: &mut Shell, waiting: Option<bool>) {
    if let (Some(host), Some(path)) = (shell.services.projects.clone(), shell.project.as_ref().map(|p| p.path.clone())) {
        let reading = Some(host.versions(&path, None, STEP_PAGE));
        shell.version_list.steps = Steps { project: Some(path), history: Vec::new(), reading, waiting };
    }
}

/// Takes the answer to the steps' read, and the step that waited for it.
fn take_steps(shell: &mut Shell) {
    let steps = &mut shell.version_list.steps;
    match steps.reading.as_ref().map(Receiver::try_recv) {
        None | Some(Err(TryRecvError::Empty)) => return,
        Some(Ok(Ok(page))) => steps.history = page.versions,
        Some(failed) => log::warn!("the versions could not be read: {failed:?}"),
    }
    steps.reading = None;
    if let Some(older) = steps.waiting.take() {
        step(shell, older);
    }
}

/// The version a step goes to: from a version, the one beside it; from current work, going older,
/// the latest when work is unsaved and otherwise the one before it, the first that differs.
fn target(shell: &Shell, older: bool) -> Option<&VersionEntry> {
    let history = &shell.version_list.steps.history;
    let to = match shell.viewing() {
        Some(commit) => {
            let at = history.iter().position(|v| v.hash == commit)?;
            if older { at + 1 } else { at.checked_sub(1)? }
        }
        None if older => usize::from(!shell.shown_unsaved()),
        None => return None,
    };
    history.get(to)
}

/// Goes one version older or newer; at the end of what is read, nowhere.
fn step(shell: &mut Shell, older: bool) {
    let clock = crate::chat_panel::clock(shell);
    if let Some((hash, label)) = target(shell, older).map(|v| (v.hash.clone(), when(&v.date, clock))) {
        view(shell, hash, label);
    }
}

/// Shows the version `hash`, read `label`, from current work or from the version in view, staying
/// on the diagram in view there; a refusal is said at the top of the canvas.
pub(crate) fn view(shell: &mut Shell, hash: String, label: String) {
    shell.version_list.shown = Some((hash.clone(), label));
    if let Err(why) = shell.move_to_version(&hash) {
        shell.canvas.notice = Some(refused_words(&why));
    }
}

/// What a person is told when a version cannot be shown.
fn refused_words(why: &HoldRefused) -> String {
    match why {
        HoldRefused::Gesture => tr("Finish or cancel your edit first."),
        HoldRefused::Another => tr("Wait for the change under way to finish."),
        HoldRefused::NoProject => tr("There is no model to show a version of."),
    }
}

/// Takes the host's answer; a list that cannot be read shows current work alone until AR-341.
fn take_page(shell: &mut Shell) {
    let l = &mut shell.version_list;
    match l.reading.as_ref().map(Receiver::try_recv) {
        None | Some(Err(TryRecvError::Empty)) => return,
        Some(Ok(Ok(page))) => l.versions = page.versions,
        Some(failed) => log::warn!("the versions could not be read: {failed:?}"),
    }
    l.reading = None;
}

/// A stored time, RFC 3339 in UTC, as the list says it, `8 Nov 14:02`, local; else as it is.
pub(crate) fn when(date: &str, clock: Clock) -> String {
    let parts: Vec<i64> = date.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    let [y, m, d, hh, mm, ss] = parts[..] else { return date.to_string() };
    let (era_year, shifted) = if m > 2 { (y, m - 3) } else { (y - 1, m + 9) };
    let (era, yoe) = (era_year.div_euclid(400), era_year.rem_euclid(400));
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + (153 * shifted + 2) / 5 + d - 1 - 719_468;
    let secs = u64::try_from(days * 86_400 + hh * 3_600 + mm * 60 + ss).unwrap_or(0);
    let (_, month_at, day) = civil(local(secs, clock.offset).div_euclid(86_400));
    fmt(&tr("{day} {month} {time}"), &[("day", &day.to_string()), ("month", &month(month_at)), ("time", &clock_time(secs, clock.offset))])
}

/// Current work, then the versions newest first, the latest reading as current work if nothing is unsaved.
fn rows(shell: &Shell) -> Vec<Row> {
    let (clock, unsaved) = (crate::chat_panel::clock(shell), shell.shown_unsaved());
    let mut out = vec![(None, tr("Current work"), if unsaved { tr("Unsaved changes") } else { tr("No unsaved changes") })];
    for (i, v) in shell.version_list.versions.iter().enumerate() {
        let detail = match v.description.as_deref().filter(|d| !d.is_empty()) {
            _ if i == 0 && !unsaved => tr("Same as current work"),
            Some(d) => d.to_string(),
            None if v.by_agent => tr("Saved by an agent"),
            None => String::new(),
        };
        out.push((Some(v.hash.clone()), when(&v.date, clock), detail));
    }
    out
}

/// Runs a row and closes the list.
fn pick(shell: &mut Shell, (hash, label, _): Row) {
    close(shell);
    match hash {
        Some(hash) => view(shell, hash, label),
        None => drop(shell.back_to_current_work()),
    }
}

/// The date of the version in view, when it is the one it was picked with.
fn shown(shell: &Shell) -> Option<&String> {
    let (commit, date) = shell.version_list.shown.as_ref()?;
    (shell.version_in_view() == Some(commit.as_str())).then_some(date)
}

/// What the banner and the status bar say while a version shows.
pub(crate) fn read_only_words(shell: &Shell) -> String {
    match shown(shell) {
        Some(date) => fmt(&tr("Viewing the version of {date} · Read only"), &[("date", date)]),
        None => tr("Viewing an earlier version · Read only"),
    }
}

/// What the chip says, whole and on its face; a date only for the version it was picked with.
fn chip_words(shell: &Shell) -> (String, String) {
    match (shell.version_in_view(), shown(shell)) {
        (Some(_), Some(date)) => (fmt(&tr("Viewing the version of {date}"), &[("date", date)]), date.clone()),
        (Some(_), None) => (tr("Viewing a version"), tr("Viewing a version")),
        (None, _) if shell.shown_unsaved() => (tr("Current work · unsaved changes"), tr("Current work")),
        (None, _) => (tr("Current work"), tr("Current work")),
    }
}

/// The chip left of search, its width by what it says until AR-342 narrows it, and its list; gives where search starts.
pub(crate) fn bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, mid: f32, start: f32, end: f32) -> f32 {
    take_page(shell);
    let width = match (shell.version_in_view(), shell.shown_unsaved()) {
        (Some(_), _) => 282.0,
        (None, true) => 147.0,
        (None, false) => 135.0,
    };
    let search = (end - 300.0).max(start + width + 12.0);
    let r = Rect::from_min_size(pos2(search - 12.0 - width, mid - 16.0), vec2(width, 32.0));
    chip(shell, ui, p, r);
    if shell.version_list.project.is_some() {
        list(shell, ui, p, r);
    }
    search
}

/// The chip: what shows, a dot for unsaved work. Viewing, the steps and the way back.
fn chip(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let ((label, text), viewing, mid, paint) = (chip_words(shell), shell.version_in_view().is_some(), r.center().y, ui.painter().clone());
    paint.rect_filled(r, 8.0, if viewing { p.accent_tint } else { p.surface });
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, if viewing { p.border_hover } else { p.border }), StrokeKind::Inside);
    if viewing {
        return viewing_chip(shell, ui, p, r, (&label, &text));
    }
    let resp = control(shell, ui, ID, "button", r, true).on_hover_text(&label);
    let _ = resp.clone().accessible_name(&label);
    ux::icon_mid(&paint, pos2(r.left() + 16.0, mid), &icons::HISTORY, p.text_secondary);
    let name = ux::text_mid(&paint, r.left() + 30.0, mid, &text, types::UI, p.text, Some(84.0));
    if shell.shown_unsaved() {
        paint.circle_filled(pos2(name.right() + 9.0, mid), 3.0, p.amber);
    }
    ux::icon_mid(&paint, pos2(r.right() - 14.0, mid), &icons::CHEVRON_DOWN, p.text_secondary);
    if resp.clicked() {
        if shell.version_list.project.is_some() { close(shell) } else { open(shell) }
    }
}

/// The chip while a version shows: the steps round its date, which opens the list, and the way back.
fn viewing_chip(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, (label, date): (&str, &str)) {
    let (mid, paint) = (r.center().y, ui.painter().clone());
    step_button(shell, ui, p, r.left() + 20.0, mid, true);
    step_button(shell, ui, p, r.left() + 121.0, mid, false);
    let face = Rect::from_min_max(pos2(r.left() + 34.0, r.top()), pos2(r.left() + 108.0, r.bottom()));
    let resp = control(shell, ui, ID, "button", face, true).on_hover_text(label);
    let _ = resp.clone().accessible_name(label);
    ux::text_mid(&paint, face.left() + 3.0, mid, date, types::SMALL, p.accent, Some(face.width() - 4.0));
    if resp.clicked() {
        if shell.version_list.project.is_some() { close(shell) } else { open(shell) }
    }
    paint.rect_filled(Rect::from_center_size(pos2(r.left() + 137.0, mid), vec2(1.0, 18.0)), 0.0, p.border_hover);
    let back = Rect::from_min_max(pos2(r.left() + 138.0, r.top()), r.max);
    let words = tr("Back to current work");
    let resp = control(shell, ui, &format!("{ID}.back"), "button", back, true).accessible_name(words.as_str());
    ux::text_mid(&paint, back.left() + 12.0, mid, &words, types::SMALL, p.accent, Some(back.width() - 16.0));
    if resp.clicked() {
        shell.back_to_current_work();
    }
}

/// ‹ or ›: muted, unavailable to a screen reader and doing nothing at the end of the history.
fn step_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, x: f32, mid: f32, older: bool) {
    let (key, icon, name, none) = if older {
        ("older", &icons::CHEVRON_LEFT, tr("Older version"), tr("No older version"))
    } else {
        ("newer", &icons::CHEVRON_RIGHT, tr("Newer version"), tr("No newer version"))
    };
    let at = Rect::from_center_size(pos2(x, mid), vec2(24.0, 24.0));
    let (live, id) = (target(shell, older).is_some() && available(shell), format!("{ID}.{key}"));
    widgets::record_visible(ui, &mut shell.registry, &id, "button", at);
    let resp = ui.interact(at, widgets::egui_id(&id), if live { egui::Sense::click() } else { egui::Sense::hover() });
    ui.ctx().accesskit_node_builder(widgets::egui_id(&id), |node| {
        node.set_role(egui::accesskit::Role::Button);
        node.set_label(name.clone());
        if !live {
            node.set_disabled();
        }
    });
    ux::icon_mid(ui.painter(), at.center(), icon, if live { p.text_secondary } else { p.text_muted });
    if resp.on_hover_text(if live { name } else { none }).clicked() {
        step(shell, older);
    }
}

/// Moves the highlight by the keys, runs Enter and closes on Esc; true when the list is gone.
fn keys(shell: &mut Shell, ui: &egui::Ui, rows: &[Row]) -> bool {
    let press = |key| ui.input_mut(|i| i.consume_key(Modifiers::NONE, key));
    let (down, up, enter, esc) = (press(Key::ArrowDown), press(Key::ArrowUp), press(Key::Enter), press(Key::Escape));
    if esc {
        close(shell);
        return true;
    }
    let last = rows.len() - 1;
    let l = &mut shell.version_list;
    l.highlight = (l.highlight + usize::from(down)).min(last).saturating_sub(usize::from(up));
    if enter {
        let row = rows[l.highlight.min(last)].clone();
        pick(shell, row);
    }
    enter
}

/// The list under the chip.
fn list(shell: &mut Shell, ui: &egui::Ui, p: &Palette, chip: Rect) {
    let rows = rows(shell);
    let height = 12.0 + rows.len() as f32 * ROW;
    let r = Rect::from_min_size(pos2(chip.right() - WIDTH, chip.bottom() + 4.0), vec2(WIDTH, height));
    if keys(shell, ui, &rows) {
        return;
    }
    let highlight = shell.version_list.highlight;
    let picked = ux::overlay(ui, widgets::egui_id(&format!("{ID}.list")), r, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, r, 10.0, p.shadow);
        paint.rect_filled(r, 10.0, p.surface);
        paint.rect_stroke(r, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let mut picked = None;
        let mut y = r.top() + 6.0;
        for (n, row @ (_, label, detail)) in rows.iter().enumerate() {
            let at = Rect::from_min_size(pos2(r.left() + 6.0, y), vec2(WIDTH - 12.0, ROW));
            let resp = control(shell, ui, &format!("{ID}.row.{n}"), "menu-item", at, true);
            let _ = resp.clone().accessible_name(format!("{label}, {detail}"));
            if n == highlight || resp.hovered() {
                paint.rect_filled(at, 6.0, p.accent_tint);
            }
            let name = ux::text_mid(&paint, at.left() + 10.0, at.center().y, label, types::UI, p.text, Some(WIDTH * 0.45));
            let room = (at.right() - 10.0 - (name.right() + 12.0)).max(0.0);
            let g = ux::galley(&paint, detail, types::CAPTION, p.text_secondary, Some(room));
            paint.galley(pos2(at.right() - 10.0 - g.size().x, at.center().y - g.size().y / 2.0), g, p.text_secondary);
            if resp.clicked() {
                picked = Some(row.clone());
            }
            y += ROW;
        }
        picked
    });
    if let Some(row) = picked {
        pick(shell, row);
    } else if ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|at| r.contains(at) || chip.contains(at))) {
        close(shell);
    }
}
