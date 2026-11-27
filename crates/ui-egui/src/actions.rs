//! The application's own actions in one table: Open, Save, Generate and the like, beside the
//! engine's commands. Menus, shortcuts and Home's search read a row for its name, ask it whether it can act and
//! run it. Availability is a function asked each time; nothing here keeps its answer.

use egui::{Key, Modifiers};

pub use archforge_shell_api::actions::Keys;
pub(crate) use archforge_shell_api::actions::{held_exactly, take, take_alone};

use crate::i18n::{tr, tr_ctx};
use crate::state::{RecentSort, RecentView};
use crate::theme::ThemeKind;
use crate::ux::{Icon, icons};
use crate::{Shell, edit_menu, model_menu, view_menu};

pub const ABOUT: &str = "app.about";
pub const SETTINGS: &str = "app.settings";
pub const QUIT: &str = "app.quit";
pub const GENERATE: &str = "file.generate";
pub const SAVE: &str = "file.save";
pub const NEW: &str = "file.new";
pub const OPEN: &str = "file.open";
pub const CLOSE: &str = "file.close";
pub const FIND: &str = "view.find";
pub const VERSIONS: &str = "versions.list";
pub const OLDER: &str = "versions.older";
pub const NEWER: &str = "versions.newer";
pub const BRANCHES: &str = "versions.branches";
pub const NEW_BRANCH: &str = "versions.new-branch";
pub const SAVE_VERSION: &str = "versions.save";
pub const COMPARE: &str = "versions.compare";
pub const BACK: &str = "versions.back";
pub const UNDO: &str = "edit.undo";
pub const REDO: &str = "edit.redo";
pub const CUT: &str = "edit.cut";
pub const COPY: &str = "edit.copy";
pub const PASTE: &str = "edit.paste";
pub const SELECT_ALL: &str = "edit.select-all";

/// The Versions menu's rows by section, on every system (`docs/design/versioning.md`, section 11).
pub static VERSIONS_MENU: [&[&str]; 4] = [&[SAVE_VERSION], &[VERSIONS, COMPARE], &[NEW_BRANCH, BRANCHES], &[OLDER, NEWER, BACK]];

/// The File menu's rows by section, the same groups on every system (AR-454): Settings… and Exit
/// end it only where the bar is in the window, as macOS keeps them in its application menu. Read
/// through `file_menu`, which leaves out the rows this system shows elsewhere.
pub static FILE_MENU: [&[&str]; 5] = [&[NEW, OPEN, CLOSE], &[SAVE], &[GENERATE], &[SETTINGS], &[QUIT]];

/// One application action.
pub struct Action {
    /// What a menu item or a search row names it by, never shown.
    pub id: &'static str,
    /// Its name, in the language in use when asked.
    pub label: fn(&Shell) -> String,
    /// Whether it can act now.
    pub available: fn(&Shell) -> bool,
    pub run: fn(&mut Shell),
    pub keys: Option<Keys>,
    pub search: Option<InSearch>,
}

/// The shortcut of the command key and `key`.
const fn command(key: Key) -> Option<Keys> {
    Some(Keys { modifiers: Modifiers::COMMAND, key })
}

/// How an action shows in Home's search.
pub struct InSearch {
    pub icon: fn(&Shell) -> &'static Icon,
    /// Its wording there, when not the action's label.
    pub label: Option<fn() -> String>,
}

