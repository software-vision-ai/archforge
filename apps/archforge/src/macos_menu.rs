//! The native menu bar on macOS: the application menu (About Archforge…, Hide and Quit) and File
//! (Open…, Save and Generate…, each enabled while it can act). Choosing an item wakes the window, so
//! it acts with the pointer and keyboard still. When it cannot be built, the window works without it.
#![cfg(target_os = "macos")]

use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender, channel};

use muda::accelerator::Accelerator;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use archforge_ui_egui::Shell;
use archforge_ui_egui::actions::{self, ABOUT, GENERATE, OPEN, QUIT, SAVE};
use archforge_ui_egui::i18n::tr;

pub struct NativeMenu {
    _menu: Menu,
    chosen: Receiver<MenuEvent>,
    open: MenuItem,
    save: MenuItem,
    generate: MenuItem,
}

/// Builds the menu bar and installs it for the application; `ctx` is the window a chosen item wakes.
pub fn build(shell: &Shell, ctx: egui::Context) -> Result<NativeMenu, String> {
    let err = |e: muda::Error| e.to_string();
    let menu = Menu::new();
    let app = Submenu::new("Archforge", true);
    let quit = MenuItem::with_id(QUIT, actions::label(shell, QUIT), true, Accelerator::from_str("CmdOrCtrl+Q").ok());
    let about = MenuItem::with_id(ABOUT, actions::label(shell, ABOUT), true, None);
    app.append_items(&[&about, &PredefinedMenuItem::separator(), &PredefinedMenuItem::hide(None), &PredefinedMenuItem::separator(), &quit])
        .map_err(err)?;
    let file = Submenu::new(tr("File"), true);
    let open = MenuItem::with_id(OPEN, actions::label(shell, OPEN), false, Accelerator::from_str("CmdOrCtrl+O").ok());
    let save = MenuItem::with_id(SAVE, actions::label(shell, SAVE), false, Accelerator::from_str("CmdOrCtrl+S").ok());
    let generate = MenuItem::with_id(GENERATE, actions::label(shell, GENERATE), false, Accelerator::from_str("CmdOrCtrl+Shift+G").ok());
    file.append_items(&[&open, &PredefinedMenuItem::separator(), &save, &PredefinedMenuItem::separator(), &generate]).map_err(err)?;
    menu.append_items(&[&app, &file]).map_err(err)?;
    let (sender, chosen) = channel();
    // Before the menu can fire: muda keeps the first answer it finds for good, and once a handler
    // is set its own receiver gets nothing.
    MenuEvent::set_event_handler(Some(forward(sender, ctx)));
    menu.init_for_nsapp();
    Ok(NativeMenu { _menu: menu, chosen, open, save, generate })
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
    /// Runs the items chosen since the last frame, then enables each for what the shell can do now.
    pub fn frame(&self, shell: &mut Shell) {
        run_chosen(&self.chosen, shell);
        self.open.set_enabled(actions::available(shell, OPEN));
        self.save.set_enabled(actions::available(shell, SAVE));
        self.generate.set_enabled(actions::available(shell, GENERATE));
    }
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

    const ITEMS: [&str; 5] = [ABOUT, QUIT, OPEN, SAVE, GENERATE];

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

    /// The bug of AR-243: without the wake the item waited for the next input. The queue is read
    /// inside the wake, so an item queued only after it would be missed here, as by its frame.
    #[test]
    fn every_item_wakes_the_window_and_is_waiting_when_it_wakes() {
        for item in ITEMS {
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

    /// Save asks the host once, and opens no picker.
    #[test]
    fn save_asks_the_host_to_save() {
        let (mut shell, host, picks) = shell_with_project();
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
