//! Archforge: the desktop application, a thin binary over the shell.
//!
//! Start-up order: arguments (an unknown option exits 2 before any I/O), `--version` (prints
//! without logging), the logger, the panic hook, the settings directory, preferences, then the
//! window, retried with the fallback graphics choice when the first one cannot start.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app_dirs;
mod build_info;
mod fonts;
mod graphics;
mod logging;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "macos")]
mod macos_window;
mod prefs;
mod projects;
mod services;
#[cfg(test)]
mod winres;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use archforge_ui_egui::Shell;

const USAGE: &str = "usage: archforge [--version | --help]";

#[derive(Debug, Default)]
struct Args {
    version: bool,
    help: bool,
}

fn parse_args(it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    for arg in it {
        match arg.as_str() {
            "--version" | "-V" => a.version = true,
            "--help" | "-h" => a.help = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(a)
}

/// The frame host: forwards per-frame logic, drawing and exit to the shell.
struct DesktopApp {
    shell: Shell,
    marker: PathBuf,
    settings: PathBuf,
    /// The projects the shell creates: their servers stop on exit.
    projects: std::sync::Arc<projects::Projects>,
    #[cfg(target_os = "macos")]
    menu: Option<macos_menu::NativeMenu>,
}

impl eframe::App for DesktopApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.menu {
            menu.frame(&mut self.shell);
        }
        self.shell.logic(ctx);
        if self.shell.frames == graphics::HEALTHY_AFTER_FRAMES {
            graphics::healthy(&self.marker);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.ui(ui);
    }

    fn on_exit(&mut self) {
        if let Err(e) = prefs::save(&self.settings, &prefs::capture(&self.shell)) {
            log::error!("the preferences could not be saved: {e}");
        }
        self.projects.shutdown();
    }
}

/// Installs the process-wide panic hook: message and location go to the logger, and nothing on
/// this path can panic or abort, even with a closed standard-error stream.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "unknown location".to_string());
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a panic without a message".to_string());
        log::error!("panic at {location}: {message}");
    }));
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            let _ = writeln_stderr(&format!("archforge: {e}\n{USAGE}"));
            return 2;
        }
    };
    if args.version {
        println!("{}", build_info::version_line());
        return 0;
    }
    if args.help {
        println!("{USAGE}");
        return 0;
    }
    logging::init();
    install_panic_hook();
    let dirs = app_dirs::AppDirs::discover();
    logging::set_dir(&dirs.logs());
    log::info!("{}", build_info::version_line());

    let (stored, warning) = prefs::load(&dirs.settings);
    if let Some(w) = warning {
        log::warn!("{w}");
    }
    let start = Rc::new(RefCell::new(Some(stored)));
    let marker = dirs.gpu_marker();
    let mut last_error = String::new();
    for choice in graphics::plan(&marker) {
        graphics::begin(&marker, choice);
        let mut options = native_options();
        graphics::configure(&mut options, choice);
        let (cell, marker, settings) = (start.clone(), marker.clone(), dirs.settings.clone());
        let result = eframe::run_native(
            "Archforge",
            options,
            Box::new(move |_cc| {
                let stored = cell.borrow_mut().take().ok_or("the application was already started")?;
                let projects = std::sync::Arc::new(projects::Projects::new(&settings, build_info::version_line()));
                let mut shell = Shell::new(services::native(projects.clone()));
                prefs::apply(&stored, &mut shell);
                #[cfg(target_os = "macos")]
                let menu = native_menu(&mut shell);
                Ok(Box::new(DesktopApp {
                    shell,
                    marker,
                    settings,
                    projects,
                    #[cfg(target_os = "macos")]
                    menu,
                }))
            }),
        );
        match result {
            Ok(()) => return 0,
            Err(e) => {
                last_error = e.to_string();
                log::error!("the window could not be opened with the {choice:?} graphics choice: {e}");
                if start.borrow().is_none() {
                    // The application had started; this is not a start-up failure to retry.
                    break;
                }
            }
        }
    }
    log::error!("no graphics choice could open the window: {last_error}");
    1
}

fn native_options() -> eframe::NativeOptions {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Archforge")
        .with_app_id("archforge")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0]);
    #[cfg(target_os = "macos")]
    let viewport = macos_window::style(viewport);
    eframe::NativeOptions { viewport, ..Default::default() }
}

#[cfg(target_os = "macos")]
fn native_menu(shell: &mut Shell) -> Option<macos_menu::NativeMenu> {
    shell.top_inset = macos_window::TITLE_BAR_INSET;
    match macos_menu::build() {
        Ok(menu) => Some(menu),
        Err(e) => {
            log::warn!("the native menu bar could not be built: {e}");
            None
        }
    }
}

