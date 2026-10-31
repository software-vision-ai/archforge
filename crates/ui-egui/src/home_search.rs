//! Home's search field: a quick switcher over the recent projects, the pinned folders and what
//! Home can do. A click on the field or the command key with K opens it; typing narrows the list
//! under it, every word of the query found in a result's name (or, for a project or a folder, in
//! where it lives); the arrow keys move the highlight, Enter runs it, Esc or a click elsewhere
//! closes the list.

use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::home::control;
use crate::i18n::tr;
use crate::state::{RecentSort, RecentView};
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// The search's state between frames: whether its list is open, the query, the highlighted row,
/// and whether the field is still to take the keyboard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    pub open: bool,
    pub query: String,
    pub highlight: usize,
    focus: bool,
}

/// The longest query kept, the most projects and folders the list shows, a row's height and the
/// list's least width.
const MAX_QUERY: usize = 120;
const MAX_PROJECTS: usize = 6;
const MAX_FOLDERS: usize = 4;
const ROW: f32 = 36.0;
const LIST_WIDTH: f32 = 420.0;

/// Something the list offers.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    Project(std::path::PathBuf),
    Folder(std::path::PathBuf),
    Action(Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    NewProject,
    OpenProject,
    PinFolder,
    Theme,
    View,
    Sort,
}

/// A row of the list: what it runs, its icon, its name and the line beside it.
struct Hit {
    target: Target,
    icon: &'static ux::Icon,
    label: String,
    detail: String,
}

/// Opens the search with the keyboard: the command key with K, while Home shows.
pub(crate) fn shortcut(shell: &mut Shell, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::K))) {
        open(shell);
    }
}

fn open(shell: &mut Shell) {
    shell.home_search.open = true;
    shell.home_search.focus = true;
    shell.home_search.highlight = 0;
}

fn close(shell: &mut Shell) {
    shell.home_search = Search::default();
}

/// Whether every word of `query` is in one of `texts`, whatever the case.
fn matches(query: &str, texts: &[&str]) -> bool {
    let texts: Vec<String> = texts.iter().map(|t| t.to_lowercase()).collect();
    query.to_lowercase().split_whitespace().all(|w| texts.iter().any(|t| t.contains(w)))
}

/// The rows for the query: the matching recent projects, pinned folders and actions, in that
/// order; with no query, the recent projects and every action.
fn hits(shell: &Shell, query: &str) -> Vec<(String, Vec<Hit>)> {
    let mut groups = Vec::new();
    let projects: Vec<Hit> = shell
        .state
        .recent
        .iter()
        .filter(|r| matches(query, &[&r.name, &r.path.to_string_lossy()]))
        .take(MAX_PROJECTS)
        .map(|r| Hit {
            target: Target::Project(r.path.clone()),
            icon: &icons::ER_DIAGRAM,
            label: r.name.clone(),
            detail: shell.location_of(&r.path).unwrap_or_default(),
        })
        .collect();
    if !projects.is_empty() {
        groups.push((tr("Projects"), projects));
    }
    let folders: Vec<Hit> = shell
        .state
        .pinned
        .iter()
        .filter(|f| matches(query, &[&f.to_string_lossy()]))
        .take(MAX_FOLDERS)
        .map(|f| Hit {
            target: Target::Folder(f.clone()),
            icon: &icons::FOLDER,
            label: f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            detail: tr("Open a project here"),
        })
        .collect();
    if !folders.is_empty() {
        groups.push((tr("Pinned folders"), folders));
    }
    let actions: Vec<Hit> = actions(shell).into_iter().filter(|h| matches(query, &[&h.label])).collect();
    if !actions.is_empty() {
        groups.push((tr("Actions"), actions));
    }
    groups
}

