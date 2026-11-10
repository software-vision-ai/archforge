//! The application's own actions in one table: Open, Save, Generate and the like, beside the
//! engine's commands. Menus, shortcuts and Home's search read a row for its name, ask it whether it can act and
//! run it. Availability is a function asked each time; nothing here keeps its answer.

use egui::{Key, Modifiers};

use crate::Shell;
use crate::i18n::tr;
use crate::state::{RecentSort, RecentView};
use crate::theme::ThemeKind;
use crate::ux::{Icon, icons};

pub const ABOUT: &str = "app.about";
pub const QUIT: &str = "app.quit";
pub const GENERATE: &str = "file.generate";
pub const SAVE: &str = "file.save";
pub const OPEN: &str = "file.open";
pub const VERSIONS: &str = "versions.list";
pub const OLDER: &str = "versions.older";
pub const NEWER: &str = "versions.newer";

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

/// An action's keyboard shortcut.
pub struct Keys {
    pub modifiers: Modifiers,
    pub key: Key,
    /// Whether the keys are taken now. This differs from the action's availability on purpose: the
    /// table moved each rule as it was (Open's keys are taken behind a dialog that keeps Open off,
    /// and do nothing), and making them one rule is a change of its own (AR-154).
    pub when: fn(&Shell) -> bool,
}

/// How an action shows in Home's search.
pub struct InSearch {
    pub icon: fn(&Shell) -> &'static Icon,
    /// Its wording there, when not the action's label.
    pub label: Option<fn() -> String>,
    /// Whether the row shows now, when not the action's availability. Open's row differs on purpose
    /// and is broader than its menu item, as with its keys (AR-154).
    pub shown: Option<fn(&Shell) -> bool>,
}

/// Every action. Keys are looked at in this order, and Home's search lists its rows in it.
pub static ACTIONS: &[Action] = &[
    Action { id: ABOUT, label: |_| tr("About Archforge…"), available: |_| true, run: Shell::open_about, keys: None, search: None },
    Action { id: QUIT, label: |_| tr("Quit Archforge"), available: |_| true, run: Shell::request_quit, keys: None, search: None },
    Action {
        id: GENERATE,
        label: |_| tr("Generate…"),
        available: Shell::can_generate,
        run: Shell::open_generate,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::G, when: |s| s.can_generate() && s.ask.is_none() }),
        search: None,
    },
    Action {
        id: SAVE,
        label: |_| tr("Save"),
        available: Shell::can_save,
        run: Shell::save,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::S, when: Shell::can_save }),
        search: None,
    },
    // Its availability is its search row's; Home's start card has a test of its own.
    Action {
        id: "file.new",
        label: |_| tr("New project"),
        available: |s| s.services.projects.is_some() && s.tabs.len() < crate::MAX_TABS,
        run: crate::new_project::open,
        keys: None,
        search: Some(InSearch { icon: |_| &icons::PLUS, label: None, shown: None }),
    },
    Action {
        id: OPEN,
        label: |_| tr("Open…"),
        available: Shell::can_open_project,
        run: Shell::open_from_picker,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::O, when: |s| s.about.is_none() }),
        search: Some(InSearch { icon: |_| &icons::FOLDER, label: Some(|| tr("Open project")), shown: Some(crate::home::can_open) }),
    },
    Action {
        id: "home.pin-folder",
        label: |_| tr("Pin a folder"),
        available: |s| s.services.pick_folder.is_some() && s.state.pinned.len() < crate::state::MAX_PINNED,
        run: Shell::pin_from_picker,
        keys: None,
        search: Some(InSearch { icon: |_| &icons::PLUS, label: None, shown: None }),
    },
    Action {
        id: "home.appearance",
        label: |s| if s.state.theme.is_dark() { tr("Switch to the light appearance") } else { tr("Switch to the dark appearance") },
        available: |_| true,
        run: switch_appearance,
        keys: None,
        search: Some(InSearch { icon: |s| if s.state.theme.is_dark() { &icons::SUN } else { &icons::MOON }, label: None, shown: None }),
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
        search: Some(InSearch {
            icon: |s| if s.state.recent_view == RecentView::List { &icons::GRID } else { &icons::LIST },
            label: None,
            shown: None,
        }),
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
        search: Some(InSearch { icon: |_| &icons::LIST, label: None, shown: None }),
    },
    Action {
        id: VERSIONS,
        label: |_| tr("Versions"),
        available: crate::version_chip::available,
        run: crate::version_chip::toggle,
        keys: Some(Keys { modifiers: Modifiers::COMMAND.plus(Modifiers::SHIFT), key: Key::H, when: crate::version_chip::available }),
        search: None,
    },
    Action {
        id: OLDER,
        label: |_| tr("Older version"),
        available: crate::version_chip::available,
        run: crate::version_chip::older,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::OpenBracket, when: crate::version_chip::available }),
        search: None,
    },
    Action {
        id: NEWER,
        label: |_| tr("Newer version"),
        available: crate::version_chip::available,
        run: crate::version_chip::newer,
        keys: Some(Keys { modifiers: Modifiers::COMMAND, key: Key::CloseBracket, when: crate::version_chip::available }),
        search: None,
    },
];

