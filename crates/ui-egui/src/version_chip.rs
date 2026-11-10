//! The version chip in the project bar and its list (`docs/design/versioning.md`, section 2): what
//! the project shows, the versions to view and the steps between them, both over one history that
//! reads on a [`PAGE`] at a time.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Key, Modifiers, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::chat_words::{Clock, civil, clock_time, local, month};
use crate::explorer::{TreeScroll, scroll_tree};
use crate::hold::HoldRefused;
use crate::home::{SEARCH_MIN, control};
use crate::i18n::{fmt, tr};
use crate::settings::{Look, button};
use crate::ux::{self, Palette, icons, types};
use crate::widgets;
use crate::{Shell, VersionEntry, VersionsPage};

/// The versions one read brings, for the list and the steps alike.
const PAGE: usize = 50;
/// The rows the list shows before it scrolls; a row; the list; its filter field.
const MOST_ROWS: usize = 10;
const ROW: f32 = 30.0;
const WIDTH: f32 = 360.0;
const FIELD: f32 = 28.0;
const ID: &str = "workspace.header.version";
/// The chip at its narrowest: one icon.
pub(crate) const NARROWEST: f32 = 32.0;
/// Between the chip and search.
const SEARCH_GAP: f32 = 12.0;

/// The list's state between frames.
#[derive(Default)]
pub struct List {
    project: Option<PathBuf>,
    highlight: usize,
    filter: String,
    scroll: TreeScroll,
    /// The version last picked, and how the chip says it.
    pub(crate) shown: Option<(String, String)>,
    pub(crate) steps: Steps,
}

/// What the list and the steps between versions have read, for which project, and a step waiting
/// for it.
#[derive(Default)]
pub struct Steps {
    project: Option<PathBuf>,
    history: Vec<VersionEntry>,
    /// Whether older versions follow what is read.
    more: bool,
    /// The one read under way: the newest page, or the page after `history`.
    pub(crate) reading: Option<Receiver<Result<VersionsPage, String>>>,
    /// The last read failed; what was read before it stays.
    failed: bool,
    /// The step to take when the read arrives: older when true.
    waiting: Option<bool>,
}

/// A row of the list: the version it views, none for current work; its name; the words beside it.
type Row = (Option<String>, String, String);

/// Whether a dialog or question is open over the window.
pub(crate) fn dialog_open(shell: &Shell) -> bool {
    let dialog = shell.ask.is_some() || shell.about.is_some() || shell.generate.is_some() || shell.new_project.is_some() || shell.settings.is_some();
    dialog || shell.explorer.delete.is_some() || shell.chat_panel.ask.is_some() || shell.diagrams.new.is_some() || shell.versions.dialog.is_some()
}

/// Whether the list can be asked for: a model shows, and no hold or dialog is up.
pub(crate) fn available(shell: &Shell) -> bool {
    shell.session.is_some() && shell.project.is_some() && !shell.is_held() && !dialog_open(shell)
}

