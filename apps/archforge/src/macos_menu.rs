//! The native menu bar on macOS, built from the table of actions: the application menu (with the
//! system's Services, Hide, Hide Others and Show All), File, Edit, View, Model and Versions, in the
//! design's order, then Window with the system's own Minimise and Zoom, made the application's
//! windows menu so macOS adds its own rows below them. Each item from the table is enabled while its action can act, and its keys are the
//! table's. Choosing an item wakes the window, so it acts with the pointer and keyboard still. When
//! it cannot be built, the window works without it.
#![cfg(target_os = "macos")]

use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender, channel};

use muda::accelerator::Accelerator;
use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use archforge_ui_egui::Shell;
use archforge_ui_egui::actions::{self, ABOUT, COPY, CUT, Keys, PASTE, QUIT, REDO, SELECT_ALL, SETTINGS, UNDO, VERSIONS_MENU};
use archforge_ui_egui::i18n::{tr, tr_ctx};
use archforge_ui_egui::model_menu;
use archforge_ui_egui::view_menu::VIEW_MENU;

/// A run of actions, set apart from the next by a separator.
type Group = &'static [&'static str];
/// Every item, by the action it runs: the plain ones, and those with a tick.
#[derive(Default)]
struct Items {
    plain: Vec<(&'static str, MenuItem)>,
    ticked: Vec<(&'static str, CheckMenuItem)>,
}

/// The menus of the bar, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Title {
    App,
    File,
    Edit,
    View,
    Model,
    Versions,
    Window,
}

const BAR: [Title; 7] = [Title::App, Title::File, Title::Edit, Title::View, Title::Model, Title::Versions, Title::Window];
/// A row of the application menu: an action of the table, or one of the system's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppRow {
    Table(&'static str),
    System(System),
}

/// The system's own rows of the application menu, which act without the table of actions and which
/// Archforge never turns off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum System {
    Services,
    Hide,
    HideOthers,
    ShowAll,
}

/// The application menu in the system's usual order, a separator between groups: About, Settings,
/// Services, the three that hide and show applications, Quit.
const APP: [&[AppRow]; 5] = [
    &[AppRow::Table(ABOUT)],
    &[AppRow::Table(SETTINGS)],
    &[AppRow::System(System::Services)],
    &[AppRow::System(System::Hide), AppRow::System(System::HideOthers), AppRow::System(System::ShowAll)],
    &[AppRow::Table(QUIT)],
];
const EDIT: [Group; 3] = [&[UNDO, REDO], &[CUT, COPY, PASTE], &[SELECT_ALL]];

pub struct NativeMenu {
    _menu: Menu,
    chosen: Receiver<MenuEvent>,
    items: Items,
    model: Submenu,
    /// The rows `model` holds, by section; none until the first frame fills it.
    model_rows: Vec<Vec<&'static str>>,
    /// Whether the last rebuild of `model` failed, so a failure is logged once, not every frame.
    failing: bool,
}

/// The keys of `keys` as the menu bar writes them, by the key's symbol or, for a key such as an
/// arrow whose symbol the menu bar has no word for, by its name; `None` when it has neither.
fn accelerator(keys: &Keys) -> Option<Accelerator> {
    let alt = if keys.modifiers.alt { "Alt+" } else { "" };
    let shift = if keys.modifiers.shift { "Shift+" } else { "" };
    let with = |key: &str| Accelerator::from_str(&format!("CmdOrCtrl+{alt}{shift}{key}")).ok();
    with(keys.key.symbol_or_name()).or_else(|| with(keys.key.name()))
}

/// The table's item for action `id`, off until the first frame asks; recorded in `items`.
fn item(shell: &Shell, id: &'static str, items: &mut Items) -> MenuItem {
    let new = MenuItem::with_id(id, actions::label(shell, id), false, actions::keys(id).and_then(accelerator));
    items.plain.push((id, new.clone()));
    new
}

