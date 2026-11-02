//! The native menu bar on macOS: the application menu (About Archforge…, Hide and Quit) and File
//! (Open…, Save and Generate…, each enabled while it can act). When it cannot be built, the window works without it.
#![cfg(target_os = "macos")]

use std::str::FromStr;

use muda::accelerator::Accelerator;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use archforge_ui_egui::Shell;
use archforge_ui_egui::i18n::tr;

const ABOUT: &str = "app.about";
const QUIT: &str = "app.quit";
const OPEN: &str = "file.open";
const SAVE: &str = "file.save";
const GENERATE: &str = "file.generate";

pub struct NativeMenu {
    _menu: Menu,
    open: MenuItem,
    save: MenuItem,
    generate: MenuItem,
}

/// Builds the menu bar and installs it for the application.
pub fn build() -> Result<NativeMenu, String> {
    let err = |e: muda::Error| e.to_string();
    let menu = Menu::new();
    let app = Submenu::new("Archforge", true);
    let quit = MenuItem::with_id(QUIT, tr("Quit Archforge"), true, Accelerator::from_str("CmdOrCtrl+Q").ok());
    let about = MenuItem::with_id(ABOUT, tr("About Archforge…"), true, None);
    app.append_items(&[&about, &PredefinedMenuItem::separator(), &PredefinedMenuItem::hide(None), &PredefinedMenuItem::separator(), &quit])
        .map_err(err)?;
    let file = Submenu::new(tr("File"), true);
    let open = MenuItem::with_id(OPEN, tr("Open…"), false, Accelerator::from_str("CmdOrCtrl+O").ok());
    let save = MenuItem::with_id(SAVE, tr("Save"), false, Accelerator::from_str("CmdOrCtrl+S").ok());
    let generate = MenuItem::with_id(GENERATE, tr("Generate…"), false, Accelerator::from_str("CmdOrCtrl+Shift+G").ok());
    file.append_items(&[&open, &PredefinedMenuItem::separator(), &save, &PredefinedMenuItem::separator(), &generate]).map_err(err)?;
    menu.append_items(&[&app, &file]).map_err(err)?;
    menu.init_for_nsapp();
    Ok(NativeMenu { _menu: menu, open, save, generate })
}

impl NativeMenu {
    /// Runs the items chosen since the last frame, then enables each for what the shell can do now.
    pub fn frame(&self, shell: &mut Shell) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id().0.as_str() {
                ABOUT => shell.open_about(),
                QUIT => shell.request_quit(),
                OPEN => shell.open_from_picker(),
                SAVE => shell.save(),
                GENERATE => shell.open_generate(),
                _ => {}
            }
        }
        self.open.set_enabled(shell.can_open_project());
        self.save.set_enabled(shell.can_save());
        self.generate.set_enabled(shell.can_generate());
    }
}