/// Opens the list when it can be asked for: over what is read, which a save, a restore or a branch
/// switch clears, reading only when nothing is. A list already open stays as it is.
pub(crate) fn open(shell: &mut Shell) {
    if available(shell) && shell.version_list.project.is_none() {
        let path = shell.project.as_ref().map(|p| p.path.clone());
        shell.version_list.project.clone_from(&path);
        let steps = &mut shell.version_list.steps;
        steps.failed = false;
        if steps.project != path || (steps.history.is_empty() && steps.reading.is_none()) {
            read_steps(shell, None);
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
    if !steps.history.is_empty() {
        step(shell, older);
    } else if steps.reading.is_none() {
        read_steps(shell, Some(older));
    }
}

/// Asks the host for the newest versions, in place of all that is read.
fn read_steps(shell: &mut Shell, waiting: Option<bool>) {
    if let (Some(host), Some(path)) = (shell.services.projects.clone(), shell.project.as_ref().map(|p| p.path.clone())) {
        let reading = Some(host.versions(&path, None, PAGE));
        shell.version_list.steps = Steps { project: Some(path), reading, waiting, ..Steps::default() };
    }
}

/// Asks the host for the page after what is read.
fn read_on(shell: &mut Shell) {
    let steps = &mut shell.version_list.steps;
    if let (Some(host), Some(path), Some(after)) =
        (shell.services.projects.clone(), steps.project.clone(), steps.history.last().map(|v| v.hash.clone()))
    {
        steps.failed = false;
        steps.reading = Some(host.versions(&path, Some(after), PAGE));
    }
}

/// Takes the answer to the read, and the step that waited for it. A failed read keeps what was
/// read and marks the list.
fn take_steps(shell: &mut Shell) {
    let steps = &mut shell.version_list.steps;
    match steps.reading.as_ref().map(Receiver::try_recv) {
        None | Some(Err(TryRecvError::Empty)) => return,
        Some(Ok(Ok(page))) => {
            // An empty page ends the list whatever it says, so reading on always stops.
            steps.more = page.more && !page.versions.is_empty();
            steps.history.extend(page.versions);
        }
        Some(failed) => {
            log::warn!("the versions could not be read: {failed:?}");
            steps.failed = true;
        }
    }
    steps.reading = None;
    if let Some(older) = steps.waiting.take() {
        step(shell, older);
    }
}

/// The version a step goes to: from a version, or one on its way, the one beside it; from current
/// work, going older, the latest when work is unsaved and otherwise the one before it, the first
/// that differs.
fn target(shell: &Shell, older: bool) -> Option<&VersionEntry> {
    let history = &shell.version_list.steps.history;
    let to = match shell.stepping_from() {
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
/// on the diagram in view there; gives whether the version is on its way. A refusal is said at
/// the top of the centre; one that goes through ends what was said before it. A refusal leaves
/// what the chip says of the version in view as it was. Chat's question counts as not on its way.
pub(crate) fn view(shell: &mut Shell, hash: String, label: String) -> bool {
    let done = shell.move_to_version(&hash, Some(label));
    shell.canvas.notice = done.err().and_then(|why| refused_words(shell, why));
    done.is_ok()
}

/// What a person is told when a version cannot be shown; nothing for Chat, whose question says it.
pub(crate) fn refused_words(shell: &Shell, why: HoldRefused) -> Option<String> {
    Some(match why {
        HoldRefused::ChatRunning => return None,
        HoldRefused::Gesture | HoldRefused::Editing => tr("Finish or cancel your edit first."),
        HoldRefused::Another => tr("Wait for the change under way to finish."),
        HoldRefused::NoProject => tr("There is no model to show a version of."),
        HoldRefused::Description if crate::panels::refused_description(&shell.panels) => shell.panels.notice.clone().unwrap_or_default(),
        HoldRefused::Description => crate::skill_tab::unkept_reason(),
    })
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

/// Current work, then the versions read, newest first, the latest reading as current work if
/// nothing is unsaved; those the filter holds, by what a row says.
fn rows(shell: &Shell) -> Vec<Row> {
    let (clock, unsaved) = (crate::chat_panel::clock(shell), shell.shown_unsaved());
    let mut out = vec![(None, tr("Current work"), if unsaved { tr("Unsaved changes") } else { tr("No unsaved changes") })];
    for (i, v) in shell.version_list.steps.history.iter().enumerate() {
        let detail = match v.description.as_deref().filter(|d| !d.is_empty()) {
            _ if i == 0 && !unsaved => tr("Same as current work"),
            Some(d) => d.to_string(),
            None if v.by_agent => tr("Saved by an agent"),
            None => String::new(),
        };
        out.push((Some(v.hash.clone()), when(&v.date, clock), detail));
    }
    let needle = shell.version_list.filter.trim().to_lowercase();
    out.retain(|(_, name, detail)| name.to_lowercase().contains(&needle) || detail.to_lowercase().contains(&needle));
    out
}

/// Runs a row and closes the list.
fn pick(shell: &mut Shell, (hash, label, _): Row) {
    match hash {
        Some(hash) => {
            // The list stays on a refusal, as the row can be tried again.
            if view(shell, hash, label) {
                close(shell);
            }
        }
        None => {
            close(shell);
            shell.back_to_current_work();
        }
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

/// The chip left of search and its list; gives where search starts. Its label goes before search
/// does: without room for the whole chip and search it is one icon, and without room for search as
/// well it sits against Help.
pub(crate) fn bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, mid: f32, start: f32, end: f32) -> f32 {
    let width = match (shell.version_in_view(), shell.shown_unsaved()) {
        (Some(_), _) => 282.0,
        (None, true) => 147.0,
        (None, false) => 135.0,
    };
    let narrow = end - start < width + SEARCH_GAP + SEARCH_MIN;
    let width = if narrow { NARROWEST } else { width };
    let search = (end - 300.0).max(start + width + SEARCH_GAP);
    let right = if end - search < SEARCH_MIN { end } else { search - SEARCH_GAP };
    let r = Rect::from_min_size(pos2(right - width, mid - 16.0), vec2(width, 32.0));
    if narrow {
        icon_chip(shell, ui, p, r);
    } else {
        chip(shell, ui, p, r);
    }
    if shell.version_list.project.is_some() {
        list(shell, ui, p, r);
    }
    search
}

/// Opens the list, or closes it when it is open.
fn toggle_list(shell: &mut Shell) {
    if shell.version_list.project.is_some() { close(shell) } else { open(shell) }
}

/// The chip at its narrowest: the clock, untinted even while viewing, and a dot for unsaved work; the
/// whole label in its tooltip and accessible name, and a click opens the list.
fn icon_chip(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let label = if shell.version_in_view().is_some() { read_only_words(shell) } else { chip_words(shell).0 };
    let resp = control(shell, ui, ID, "button", r, true).on_hover_text(&label);
    let _ = resp.clone().accessible_name(&label);
    ux::icon_mid(&paint, r.center(), &icons::VERSION_CLOCK, p.text_secondary);
    if shell.shown_unsaved() {
        paint.circle_filled(pos2(r.left() + 24.0, r.top() + 8.0), 3.0, p.amber);
    }
    if resp.clicked() {
        toggle_list(shell);
    }
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
    ux::icon_mid(&paint, pos2(r.left() + 18.0, mid), &icons::VERSION_CLOCK, p.text_secondary);
    let name = ux::text_mid(&paint, r.left() + 31.0, mid, &text, types::SMALL, p.text, Some(84.0));
    if shell.shown_unsaved() {
        paint.circle_filled(pos2(name.right() + 9.0, mid), 3.0, p.amber);
    }
    ux::icon_mid(&paint, pos2(r.right() - 15.0, mid), &icons::CHEVRON_DOWN, p.text_secondary);
    if resp.clicked() {
        toggle_list(shell);
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
        toggle_list(shell);
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

/// ‹ or ›: muted, unavailable to a screen reader and doing nothing at the end of the history. When
/// nothing could be read it stays live, to read again.
fn step_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, x: f32, mid: f32, older: bool) {
    let (key, icon, name, none) = if older {
        ("older", &icons::CHEVRON_LEFT, tr("Older version"), tr("No older version"))
    } else {
        ("newer", &icons::CHEVRON_RIGHT, tr("Newer version"), tr("No newer version"))
    };
    let at = Rect::from_center_size(pos2(x, mid), vec2(24.0, 24.0));
    let unread = shell.version_list.steps.failed && shell.version_list.steps.history.is_empty();
    let (live, id) = ((target(shell, older).is_some() || unread) && available(shell), format!("{ID}.{key}"));
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
        ask_step(shell, older);
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
    let last = rows.len().saturating_sub(1);
    let l = &mut shell.version_list;
    l.highlight = (l.highlight + usize::from(down)).min(last).saturating_sub(usize::from(up));
    match rows.get(l.highlight).filter(|_| enter) {
        Some(row) => {
            pick(shell, row.clone());
            true
        }
        None => false,
    }
}

/// The field that narrows the list as it is typed. It keeps the keyboard while the list is open,
/// so no key meant for it reaches the window behind.
fn filter_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 14.0, p.page);
    paint.rect_stroke(field, 14.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon_mid(&paint, pos2(field.left() + 17.0, field.center().y), &icons::SEARCH, p.text_secondary);
    let id = format!("{ID}.filter");
    widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
    let inner = crate::panels::field_inner(ui, field.with_min_x(field.left() + 22.0));
    let words = tr("Filter versions…");
    let edit = egui::TextEdit::singleline(&mut shell.version_list.filter)
        .id(widgets::egui_id(&id))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(words.clone()).color(p.text_secondary).font(types::SMALL.font()))
        .char_limit(256)
        .desired_width(inner.width());
    let resp = ui.put(inner, edit);
    resp.request_focus();
    if resp.changed() {
        (shell.version_list.highlight, shell.version_list.scroll) = (0, TreeScroll::default());
    }
    let _ = resp.accessible_name(words);
}

/// Under the rows: what could not be read, the newest versions or the next ones, and Retry for it.
fn failure_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let first = shell.version_list.steps.history.is_empty();
    let words = if first { tr("The versions couldn't be read.") } else { tr("The next versions couldn't be read.") };
    let label = tr("Retry");
    let width = (ux::galley(ui.painter(), &label, types::UI, p.text, None).size().x + 20.0).ceil();
    let area = Rect::from_min_max(r.min, pos2(r.right() - width - 6.0, r.bottom()));
    let _ = control(shell, ui, &format!("{ID}.failed"), "status", area, false).accessible_name(words.as_str());
    ux::text_mid(ui.painter(), r.left() + 10.0, r.center().y, &words, types::SMALL, p.danger, Some((area.width() - 16.0).max(0.0)));
    if button(shell, ui, p, &format!("{ID}.retry"), pos2(r.right() - width, r.top() + 1.0), &label, Look::Secondary, true, None).1.clicked() {
        if first { read_steps(shell, None) } else { read_on(shell) }
    }
}

/// The list under the chip: the filter, the rows the wheel and the keys move through, reading on
/// as the end comes into view, and what could not be read.
fn list(shell: &mut Shell, ui: &egui::Ui, p: &Palette, chip: Rect) {
    let rows = rows(shell);
    if keys(shell, ui, &rows) {
        return;
    }
    let failed = shell.version_list.steps.failed;
    let footer = if failed { ROW } else { 0.0 };
    let top = chip.bottom() + 4.0;
    let wanted = 6.0 + FIELD + 6.0 + rows.len().clamp(1, MOST_ROWS) as f32 * ROW + footer + 8.0;
    let room = (ui.ctx().content_rect().bottom() - 8.0 - top).max(6.0 + FIELD + 6.0 + ROW + footer + 8.0);
    let r = Rect::from_min_size(pos2(chip.right() - WIDTH, top), vec2(WIDTH, wanted.min(room)));
    let highlight = shell.version_list.highlight;
    let picked = ux::overlay(ui, widgets::egui_id(&format!("{ID}.list")), r, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, r, 10.0, p.shadow);
        paint.rect_filled(r, 10.0, p.surface);
        paint.rect_stroke(r, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        filter_field(shell, ui, p, Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(WIDTH - 12.0, FIELD)));
        let first = r.top() + 6.0 + FIELD + 6.0;
        let bottom = r.bottom() - 8.0 - footer;
        // The highlight's place stands for its row: scroll_tree follows a row when its id changes.
        let follow = Some((shell.version_list.highlight, archforge_mof::Uuid((shell.version_list.highlight as u128).to_be_bytes())));
        let tree = Rect::from_min_max(pos2(r.left(), first), pos2(r.right() - 1.0, bottom + 8.0));
        let offset = scroll_tree(ui, p, tree, first, rows.len(), ROW, 0.0, true, follow, &mut shell.version_list.scroll);
        let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
        clip.set_clip_rect(Rect::from_min_max(pos2(r.left(), first), pos2(r.right(), bottom)));
        let mut picked = None;
        for (n, row @ (_, label, detail)) in rows.iter().enumerate() {
            let y = first + n as f32 * ROW - offset;
            if y + ROW <= first || y >= bottom {
                continue;
            }
            let at = Rect::from_min_size(pos2(r.left() + 6.0, y), vec2(WIDTH - 12.0, ROW));
            let resp = control(shell, &clip, &format!("{ID}.row.{n}"), "menu-item", at, true);
            let _ = resp.clone().accessible_name(format!("{label}, {detail}"));
            if n == highlight || resp.hovered() {
                clip.painter().rect_filled(at, 6.0, p.accent_tint);
            }
            let name = ux::text_mid(clip.painter(), at.left() + 10.0, at.center().y, label, types::UI, p.text, Some(WIDTH * 0.45));
            let room = (at.right() - 10.0 - (name.right() + 12.0)).max(0.0);
            let g = ux::galley(clip.painter(), detail, types::CAPTION, p.text_secondary, Some(room));
            clip.painter().galley(pos2(at.right() - 10.0 - g.size().x, at.center().y - g.size().y / 2.0), g, p.text_secondary);
            if resp.clicked() {
                picked = Some(row.clone());
            }
        }
        let steps = &shell.version_list.steps;
        let reading_on = steps.more && !steps.failed;
        if rows.is_empty() && !failed {
            let words = if steps.reading.is_some() || reading_on { tr("Loading") } else { tr("Nothing matches the filter") };
            ux::text_mid(&paint, r.left() + 16.0, first + ROW / 2.0, &words, types::SMALL, p.text_muted, Some(WIDTH - 32.0));
        }
        if failed {
            failure_row(shell, ui, p, Rect::from_min_size(pos2(r.left() + 6.0, bottom), vec2(WIDTH - 12.0, ROW)));
        } else if reading_on && steps.reading.is_none() && offset + (bottom - first) + ROW >= rows.len() as f32 * ROW {
            read_on(shell);
        }
        picked
    });
    if let Some(row) = picked {
        pick(shell, row);
    } else if ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|at| r.contains(at) || chip.contains(at))) {
        close(shell);
    }
}
