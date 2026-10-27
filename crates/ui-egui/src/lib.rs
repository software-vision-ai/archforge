//! The user interface: the only library crate that may use the toolkit.
//!
//! `Shell` owns the host's injected `Services`, the serialisable `UiState` and what Home lists.
//! Each frame `logic` takes in the installed fonts and applies the appearance; `ui` draws Home,
//! the start screen. The shell never touches the file system or the operating system: the host
//! supplies `Services`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod fonts;
pub mod home;
pub mod i18n;
#[cfg(feature = "snapshot")]
pub mod offscreen;
pub mod state;
pub mod theme;
pub mod ux;
pub mod widgets;

pub use fonts::{FontEntry, FontSource};
pub use state::UiState;

/// Host functions the shell may call; each is optional, and a missing one degrades rather than
/// failing.
#[derive(Default)]
pub struct Services {
    /// The installed fonts: a language whose script the bundled faces do not draw is drawn in an
    /// installed family that does; without them its text shows as boxes.
    pub fonts: Option<std::sync::Arc<dyn FontSource>>,
    /// The full name of the account the app runs under, for Home's avatar; none hides it.
    pub user_name: Option<String>,
}

pub struct Shell {
    pub services: Services,
    pub state: UiState,
    /// Frames run so far.
    pub frames: u64,
    pub(crate) registry: widgets::Registry,
    /// The installed fonts as this window knows them.
    pub(crate) fonts: fonts::FontState,
    /// The font notices logged so far, oldest first (tests only).
    #[cfg(test)]
    pub(crate) font_notices: Vec<String>,
    applied_theme: Option<theme::ThemeKind>,
    /// The window title last sent to the host, so it is sent again only when it changes.
    last_title: String,
    /// Logical pixels at the top the host draws over (a full-size-content title bar).
    pub top_inset: f32,
    /// What Home lists: the pinned folders and the recent projects.
    pub home: home::Data,
    /// Set by `request_quit`; the next frame closes the window.
    quit_requested: bool,
}

impl Shell {
    pub fn new(services: Services) -> Self {
        Self {
            services,
            state: UiState::default(),
            frames: 0,
            registry: widgets::Registry::default(),
            fonts: fonts::FontState::default(),
            #[cfg(test)]
            font_notices: Vec::new(),
            applied_theme: None,
            last_title: String::new(),
            top_inset: 0.0,
            home: home::Data::default(),
            quit_requested: false,
        }
    }

    /// Asks the window to quit through the normal close path at the next frame.
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// The widgets registered in the most recent frame, with their automation identifiers.
    pub fn widgets(&self) -> &[widgets::Entry] {
        self.registry.entries()
    }

    /// Once per frame, before `ui`: the language, the fonts, the appearance and the exit.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.frames = self.frames.wrapping_add(1);
        // The stored language, once registered, is the one drawn.
        if crate::i18n::language() != self.state.language && !crate::i18n::set_language(&self.state.language) {
            log::warn!("{:?} is not a registered language; {} is used", self.state.language, crate::i18n::language());
            self.state.language = crate::i18n::language().to_string();
        }
        self.fonts.frame(self.services.fonts.as_ref(), ctx);
        // Started in a language whose script no face draws: said once in the log.
        if let Some(code) = self.fonts.take_notice() {
            let notice = fonts::unserved_message(code);
            log::warn!("{notice}");
            #[cfg(test)]
            self.font_notices.push(notice);
        }
        if self.applied_theme != Some(self.state.theme) {
            theme::apply(ctx, self.state.theme);
            self.applied_theme = Some(self.state.theme);
        }
        if std::mem::take(&mut self.quit_requested) {
            // Quitting goes through the normal close path, so the exit hook runs.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The window title: the application's name.
    pub fn title(&self) -> String {
        "Archforge".to_string()
    }

    /// Draws the window: Home, the start screen.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.registry.begin_frame();
        // The OS window title follows the in-app title; on a host with a native title bar it is
        // the only title the user sees.
        let title = self.title();
        if self.last_title != title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        home::show(self, ui);
    }
}

#[cfg(test)]
mod tests;