/// Every action. Keys are looked at in this order, and Home's search lists its rows in it.
pub static ACTIONS: &[Action] = &[
    Action { id: ABOUT, label: |_| tr("About Archforge…"), available: |_| true, run: Shell::open_about, keys: None, search: None },
    Action {
        id: SETTINGS,
        label: |_| tr("Settings…"),
        available: free,
        run: crate::settings::open,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::Comma }),
        search: None,
    },
    Action {
        id: QUIT,
        label: |s| if native_bar(s) { tr("Quit Archforge") } else { tr("Exit") },
        available: |_| true,
        run: Shell::request_quit,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::Q }),
        search: None,
    },
    Action {
        id: GENERATE,
        label: |_| tr("Generate…"),
        available: |s| free(s) && s.can_generate(),
        run: Shell::open_generate,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::G }),
        search: None,
    },
    Action {
        id: SAVE,
        label: |_| tr("Save"),
        available: |s| free(s) && s.can_save(),
        run: Shell::save,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::S }),
        search: None,
    },
    Action {
        id: NEW,
        label: |_| tr("New project…"),
        // A folder pick waiting keeps it off: its answer would find the dialog in the way.
        available: |s| free(s) && s.picking.is_none() && s.services.projects.is_some() && s.tabs.len() < crate::MAX_TABS,
        run: crate::new_project::open,
        keys: None,
        search: Some(InSearch { icon: |_| &icons::PLUS, label: Some(|| tr("New project")) }),
    },
    Action {
        id: OPEN,
        label: |_| tr("Open project…"),
        available: |s| free(s) && s.can_open_project(),
        run: open,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::O }),
        search: Some(InSearch { icon: |_| &icons::FOLDER, label: None }),
    },
    Action {
        id: CLOSE,
        label: |_| tr("Close project"),
        available: |s| free(s) && s.active.and_then(|i| s.tabs.get(i)).is_some_and(|t| t.closing.is_none()),
        run: close,
        // Each system's close key (AR-454): ⌘W, and Ctrl+W on Windows and Linux.
        keys: command(Key::W),
        search: None,
    },
    Action {
        id: FIND,
        label: |_| tr("Find…"),
        // A field with the keyboard keeps the keys: in a skill's text they make a link. A bar with
        // no room for the search's field has nowhere to show the list.
        available: |s| free(s) && !edit_menu::on_field(s) && !s.home_search.cramped,
        run: |s| s.home_search.asked = true,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::K }),
        search: None,
    },
    Action {
        id: "home.pin-folder",
        label: |_| tr("Pin a folder"),
        available: |s| s.services.pick_folder.is_some() && s.picking.is_none() && !s.open_waits() && s.state.pinned.len() < crate::state::MAX_PINNED,
        run: Shell::pin_from_picker,
        keys: None,
        search: Some(InSearch { icon: |_| &icons::PLUS, label: None }),
    },
    Action {
        id: "home.appearance",
        label: |s| if s.state.theme.is_dark() { tr("Switch to the light appearance") } else { tr("Switch to the dark appearance") },
        available: |_| true,
        run: switch_appearance,
        keys: None,
        search: Some(InSearch { icon: |s| if s.state.theme.is_dark() { &icons::SUN } else { &icons::MOON }, label: None }),
    },
    Action {
        id: "home.recent-view",
        label: |s| match s.state.recent_view {
            RecentView::List => tr("Show recent projects as a gallery"),
            RecentView::Gallery => tr("Show recent projects as a list"),
        },
        available: |_| true,
        run: switch_recent_view,
        keys: None,
        search: Some(InSearch { icon: |s| if s.state.recent_view == RecentView::List { &icons::GRID } else { &icons::LIST }, label: None }),
    },
    Action {
        id: "home.recent-sort",
        label: |s| match s.state.recent_sort {
            RecentSort::LastOpened => tr("Sort recent projects by name"),
            RecentSort::Name => tr("Sort recent projects by last opened"),
        },
        available: |_| true,
        run: switch_recent_sort,
        keys: None,
        search: Some(InSearch { icon: |_| &icons::LIST, label: None }),
    },
    Action {
        id: SAVE_VERSION,
        label: |_| tr("Save version…"),
        available: |s| free(s) && s.project.is_some() && crate::save_version::off_because(s).is_none(),
        run: crate::save_version::open,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::S }),
        search: None,
    },
    Action {
        id: VERSIONS,
        label: |_| tr("History…"),
        // Its keys are Back to current work's while a version is in view.
        available: |s| crate::version_chip::available(s) && s.version_in_view().is_none(),
        run: crate::version_chip::open,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::H }),
        search: None,
    },
    Action {
        id: COMPARE,
        label: |_| tr("Compare…"),
        available: crate::version_details::can_compare,
        run: crate::version_details::compare_from_menu,
        keys: None,
        search: None,
    },
    Action {
        id: OLDER,
        label: |_| tr("Older version"),
        available: crate::version_chip::available,
        run: crate::version_chip::older,
        // The arrows, not the brackets: a bracket's key follows the keyboard layout (AR-454).
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::ALT), key: Key::ArrowLeft }),
        search: None,
    },
    Action {
        id: NEWER,
        label: |_| tr("Newer version"),
        // From current work there is nothing newer; from a version the step reads what it needs.
        available: |s| crate::version_chip::available(s) && s.stepping_from().is_some(),
        run: crate::version_chip::newer,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::ALT), key: Key::ArrowRight }),
        search: None,
    },
    Action {
        id: NEW_BRANCH,
        label: |_| tr("New branch…"),
        // As the Branches header's + is: the host keeps branches, and nothing holds the project.
        available: |s| free(s) && crate::branch_ask::offered(s),
        run: crate::branch_ask::open,
        keys: None,
        search: None,
    },
    Action {
        id: BRANCHES,
        label: |_| tr("Switch branch…"),
        // While the list can open and there are branches to switch between.
        available: |s| crate::version_chip::available(s) && crate::version_chip::branch_name(s).is_some(),
        run: crate::version_chip::open_branches,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::B }),
        search: None,
    },
    Action {
        id: BACK,
        label: |_| tr("Back to current work"),
        available: |s| free(s) && s.version_in_view().is_some(),
        run: |s| _ = s.back_to_current_work(),
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::H }),
        search: None,
    },
    Action {
        id: UNDO,
        label: |_| tr("Undo"),
        available: edit_menu::can_undo,
        run: edit_menu::undo,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::Z }),
        search: None,
    },
    Action {
        id: REDO,
        label: |_| tr("Redo"),
        available: edit_menu::can_redo,
        run: edit_menu::redo,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::Z }),
        search: None,
    },
    Action { id: CUT, label: |_| tr("Cut"), available: edit_menu::on_field, run: edit_menu::cut, keys: command(Key::X), search: None },
    Action { id: COPY, label: |_| tr("Copy"), available: edit_menu::can_copy, run: edit_menu::copy, keys: command(Key::C), search: None },
    Action { id: PASTE, label: |_| tr("Paste"), available: edit_menu::on_field, run: edit_menu::paste, keys: command(Key::V), search: None },
    Action {
        id: SELECT_ALL,
        label: |s| if native_bar(s) { tr("Select All") } else { tr("Select all") },
        available: edit_menu::can_select_all,
        run: edit_menu::select_all,
        keys: command(Key::A),
        search: None,
    },
];

/// Whether the menus are the system's own bar, as on macOS, and not the bar drawn in the window, as
/// on Windows and Linux: the one answer every wording that differs between the two asks for.
fn native_bar(shell: &Shell) -> bool {
    !shell.in_window_menu
}

/// The File menu's rows on this system, by section: `FILE_MENU` without Settings… and Exit in the
/// native bar, which has them in its application menu, and without a section that has no row left,
/// so no divider stands at an edge or beside another.
pub fn file_menu(shell: &Shell) -> Vec<Vec<&'static str>> {
    let shown = |id: &&str| !(native_bar(shell) && [SETTINGS, QUIT].contains(id));
    let sections = FILE_MENU.iter().map(|section| section.iter().copied().filter(shown).collect::<Vec<_>>());
    sections.filter(|section| !section.is_empty()).collect()
}

/// The File menu's title on this system: the native bar takes the system's own word where a
/// language has one, as German does.
pub fn file_title(shell: &Shell) -> String {
    if native_bar(shell) { tr_ctx("macOS", "File") } else { tr("File") }
}

/// Whether the row `id` is ticked, for a row that shows and hides something; `None` for a row
/// that has no tick.
pub fn ticked(shell: &Shell, id: &str) -> Option<bool> {
    view_menu::ticked(shell, id)
}

/// The action `id` among the table's own and the View menu's rows. The Model menu's rows are made
/// from the data of the shell's kinds and answer by their id ([`model_menu`]).
fn find(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().chain(view_menu::ROWS).find(|a| a.id == id)
}

/// The name of the action `id`, a row of the Model menu's included; empty when the table has none.
pub fn label(shell: &Shell, id: &str) -> String {
    find(id).map(|a| (a.label)(shell)).or_else(|| model_menu::label(shell, id)).unwrap_or_default()
}

/// The keys of the action `id`, when it has any; no row of the Model menu has keys.
pub fn keys(id: &str) -> Option<&'static Keys> {
    find(id)?.keys.as_ref()
}

/// Whether the action `id`, a row of the Model menu's included, can act now; an action the table
/// lacks cannot.
pub fn available(shell: &Shell, id: &str) -> bool {
    match find(id) {
        Some(a) => (a.available)(shell),
        None => model_menu::available(shell, id),
    }
}