/// As `item`, for an action that shows and hides something: an item that can wear a tick.
fn ticked_item(shell: &Shell, id: &'static str, items: &mut Items) -> CheckMenuItem {
    let new = CheckMenuItem::with_id(id, actions::label(shell, id), false, false, actions::keys(id).and_then(accelerator));
    items.ticked.push((id, new.clone()));
    new
}

/// Appends `groups` to `menu`, with a separator between groups; an action with a tick gets an item that shows it.
fn append_groups<G: AsRef<[&'static str]>>(shell: &Shell, menu: &Submenu, groups: &[G], items: &mut Items) -> Result<(), muda::Error> {
    for (n, group) in groups.iter().enumerate() {
        if n > 0 {
            menu.append(&PredefinedMenuItem::separator())?;
        }
        for id in group.as_ref() {
            match actions::ticked(shell, id) {
                Some(_) => menu.append(&ticked_item(shell, id, items))?,
                None => menu.append(&item(shell, id, items))?,
            }
        }
    }
    Ok(())
}

/// The system's own item `row`, in the catalogues' words, with the keys muda gives it: ⌘H for
/// Hide and ⌥⌘H for Hide Others. Services is the submenu macOS fills with its services.
fn system_item(row: System) -> PredefinedMenuItem {
    match row {
        System::Services => PredefinedMenuItem::services(Some(&tr("Services"))),
        System::Hide => PredefinedMenuItem::hide(Some(&tr("Hide Archforge"))),
        System::HideOthers => PredefinedMenuItem::hide_others(Some(&tr("Hide Others"))),
        System::ShowAll => PredefinedMenuItem::show_all(Some(&tr("Show All"))),
    }
}

/// `APP`'s rows from the top, `None` for the separator between two groups.
fn app_rows() -> impl Iterator<Item = Option<AppRow>> {
    APP.iter().enumerate().flat_map(|(n, group)| (n > 0).then_some(None).into_iter().chain(group.iter().map(|&row| Some(row))))
}

/// The application menu with `APP`'s rows, those of the table recorded in `items`.
fn app_menu(shell: &Shell, items: &mut Items) -> Result<Submenu, muda::Error> {
    let menu = Submenu::new("Archforge", true);
    for row in app_rows() {
        match row {
            None => menu.append(&PredefinedMenuItem::separator())?,
            Some(AppRow::Table(id)) => menu.append(&item(shell, id, items))?,
            Some(AppRow::System(system)) => menu.append(&system_item(system))?,
        }
    }
    Ok(menu)
}

/// The Window menu's items, in its order: the system's own, which act on the window without the
/// table of actions, Minimise with the system's ⌘M.
fn window_items() -> [PredefinedMenuItem; 2] {
    [PredefinedMenuItem::minimize(Some(&tr("Minimise"))), PredefinedMenuItem::zoom(Some(&tr_ctx("window", "Zoom")))]
}

