//! The native menu bar on macOS, built from the table of actions: the application menu (with the
//! system's Hide), File, Edit, Model and Versions, in the design's order. Each item is enabled while its action can
//! act, and its keys are the table's. Choosing an item wakes the window, so it acts with the pointer
//! and keyboard still. When it cannot be built, the window works without it.
#![cfg(target_os = "macos")]

use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender, channel};

use muda::accelerator::Accelerator;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use archforge_ui_egui::Shell;
use archforge_ui_egui::actions::{self, ABOUT, CLOSE, GENERATE, Keys, NEW, OPEN, QUIT, REDO, SAVE, SETTINGS, UNDO, VERSIONS_MENU};
use archforge_ui_egui::i18n::tr;
use archforge_ui_egui::model_menu;

/// A run of actions, set apart from the next by a separator.
type Group = &'static [&'static str];
/// Every item, by the action it runs.
type Items = Vec<(&'static str, MenuItem)>;

const APP: [Group; 2] = [&[ABOUT], &[SETTINGS]];
const FILE: [Group; 3] = [&[NEW, OPEN, CLOSE], &[SAVE], &[GENERATE]];
const EDIT: [Group; 1] = [&[UNDO, REDO]];

pub struct NativeMenu {
    _menu: Menu,
    chosen: Receiver<MenuEvent>,
    /// Every item, by the action it runs.
    items: Vec<(&'static str, MenuItem)>,
    model: Submenu,
    /// The rows `model` holds, by section; none until the first frame fills it.
    model_rows: Vec<Vec<&'static str>>,
    /// Whether the last rebuild of `model` failed, so a failure is logged once, not every frame.
    failing: bool,
}

/// The keys of `keys` as the menu bar writes them; `None` for a key it has no word for.
fn accelerator(keys: &Keys) -> Option<Accelerator> {
    let shift = if keys.modifiers.shift { "Shift+" } else { "" };
    Accelerator::from_str(&format!("CmdOrCtrl+{shift}{}", keys.key.symbol_or_name())).ok()
}

/// The table's item for action `id`, off until the first frame asks; recorded in `items`.
fn item(shell: &Shell, id: &'static str, items: &mut Vec<(&'static str, MenuItem)>) -> MenuItem {
    let new = MenuItem::with_id(id, actions::label(shell, id), false, actions::keys(id).and_then(accelerator));
    items.push((id, new.clone()));
    new
}

/// Appends `groups` to `menu`, with a separator between groups.
fn append_groups<G: AsRef<[&'static str]>>(shell: &Shell, menu: &Submenu, groups: &[G], items: &mut Items) -> Result<(), muda::Error> {
    for (n, group) in groups.iter().enumerate() {
        if n > 0 {
            menu.append(&PredefinedMenuItem::separator())?;
        }
        for id in group.as_ref() {
            menu.append(&item(shell, id, items))?;
        }
    }
    Ok(())
}

/// Builds the menu bar and installs it for the application; `ctx` is the window a chosen item wakes.
pub fn build(shell: &Shell, ctx: egui::Context) -> Result<NativeMenu, String> {
    let err = |e: muda::Error| e.to_string();
    let menu = Menu::new();
    let mut items = Vec::new();
    let app = Submenu::new("Archforge", true);
    append_groups(shell, &app, &APP, &mut items).map_err(err)?;
    let quit = item(shell, QUIT, &mut items);
    app.append_items(&[&PredefinedMenuItem::separator(), &PredefinedMenuItem::hide(None), &PredefinedMenuItem::separator(), &quit]).map_err(err)?;
    let file = Submenu::new(tr("File"), true);
    append_groups(shell, &file, &FILE, &mut items).map_err(err)?;
    let edit = Submenu::new(tr("Edit"), true);
    append_groups(shell, &edit, &EDIT, &mut items).map_err(err)?;
    let model = Submenu::new(tr("Model"), true);
    let versions = Submenu::new(tr("Versions"), true);
    append_groups(shell, &versions, &VERSIONS_MENU, &mut items).map_err(err)?;
    menu.append_items(&[&app, &file, &edit, &model, &versions]).map_err(err)?;
    let (sender, chosen) = channel();
    // Before the menu can fire: muda keeps the first answer it finds for good, and once a handler
    // is set its own receiver gets nothing.
    MenuEvent::set_event_handler(Some(forward(sender, ctx)));
    menu.init_for_nsapp();
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
    /// Runs the items chosen since the last frame, then fits Model to the diagram and enables each for what the shell can do now.
    pub fn frame(&mut self, shell: &mut Shell) {
        run_chosen(&self.chosen, shell);
        self.follow_diagram(shell);
        for (id, item) in &self.items {
            item.set_enabled(actions::available(shell, id));
        }
    }

    /// Rebuilds the Model menu when the diagram's rows have changed, again each frame while that fails.
    fn follow_diagram(&mut self, shell: &Shell) {
        let rows = model_menu::sections(shell);
        if rows == self.model_rows {
            return;
        }
        while self.model.remove_at(0).is_some() {}
        self.items.retain(|(id, _)| !id.starts_with("model."));
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

    use archforge_ui_egui::actions::{BACK, BRANCHES, COMPARE, NEW_BRANCH, NEWER, OLDER, SAVE_VERSION, VERSIONS};

    const ITEMS: [&str; 10] = [ABOUT, SETTINGS, QUIT, NEW, OPEN, CLOSE, SAVE, GENERATE, UNDO, REDO];
    /// The Versions menu's items, after the rest.
    const VERSION_ITEMS: [&str; 8] = [SAVE_VERSION, VERSIONS, COMPARE, NEW_BRANCH, BRANCHES, OLDER, NEWER, BACK];

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
        let listed: Vec<&str> = [&APP[..], &[&[QUIT]], &FILE[..], &EDIT[..], &VERSIONS_MENU[..]].concat().concat();
        let mut sorted_items = [&ITEMS[..], &VERSION_ITEMS[..]].concat();
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
            (SAVE_VERSION, Some("CmdOrCtrl+Shift+S")),
            (VERSIONS, Some("CmdOrCtrl+Shift+H")),
            (BRANCHES, Some("CmdOrCtrl+Shift+B")),
            (OLDER, Some("CmdOrCtrl+BracketLeft")),
            (NEWER, Some("CmdOrCtrl+BracketRight")),
            (BACK, Some("CmdOrCtrl+Shift+H")),
            (COMPARE, None),
            (NEW_BRANCH, None),
            (ABOUT, None),
            (NEW, None),
            (CLOSE, None),
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

    /// The bug of AR-243: without the wake the item waited for the next input. The queue is read
    /// inside the wake, so an item queued only after it would be missed here, as by its frame.
    #[test]
    fn every_item_wakes_the_window_and_is_waiting_when_it_wakes() {
        for item in ITEMS.into_iter().chain(VERSION_ITEMS) {
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

    /// Open… asks the picker once, and saves nothing.
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
