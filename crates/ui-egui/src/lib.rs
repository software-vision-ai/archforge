//! The user interface: the only library crate that may use the toolkit.
//!
//! `Shell` owns the host's injected `Services`, the serialisable `UiState` and what Home lists.
//! Each frame `logic` takes in the installed fonts and applies the appearance; `ui` draws Home,
//! the start screen, and the New project dialog over it while it is open. The shell never touches the file system or the operating system: the host
//! supplies `Services`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod canvas;
pub mod connect;
pub mod diagrams;
pub mod editing;
pub mod explorer;
pub mod fonts;
pub mod home;
pub mod i18n;
pub mod new_project;
#[cfg(feature = "snapshot")]
pub mod offscreen;
pub mod panels;
pub mod state;
pub mod theme;
pub mod ux;
pub mod widgets;
pub mod workspace;

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
    /// Where new projects go unless the user picks another folder; none leaves the choice open.
    pub projects_folder: Option<Folder>,
    /// Creates projects; none keeps Create project unavailable.
    pub projects: Option<std::sync::Arc<dyn ProjectHost>>,
    /// Mints ids for the elements of an open project's model (version 7, from the host's clock
    /// and randomness); none keeps a project's model from opening, and the status bar says so.
    pub ids: Option<std::sync::Arc<dyn Fn() -> archforge_mof::Uuid + Send + Sync>>,
}

/// The host's part in making projects. The shell never touches a project's files or its database:
/// it asks, and draws the answer.
pub trait ProjectHost: Send + Sync {
    /// Starts creating the project `name` in `folder` and returns at once; the host works on its
    /// own thread and sends one answer. The project created is open afterwards.
    fn create(&self, folder: &std::path::Path, name: &str) -> std::sync::mpsc::Receiver<Result<Opened, CreateError>>;
}

/// An open project as the shell shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub name: String,
    pub path: std::path::PathBuf,
}

/// Why a project could not be created, in the kinds the dialog explains; a detail is the host's
/// own technical text, shown small and untranslated under the translated sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateError {
    BadName,
    NameTaken,
    /// A creation that did not finish left a folder of that name.
    Unfinished,
    FolderUnusable {
        detail: String,
    },
    DoltMissing {
        detail: String,
    },
    Failed {
        detail: String,
    },
}

/// Why an open project's model could not be made: the host gave no id source, or the engine
/// refused with its own words, which the status bar shows small and untranslated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    NoIdSource,
    Engine(String),
}

/// A folder as the shell shows it: where it is, and how the interface names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub path: std::path::PathBuf,
    /// Its name for people, such as `Documents / Archforge`.
    pub label: String,
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
    /// The New project dialog, while it is open over Home.
    pub new_project: Option<new_project::Dialog>,
    /// The open project, once one is created.
    pub project: Option<Opened>,
    /// The open project's model: the ER session over a store with the project's Model root, made
    /// once when the project opens and dropped when it closes. Nothing reaches the project's
    /// database yet; the status bar says so.
    pub session: Option<archforge_er::Session>,
    /// Why the session could not be made, when it could not.
    pub session_error: Option<SessionError>,
    /// The project the session was made for, so it is made once.
    adopted: Option<std::path::PathBuf>,
    /// The Models explorer's own state: folds, filter, a name being typed, the dialogs.
    pub explorer: explorer::State,
    /// The Diagrams explorer's own state: folds, filter, the New diagram dialog.
    pub diagrams: diagrams::State,
    /// The centre's own state: the open tabs, the view, the tool and the toggles.
    pub canvas: canvas::State,
    /// The right panels' own state: the open editors and the last refusal.
    pub panels: panels::State,
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
            new_project: None,
            project: None,
            session: None,
            session_error: None,
            adopted: None,
            explorer: explorer::State::default(),
            diagrams: diagrams::State::default(),
            canvas: canvas::State::default(),
            panels: panels::State::default(),
            quit_requested: false,
        }
    }

    /// Opens a project in the window: the workspace replaces Home at the next frame.
    pub fn open_project(&mut self, opened: Opened) {
        self.project = Some(opened);
        self.session = None;
        self.session_error = None;
        self.adopted = None;
    }

    /// Closes the open project: Home comes back at the next frame.
    pub fn close_project(&mut self) {
        self.project = None;
        self.session = None;
        self.session_error = None;
        self.adopted = None;
        self.explorer = explorer::State::default();
        self.diagrams = diagrams::State::default();
        self.canvas = canvas::State::default();
        self.panels = panels::State::default();
    }

    /// Where the open project lives, as the status bar says it: the label of the folder new
    /// projects go in when the project is inside it, else its parent folder's name.
    pub fn location(&self) -> Option<String> {
        let path = &self.project.as_ref()?.path;
        if let Some(folder) = &self.services.projects_folder
            && path.starts_with(&folder.path)
        {
            return Some(folder.label.clone());
        }
        path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned())
    }

    /// The session of the open project, made once per project through the ER crate, with the
    /// host's ids. Without an id source, or when the engine refuses, the reason is kept for the
    /// status bar and said once in the log.
    pub(crate) fn adopt_project(&mut self) {
        let Some(project) = &self.project else { return };
        if self.adopted.as_ref() == Some(&project.path) {
            return;
        }
        self.adopted = Some(project.path.clone());
        let built = match &self.services.ids {
            Some(source) => {
                let source = source.clone();
                archforge_er::new_project_session(&project.name, Box::new(move || source())).map_err(SessionError::Engine)
            }
            None => Err(SessionError::NoIdSource),
        };
        match built {
            Ok(s) => self.session = Some(s),
            Err(e) => {
                log::error!("the project's model could not be opened: {e:?}");
                self.session_error = Some(e);
            }
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
        self.adopt_project();
        if let Some(session) = &self.session {
            explorer::retain_valid(&mut self.explorer, session.doc());
            diagrams::retain_valid(&mut self.diagrams, session.doc());
            canvas::retain_valid(&mut self.canvas, session.doc());
            panels::retain_valid(&mut self.panels, session.doc());
        }
        if std::mem::take(&mut self.quit_requested) {
            // Quitting goes through the normal close path, so the exit hook runs.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The window title: the open project's name and the application's, or the application's alone.
    pub fn title(&self) -> String {
        match &self.project {
            Some(p) => crate::i18n::fmt(&crate::i18n::tr("{name} — Archforge"), &[("name", &p.name)]),
            None => "Archforge".to_string(),
        }
    }

    /// Draws the window: the workspace when a project is open, otherwise Home, and the New project
    /// dialog over Home while it is open.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.registry.begin_frame();
        // The OS window title follows the in-app title; on a host with a native title bar it is
        // the only title the user sees.
        let title = self.title();
        if self.last_title != title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if self.project.is_some() {
            workspace::show(self, ui);
        } else {
            home::show(self, ui);
        }
        new_project::show(self, ui);
    }
}

#[cfg(test)]
mod tests;