/// What Home can do now, worded for the state it is in.
fn actions(shell: &Shell) -> Vec<Hit> {
    let mut out = Vec::new();
    let mut add = |action: Action, icon: &'static ux::Icon, label: String| {
        out.push(Hit { target: Target::Action(action), icon, label, detail: String::new() });
    };
    if shell.services.projects.is_some() && shell.tabs.len() < crate::MAX_TABS {
        add(Action::NewProject, &icons::PLUS, tr("New project"));
    }
    if crate::home::can_open(shell) {
        add(Action::OpenProject, &icons::FOLDER, tr("Open project"));
    }
    if shell.services.pick_folder.is_some() && shell.state.pinned.len() < crate::state::MAX_PINNED {
        add(Action::PinFolder, &icons::PLUS, tr("Pin a folder"));
    }
    if shell.state.theme.is_dark() {
        add(Action::Theme, &icons::SUN, tr("Switch to the light appearance"));
    } else {
        add(Action::Theme, &icons::MOON, tr("Switch to the dark appearance"));
    }
    match shell.state.recent_view {
        RecentView::List => add(Action::View, &icons::GRID, tr("Show recent projects as a gallery")),
        RecentView::Gallery => add(Action::View, &icons::LIST, tr("Show recent projects as a list")),
    }
    match shell.state.recent_sort {
        RecentSort::LastOpened => add(Action::Sort, &icons::LIST, tr("Sort recent projects by name")),
        RecentSort::Name => add(Action::Sort, &icons::LIST, tr("Sort recent projects by last opened")),
    }
    out
}

/// Runs a row and closes the search.
fn run(shell: &mut Shell, target: Target) {
    close(shell);
    match target {
        Target::Project(path) => shell.open_path(&path),
        Target::Folder(path) => shell.open_from_picker_in(Some(&path)),
        Target::Action(Action::NewProject) => crate::new_project::open(shell),
        Target::Action(Action::OpenProject) => shell.open_from_picker(),
        Target::Action(Action::PinFolder) => shell.pin_from_picker(),
        Target::Action(Action::Theme) => {
            shell.state.theme = if shell.state.theme.is_dark() { crate::theme::ThemeKind::Light } else { crate::theme::ThemeKind::Dark };
        }
        Target::Action(Action::View) => {
            shell.state.recent_view = match shell.state.recent_view {
                RecentView::List => RecentView::Gallery,
                RecentView::Gallery => RecentView::List,
            };
        }
        Target::Action(Action::Sort) => {
            shell.state.recent_sort = match shell.state.recent_sort {
                RecentSort::LastOpened => RecentSort::Name,
                RecentSort::Name => RecentSort::LastOpened,
            };
        }
    }
}

/// The field in the bar at `field`, with its placeholder and shortcut while closed, the query
/// and its list while open.
pub(crate) fn field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect, placeholder: &str) {
    let paint = ui.painter().clone();
    let mid = field.center().y;
    let open_now = shell.home_search.open;
    paint.rect_filled(field, 15.0, if open_now { p.surface } else { p.page });
    paint.rect_stroke(field, 15.0, Stroke::new(1.0, if open_now { p.accent } else { p.border }), StrokeKind::Inside);
    ux::icon_mid(&paint, pos2(field.left() + 18.0, mid), &icons::SEARCH, p.text_secondary);
    let chord = if ui.ctx().os() == egui::os::OperatingSystem::Mac { "⌘K" } else { "Ctrl K" };
    let key = ux::galley(&paint, chord, types::CAPTION, p.text_muted, None);
    let key_at = pos2(field.right() - 14.0 - key.size().x, mid - key.size().y / 2.0);
    let text = Rect::from_min_max(pos2(field.left() + 32.0, field.top()), pos2((key_at.x - 8.0).max(field.left() + 32.0), field.bottom()));
    if !open_now {
        paint.galley(key_at, key, p.text_muted);
        ux::text_mid(&paint, text.left(), mid, placeholder, types::UI, p.text_secondary, Some(text.width()));
        if control(shell, ui, "home.header.search", "field", field, true).clicked() {
            open(shell);
        }
        return;
    }
    widgets::record_visible(ui, &mut shell.registry, "home.header.search", "field", field);
    // The list's keys before the field sees them: the arrows move, Enter runs, Esc closes.
    let groups = hits(shell, &shell.home_search.query.clone());
    let count = groups.iter().map(|(_, h)| h.len()).sum::<usize>();
    let (up, down, enter, escape) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if escape {
        close(shell);
        return;
    }
    let search = &mut shell.home_search;
    if count > 0 {
        if down {
            search.highlight = (search.highlight + 1) % count;
        }
        if up {
            search.highlight = (search.highlight + count - 1) % count;
        }
        search.highlight = search.highlight.min(count - 1);
    }
    if enter && count > 0 {
        let target = groups.iter().flat_map(|(_, h)| h).nth(shell.home_search.highlight).map(|h| h.target.clone());
        if let Some(target) = target {
            run(shell, target);
        }
        return;
    }
    let row = ui.fonts_mut(|f| f.row_height(&types::UI.font()));
    let inner = Rect::from_min_max(pos2(text.left(), mid - row / 2.0), pos2(text.right(), mid + row / 2.0));
    let id = widgets::egui_id("home.header.search.input");
    let before = shell.home_search.query.clone();
    let edit = egui::TextEdit::singleline(&mut shell.home_search.query)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::UI.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(placeholder).color(p.text_secondary).font(types::UI.font()))
        .char_limit(MAX_QUERY)
        .desired_width(inner.width());
    let resp = ui.put(inner, edit);
    let _ = resp.clone().accessible_name(placeholder);
    if std::mem::take(&mut shell.home_search.focus) {
        resp.request_focus();
    }
    if shell.home_search.query != before {
        shell.home_search.highlight = 0;
    }
    list(shell, ui, p, field, &groups);
}

