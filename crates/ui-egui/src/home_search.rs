//! The top bar's search field. On Home, a quick switcher over the recent projects, the projects in
//! the pinned folders (the host looks for them when the search opens, one search at a time), the
//! pinned folders and what Home can do; in a project, its diagrams and elements
//! (crate::project_search). A click on the field or the command key with K opens it; typing
//! narrows the list under it, every word of the query found in a result's name (or in where it
//! lives); the arrow keys move the highlight, Enter runs it, Esc or a click elsewhere closes the
//! list.

use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::home::control;
use crate::i18n::tr;
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// The search's state between frames: whether its list is open, the query, the highlighted row,
/// whether the field is still to take the keyboard, the projects last found in the pinned folders
/// and the host's answer still to come. Both outlive closing the list, so one search of the folders
/// runs at a time however often the list opens.
#[derive(Debug, Default)]
pub struct Search {
    pub open: bool,
    pub query: String,
    pub highlight: usize,
    focus: bool,
    found: Vec<Found>,
    finding: Option<std::sync::mpsc::Receiver<Vec<crate::FoundProject>>>,
    /// Where the list was opened (a project's folder, none for Home): moving elsewhere closes it.
    scope: Option<std::path::PathBuf>,
    /// The open project's index, and the project and model revision it was made from.
    index: Vec<crate::project_search::Entry>,
    indexed: Option<(std::path::PathBuf, u64)>,
}

/// A project found in a pinned folder, as its row shows it: where it is, its name, where it lives
/// from its pinned folder down, and all three in small letters for matching.
#[derive(Debug)]
struct Found {
    path: std::path::PathBuf,
    label: String,
    detail: String,
    text: String,
}

/// The longest query kept, the most projects and folders the list shows, a row's height and the
/// list's least width.
const MAX_QUERY: usize = 120;
const MAX_PROJECTS: usize = 8;
const MAX_FOLDERS: usize = 4;
const ROW: f32 = 36.0;
const LIST_WIDTH: f32 = 420.0;

/// Something the list offers.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    Project(std::path::PathBuf),
    Folder(std::path::PathBuf),
    /// The application action of this id.
    Action(&'static str),
    /// In a project: a diagram or an element.
    Model(crate::project_search::Found),
}

/// A row of the list: what it runs, its icon, its name and the line beside it.
struct Hit {
    target: Target,
    icon: &'static ux::Icon,
    label: String,
    detail: String,
}

/// Opens the search with the keyboard: the command key with K, on Home or in a project, unless a
/// text is being typed (in a skill's text the same keys make a link).
pub(crate) fn shortcut(shell: &mut Shell, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::K))) {
        open(shell, ctx);
    }
}

fn open(shell: &mut Shell, ctx: &egui::Context) {
    let scope = scope(shell);
    let search = &mut shell.home_search;
    search.focus = true;
    if search.open {
        return;
    }
    search.open = true;
    search.highlight = 0;
    search.scope = scope;
    if search.scope.is_some() {
        return;
    }
    // The pinned folders' projects, looked for again each time the list opens (folders change),
    // but never while a search of them is still running; its answer wakes the window.
    if search.finding.is_none()
        && let Some(find) = shell.services.find_projects.clone()
        && !shell.state.pinned.is_empty()
    {
        let ctx = ctx.clone();
        shell.home_search.finding = Some(find(shell.state.pinned.clone(), Box::new(move || ctx.request_repaint())));
    }
}

/// Takes the host's answer when it has come, its rows' words made once.
fn take_found(shell: &mut Shell) {
    let Some(rx) = &shell.home_search.finding else { return };
    let found = match rx.try_recv() {
        Ok(found) => found,
        Err(std::sync::mpsc::TryRecvError::Empty) => return,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Vec::new(),
    };
    shell.home_search.finding = None;
    shell.home_search.found = found
        .into_iter()
        .map(|f| {
            let label = f.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let base = shell.state.pinned.get(f.folder).and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
            let detail = base.into_iter().chain(f.within).collect::<Vec<_>>().join(" / ");
            let text = format!("{label}\n{detail}\n{}", f.path.to_string_lossy()).to_lowercase();
            Found { path: f.path, label, detail, text }
        })
        .collect();
}

/// Opens the search in the open project with `query` typed, for the pictures of the design and
/// the tests.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn open_with(shell: &mut Shell, query: &str) {
    shell.home_search.open = true;
    shell.home_search.scope = scope(shell);
    shell.home_search.query = query.to_string();
    shell.home_search.highlight = 0;
}