/// The menu `title` with its items, recorded in `items`; for Model, `model`, which the first frame fills.
fn submenu(shell: &Shell, title: Title, model: &Submenu, items: &mut Items) -> Result<Submenu, muda::Error> {
    let filled = |name: String, groups: &[&[&'static str]], items: &mut Items| {
        let menu = Submenu::new(name, true);
        append_groups(shell, &menu, groups, items).map(|()| menu)
    };
    match title {
        Title::App => app_menu(shell, items),
        Title::File => {
            let menu = Submenu::new(actions::file_title(shell), true);
            append_groups(shell, &menu, &actions::file_menu(shell), items).map(|()| menu)
        }
        Title::Edit => filled(tr("Edit"), &EDIT, items),
        Title::View => filled(tr_ctx("menu", "View"), &VIEW_MENU, items),
        Title::Model => Ok(model.clone()),
        Title::Versions => filled(tr("Versions"), &VERSIONS_MENU, items),
        Title::Window => {
            let window = Submenu::new(tr_ctx("menu", "Window"), true);
            let [minimise, zoom] = window_items();
            window.append_items(&[&minimise, &zoom]).map(|()| window)
        }
    }
}

/// Makes the bar the application's with `install`, then hands the Window menu among `menus` to
/// `register`, which makes it the application's windows menu: muda finds that menu only through the
/// bar the application already shows, and macOS then adds its own rows below Minimise and Zoom.
fn install_and_register<M>(menus: &[(Title, M)], install: impl FnOnce(), register: impl FnOnce(&M)) {
    install();
    if let Some((_, window)) = menus.iter().find(|(title, _)| *title == Title::Window) {
        register(window);
    }
}

/// Builds the menu bar and installs it for the application; `ctx` is the window a chosen item wakes.
pub fn build(shell: &Shell, ctx: egui::Context) -> Result<NativeMenu, String> {
    let err = |e: muda::Error| e.to_string();
    let menu = Menu::new();
    let mut items = Items::default();
    let model = Submenu::new(tr("Model"), true);
    let mut menus = Vec::with_capacity(BAR.len());
    for title in BAR {
        let built = submenu(shell, title, &model, &mut items).map_err(err)?;
        menu.append(&built).map_err(err)?;
        menus.push((title, built));
    }
    let (sender, chosen) = channel();
    // Before the menu can fire: muda keeps the first answer it finds for good, and once a handler
    // is set its own receiver gets nothing.
    MenuEvent::set_event_handler(Some(forward(sender, ctx)));
    install_and_register(&menus, || menu.init_for_nsapp(), Submenu::set_as_windows_menu_for_nsapp);
    Ok(NativeMenu { _menu: menu, chosen, items, model, model_rows: Vec::new(), failing: false })
}

/// What the menu bar calls for each chosen item: queues it for the next frame and wakes the window,
/// since egui draws a frame only on input or a repaint request. The item is queued first, so the
/// frame the wake brings finds it. A send fails only once the window is gone: nothing to wake.
fn forward(sender: Sender<MenuEvent>, ctx: egui::Context) -> impl Fn(MenuEvent) + Send + Sync + 'static {
    move |event| {
        if sender.send(event).is_ok() {
            ctx.request_repaint();
        }
    }
}

/// Runs every item waiting in `chosen`, in the order they were chosen.
fn run_chosen(chosen: &Receiver<MenuEvent>, shell: &mut Shell) {
    while let Ok(event) = chosen.try_recv() {
        actions::run(shell, event.id().0.as_str());
    }
}

impl NativeMenu {
    /// Runs the items chosen since the last frame, then fits Model to the diagram, enables each for
    /// what the shell can do now and ticks those whose thing shows: the system flips a tick when
    /// its item is chosen, so each is set again from the shell.
    pub fn frame(&mut self, shell: &mut Shell) {
        run_chosen(&self.chosen, shell);
        self.follow_diagram(shell);
        for (id, item) in &self.items.plain {
            item.set_enabled(actions::available(shell, id));
        }
        for (id, item) in &self.items.ticked {
            item.set_enabled(actions::available(shell, id));
            item.set_checked(actions::ticked(shell, id) == Some(true));
        }
    }

    /// Rebuilds the Model menu when the diagram's rows have changed, again each frame while that fails.
    fn follow_diagram(&mut self, shell: &Shell) {
        let rows = model_menu::sections(shell);
        if rows == self.model_rows {
            return;
        }
        while self.model.remove_at(0).is_some() {}
        self.items.plain.retain(|(id, _)| !id.starts_with("model."));
        let built = append_groups(shell, &self.model, &rows, &mut self.items);
        self.model_rows = settle(rows, built, &mut self.failing);
    }
}