/// The list under the field, its rows grouped under their headings; a click elsewhere closes it.
fn list(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, groups: &[(String, Vec<Hit>)]) {
    let rows = groups.iter().map(|(_, h)| h.len()).sum::<usize>() as f32;
    let height = if groups.is_empty() { 52.0 } else { 8.0 + groups.len() as f32 * 28.0 + rows * ROW };
    let width = field.width().max(LIST_WIDTH);
    let r = Rect::from_min_size(pos2(field.right() - width, field.bottom() + 6.0), vec2(width, height));
    let picked = ux::overlay(ui, widgets::egui_id("home.header.search.list"), r, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, r, 10.0, p.shadow);
        paint.rect_filled(r, 10.0, p.surface);
        paint.rect_stroke(r, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        if groups.is_empty() {
            ux::text_mid(&paint, r.left() + 16.0, r.center().y, &tr("No matches"), types::UI, p.text_secondary, Some(r.width() - 32.0));
            return None;
        }
        let mut picked = None;
        let mut y = r.top() + 4.0;
        let mut n = 0;
        for (heading, hits) in groups {
            ux::text(&paint, pos2(r.left() + 16.0, y + 10.0), &heading.to_uppercase(), types::EYEBROW, p.text_secondary, Some(r.width() - 32.0));
            y += 28.0;
            for hit in hits {
                let row = Rect::from_min_size(pos2(r.left() + 4.0, y), vec2(r.width() - 8.0, ROW));
                let resp = control(shell, ui, &format!("home.header.search.result.{n}"), "menu-item", row, true);
                if resp.hovered() {
                    shell.home_search.highlight = n;
                }
                if n == shell.home_search.highlight {
                    paint.rect_filled(row, 6.0, p.accent_tint);
                }
                let on = n == shell.home_search.highlight;
                ux::icon_mid(&paint, pos2(row.left() + 20.0, row.center().y), hit.icon, if on { p.accent } else { p.text_secondary });
                let name = ux::text_mid(&paint, row.left() + 40.0, row.center().y, &hit.label, types::UI, p.text, Some(row.width() * 0.6));
                if !hit.detail.is_empty() {
                    let room = (row.right() - 12.0 - (name.right() + 12.0)).max(0.0);
                    let g = ux::galley(&paint, &hit.detail, types::CAPTION, p.text_secondary, Some(room));
                    paint.galley(pos2(row.right() - 12.0 - g.size().x, row.center().y - g.size().y / 2.0), g, p.text_secondary);
                }
                if resp.clicked() {
                    picked = Some(hit.target.clone());
                }
                y += ROW;
                n += 1;
            }
        }
        picked
    });
    if let Some(target) = picked {
        run(shell, target);
        return;
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || field.contains(pos)));
    if outside {
        close(shell);
    }
}
