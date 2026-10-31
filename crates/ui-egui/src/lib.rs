//! The user interface: the only library crate that may use the toolkit.
//!
//! `Shell` owns the host's injected `Services`, the serialisable `UiState` and what Home lists.
//! Each frame `logic` takes in the installed fonts and applies the appearance; `ui` draws Home,
//! the start screen, and the New project dialog over it while it is open. The shell never touches the file system or the operating system: the host
//! supplies `Services`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod canvas;
pub mod checks;
pub mod connect;
pub mod diagrams;
pub mod editing;
pub mod explorer;
pub mod fonts;
pub mod home;
pub mod i18n;
pub mod indexes;
pub mod keys;
pub mod new_project;
pub mod objects;
#[cfg(feature = "snapshot")]
pub mod offscreen;
pub mod panels;
pub mod problems;
pub mod relation;
pub mod state;
pub mod subtypes;
pub mod theme;
pub mod types;
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
    /// Asks the user for a project to open (the native folder picker); none, or none chosen,
    /// opens nothing.
    pub pick_project: Option<std::sync::Arc<dyn Fn() -> Option<std::path::PathBuf> + Send + Sync>>,
}

/// How writing the open project's model stands: rows waiting to be written, and why the last
/// write failed, if it did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Writing {
    pub pending: bool,
    pub error: Option<String>,
}

/// The host's part in making projects. The shell never touches a project's files or its database:
/// it asks, and draws the answer.
pub trait ProjectHost: Send + Sync {
    /// Starts creating the project `name` in `folder` and returns at once; the host works on its
    /// own thread and sends one answer. The project created is open afterwards.
    fn create(&self, folder: &std::path::Path, name: &str) -> std::sync::mpsc::Receiver<Result<Opened, CreateError>>;

    /// Starts opening the project at `path` and returns at once, with its model's rows in the
    /// answer; the project opened replaces the open one. Without it, opening is not available.
    fn open(&self, _path: &std::path::Path) -> std::sync::mpsc::Receiver<Result<Opened, CreateError>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err(CreateError::Failed { detail: "this host opens no projects".into() }));
        rx
    }

    /// Writes the model of the project at `project` as these rows a moment from now; newer rows
    /// replace ones not yet written (owner decision archforge-gm2.17: every edit reaches the
    /// project). Rows for a project that is no longer open are dropped.
    fn write(&self, _project: &std::path::Path, _rows: archforge_mof::rows::Rows) {}

    /// Save: writes what waits, then commits it in the project at `project`; true when there was
    /// something to commit. A save for a project that is no longer open fails.
    fn save(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<bool, String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err("this host saves no projects".into()));
        rx
    }

    /// How writing stands now.
    fn writing(&self) -> Writing {
        Writing::default()
    }
}