/// The action `id`.
fn find(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// The name of the action `id`; empty when the table has none.
pub fn label(shell: &Shell, id: &str) -> String {
    find(id).map(|a| (a.label)(shell)).unwrap_or_default()
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

/// Runs the actions whose keys were pressed in this frame, on every platform.
pub(crate) fn run_shortcuts(shell: &mut Shell, ctx: &egui::Context) {
    for (action, keys) in ACTIONS.iter().filter_map(|a| Some((a, a.keys.as_ref()?))) {
        if (keys.when)(shell) && ctx.input_mut(|i| i.consume_key(keys.modifiers, keys.key)) {
            (action.run)(shell);
        }
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

    const NEW: &str = "file.new";
    const PIN: &str = "home.pin-folder";
    const APPEARANCE: &str = "home.appearance";
    const VIEW: &str = "home.recent-view";
    const SORT: &str = "home.recent-sort";
    const ALL: [&str; 10] = [ABOUT, QUIT, GENERATE, SAVE, NEW, OPEN, PIN, APPEARANCE, VIEW, SORT];
    const WITH_KEYS: [&str; 3] = [GENERATE, SAVE, OPEN];
    const IN_SEARCH: [&str; 6] = [NEW, OPEN, PIN, APPEARANCE, VIEW, SORT];

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
    fn session() -> archforge_notations::Session {
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
        (shell, host, picks)
    }

    /// A question about the open project, which keeps Generate's keys off.
    fn question() -> crate::close_ask::Ask {
        crate::close_ask::Ask::Unsaved { path: "/shop.archforge".into() }
    }

    /// A tab, to take up one of the places.
    fn tab() -> Tab {
        Tab { path: "/shop.archforge".into(), name: "Shop".to_string(), parked: None, closing: None, close_error: None }
    }

    /// `all` less what is `off`.
    fn without(all: &[&'static str], off: &[&str]) -> Vec<&'static str> {
        all.iter().copied().filter(|id| !off.contains(id)).collect()
    }

    /// The ids of the rows of the table `on` answers yes for.
    fn answering(on: impl Fn(&Action) -> bool) -> Vec<&'static str> {
        ACTIONS.iter().filter(|a| !a.id.starts_with("versions.") && on(a)).map(|a| a.id).collect()
    }

    /// The whole table, state by state, written out by hand from what each menu item and key did
    /// before there was a table: which actions are off, which keys are left alone and which search
    /// rows go.
    #[test]
    fn every_action_is_available_when_it_was() {
        type Setup = fn(&mut Shell);
        let states: [(&str, Setup, &[&str], &[&str], &[&str]); 15] = [
            ("a project", |_| {}, &[], &[], &[]),
            ("saving", |s| s.save(), &[SAVE], &[SAVE], &[]),
            ("held", |s| s.begin_hold().unwrap(), &[SAVE], &[SAVE], &[]),
            ("viewing a version", |s| s.show_version(session(), "c1".to_string(), crate::viewing::OpensOn::Showing), &[SAVE], &[SAVE], &[]),
            ("About open", crate::support::open, &[GENERATE, OPEN], &[GENERATE, OPEN], &[]),
            ("Generate open", Shell::open_generate, &[GENERATE], &[GENERATE], &[]),
            ("New project open", crate::new_project::open, &[OPEN], &[], &[]),
            ("an opening under way", |s| s.opening = Some(channel().1), &[OPEN], &[], &[]),
            ("a question open", |s| s.ask = Some(question()), &[], &[GENERATE], &[]),
            ("every tab taken", |s| s.tabs = (0..crate::MAX_TABS).map(|_| tab()).collect(), &[NEW, OPEN], &[], &[NEW, OPEN]),
            ("every pin taken", |s| s.state.pinned = (0..crate::state::MAX_PINNED).map(|i| i.to_string().into()).collect(), &[PIN], &[], &[PIN]),
            ("no folder picker", |s| s.services.pick_folder = None, &[PIN], &[], &[PIN]),
            ("no model", |s| s.session = None, &[GENERATE, SAVE], &[GENERATE, SAVE], &[]),
            ("no host", |s| s.services.projects = None, &[NEW, OPEN], &[], &[NEW, OPEN]),
            ("no project picker", |s| s.services.pick_project = None, &[OPEN], &[], &[OPEN]),
        ];
        for (name, setup, off, keys_off, search_off) in states {
            let (mut shell, ..) = shell();
            setup(&mut shell);
            let shell = &shell;
            assert_eq!(answering(|a| (a.available)(shell)), without(&ALL, off), "available, {name}");
            assert_eq!(answering(|a| a.keys.as_ref().is_some_and(|k| (k.when)(shell))), without(&WITH_KEYS, keys_off), "keys, {name}");
            let shown = |a: &Action| a.search.as_ref().is_some_and(|s| s.shown.unwrap_or(a.available)(shell));
            assert_eq!(answering(shown), without(&IN_SEARCH, search_off), "search, {name}");
        }
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
            (OPEN, picks.load(Ordering::SeqCst) == 1),
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
            ACTIONS.iter().map(|a| a.id).filter(|id| !id.starts_with("versions.")).collect::<Vec<_>>(),
            ALL,
            "the chip's tests walk its own row"
        );
        for id in ALL {
            let (mut shell, saves, picks) = shell();
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
            "Quit Archforge",
            "Generate…",
            "Save",
            "New project",
            "Open…",
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

    /// A shortcut pressed runs its own action, and a key left alone runs nothing.
    #[test]
    fn every_shortcut_runs_its_action() {
        let press = |shell: &mut Shell, modifiers, key| {
            let event = egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
            let input = egui::RawInput { events: vec![event], ..Default::default() };
            let mut output = egui::Context::default().run_ui(input, |ui| run_shortcuts(shell, ui.ctx()));
            output.textures_delta.clear();
        };
        for (id, modifiers, key) in
            [(GENERATE, Modifiers::COMMAND | Modifiers::SHIFT, Key::G), (SAVE, Modifiers::COMMAND, Key::S), (OPEN, Modifiers::COMMAND, Key::O)]
        {
            let (mut shell, saves, picks) = shell();
            press(&mut shell, modifiers, key);
            assert_eq!(effects(&shell, &saves, &picks), [id]);
        }
        let (mut shell, saves, picks) = shell();
        shell.ask = Some(question());
        press(&mut shell, Modifiers::COMMAND | Modifiers::SHIFT, Key::G);
        shell.ask = None;
        press(&mut shell, Modifiers::COMMAND, Key::G);
        press(&mut shell, Modifiers::NONE, Key::S);
        assert_eq!(effects(&shell, &saves, &picks), Vec::<&str>::new(), "not behind a question, nor with the wrong modifiers");
    }
}