/// Runs the action `id`; an id the table lacks does nothing. Asks nothing first: what offers an
/// action shows whether it can act, and each handler keeps its own guards.
pub fn run(shell: &mut Shell, id: &str) {
    match find(id) {
        Some(action) => (action.run)(shell),
        None => model_menu::run(shell, id),
    }
}

/// Whether nothing is up over the window that the menu or a key would reach behind: a question or
/// a dialog, in the window or in a panel. One rule, since a native menu is outside every veil.
pub(crate) fn free(shell: &Shell) -> bool {
    let window = shell.ask.is_none()
        && shell.about.is_none()
        && shell.settings.is_none()
        && shell.generate.is_none()
        && shell.new_project.is_none()
        && shell.versions.dialog.is_none();
    window && shell.explorer.delete.is_none() && shell.chat_panel.ask.is_none() && shell.diagrams.new.is_none()
}

/// Redo's second key, ⌘Y (Ctrl Y), which no menu shows.
const REDO_TOO: Keys = Keys { modifiers: Modifiers::COMMAND, key: Key::Y };

/// Whether `key` was pressed in `input` with no command key, Control or Alt held, with Shift or
/// without: the press an arrow's own reader acts on. With one of them held it is a chord's, as
/// ⌥⌘← is Older version's, and a chord whose action is off stays in the input: no reader behind it
/// takes it for a plain arrow.
pub(crate) fn plain_press(input: &egui::InputState, key: Key) -> bool {
    input.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && plain(modifiers)))
}

/// Whether `held` has no command key, Control or Alt in it: Shift alone, or nothing.
fn plain(held: &Modifiers) -> bool {
    !(held.command || held.ctrl || held.mac_cmd || held.alt)
}

/// Whether `key` was pressed in `input` plainly, as `plain_press` reads it, and takes those
/// presses. A press with a command key, Control or Alt stays in the input for its own chord.
pub(crate) fn take_plain(input: &mut egui::InputState, key: Key) -> bool {
    let before = input.events.len();
    input.events.retain(|e| !matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && plain(modifiers)));
    input.events.len() != before
}

/// Whether `key` was pressed in `input` with exactly the modifiers `wanted` held (`held_exactly`):
/// not with Shift or Alt more or less, and not with a Mac's Control. Takes nothing.
pub(crate) fn chord_press(input: &egui::InputState, key: Key, wanted: Modifiers) -> bool {
    input
        .events
        .iter()
        .any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && held_exactly(*modifiers, wanted)))
}

/// Whether `keys` were pressed in this frame, exactly; takes the press as `take` does.
fn pressed(ctx: &egui::Context, keys: &Keys) -> bool {
    ctx.input_mut(|i| take(i, keys))
}

/// Runs the actions whose keys were pressed in this frame, on every platform, while each is
/// available. A field reads the Edit menu's keys itself, so they stay in the input while it has the
/// keyboard. ⌘Y (Ctrl Y) is Redo's second key, matched as exactly as the table's. Keys are asked for
/// only while their action is available, so an action that is off leaves every press where it was.
pub(crate) fn run_shortcuts(shell: &mut Shell, ctx: &egui::Context) {
    let field = edit_menu::on_field(shell);
    for (action, keys) in ACTIONS.iter().filter_map(|a| Some((a, a.keys.as_ref()?))) {
        if !(field && action.id.starts_with("edit.")) && (action.available)(shell) && pressed(ctx, keys) {
            (action.run)(shell);
        }
    }
    if !field && available(shell, REDO) && pressed(ctx, &REDO_TOO) {
        run(shell, REDO);
    }
}

/// An action that leaves the work shown, asked for while a text field had the keyboard: each but
/// Quit with the project shown when it was asked.
pub(crate) enum Then {
    /// Close project.
    Close(std::path::PathBuf),
    /// Open project…, which shows the project picked in place of this one.
    Open(std::path::PathBuf),
    /// Home, from the bar's tabs.
    Home(std::path::PathBuf),
    /// Another project's tab, by its path.
    Show(std::path::PathBuf, std::path::PathBuf),
    /// Quit, by the menu, its key or the window's close button.
    Quit,
}

/// An action asked for while a text field had the keyboard.
pub(crate) enum Closing {
    /// Asked in this frame: the field lets go of the keyboard as it is drawn.
    Asked(Then),
    /// The field has let go, committing a name typed in place as a click elsewhere does: it runs.
    LetGo(Then),
}

/// The path of the project shown.
fn shown(shell: &Shell) -> Option<&std::path::Path> {
    shell.active.and_then(|i| shell.tabs.get(i)).map(|t| t.path.as_path())
}

/// Runs `now`, or, with a text field holding the keyboard on a project shown, as when a chord or
/// the native bar's row is chosen while a name is typed in place, asks for `then` on that project:
/// the field lets go first and it runs a frame later (`follow_close`), finding the name committed.
pub(crate) fn after_field(shell: &mut Shell, then: impl FnOnce(std::path::PathBuf) -> Then, now: impl FnOnce(&mut Shell)) {
    match shown(shell).map(std::path::Path::to_path_buf) {
        Some(path) if edit_menu::on_field(shell) => shell.edit.closing = Some(Closing::Asked(then(path))),
        _ => now(shell),
    }
}

/// Closes the project shown, after the field with the keyboard lets go, so Close asks about a name
/// typed in place.
fn close(shell: &mut Shell) {
    after_field(shell, Then::Close, Shell::close_project);
}

/// Open project…, after the field with the keyboard lets go, so the project shown is parked with
/// a name typed in place committed.
fn open(shell: &mut Shell) {
    after_field(shell, Then::Open, Shell::open_from_picker);
}

/// Takes the keyboard from the field that has it and asks for the next frame, in which `then` runs.
pub(crate) fn let_go(shell: &mut Shell, ctx: &egui::Context, then: Then) {
    ctx.memory_mut(egui::Memory::stop_text_input);
    ctx.request_repaint();
    shell.edit.closing = Some(Closing::LetGo(then));
}