/// An open project as the shell shows it: its model's rows when it was opened from disk (none for
/// a new one), and whether it held changes not yet saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opened {
    pub name: String,
    pub path: std::path::PathBuf,
    pub model: Option<std::sync::Arc<archforge_mof::rows::Rows>>,
    pub unsaved: bool,
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
    /// The model's revision last handed to the host to write.
    written: Option<u64>,
    /// The model's revision that could not be turned into rows, so it is not tried every frame.
    refused: Option<u64>,
    /// A save the host works on, and the state it writes.
    saving: Option<(std::sync::mpsc::Receiver<Result<bool, String>>, u64)>,
    /// An opening the host works on.
    opening: Option<std::sync::mpsc::Receiver<Result<Opened, CreateError>>>,
    /// Why the last save or the last opening failed, for the status bar and Home.
    pub save_error: Option<String>,
    pub open_error: Option<String>,
    /// The Models explorer's own state: folds, filter, a name being typed, the dialogs.
    pub explorer: explorer::State,
    /// The Diagrams explorer's own state: folds, filter, the New diagram dialog.
    pub diagrams: diagrams::State,
    /// The centre's own state: the open tabs, the view, the tool and the toggles.
    pub canvas: canvas::State,
    /// The right panels' own state: the open editors and the last refusal.
    pub panels: panels::State,
    /// The problems the rules find, by element, read once per frame while a session is open.
    pub problems: std::collections::BTreeMap<archforge_mof::Obj, Vec<String>>,
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
            written: None,
            refused: None,
            saving: None,
            opening: None,
            save_error: None,
            open_error: None,
            explorer: explorer::State::default(),
            diagrams: diagrams::State::default(),
            canvas: canvas::State::default(),
            panels: panels::State::default(),
            problems: Default::default(),
            quit_requested: false,
        }
    }

    /// Opens a project in the window: the workspace replaces Home at the next frame.
    pub fn open_project(&mut self, opened: Opened) {
        self.project = Some(opened);
        self.session = None;
        self.session_error = None;
        self.adopted = None;
        self.written = None;
        self.refused = None;
        // A save of the project open before answers for that one, never for this.
        self.saving = None;
        self.save_error = None;
        self.open_error = None;
    }

    /// Closes the open project: Home comes back at the next frame.
    pub fn close_project(&mut self) {
        self.project = None;
        self.session = None;
        self.session_error = None;
        self.adopted = None;
        self.written = None;
        self.refused = None;
        self.saving = None;
        self.save_error = None;
        self.explorer = explorer::State::default();
        self.diagrams = diagrams::State::default();
        self.canvas = canvas::State::default();
        self.panels = panels::State::default();
        self.problems.clear();
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
        // A project opened from disk is its rows read back; a new one starts with its Model root,
        // and so does one whose tables hold no row at all (made before projects were saved, or
        // closed before its first change was written): it never held a model.
        let rows = project.model.as_deref().filter(|rows| !rows.is_empty());
        let built = match &self.services.ids {
            Some(source) => {
                let source = source.clone();
                let ids = Box::new(move || source());
                match rows {
                    Some(rows) => archforge_er::session_from_rows(rows, ids).map(|mut s| {
                        if project.unsaved {
                            s.mark_unsaved();
                        }
                        s
                    }),
                    None => archforge_er::new_project_session(&project.name, ids),
                }
                .map_err(SessionError::Engine)
            }
            None => Err(SessionError::NoIdSource),
        };
        let read = rows.is_some();
        // The rows read are not kept beside the model built from them.
        if let Some(p) = self.project.as_mut() {
            p.model = None;
        }
        match built {
            Ok(s) => {
                // What was read is what the project holds: nothing to write yet.
                if read {
                    self.written = Some(s.revision());
                }
                self.session = Some(s);
            }
            Err(e) => {
                log::error!("the project's model could not be opened: {e:?}");
                self.session_error = Some(e);
            }
        }
    }

    /// Hands the model to the host when it changed since the last hand-over; the host writes it a
    /// moment later. Nothing is handed over in the middle of a drag.
    fn write_changes(&mut self) {
        let (Some(session), Some(host), Some(project)) = (&self.session, &self.services.projects, &self.project) else { return };
        let revision = session.revision();
        if self.written == Some(revision) || self.refused == Some(revision) || session.interacting() {
            return;
        }
        let (tables, junctions) = archforge_er::storage();
        match archforge_mof::rows::snapshot(session.doc(), &tables, &junctions) {
            Ok(rows) => {
                host.write(&project.path, rows);
                self.written = Some(revision);
                // The model could not be turned into rows before; now it could.
                if self.refused.take().is_some() {
                    self.save_error = None;
                }
                // A failed opening is old news once the open project changes.
                self.open_error = None;
            }
            Err(e) => {
                log::error!("the model could not be turned into rows to save: {e}");
                self.save_error = Some(e.to_string());
                self.refused = Some(revision);
            }
        }
    }

    /// Save: the host writes what waits and commits it.
    pub fn save(&mut self) {
        if self.saving.is_some() || self.session.is_none() {
            return;
        }
        self.write_changes();
        // A state that could not be written cannot be saved: its error stays on the status bar.
        let current = self.session.as_ref().map(|s| s.revision());
        if current.is_some() && self.refused == current {
            return;
        }
        let Some(revision) = self.written else { return };
        if let (Some(host), Some(project)) = (&self.services.projects, &self.project) {
            self.saving = Some((host.save(&project.path), revision));
            self.save_error = None;
            self.open_error = None;
        }
    }

    /// Open project: asks the user for a project, then the host opens it.
    pub fn open_from_picker(&mut self) {
        // One opening at a time, and none behind the New project dialog.
        if !self.can_open_project() {
            return;
        }
        let Some(pick) = self.services.pick_project.clone() else { return };
        if let Some(path) = pick() {
            // The open model's newest state goes to the host first, so it is written before the
            // project is replaced.
            self.write_changes();
            let Some(host) = &self.services.projects else { return };
            self.opening = Some(host.open(&path));
            self.open_error = None;
        }
    }

    /// The answers of a save and of an opening the host was working on.
    fn poll_host(&mut self) {
        if let Some((rx, revision)) = &self.saving {
            let revision = *revision;
            match rx.try_recv() {
                Ok(Ok(_)) => {
                    self.saving = None;
                    // What the save wrote is saved; an edit made while it ran is not.
                    if let Some(s) = self.session.as_mut()
                        && let Err(e) = s.mark_saved_at(revision)
                    {
                        log::warn!("the session could not be marked saved: {e}");
                    }
                }
                Ok(Err(e)) => {
                    self.saving = None;
                    self.save_error = Some(e);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.saving = None;
                    self.save_error = Some(crate::i18n::tr("The save stopped without an answer"));
                }
            }
        }
        if let Some(rx) = &self.opening {
            match rx.try_recv() {
                Ok(Ok(opened)) => {
                    self.opening = None;
                    new_project::remember(self, &opened);
                    self.open_project(opened);
                }
                Ok(Err(e)) => {
                    self.opening = None;
                    self.open_error = Some(match e {
                        CreateError::Failed { detail } | CreateError::FolderUnusable { detail } | CreateError::DoltMissing { detail } => detail,
                        other => format!("{other:?}"),
                    });
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.opening = None;
                    self.open_error = Some(crate::i18n::tr("The opening stopped without an answer"));
                }
            }
        }
    }

    /// Whether Save can act now: a model is open and no save is under way.
    pub fn can_save(&self) -> bool {
        self.session.is_some() && self.saving.is_none()
    }

    /// Whether Open can act now: the host opens projects, no opening is under way and the New
    /// project dialog is closed.
    pub fn can_open_project(&self) -> bool {
        home::can_open(self) && self.opening.is_none() && self.new_project.is_none()
    }

    /// Whether an opening is under way.
    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    /// Whether a save is under way.
    pub fn is_saving(&self) -> bool {
        self.saving.is_some()
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
        self.poll_host();
        // While the host works on an opening, a save or a write, the window looks again soon, so
        // its answer shows without waiting for the pointer to move.
        // A write that failed waits for the next change or Save, and the status bar already says
        // why: nothing to look for until then.
        let writing = self.services.projects.as_ref().is_some_and(|h| {
            let w = h.writing();
            w.pending && w.error.is_none()
        });
        if self.opening.is_some() || self.saving.is_some() || writing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // Cmd+S (Ctrl+S) saves the open project and Cmd+O (Ctrl+O) opens one; on macOS the menu
        // bar takes both first.
        if self.session.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_from_picker();
        }
        self.write_changes();
        if let Some(session) = &self.session {
            explorer::retain_valid(&mut self.explorer, session.doc());
            diagrams::retain_valid(&mut self.diagrams, session.doc());
            canvas::retain_valid(&mut self.canvas, session.doc());
            panels::retain_valid(&mut self.panels, session.doc());
            self.problems = problems::by_element(session.doc());
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