fn writeln_stderr(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    writeln!(std::io::stderr(), "{text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn an_unknown_argument_is_a_usage_error() {
        assert!(args(&["--bogus"]).unwrap_err().contains("--bogus"));
        assert!(args(&["model.archforge"]).is_err());
    }

    #[test]
    fn version_and_help_parse() {
        assert!(args(&["--version"]).unwrap().version);
        assert!(args(&["-h"]).unwrap().help);
        let none = args(&[]).unwrap();
        assert!(!none.version && !none.help);
    }

    #[test]
    fn every_preference_is_applied_and_a_damaged_file_is_set_aside() {
        let stored = prefs::Prefs {
            ui: archforge_ui_egui::UiState {
                theme: archforge_ui_egui::theme::ThemeKind::Dark,
                language: "de".to_string(),
                home_banner_closed: true,
                ..archforge_ui_egui::UiState::default()
            },
        };
        let mut shell = Shell::new(archforge_ui_egui::Services::default());
        prefs::apply(&stored, &mut shell);
        assert_eq!(prefs::capture(&shell), stored);
        assert_eq!(archforge_ui_egui::i18n::language(), "de");
        // A language no catalogue is registered under is said and replaced with English.
        let said = prefs::unknown_language("tlh").unwrap();
        assert!(said.contains("tlh") && said.contains("en"), "{said}");
        let mut klingon = stored.clone();
        klingon.ui.language = "tlh".to_string();
        prefs::apply(&klingon, &mut shell);
        assert_eq!((shell.state.language.as_str(), archforge_ui_egui::i18n::language()), ("en", "en"));
        let dir = std::env::temp_dir().join(format!("archforge-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        prefs::save(&dir, &stored).unwrap();
        assert_eq!(prefs::load(&dir), (stored, None));
        std::fs::write(dir.join(prefs::FILE), b"{ damaged").unwrap();
        let (loaded, warning) = prefs::load(&dir);
        assert_eq!(loaded, prefs::Prefs::default());
        assert!(warning.unwrap().contains("set aside"));
        assert!(std::fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains("damaged")));
        std::fs::write(dir.join(prefs::FILE), vec![b' '; usize::try_from(prefs::MAX_BYTES).unwrap() + 1]).unwrap();
        let (loaded, warning) = prefs::load(&dir);
        assert_eq!(loaded, prefs::Prefs::default());
        assert!(warning.unwrap().contains("bytes long"));
        // A file that is not text is set aside too, never overwritten unseen.
        std::fs::write(dir.join(prefs::FILE), [0xff, 0xfe, 0x00]).unwrap();
        let (loaded, warning) = prefs::load(&dir);
        assert_eq!(loaded, prefs::Prefs::default());
        assert!(warning.unwrap().contains("set aside"));
        assert!(!dir.join(prefs::FILE).exists(), "the unreadable file was moved aside");
        std::fs::remove_dir_all(&dir).unwrap();
        archforge_ui_egui::i18n::set_language("en");
    }

    #[test]
    fn the_graphics_plan_skips_a_choice_that_crashed_and_prefers_the_monitors_gpu() {
        let dir = std::env::temp_dir().join(format!("archforge-gpu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("gpu-starting");
        assert_eq!(graphics::plan(&marker), vec![graphics::Choice::Primary, graphics::Choice::Fallback]);
        graphics::begin(&marker, graphics::Choice::Primary);
        assert_eq!(graphics::plan(&marker), vec![graphics::Choice::Fallback]);
        graphics::healthy(&marker);
        assert!(!marker.exists());
        std::fs::remove_dir_all(&dir).unwrap();
        let adapters = [(0x8086, 1, true), (0x10de, 2, true), (0x1002, 3, false)];
        assert_eq!(graphics::choose_adapter(&adapters, &[(0x10de, 2)]), Some(1));
        assert_eq!(graphics::choose_adapter(&adapters, &[(0x1002, 3)]), Some(0), "an adapter that cannot present is never chosen");
        assert_eq!(graphics::choose_adapter(&adapters, &[]), Some(0));
        assert_eq!(graphics::choose_adapter(&[], &[]), None);
    }

    #[test]
    fn the_version_line_says_dev_build_without_metadata() {
        if build_info::COMMIT.is_none() {
            assert!(build_info::version_line().ends_with("(dev build)"));
        }
    }
}