/// Where the search works now: the open project's folder, or none on Home.
fn scope(shell: &Shell) -> Option<std::path::PathBuf> {
    shell.active.and(shell.project.as_ref()).map(|p| p.path.clone()).filter(|_| shell.session.is_some())
}

/// The open project's index, made again when the project or its model changed since.
fn fresh_index(shell: &mut Shell) {
    let (Some(session), Some(path)) = (shell.session.as_ref(), scope(shell)) else { return };
    let key = (path, session.revision());
    if shell.home_search.indexed.as_ref() != Some(&key) {
        shell.home_search.index = crate::project_search::index(shell.kinds, shell.notation_uis, session.doc());
        shell.home_search.indexed = Some(key);
    }
}

/// Closes the list and forgets the query; the projects found stay for the next time.
pub(crate) fn close(shell: &mut Shell) {
    let search = &mut shell.home_search;
    search.open = false;
    search.query.clear();
    search.highlight = 0;
    search.focus = false;
}

/// Whether every word of `query` is in one of `texts`, whatever the case.
fn matches(query: &str, texts: &[&str]) -> bool {
    let texts: Vec<String> = texts.iter().map(|t| t.to_lowercase()).collect();
    query.to_lowercase().split_whitespace().all(|w| texts.iter().any(|t| t.contains(w)))
}

/// The rows for the query: the matching recent projects, then the projects in the pinned folders
/// not among them, the pinned folders and the actions, in that order; with no query, all of them.
fn hits(shell: &Shell, query: &str) -> Vec<(String, Vec<Hit>)> {
    let mut groups = Vec::new();
    if shell.home_search.scope.is_some() {
        let (diagrams, elements) = crate::project_search::hits(&shell.home_search.index, query);
        for (heading, entries) in [(tr("Diagrams"), diagrams), (tr("Elements"), elements)] {
            let rows: Vec<Hit> = entries
                .into_iter()
                .map(|e| Hit { target: Target::Model(e.found), icon: e.icon, label: e.label.clone(), detail: e.detail.clone() })
                .collect();
            if !rows.is_empty() {
                groups.push((heading, rows));
            }
        }
        return groups;
    }
    let recent = shell.state.recent.iter().map(|r| Hit {
        target: Target::Project(r.path.clone()),
        icon: &icons::ER_DIAGRAM,
        label: r.name.clone(),
        detail: shell.location_of(&r.path).unwrap_or_default(),
    });
    let recent = recent.filter(|h| match &h.target {
        Target::Project(path) => matches(query, &[&h.label, &h.detail, &path.to_string_lossy()]),
        _ => false,
    });
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    let found = shell
        .home_search
        .found
        .iter()
        .filter(|f| !shell.state.recent.iter().any(|r| r.path == f.path) && words.iter().all(|w| f.text.contains(w.as_str())))
        .map(|f| Hit { target: Target::Project(f.path.clone()), icon: &icons::ER_DIAGRAM, label: f.label.clone(), detail: f.detail.clone() });
    let projects: Vec<Hit> = recent.chain(found).take(MAX_PROJECTS).collect();
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
    let rows = crate::actions::ACTIONS.iter().filter_map(|a| Some((a, a.search.as_ref()?)));
    rows.filter(|(a, _)| (a.available)(shell))
        .map(|(a, search)| {
            let label = search.label.map_or_else(|| (a.label)(shell), |own| own());
            Hit { target: Target::Action(a.id), icon: (search.icon)(shell), label, detail: String::new() }
        })
        .collect()
}

/// Runs a row and closes the search.
fn run(shell: &mut Shell, target: Target) {
    close(shell);
    match target {
        Target::Project(path) => shell.open_path(&path),
        Target::Model(found) => crate::project_search::go(shell, found),
        Target::Folder(path) => shell.open_from_picker_in(Some(&path)),
        Target::Action(id) => crate::actions::run(shell, id),
    }
}