/// Once a frame, after the menus and the keys: an action asked on a field takes the keyboard from
/// it, and runs in the next frame while the project it was asked on still shows and it can act.
/// Quit only asks again: whether it goes ahead is decided where every quit is.
pub(crate) fn follow_close(shell: &mut Shell, ctx: &egui::Context) {
    let Some(closing) = shell.edit.closing.take() else { return };
    let then = match closing {
        Closing::Asked(then) => return let_go(shell, ctx, then),
        Closing::LetGo(then) => then,
    };
    let on = |path: &std::path::Path| shown(shell) == Some(path);
    match then {
        Then::Quit => shell.quit_requested = true,
        Then::Close(path) if on(&path) && available(shell, CLOSE) => shell.close_project(),
        Then::Open(path) if on(&path) && available(shell, OPEN) => shell.open_from_picker(),
        Then::Home(path) if on(&path) && free(shell) => shell.go_home(),
        Then::Show(path, to) if on(&path) && free(shell) => {
            if let Some(i) = shell.tab_of(&to) {
                shell.activate(i);
            }
        }
        _ => {}
    }
}

/// Changes between the light and the dark appearance.
fn switch_appearance(shell: &mut Shell) {
    shell.state.theme = if shell.state.theme.is_dark() { ThemeKind::Light } else { ThemeKind::Dark };
}

/// Changes between the list and the gallery of recent projects.
fn switch_recent_view(shell: &mut Shell) {
    shell.state.recent_view = match shell.state.recent_view {
        RecentView::List => RecentView::Gallery,
        RecentView::Gallery => RecentView::List,
    };
}

