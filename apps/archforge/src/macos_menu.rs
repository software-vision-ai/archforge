//! The native menu bar on macOS. Home, the start screen, is the whole window, so the bar is the
//! application menu alone: Hide and Quit. When it cannot be built, the window works without it.
#![cfg(target_os = "macos")]

use std::str::FromStr;

use muda::accelerator::Accelerator;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use archforge_ui_egui::Shell;
use archforge_ui_egui::i18n::tr;

const QUIT: &str = "app.quit";

pub struct NativeMenu {
    _menu: Menu,
}

/// Builds the menu bar and installs it for the application.
pub fn build() -> Result<NativeMenu, String> {
    let err = |e: muda::Error| e.to_string();
    let menu = Menu::new();
    let app = Submenu::new("Archforge", true);
    let quit = MenuItem::with_id(QUIT, tr("Quit Archforge"), true, Accelerator::from_str("CmdOrCtrl+Q").ok());
    app.append_items(&[&PredefinedMenuItem::hide(None), &PredefinedMenuItem::separator(), &quit]).map_err(err)?;
    menu.append(&app).map_err(err)?;
    menu.init_for_nsapp();
    Ok(NativeMenu { _menu: menu })
}

impl NativeMenu {
    /// Runs the items chosen since the last frame.
    pub fn frame(&self, shell: &mut Shell) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id().0 == QUIT {
                shell.request_quit();
            }
        }
    }
}