/// The field in the bar at `field`, with its placeholder and shortcut while closed, the query
/// and its list while open.
pub(crate) fn field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect, placeholder: &str, prefix: &str) {
    let paint = ui.painter().clone();
    let mid = field.center().y;
    // A list opened on Home or for another project closes when the window shows something else.
    if shell.home_search.open && shell.home_search.scope != scope(shell) {
        close(shell);
    }
    fresh_index(shell);
    let id = format!("{prefix}.search");
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
        if control(shell, ui, &id, "field", field, true).clicked() {
            open(shell, ui.ctx());
        }
        return;
    }
    widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
    // The list's keys before the field sees them: the arrows move, Enter runs, Esc closes; none
    // while a dialog over the window has them (About).
    let groups = hits(shell, &shell.home_search.query.clone());
    let count = groups.iter().map(|(_, h)| h.len()).sum::<usize>();
    let modal = shell.about.is_some();
    let (up, down, enter, escape) = ui.input_mut(|i| {
        if modal {
            return (false, false, false, false);
        }
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
    let input = widgets::egui_id(&format!("{id}.input"));
    let before = shell.home_search.query.clone();
    let edit = egui::TextEdit::singleline(&mut shell.home_search.query)
        .id(input)
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
    list(shell, ui, p, field, &groups, &id);
    // The host's answer only now, so the rows a key or a click ran on are the rows drawn; a list
    // that grows keeps its highlight on the first row.
    let had = shell.home_search.found.len();
    take_found(shell);
    if shell.home_search.found.len() != had {
        shell.home_search.highlight = 0;
    }
}

/// The list under the field, its rows grouped under their headings; a click elsewhere closes it.
fn list(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, groups: &[(String, Vec<Hit>)], id: &str) {
    let rows = groups.iter().map(|(_, h)| h.len()).sum::<usize>() as f32;
    let height = if groups.is_empty() { 52.0 } else { 8.0 + groups.len() as f32 * 28.0 + rows * ROW };
    let width = field.width().max(LIST_WIDTH);
    let r = Rect::from_min_size(pos2(field.right() - width, field.bottom() + 6.0), vec2(width, height));
    let picked = ux::overlay(ui, widgets::egui_id(&format!("{id}.list")), r, |ui| {
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
                let resp = control(shell, ui, &format!("{id}.result.{n}"), "menu-item", row, true);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::tests::shell;
    use crate::state::{RecentSort, RecentView};
    use crate::theme::ThemeKind;

    /// The rows of the list, as (target, icon, wording), each in its table order.
    fn rows(shell: &Shell) -> Vec<(Target, ux::Icon, String)> {
        actions(shell).into_iter().map(|h| (h.target, *h.icon, h.label)).collect()
    }

    /// One row of the list.
    fn row(id: &'static str, icon: ux::Icon, label: &str) -> (Target, ux::Icon, String) {
        (Target::Action(id), icon, label.to_string())
    }

    /// The list shows what it showed before the table, with Open's own wording and each toggle
    /// worded and drawn for the state it is in.
    #[test]
    fn the_lists_actions_are_worded_and_drawn_as_they_were() {
        let (mut shell, ..) = shell();
        shell.state.theme = ThemeKind::Light;
        shell.state.recent_view = RecentView::List;
        shell.state.recent_sort = RecentSort::LastOpened;
        assert_eq!(
            rows(&shell),
            [
                row("file.new", icons::PLUS, "New project"),
                row("file.open", icons::FOLDER, "Open project"),
                row("home.pin-folder", icons::PLUS, "Pin a folder"),
                row("home.appearance", icons::MOON, "Switch to the dark appearance"),
                row("home.recent-view", icons::GRID, "Show recent projects as a gallery"),
                row("home.recent-sort", icons::LIST, "Sort recent projects by name"),
            ]
        );
        shell.state.theme = ThemeKind::Dark;
        shell.state.recent_view = RecentView::Gallery;
        let flipped: Vec<_> = rows(&shell).into_iter().skip(3).take(2).collect();
        let wanted = [
            row("home.appearance", icons::SUN, "Switch to the light appearance"),
            row("home.recent-view", icons::LIST, "Show recent projects as a list"),
        ];
        assert_eq!(flipped, wanted);
    }

    /// A row shows while its action is available: Open not behind a dialog that turns off its menu
    /// item, and neither New project nor Open without a host.
    #[test]
    fn the_list_shows_a_row_while_its_action_is_available() {
        let open = Target::Action("file.open");
        let shown = |shell: &Shell| rows(shell).into_iter().map(|(target, ..)| target).collect::<Vec<_>>();
        let (mut shell, ..) = shell();
        crate::new_project::open(&mut shell);
        assert!(!shown(&shell).contains(&open) && !crate::actions::available(&shell, "file.open"));
        shell.new_project = None;
        assert!(shown(&shell).contains(&open) && crate::actions::available(&shell, "file.open"));
        shell.services.projects = None;
        assert!(!shown(&shell).contains(&open) && !shown(&shell).contains(&Target::Action("file.new")));
    }

    /// Running a row closes the search and runs its action.
    #[test]
    fn running_an_action_row_closes_the_search_and_runs_it() {
        let (mut shell, ..) = shell();
        let dark = shell.state.theme.is_dark();
        shell.home_search.open = true;
        run(&mut shell, Target::Action("home.appearance"));
        assert!(!shell.home_search.open);
        assert_ne!(shell.state.theme.is_dark(), dark);
    }
}