/// The rows the Model menu holds after a rebuild of `rows`: all of them when it worked, none when it
/// failed, so the next frame tries again. Only the first of failures in a row is logged.
fn settle(rows: Vec<Vec<&'static str>>, built: Result<(), muda::Error>, failing: &mut bool) -> Vec<Vec<&'static str>> {
    if !*failing && let Err(e) = &built {
        log::warn!("the Model menu could not be rebuilt, trying again: {e}");
    }
    *failing = built.is_err();
    if *failing { Vec::new() } else { rows }
}

// The queue and the wake, free of the operating system's menu bar: building a real `Menu` needs the
// main thread, and the call from a click to `forward` is the system's. Release checklist item 5
// covers that part.
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use archforge_ui_egui::{CreateError, Opened, ProjectHost, SaveError, Saved, SavedBy, Services};
    use muda::MenuId;

    use archforge_ui_egui::actions::{
        BACK, BRANCHES, CLOSE, COMPARE, FIND, GENERATE, NEW, NEW_BRANCH, NEWER, OLDER, OPEN, SAVE, SAVE_VERSION, VERSIONS,
    };
    use archforge_ui_egui::view_menu::{APPEARANCE, CHAT, DIAGRAMS, MINIMAP, MODELS, PROPERTIES, SKILLS, WHOLE, ZOOM_IN, ZOOM_OUT};

    const ITEMS: [&str; 14] = [ABOUT, SETTINGS, QUIT, NEW, OPEN, CLOSE, SAVE, GENERATE, UNDO, REDO, CUT, COPY, PASTE, SELECT_ALL];
    /// The Versions menu's items, after the rest.
    const VERSION_ITEMS: [&str; 8] = [SAVE_VERSION, VERSIONS, COMPARE, NEW_BRANCH, BRANCHES, OLDER, NEWER, BACK];
    /// The View menu's items.
    const VIEW_ITEMS: [&str; 12] =
        [FIND, ZOOM_IN, ZOOM_OUT, WHOLE, MINIMAP, DIAGRAMS, MODELS, archforge_ui_egui::view_menu::VERSIONS, CHAT, APPEARANCE, PROPERTIES, SKILLS];

    /// The event the menu bar sends when the item `id` is chosen.
    fn event(id: &str) -> MenuEvent {
        MenuEvent { id: MenuId::new(id) }
    }

    /// A host that counts the saves asked of it; the saves never finish.
    #[derive(Default)]
    struct CountingHost(AtomicUsize);

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

    /// A shell with a project open and a picker and host that count their calls.
    fn shell_with_project() -> (Shell, Arc<CountingHost>, Arc<AtomicUsize>) {
        let host = Arc::new(CountingHost::default());
        let picks = Arc::new(AtomicUsize::new(0));
        let counted = picks.clone();
        let services = Services {
            projects: Some(host.clone()),
            pick_project: Some(Arc::new(move |_| {
                counted.fetch_add(1, Ordering::SeqCst);
                None
            })),
            ..Default::default()
        };
        let mut shell = Shell::new(services);
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid(n.to_le_bytes())
        });
        shell.session = Some(archforge_notations::new_project_session("Shop", ids).unwrap());
        shell.project = Some(Opened { name: "Shop".to_string(), path: "/shop.archforge".into(), model: None, unsaved: false, notice: None });
        (shell, host, picks)
    }

    /// Chooses `item` as the menu bar does, then runs what the woken frame finds.
    fn choose(item: &str, shell: &mut Shell) {
        let (sender, chosen) = channel();
        forward(sender, egui::Context::default())(event(item));
        run_chosen(&chosen, shell);
    }

    /// The menus hold every item the table should give them, each with a name, and with the keys of
    /// the design's map.
    #[test]
    fn the_menus_are_the_tables_with_the_designs_keys() {
        let (shell, ..) = shell_with_project();
        let file = actions::file_menu(&shell);
        let file: Vec<&[&str]> = file.iter().map(Vec::as_slice).collect();
        let app: Vec<&str> = APP.concat().into_iter().filter_map(|row| if let AppRow::Table(id) = row { Some(id) } else { None }).collect();
        let listed: Vec<&str> = [&[&app[..]][..], &file[..], &EDIT[..], &VIEW_MENU[..], &VERSIONS_MENU[..]].concat().concat();
        let mut sorted_items = [&ITEMS[..], &VERSION_ITEMS[..], &VIEW_ITEMS[..]].concat();
        sorted_items.sort_unstable();
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, sorted_items);
        let keyed = [
            (SETTINGS, Some("CmdOrCtrl+Comma")),
            (QUIT, Some("CmdOrCtrl+Q")),
            (OPEN, Some("CmdOrCtrl+O")),
            (SAVE, Some("CmdOrCtrl+S")),
            (GENERATE, Some("CmdOrCtrl+Shift+G")),
            (UNDO, Some("CmdOrCtrl+Z")),
            (REDO, Some("CmdOrCtrl+Shift+Z")),
            (CUT, Some("CmdOrCtrl+X")),
            (COPY, Some("CmdOrCtrl+C")),
            (PASTE, Some("CmdOrCtrl+V")),
            (SELECT_ALL, Some("CmdOrCtrl+A")),
            (SAVE_VERSION, Some("CmdOrCtrl+Shift+S")),
            (VERSIONS, Some("CmdOrCtrl+Shift+H")),
            (BRANCHES, Some("CmdOrCtrl+Shift+B")),
            (OLDER, Some("CmdOrCtrl+Alt+ArrowLeft")),
            (NEWER, Some("CmdOrCtrl+Alt+ArrowRight")),
            (BACK, Some("CmdOrCtrl+Shift+H")),
            (FIND, Some("CmdOrCtrl+K")),
            (CLOSE, Some("CmdOrCtrl+W")),
            (COMPARE, None),
            (NEW_BRANCH, None),
            (ZOOM_IN, None),
            (WHOLE, None),
            (SKILLS, None),
            (ABOUT, None),
            (NEW, None),
        ];
        for (id, wanted) in keyed {
            assert!(!actions::label(&shell, id).is_empty(), "{id}");
            let keys = actions::keys(id).and_then(accelerator);
            assert_eq!(keys, wanted.map(|w| Accelerator::from_str(w).unwrap()), "{id}");
        }
    }

    /// A Model menu that failed to rebuild is tried again on the next frame, as no real rows are
    /// empty; once it works its rows are kept.
    #[test]
    fn a_failed_rebuild_of_model_is_tried_again() {
        let (rows, mut failing) = (vec![vec!["model.note", "model.link"]], false);
        assert_eq!((settle(rows.clone(), Err(muda::Error::NotAChildOfThisMenu), &mut failing), failing), (vec![], true));
        assert_eq!((settle(rows.clone(), Ok(()), &mut failing), failing), (rows, false));
    }

    /// The application menu holds About, Settings…, Services, Hide Archforge, Hide Others, Show All
    /// and Quit Archforge in the system's usual order and groups, the system's rows in the
    /// catalogues' words; what each does is seen by hand (release checklist item 5).
    #[test]
    fn the_application_menu_has_the_systems_rows_in_their_usual_order() {
        use AppRow::{System as Own, Table};
        let own = [System::Services, System::Hide, System::HideOthers, System::ShowAll];
        let [services, hide, others, all] = own.map(|row| Some(Own(row)));
        let rows = [Some(Table(ABOUT)), None, Some(Table(SETTINGS)), None, services, None, hide, others, all, None, Some(Table(QUIT))];
        assert_eq!(app_rows().collect::<Vec<_>>(), rows);
        for (code, words) in [
            ("en", ["Services", "Hide Archforge", "Hide Others", "Show All"]),
            ("de", ["Dienste", "Archforge ausblenden", "Andere ausblenden", "Alle einblenden"]),
        ] {
            assert!(archforge_ui_egui::i18n::set_language(code), "{code}");
            assert_eq!(own.map(|row| system_item(row).text()), words, "{code}");
        }
    }

    /// The bar ends in the Window menu, after Versions, and Window holds the system's Minimise and
    /// then Zoom, in the catalogues' words.
    #[test]
    fn the_bar_ends_in_a_window_menu_of_minimise_and_zoom() {
        use Title::{App, Edit, File, Model, Versions, View, Window};
        assert_eq!(BAR, [App, File, Edit, View, Model, Versions, Window]);
        for (code, words) in [("en", ["Minimise", "Zoom"]), ("de", ["Minimieren", "Zoomen"])] {
            assert!(archforge_ui_egui::i18n::set_language(code), "{code}");
            assert_eq!(window_items().map(|item| item.text()), words, "{code}");
        }
    }

    /// The bar is the application's before Window becomes its windows menu, as muda finds that menu
    /// only through the bar the application shows, and Window is found by its title wherever it
    /// stands; the rows macOS then adds are seen by hand (release checklist item 5).
    #[test]
    fn window_becomes_the_windows_menu_once_the_bar_is_installed() {
        let menus = [(Title::Versions, 1), (Title::Window, 2), (Title::Model, 3)];
        let steps = std::cell::RefCell::new(Vec::new());
        install_and_register(&menus, || steps.borrow_mut().push(None), |menu| steps.borrow_mut().push(Some(*menu)));
        assert_eq!(steps.into_inner(), vec![None, Some(2)]);
    }

    /// The bug of AR-243: without the wake the item waited for the next input. The queue is read
    /// inside the wake, so an item queued only after it would be missed here, as by its frame.
    #[test]
    fn every_item_wakes_the_window_and_is_waiting_when_it_wakes() {
        for item in ITEMS.into_iter().chain(VERSION_ITEMS).chain(VIEW_ITEMS) {
            let ctx = egui::Context::default();
            let (sender, chosen) = channel();
            let chosen = Mutex::new(chosen);
            let woken = Arc::new(Mutex::new(Vec::new()));
            let seen = woken.clone();
            ctx.set_request_repaint_callback(move |_| {
                let waiting = chosen.lock().unwrap().try_recv().map(|e: MenuEvent| e.id.0);
                seen.lock().unwrap().push(waiting);
            });
            forward(sender, ctx.clone())(event(item));
            assert_eq!(*woken.lock().unwrap(), vec![Ok(item.to_string())], "{item} must wake the window once, already queued");
        }
    }

    /// About opens its dialog, and nothing else.
    #[test]
    fn about_opens_its_dialog() {
        let (mut shell, ..) = shell_with_project();
        choose(ABOUT, &mut shell);
        assert!(shell.about.is_some());
        assert!(shell.generate.is_none(), "only About was chosen");
    }

    /// Generate… opens its dialog, and nothing else.
    #[test]
    fn generate_opens_its_dialog() {
        let (mut shell, ..) = shell_with_project();
        choose(GENERATE, &mut shell);
        assert!(shell.generate.is_some());
        assert!(shell.about.is_none(), "only Generate was chosen");
    }

    /// Open project… asks the picker once, and saves nothing.
    #[test]
    fn open_asks_for_a_project() {
        let (mut shell, host, picks) = shell_with_project();
        choose(OPEN, &mut shell);
        assert_eq!(picks.load(Ordering::SeqCst), 1);
        assert_eq!(host.0.load(Ordering::SeqCst), 0, "only Open was chosen");
    }

    /// Save asks the host once for unsaved work, and opens no picker.
    #[test]
    fn save_asks_the_host_to_save() {
        let (mut shell, host, picks) = shell_with_project();
        if let Some(s) = shell.session.as_mut() {
            s.mark_unsaved();
        }
        choose(SAVE, &mut shell);
        assert_eq!(host.0.load(Ordering::SeqCst), 1);
        assert_eq!(picks.load(Ordering::SeqCst), 0, "only Save was chosen");
    }

    /// With nothing unsaved, Quit closes the window in the frame that runs it.
    #[test]
    fn quit_closes_the_window_on_the_next_frame() {
        let mut shell = Shell::new(Services::default());
        choose(QUIT, &mut shell);
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| shell.logic(ui.ctx()));
        output.textures_delta.clear();
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&egui::ViewportCommand::Close), "{commands:?}");
    }
}
