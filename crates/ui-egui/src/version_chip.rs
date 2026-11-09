//! The version chip in the project bar and its list (`docs/design/versioning.md`, section 2): what
//! the project shows, and the versions to view. Transitional until AR-339, which brings the full
//! viewing chip, its steps, the filter, paging past the newest [`PAGE`] and the narrowing order.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::chat_words::{Clock, civil, clock_time, local, month};
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::ux::{self, Palette, icons, types};
use crate::widgets;
use crate::{Shell, VersionEntry, VersionsPage};

/// The versions read: with Current work, eight rows, all in reach until AR-339 pages; a row; the list.
const PAGE: usize = 7;
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
    if shell.version_list.project.is_some() && (shell.version_list.project != shell.project.as_ref().map(|o| o.path.clone()) || !available(shell)) {
        close(shell);
    }
}

/// Closes the list; what it read goes with it.
pub(crate) fn close(shell: &mut Shell) {
    shell.version_list = List { shown: shell.version_list.shown.take(), ..List::default() };
}

/// Takes the host's answer; a list that cannot be read shows current work alone until AR-339.
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

/// Runs a row and closes the list. A refusal goes to the log until AR-339 words it.
fn pick(shell: &mut Shell, (hash, label, _): Row) {
    close(shell);
    let Some(hash) = hash else {
        shell.back_to_current_work();
        return;
    };
    shell.leave_version();
    shell.version_list.shown = Some((hash.clone(), label));
    if let Err(e) = shell.view_version(&hash) {
        log::warn!("viewing {hash} was refused: {e:?}");
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

/// The chip left of search, its width fixed until AR-339, and its list; gives where search starts.
pub(crate) fn bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, mid: f32, start: f32, end: f32) -> f32 {
    take_page(shell);
    let search = (end - 300.0).max(start + 159.0);
    let width = if shell.shown_unsaved() && shell.version_in_view().is_none() { 147.0 } else { 135.0 };
    let r = Rect::from_min_size(pos2(search - 12.0 - width, mid - 16.0), vec2(width, 32.0));
    chip(shell, ui, p, r);
    if shell.version_list.project.is_some() {
        list(shell, ui, p, r);
    }
    search
}

/// The chip: what shows, a dot for unsaved work. Viewing, tinted with the date until AR-339.
fn chip(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let ((label, text), viewing, mid, paint) = (chip_words(shell), shell.version_in_view().is_some(), r.center().y, ui.painter().clone());
    paint.rect_filled(r, 8.0, if viewing { p.accent_tint } else { p.surface });
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, if viewing { p.accent } else { p.border }), StrokeKind::Inside);
    let resp = control(shell, ui, ID, "button", r, true).on_hover_text(&label);
    let _ = resp.clone().accessible_name(&label);
    ux::icon_mid(&paint, pos2(r.left() + 16.0, mid), &icons::HISTORY, if viewing { p.accent } else { p.text_secondary });
    let name = ux::text_mid(&paint, r.left() + 30.0, mid, &text, types::UI, if viewing { p.accent } else { p.text }, Some(84.0));
    if shell.shown_unsaved() && !viewing {
        paint.circle_filled(pos2(name.right() + 9.0, mid), 3.0, p.amber);
    }
    ux::icon_mid(&paint, pos2(r.right() - 14.0, mid), &icons::CHEVRON_DOWN, p.text_secondary);
    if resp.clicked() {
        if shell.version_list.project.is_some() { close(shell) } else { open(shell) }
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
