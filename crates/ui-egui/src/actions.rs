//! The application's own actions in one table: Open, Save, Generate and the like, beside the
//! engine's commands. Menus, shortcuts and Home's search read a row for its name, ask it whether it can act and
//! run it. Availability is a function asked each time; nothing here keeps its answer.

use egui::{Key, Modifiers};

use crate::i18n::tr;
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

/// An action's keyboard shortcut, taken while the action is available.
pub struct Keys {
    pub modifiers: Modifiers,
    pub key: Key,
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
        label: |_| tr("Quit Archforge"),
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
        available: |s| free(s) && s.services.projects.is_some() && s.tabs.len() < crate::MAX_TABS,
        run: crate::new_project::open,
        keys: None,
        search: Some(InSearch { icon: |_| &icons::PLUS, label: Some(|| tr("New project")) }),
    },
    Action {
        id: OPEN,
        label: |_| tr("Open…"),
        available: |s| free(s) && s.can_open_project(),
        run: Shell::open_from_picker,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::O }),
        search: Some(InSearch { icon: |_| &icons::FOLDER, label: Some(|| tr("Open project")) }),
    },
    Action {
        id: CLOSE,
        label: |_| tr("Close project"),
        available: |s| free(s) && s.active.and_then(|i| s.tabs.get(i)).is_some_and(|t| t.closing.is_none()),
        run: Shell::close_project,
        keys: None,
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
        available: |s| s.services.pick_folder.is_some() && s.state.pinned.len() < crate::state::MAX_PINNED,
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
        label: |_| tr("History"),
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
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::OpenBracket }),
        search: None,
    },
    Action {
        id: NEWER,
        label: |_| tr("Newer version"),
        // From current work there is nothing newer; from a version the step reads what it needs.
        available: |s| crate::version_chip::available(s) && s.stepping_from().is_some(),
        run: crate::version_chip::newer,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::CloseBracket }),
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
        label: |_| tr("Switch branch"),
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
        label: |_| tr("Select all"),
        available: edit_menu::can_select_all,
        run: edit_menu::select_all,
        keys: command(Key::A),
        search: None,
    },
];

/// The action `id`.
fn find(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().chain(model_menu::rows()).chain(view_menu::ROWS).find(|a| a.id == id)
}

/// The name of the action `id`; empty when the table has none.
pub fn label(shell: &Shell, id: &str) -> String {
    find(id).map(|a| (a.label)(shell)).unwrap_or_default()
}

/// The keys of the action `id`, when it has any.
pub fn keys(id: &str) -> Option<&'static Keys> {
    find(id)?.keys.as_ref()
}

/// Whether the action `id` can act now; an action the table lacks cannot.
pub fn available(shell: &Shell, id: &str) -> bool {
    find(id).is_some_and(|a| (a.available)(shell))
}

/// Runs the action `id`; an id the table lacks does nothing. Asks nothing first: what offers an
/// action shows whether it can act, and each handler keeps its own guards.
pub fn run(shell: &mut Shell, id: &str) {
    if let Some(action) = find(id) {
        (action.run)(shell);
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

/// Whether `keys` were pressed in this frame, and takes them. Matched exactly: `consume_key` lets an
/// extra Shift through, which would make ⇧⌘S save with Save version… off, and ⇧⌘Z undo with Redo off.
fn pressed(ctx: &egui::Context, keys: &Keys) -> bool {
    let exact = |e: &egui::Event| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if *key == keys.key && modifiers.matches_exact(keys.modifiers));
    ctx.input_mut(|i| i.events.iter().any(exact) && i.consume_key(keys.modifiers, keys.key))
}

/// Runs the actions whose keys were pressed in this frame, on every platform, while each is
/// available. A field reads the Edit menu's keys itself, so they stay in the input while it has the
/// keyboard. ⌘Y (Ctrl Y) is Redo's second key, matched as exactly as the table's.
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
    use crate::{CreateError, Opened, ProjectHost, SaveError, Saved, SavedBy, Services, Tab};

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
            None
        }) as _);
        let pick_folder = Some(Arc::new(|_: &str| Some("/pinned".into())) as _);
        let mut shell = Shell::new(Services { projects: Some(host.clone()), pick_project, pick_folder, ..Default::default() });
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
        let states: [(&str, Setup, &[&str]); 18] = [
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
        let fresh = Shell::new(Services::default()).state;
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
            "Open…",
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

    /// No two rows of any menu share their keys but History and Back to current work, which are
    /// never on together; and none has Redo's second key.
    #[test]
    fn only_history_and_back_share_their_keys() {
        let keyed: Vec<(&str, &Keys)> =
            ACTIONS.iter().chain(model_menu::rows()).chain(view_menu::ROWS).filter_map(|a| Some((a.id, a.keys.as_ref()?))).collect();
        let same = |a: &Keys, b: &Keys| a.key == b.key && a.modifiers == b.modifiers;
        let mut shared = Vec::new();
        for (n, (first, keys)) in keyed.iter().enumerate() {
            shared.extend(keyed[n + 1..].iter().filter(|(_, other)| same(keys, other)).map(|(second, _)| (*first, *second)));
        }
        assert_eq!(shared, [(VERSIONS, BACK)]);
        assert!(keyed.iter().all(|(_, keys)| !same(keys, &REDO_TOO)));
    }

    /// Presses `key` with `modifiers`, the only input of a frame.
    fn press(shell: &mut Shell, modifiers: Modifiers, key: Key) {
        let event = egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        let input = egui::RawInput { events: vec![event], ..Default::default() };
        let mut output = egui::Context::default().run_ui(input, |ui| run_shortcuts(shell, ui.ctx()));
        output.textures_delta.clear();
    }

    /// A shortcut pressed runs its own action, and a key left alone runs nothing.
    #[test]
    fn every_shortcut_runs_its_action() {
        let shift_cmd = Modifiers::COMMAND | Modifiers::SHIFT;
        let pressed = [
            (GENERATE, shift_cmd, Key::G),
            (SAVE, Modifiers::COMMAND, Key::S),
            (OPEN, Modifiers::COMMAND, Key::O),
            (SETTINGS, Modifiers::COMMAND, Key::Comma),
        ];
        for (id, modifiers, key) in pressed {
            let (mut shell, saves, picks) = shell();
            unsaved(&mut shell);
            press(&mut shell, modifiers, key);
            assert_eq!(effects(&shell, &saves, &picks), [id]);
        }
        let (mut shell, saves, picks) = shell();
        // Unsaved, so a key that reached Save would show.
        unsaved(&mut shell);
        shell.ask = Some(question());
        press(&mut shell, shift_cmd, Key::G);
        press(&mut shell, Modifiers::COMMAND, Key::O);
        shell.ask = None;
        press(&mut shell, Modifiers::COMMAND, Key::G);
        press(&mut shell, Modifiers::NONE, Key::S);
        assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "not behind a question, nor with the wrong modifiers");
        press(&mut shell, shift_cmd, Key::S);
        assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "⇧⌘S is Save version…'s, never Save's");
    }
}