/// Changes the order of the recent projects.
fn switch_recent_sort(shell: &mut Shell) {
    shell.state.recent_sort = match shell.state.recent_sort {
        RecentSort::LastOpened => RecentSort::Name,
        RecentSort::Name => RecentSort::LastOpened,
    };
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{Receiver, channel};

    use super::*;
    use crate::{CreateError, Opened, Picked, ProjectHost, SaveError, Saved, SavedBy, Services, Tab};

    const PIN: &str = "home.pin-folder";
    const APPEARANCE: &str = "home.appearance";
    const VIEW: &str = "home.recent-view";
    const SORT: &str = "home.recent-sort";
    const ALL: [&str; 13] = [ABOUT, SETTINGS, QUIT, GENERATE, SAVE, NEW, OPEN, CLOSE, FIND, PIN, APPEARANCE, VIEW, SORT];
    const IN_SEARCH: [&str; 6] = [NEW, OPEN, PIN, APPEARANCE, VIEW, SORT];
    /// What a dialog or a question over the window turns off.
    const BEHIND_A_DIALOG: [&str; 7] = [SETTINGS, GENERATE, SAVE, NEW, OPEN, CLOSE, FIND];

    /// A host that counts the saves asked of it; the saves never finish.
    #[derive(Default)]
    pub(crate) struct CountingHost(AtomicUsize);

    impl ProjectHost for CountingHost {
        /// Never answers: no test here creates a project.
        fn create(&self, _folder: &Path, _name: &str) -> Receiver<Result<Opened, CreateError>> {
            channel().1
        }

        /// Counts the save and never answers.
        fn save(&self, _project: &Path, _description: Option<String>, _by: SavedBy) -> Receiver<Result<Saved, SaveError>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            channel().1
        }
    }

    /// A new model named Shop, as a project holds one.
    pub(crate) fn session() -> archforge_notations::Session {
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid(n.to_le_bytes())
        });
        archforge_notations::new_project_session("Shop", ids).unwrap()
    }

    /// A shell with a project open, a host and both pickers, none of them asked yet; and the counts
    /// of the saves and the project picks asked of them. The folder picker finds a folder to pin.
    pub(crate) fn shell() -> (Shell, Arc<CountingHost>, Arc<AtomicUsize>) {
        let host = Arc::new(CountingHost::default());
        let picks = Arc::new(AtomicUsize::new(0));
        let counted = picks.clone();
        let pick_project = Some(Arc::new(move |_: Option<&Path>| {
            counted.fetch_add(1, Ordering::SeqCst);
            Picked::at_once(None)
        }) as _);
        let pick_folder = Some(Arc::new(|_: &str| Picked::at_once(Some("/pinned".into()))) as _);
        let mut shell =
            Shell::new(Services { projects: Some(host.clone()), pick_project, pick_folder, ..Default::default() }, &crate::tests::SCREENS);
        shell.session = Some(session());
        shell.project = Some(Opened { name: "Shop".to_string(), path: "/shop.archforge".into(), model: None, unsaved: false, notice: None });
        shell.tabs = vec![tab()];
        shell.active = Some(0);
        (shell, host, picks)
    }

    /// `shell` with unsaved work, which Save makes a version of: with none it asks for nothing.
    fn unsaved(shell: &mut Shell) -> &mut Shell {
        if let Some(s) = shell.session.as_mut() {
            s.mark_unsaved();
        }
        shell
    }

    /// A question about the open project, which keeps everything behind it off.
    fn question() -> crate::close_ask::Ask {
        crate::close_ask::Ask::Unsaved { path: "/shop.archforge".into() }
    }

    /// A tab, to take up one of the places.
    pub(crate) fn tab() -> Tab {
        Tab { path: "/shop.archforge".into(), name: "Shop".to_string(), parked: None, closing: None, close_error: None }
    }

    /// `all` less what is `off`.
    fn without(all: &[&'static str], off: &[&str]) -> Vec<&'static str> {
        all.iter().copied().filter(|id| !off.contains(id)).collect()
    }

    /// The ids of the rows of the table `on` answers yes for, the chip's rows and the Edit menu's aside.
    fn answering(on: impl Fn(&Action) -> bool) -> Vec<&'static str> {
        ACTIONS.iter().filter(|a| !a.id.starts_with("versions.") && !a.id.starts_with("edit.") && on(a)).map(|a| a.id).collect()
    }

    /// The whole table, state by state, written out by hand: which actions are off. Search rows and
    /// the keys answer to the same rule, so they have no columns of their own.
    #[test]
    fn every_action_follows_one_rule_in_every_state() {
        type Setup = fn(&mut Shell);
        let states: [(&str, Setup, &[&str]); 21] = [
            ("a project", |_| {}, &[]),
            ("saving", |s| unsaved(s).save(), &[SAVE]),
            ("held", |s| s.begin_hold().unwrap(), &[SAVE]),
            ("viewing a version", |s| s.show_version(session(), "c1".to_string()), &[SAVE]),
            ("About open", crate::support::open, &BEHIND_A_DIALOG),
            ("Settings open", crate::settings::open, &BEHIND_A_DIALOG),
            ("Generate open", Shell::open_generate, &BEHIND_A_DIALOG),
            ("New project open", crate::new_project::open, &BEHIND_A_DIALOG),
            ("a question open", |s| s.ask = Some(question()), &BEHIND_A_DIALOG),
            ("a panel's dialog", |s| crate::diagrams::open_new_of(s, None), &BEHIND_A_DIALOG),
            ("an opening under way", |s| s.opening = Some(channel().1), &[OPEN]),
            ("a folder pick under way", |s| s.ask_pick(crate::Pick::Pin, || channel().1), &[NEW, OPEN, PIN]),
            (
                "an opening waiting for the hold",
                |s| s.begin_hold().map(|()| s.open_path(Path::new("/tmp/Later.archforge"))).unwrap(),
                &[SAVE, OPEN, PIN],
            ),
            ("a project opened waiting for the hold", |s| s.begin_hold().map(|()| s.open_project(Opened::default())).unwrap(), &[SAVE, OPEN, PIN]),
            ("every tab taken", |s| s.tabs = (0..crate::MAX_TABS).map(|_| tab()).collect(), &[NEW, OPEN]),
            ("every pin taken", |s| s.state.pinned = (0..crate::state::MAX_PINNED).map(|i| i.to_string().into()).collect(), &[PIN]),
            ("no folder picker", |s| s.services.pick_folder = None, &[PIN]),
            ("no model", |s| s.session = None, &[GENERATE, SAVE]),
            ("no host", |s| s.services.projects = None, &[NEW, OPEN]),
            ("no project picker", |s| s.services.pick_project = None, &[OPEN]),
            ("no project shown", |s| s.active = None, &[CLOSE]),
        ];
        for (name, setup, off) in states {
            let (mut shell, ..) = shell();
            setup(&mut shell);
            let shell = &shell;
            assert_eq!(answering(|a| (a.available)(shell)), without(&ALL, off), "available, {name}");
            let in_search: Vec<&str> = without(&IN_SEARCH, off);
            assert_eq!(answering(|a| a.search.is_some() && (a.available)(shell)), in_search, "search, {name}");
        }
    }

    /// Close project waits for a tab that is already closing, as its tab's button does.
    #[test]
    fn close_project_waits_for_a_closing_tab() {
        let (mut shell, ..) = shell();
        assert!(available(&shell, CLOSE));
        shell.tabs[0].closing = Some(channel().1);
        assert!(!available(&shell, CLOSE));
    }

    /// Availability is asked each time: the same row answers differently once the shell has changed.
    #[test]
    fn availability_is_asked_each_time() {
        let (mut shell, ..) = shell();
        assert!(available(&shell, OPEN));
        crate::support::open(&mut shell);
        assert!(!available(&shell, OPEN));
        shell.about = None;
        assert!(available(&shell, OPEN));
    }

    /// Which actions' effects show in `shell`, in the table's order.
    fn effects(shell: &Shell, saves: &CountingHost, picks: &AtomicUsize) -> Vec<&'static str> {
        let fresh = Shell::new(Services::default(), &crate::tests::SCREENS).state;
        let shown = [
            (ABOUT, shell.about.is_some()),
            (QUIT, shell.quit_requested),
            (GENERATE, shell.generate.is_some()),
            (SAVE, saves.0.load(Ordering::SeqCst) == 1),
            (NEW, shell.new_project.is_some()),
            (SETTINGS, shell.settings.is_some()),
            (CLOSE, shell.project.is_none()),
            (OPEN, picks.load(Ordering::SeqCst) == 1),
            (FIND, shell.home_search.asked),
            (PIN, !shell.state.pinned.is_empty()),
            (APPEARANCE, shell.state.theme != fresh.theme),
            (VIEW, shell.state.recent_view != fresh.recent_view),
            (SORT, shell.state.recent_sort != fresh.recent_sort),
        ];
        shown.iter().filter(|(_, on)| *on).map(|(id, _)| *id).collect()
    }

    /// Each row runs what its menu item or key ran, and nothing else; the table has exactly these
    /// rows, each once, and an id it lacks runs nothing, has no name and is never available.
    #[test]
    fn every_action_runs_its_handler_and_only_that() {
        assert_eq!(
            ACTIONS.iter().map(|a| a.id).filter(|id| !id.starts_with("versions.") && !id.starts_with("edit.")).collect::<Vec<_>>(),
            ALL,
            "the chip's tests walk its own row, and the Edit menu's are in edit_menu"
        );
        for id in ALL {
            let (mut shell, saves, picks) = shell();
            if id == SAVE {
                unsaved(&mut shell);
            }
            run(&mut shell, id);
            // A pick's answer, as the next frame takes it.
            shell.take_pick();
            assert_eq!(effects(&shell, &saves, &picks), [id]);
        }
        for id in [APPEARANCE, VIEW, SORT] {
            let (mut shell, saves, picks) = shell();
            run(&mut shell, id);
            run(&mut shell, id);
            assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "{id} comes back");
        }
        let (mut shell, saves, picks) = shell();
        run(&mut shell, "no.such.action");
        assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new());
        assert!(!available(&shell, "no.such.action") && label(&shell, "no.such.action").is_empty());
    }

    /// Each row keeps its name, the catalogue's own key, the wording that depends on the state with
    /// it.
    #[test]
    fn every_action_keeps_its_name() {
        let (mut shell, ..) = shell();
        shell.state.theme = ThemeKind::Light;
        shell.state.recent_view = RecentView::List;
        shell.state.recent_sort = RecentSort::LastOpened;
        let names = [
            "About Archforge…",
            "Settings…",
            "Quit Archforge",
            "Generate…",
            "Save",
            "New project…",
            "Open project…",
            "Close project",
            "Find…",
            "Pin a folder",
            "Switch to the dark appearance",
            "Show recent projects as a gallery",
            "Sort recent projects by name",
        ];
        assert_eq!(ALL.map(|id| label(&shell, id)), names);
        shell.state.theme = ThemeKind::Dark;
        shell.state.recent_view = RecentView::Gallery;
        shell.state.recent_sort = RecentSort::Name;
        let flipped = [APPEARANCE, VIEW, SORT].map(|id| label(&shell, id));
        assert_eq!(flipped, ["Switch to the light appearance", "Show recent projects as a list", "Sort recent projects by last opened"]);
    }

    /// File has the same groups on every system: New project…, Open project… and Close project, then
    /// Save, then Generate…. The bar in the window ends it with Settings… and Exit, each set apart;
    /// the native bar has both in its application menu, so leaves them out with no group left empty.
    #[test]
    fn file_has_the_same_groups_on_every_system() {
        let (mut shell, ..) = shell();
        shell.in_window_menu = false;
        assert_eq!(file_menu(&shell), [vec![NEW, OPEN, CLOSE], vec![SAVE], vec![GENERATE]], "the native bar");
        shell.in_window_menu = true;
        assert_eq!(file_menu(&shell), [vec![NEW, OPEN, CLOSE], vec![SAVE], vec![GENERATE], vec![SETTINGS], vec![QUIT]], "the bar in the window");
    }

    /// The wording that differs by system, each asked with the native bar and with the bar in the
    /// window: Quit Archforge and Exit, Select All and Select all, and in German the File menu's
    /// title, Ablage and Datei. Every other language has one word for File.
    #[test]
    fn the_wording_follows_the_systems_bar() {
        let (mut shell, ..) = shell();
        let words = |s: &Shell| [label(s, QUIT), label(s, SELECT_ALL), file_title(s)];
        for (language, native, in_window) in [
            ("en", ["Quit Archforge", "Select All", "File"], ["Exit", "Select all", "File"]),
            ("de", ["Archforge beenden", "Alles auswählen", "Ablage"], ["Beenden", "Alles auswählen", "Datei"]),
            ("fr", ["Quitter Archforge", "Tout sélectionner", "Fichier"], ["Quitter", "Tout sélectionner", "Fichier"]),
        ] {
            crate::i18n::set_language(language);
            shell.in_window_menu = false;
            assert_eq!(words(&shell), native, "{language}: the native bar");
            shell.in_window_menu = true;
            assert_eq!(words(&shell), in_window, "{language}: the bar in the window");
        }
        crate::i18n::set_language("en");
    }

    /// No two rows of any menu share their keys but History and Back to current work, which are
    /// never on together; and none has Redo's second key.
    #[test]
    fn only_history_and_back_share_their_keys() {
        let keyed: Vec<(&str, &Keys)> = ACTIONS.iter().chain(view_menu::ROWS).filter_map(|a| Some((a.id, a.keys.as_ref()?))).collect();
        let same = |a: &Keys, b: &Keys| a.key == b.key && a.modifiers == b.modifiers;
        let mut shared = Vec::new();
        for (n, (first, keys)) in keyed.iter().enumerate() {
            shared.extend(keyed[n + 1..].iter().filter(|(_, other)| same(keys, other)).map(|(second, _)| (*first, *second)));
        }
        assert_eq!(shared, [(VERSIONS, BACK)]);
        assert!(keyed.iter().all(|(_, keys)| !same(keys, &REDO_TOO)));
    }

    /// Presses `key` with `modifiers`, the only input of a frame; what the shortcuts left of it.
    fn press(shell: &mut Shell, modifiers: Modifiers, key: Key) -> Vec<egui::Event> {
        let input = egui::RawInput { events: vec![down(modifiers, key)], ..Default::default() };
        let mut left = Vec::new();
        let mut output = egui::Context::default().run_ui(input, |ui| {
            run_shortcuts(shell, ui.ctx());
            left = ui.input(|i| i.events.clone());
        });
        output.textures_delta.clear();
        left
    }

    /// A press of `key` with `modifiers`.
    fn down(modifiers: Modifiers, key: Key) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    /// The command key as a keyboard reports it: a Mac's ⌘, and the Ctrl of Windows and Linux,
    /// which sets `ctrl` with `command`. `Modifiers::COMMAND` alone is a shape no keyboard sends.
    const SHAPES: [(&str, Modifiers); 2] =
        [("macOS", Modifiers::MAC_CMD.plus(Modifiers::COMMAND)), ("Windows and Linux", Modifiers::CTRL.plus(Modifiers::COMMAND))];

    /// What is left of an input holding only `events` once `keys` are asked for, and whether they
    /// counted as pressed.
    fn taken(keys: &Keys, events: &[egui::Event]) -> (bool, Vec<egui::Event>) {
        let mut input = egui::InputState::default();
        input.events = events.to_vec();
        (take(&mut input, keys), input.events)
    }

    /// Every chord of the table, and Redo's second key, in both shapes: its own press counts and is
    /// taken. With Shift or Alt more, or one less, it is another chord's press: it counts for nothing
    /// and stays. A Mac's Control with it counts for nothing and is taken.
    #[test]
    fn every_chord_is_matched_exactly_in_both_shapes() {
        let chords: Vec<(&str, &Keys)> = ACTIONS.iter().filter_map(|a| Some((a.id, a.keys.as_ref()?))).chain([("⌘Y", &REDO_TOO)]).collect();
        assert_eq!(chords.len(), 20, "every row with keys, and Redo's second key");
        for (id, keys) in chords {
            let own = keys.modifiers;
            assert!(own.command && !own.ctrl && !own.mac_cmd, "{id}: a chord of the command key, which both shapes hold");
            let others =
                [Modifiers { shift: !own.shift, ..own }, Modifiers { alt: !own.alt, ..own }, Modifiers { shift: !own.shift, alt: !own.alt, ..own }];
            for (system, shape) in SHAPES {
                assert_eq!(taken(keys, &[down(own | shape, keys.key)]), (true, vec![]), "{id} on {system}");
                for other in others.map(|other| down(other | shape, keys.key)) {
                    assert_eq!(taken(keys, std::slice::from_ref(&other)), (false, vec![other.clone()]), "{id} on {system}: {other:?}");
                }
            }
            let control = down(own | SHAPES[0].1 | Modifiers::CTRL, keys.key);
            assert_eq!(taken(keys, std::slice::from_ref(&control)), (false, vec![]), "{id}: a Mac's Control with the chord");
            assert_eq!(taken(keys, &[down(own | SHAPES[0].1, keys.key), control]), (true, vec![]), "{id}: and after the chord itself");
        }
    }

    /// A key pressed alone counts and is taken; with Shift, Alt, Control or a command key of either
    /// shape it counts for nothing and stays, and so do another key's press and the key's release.
    #[test]
    fn a_key_alone_is_taken_and_with_a_modifier_it_stays() {
        let alone = |events: &[egui::Event]| {
            let mut input = egui::InputState::default();
            input.events = events.to_vec();
            (take_alone(&mut input, Key::Escape), input.events)
        };
        let other = down(Modifiers::NONE, Key::Enter);
        assert_eq!(alone(&[down(Modifiers::NONE, Key::Escape), other.clone()]), (true, vec![other.clone()]));
        let release = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE };
        assert_eq!(alone(std::slice::from_ref(&release)), (false, vec![release.clone()]));
        for held in [Modifiers::SHIFT, Modifiers::ALT, Modifiers::SHIFT | Modifiers::ALT, Modifiers::CTRL, SHAPES[0].1, SHAPES[1].1] {
            let press = down(held, Key::Escape);
            assert_eq!(alone(std::slice::from_ref(&press)), (false, vec![press.clone()]), "{held:?}");
        }
    }

    /// An arrow is plain with nothing held or Shift alone, in the frame it is pressed: not with a
    /// command key of either shape, Control or Alt, not its release, and not another key.
    #[test]
    fn a_plain_press_holds_no_command_key_control_or_alt() {
        let plain = |events: &[egui::Event]| {
            let mut input = egui::InputState::default();
            input.events = events.to_vec();
            plain_press(&input, Key::ArrowLeft)
        };
        assert!(plain(&[down(Modifiers::NONE, Key::ArrowLeft)]) && plain(&[down(Modifiers::SHIFT, Key::ArrowLeft)]));
        for held in [SHAPES[0].1, SHAPES[1].1, Modifiers::MAC_CMD, Modifiers::COMMAND, Modifiers::CTRL, Modifiers::ALT] {
            assert!(!plain(&[down(held, Key::ArrowLeft)]), "{held:?}");
            assert!(plain(&[down(held, Key::ArrowLeft), down(Modifiers::NONE, Key::ArrowLeft)]), "{held:?}, then the arrow alone");
        }
        let release = egui::Event::Key { key: Key::ArrowLeft, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE };
        assert!(!plain(&[release, down(Modifiers::NONE, Key::ArrowRight)]));
    }

    /// A chord without the command key, as ⇧F10, takes its own press only: with a modifier more or
    /// less, a Mac's Control and the command key of either shape included, it counts for nothing and
    /// stays.
    #[test]
    fn a_chord_without_the_command_key_takes_only_its_own_press() {
        let keys = Keys { modifiers: Modifiers::SHIFT, key: Key::F10 };
        assert_eq!(taken(&keys, &[down(Modifiers::SHIFT, Key::F10)]), (true, vec![]));
        let shift = Modifiers::SHIFT;
        for other in [Modifiers::NONE, shift | Modifiers::ALT, shift | Modifiers::CTRL, shift | SHAPES[0].1, shift | SHAPES[1].1] {
            let press = down(other, Key::F10);
            assert_eq!(taken(&keys, std::slice::from_ref(&press)), (false, vec![press.clone()]), "{other:?}");
        }
    }

    /// A plain press of the key is taken, with Shift too; one with a command key of either shape,
    /// Control or Alt stays in the input, and so do other keys.
    #[test]
    fn taking_a_plain_press_leaves_the_chords() {
        let taken = |events: &[egui::Event]| {
            let mut input = egui::InputState::default();
            input.events = events.to_vec();
            (take_plain(&mut input, Key::Minus), input.events)
        };
        let other = down(Modifiers::NONE, Key::Plus);
        for held in [Modifiers::NONE, Modifiers::SHIFT] {
            assert_eq!(taken(&[down(held, Key::Minus), other.clone()]), (true, vec![other.clone()]), "{held:?}");
        }
        for held in [SHAPES[0].1, SHAPES[1].1, Modifiers::CTRL, Modifiers::ALT, Modifiers::ALT | Modifiers::SHIFT] {
            let chord = down(held, Key::Minus);
            assert_eq!(taken(std::slice::from_ref(&chord)), (false, vec![chord.clone()]), "{held:?}");
        }
    }

    /// A chord's press counts in both shapes with exactly its modifiers; with Shift or Alt more or
    /// less, with a Mac's Control, as another key or as a release it does not. Nothing is taken.
    #[test]
    fn a_chord_press_holds_exactly_its_modifiers() {
        let pressed = |events: &[egui::Event], key, wanted| {
            let mut input = egui::InputState::default();
            input.events = events.to_vec();
            (chord_press(&input, key, wanted), input.events.len() == events.len())
        };
        for (key, own) in
            [(Key::Enter, Modifiers::COMMAND), (Key::Enter, Modifiers::COMMAND | Modifiers::SHIFT), (Key::G, Modifiers::COMMAND | Modifiers::ALT)]
        {
            for (system, shape) in SHAPES {
                assert_eq!(pressed(&[down(own | shape, key)], key, own), (true, true), "{key:?} {own:?} on {system}");
                for other in [Modifiers { shift: !own.shift, ..own }, Modifiers { alt: !own.alt, ..own }] {
                    assert_eq!(pressed(&[down(other | shape, key)], key, own), (false, true), "{key:?} {own:?} on {system}: {other:?}");
                }
            }
            assert_eq!(pressed(&[down(own | SHAPES[0].1 | Modifiers::CTRL, key)], key, own), (false, true), "{key:?} {own:?}: a Mac's Control");
            let release = egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: own | SHAPES[0].1 };
            assert_eq!(pressed(&[release, down(own | SHAPES[0].1, Key::Y)], key, own), (false, true), "{key:?} {own:?}");
        }
    }

    /// A chord takes its own press only: another chord's press on the same key in the same frame
    /// stays for its own row, and so do the key's release and a press with no command key.
    #[test]
    fn a_chord_leaves_the_other_presses_of_its_key() {
        let (save, version) = (keys(SAVE).unwrap(), keys(SAVE_VERSION).unwrap());
        for (system, shape) in SHAPES {
            let release = egui::Event::Key { key: Key::S, physical_key: None, pressed: false, repeat: false, modifiers: shape };
            let rest = [down(shape | Modifiers::SHIFT, Key::S), release, down(Modifiers::NONE, Key::S), down(shape, Key::O)];
            let all: Vec<egui::Event> = [down(shape, Key::S)].into_iter().chain(rest.clone()).collect();
            assert_eq!(taken(save, &all), (true, rest.to_vec()), "{system}");
            assert_eq!(taken(version, &all), (true, [&all[..1], &all[2..]].concat()), "{system}");
        }
    }

    /// A shortcut pressed runs its own action, in both shapes, and a key left alone runs nothing.
    #[test]
    fn every_shortcut_runs_its_action() {
        let shift = Modifiers::SHIFT;
        for (system, cmd) in SHAPES {
            let pressed = [(GENERATE, cmd | shift, Key::G), (SAVE, cmd, Key::S), (OPEN, cmd, Key::O), (SETTINGS, cmd, Key::Comma)];
            for (id, modifiers, key) in pressed {
                let (mut shell, saves, picks) = shell();
                unsaved(&mut shell);
                press(&mut shell, modifiers, key);
                assert_eq!(effects(&shell, &saves, &picks), [id], "{system}");
            }
            let (mut shell, saves, picks) = shell();
            // Unsaved, so a key that reached Save would show.
            unsaved(&mut shell);
            shell.ask = Some(question());
            press(&mut shell, cmd | shift, Key::G);
            press(&mut shell, cmd, Key::O);
            shell.ask = None;
            press(&mut shell, cmd, Key::G);
            press(&mut shell, Modifiers::NONE, Key::S);
            assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "{system}: not behind a question, nor with the wrong modifiers");
            press(&mut shell, cmd | shift, Key::S);
            assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "{system}: ⇧⌘S is Save version…'s, never Save's");
            for more in [Modifiers::ALT, shift | Modifiers::ALT] {
                press(&mut shell, cmd | more, Key::S);
                press(&mut shell, cmd | more, Key::O);
                assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "{system}: {more:?} more makes it nobody's chord");
            }
        }
        let (mut shell, saves, picks) = shell();
        unsaved(&mut shell);
        let control = SHAPES[0].1 | Modifiers::CTRL;
        for (modifiers, key) in [(control, Key::S), (control, Key::O), (control, Key::Comma), (control | shift, Key::G)] {
            press(&mut shell, modifiers, key);
            assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "a Mac's Control with ⌘ and {key:?} is nobody's chord");
        }
    }

    /// Close project asked on a field lets the field go in that frame, asking for the next, and runs
    /// in the next only while the project it was asked on still shows and Close can act.
    #[test]
    fn a_close_asked_on_a_field_runs_a_frame_later_on_its_own_project() {
        let ctx = egui::Context::default();
        let frame = |shell: &mut Shell| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| follow_close(shell, ui.ctx()));
            output.textures_delta.clear();
        };
        let meanwhile: [(&str, fn(&mut Shell)); 3] = [
            ("nothing", |_| {}),
            ("another project shows", |s| s.tabs[0].path = "/other.archforge".into()),
            ("a dialog is up", crate::support::open),
        ];
        for (what, change) in meanwhile {
            let (mut shell, ..) = shell();
            shell.edit.keyboard = edit_menu::Keyboard::Field;
            run(&mut shell, CLOSE);
            frame(&mut shell);
            assert!(shell.project.is_some(), "{what}: the field lets go first");
            change(&mut shell);
            frame(&mut shell);
            assert!(ctx.repaint_causes().iter().any(|c| c.file.ends_with("actions.rs")), "{what}: the letting go asked for the next frame");
            assert_eq!((shell.project.is_none(), shell.edit.closing.is_none()), (what == "nothing", true), "{what}");
        }
    }

    /// ⌘W (Ctrl+W) closes the project shown, in both shapes, and takes the press. On Home, with
    /// the project parked in its tab or with none open, Close project is off: the press closes
    /// nothing and stays in the input.
    #[test]
    fn the_close_key_closes_the_project_shown_and_nothing_on_home() {
        for (system, cmd) in SHAPES {
            let (mut shown, saves, picks) = shell();
            assert_eq!(press(&mut shown, cmd, Key::W), [], "{system}: the press is taken");
            assert_eq!(effects(&shown, &saves, &picks), [CLOSE], "{system}");
            let (mut parked, ..) = shell();
            parked.go_home();
            for (home, mut on_home, tabs) in
                [("a project parked", parked, 1), ("nothing open", Shell::new(Services::default(), &crate::tests::SCREENS), 0)]
            {
                assert_eq!(press(&mut on_home, cmd, Key::W), [down(cmd, Key::W)], "{system}, {home}: the press stays");
                assert!(on_home.tabs.len() == tabs && on_home.tabs.iter().all(|t| t.closing.is_none()), "{system}, {home}: no tab closes");
            }
        }
    }

    /// Open project…, Home and another project's tab, asked on a field, wait for it to let go and run
    /// a frame later only while the project they were asked on still shows and nothing is up over
    /// it; Quit is asked again then, where every quit is decided.
    #[test]
    fn a_switch_or_a_quit_asked_on_a_field_runs_a_frame_later() {
        let ctx = egui::Context::default();
        let frame = |shell: &mut Shell| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| follow_close(shell, ui.ctx()));
            output.textures_delta.clear();
        };
        // Settings, which Open project…'s own picker check lets through: only the row's rule stops it.
        let meanwhile: [(&str, fn(&mut Shell)); 3] = [
            ("nothing", |_| {}),
            ("another project shows", |s| s.tabs[0].path = "/other.archforge".into()),
            ("a dialog is up", crate::settings::open),
        ];
        let asks: [(&str, fn(&mut Shell)); 4] = [
            ("open", |s| run(s, OPEN)),
            ("home", |s| after_field(s, Then::Home, Shell::go_home)),
            ("tab", |s| after_field(s, |from| Then::Show(from, "/next.archforge".into()), |s| s.activate(1))),
            ("quit", |s| s.edit.closing = Some(Closing::Asked(Then::Quit))),
        ];
        for ((asked, ask), (what, change)) in asks.into_iter().flat_map(|a| meanwhile.map(|m| (a, m))) {
            let (mut shell, _, picks) = shell();
            shell.open_project(Opened { name: "Next".into(), path: "/next.archforge".into(), ..Default::default() });
            shell.activate(0);
            shell.edit.keyboard = edit_menu::Keyboard::Field;
            ask(&mut shell);
            frame(&mut shell);
            let now = |s: &Shell| (picks.load(Ordering::SeqCst), s.active, s.quit_requested);
            assert_eq!(now(&shell), (0, Some(0), false), "{asked}, {what}: the field lets go first");
            change(&mut shell);
            frame(&mut shell);
            let ran = match asked {
                "open" => (1, Some(0), false),
                "home" => (0, None, false),
                "tab" => (0, Some(1), false),
                _ => (0, Some(0), true),
            };
            let wanted = if what == "nothing" || asked == "quit" { ran } else { (0, Some(0), false) };
            assert_eq!((now(&shell), shell.edit.closing.is_none()), (wanted, true), "{asked}, {what}");
        }
    }
}
