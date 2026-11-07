//! The user interface: the only library crate that may use the toolkit.
//!
//! `Shell` owns the host's injected `Services`, the serialisable `UiState` and what Home lists.
//! Each frame `logic` takes in the installed fonts and applies the appearance; `ui` draws Home,
//! the start screen, and the New project dialog over it while it is open. The shell never touches the file system or the operating system: the host
//! supplies `Services`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod ai_agents;
pub mod appearance;
pub mod canvas;
pub mod chat;
pub mod checks;
pub mod close_ask;
pub mod connect;
pub mod control;
pub mod diagrams;
pub mod editing;
mod er_kind;
mod er_ui;
pub mod explorer;
pub mod fonts;
pub mod generate;
pub mod home;
pub mod home_search;
pub mod i18n;
pub mod indexes;
pub mod keys;
mod kinds;
pub mod markdown;
pub mod new_project;
mod notation_ui;
pub mod objects;
#[cfg(any(test, feature = "snapshot"))]
pub mod offscreen;
pub(crate) mod package_nav;
pub mod panels;
pub mod problems;
mod project_search;
pub mod relation;
pub mod settings;
mod settings_agents;
pub mod skill_tab;
pub mod state;
pub mod subtypes;
pub mod support;
pub mod theme;
pub mod types;
mod uml_bindings;
mod uml_classifier;
mod uml_constraints;
mod uml_inline;
mod uml_nary;
mod uml_nesting;
mod uml_panel;
mod uml_profiles;
mod uml_qualifiers;
mod uml_relate;
mod uml_sets;
mod uml_stereotypes;
mod uml_templates;
mod uml_ui;
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
    /// Where new projects go unless the user picks another folder; none leaves the choice open.
    pub projects_folder: Option<Folder>,
    /// Creates projects; none keeps Create project unavailable.
    pub projects: Option<std::sync::Arc<dyn ProjectHost>>,
    /// Mints ids for the elements of an open project's model (version 7, from the host's clock
    /// and randomness); none keeps a project's model from opening, and the status bar says so.
    pub ids: Option<std::sync::Arc<dyn Fn() -> archforge_mof::Uuid + Send + Sync>>,
    /// Asks the user for a project to open (the native folder picker), starting in the folder
    /// given or else in the projects folder; none, or none chosen, opens nothing.
    pub pick_project: Option<std::sync::Arc<dyn Fn(Option<&std::path::Path>) -> Option<std::path::PathBuf> + Send + Sync>>,
    /// Finds the projects in the folders given (Home's pinned folders) on the host's own thread,
    /// sends them once and then calls the wake it was given; none leaves Home's search to the
    /// recent projects.
    pub find_projects: Option<std::sync::Arc<FindProjects>>,
    /// Asks the user for a folder (the native folder picker), headed with the title given, which
    /// says what the folder is for (Pin a folder on Home, Choose project location in New project);
    /// none keeps both unavailable.
    pub pick_folder: Option<std::sync::Arc<dyn Fn(&str) -> Option<std::path::PathBuf> + Send + Sync>>,
    /// Why the native folder picker cannot appear, when the platform can tell (Linux without a
    /// desktop portal answers nothing, as a cancel does); asked when a pick comes back empty.
    pub picker_unavailable: Option<std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    /// Where About Archforge sends support: the provider pages the host has set up; a
    /// destination left out says it is not set up yet.
    pub support: support::Destinations,
    /// AI Agents and Chat: finding the agent programs, keeping their API keys in the system's secure
    /// store, the test message, Codex's ChatGPT sign-in, and Chat's conversations with their agent
    /// access; none leaves the page saying it is not available and runs no conversation.
    pub agents: Option<std::sync::Arc<dyn ai_agents::AgentHost>>,
}

/// The host's search for projects in folders: the folders and a wake in, the projects found sent
/// once.
pub type FindProjects = dyn Fn(Vec<std::path::PathBuf>, Box<dyn Fn() + Send>) -> std::sync::mpsc::Receiver<Vec<FoundProject>> + Send + Sync;

/// A project the host found in one of the folders it was given: where it is (made canonical where
/// the host could), which of the folders holds it, and the folders between that one and it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundProject {
    pub path: std::path::PathBuf,
    pub folder: usize,
    pub within: Vec<String>,
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
    /// answer; the project opened shows in a tab of its own. Without it, opening is not available.
    fn open(&self, _path: &std::path::Path) -> std::sync::mpsc::Receiver<Result<Opened, CreateError>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err(CreateError::Failed { detail: "this host opens no projects".into() }));
        rx
    }

    /// Writes the model of the project at `project` as these rows a moment from now; newer rows
    /// replace ones not yet written (owner decision: every edit reaches the
    /// project). Rows for a project that is no longer open are dropped.
    fn write(&self, _project: &std::path::Path, _rows: archforge_mof::rows::Rows) {}

    /// Save: writes what waits, then commits it in the project at `project`; true when there was
    /// something to commit. A save for a project that is no longer open fails.
    fn save(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<bool, String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Err("this host saves no projects".into()));
        rx
    }

    /// How writing the project at `project` stands now.
    fn writing(&self, _project: &std::path::Path) -> Writing {
        Writing::default()
    }

    /// Starts closing the project at `project` without writing what waits for it, and returns at
    /// once: its changes since the last save are thrown away, the project put back to that save
    /// (Don't save); a project that cannot be reached any more closes all the same. The answer
    /// says why when it stays open.
    fn discard_and_close(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Ok(()));
        rx
    }

    /// Starts closing the project at `project` without writing what waits for it, and returns at
    /// once: what was written stays, unsaved, for the next opening (Close without writing).
    fn close_without_writing(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Ok(()));
        rx
    }

    /// Starts looking for a Dolt Archforge can keep projects in, and returns at once: the answer
    /// is nothing when there is one, else why not, in the host's own words. Without it, Dolt is
    /// taken as there.
    fn find_dolt(&self) -> std::sync::mpsc::Receiver<Result<(), String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Ok(()));
        rx
    }

    /// Starts closing the project at `project` and returns at once: what waits for it is written,
    /// then it closes. The answer says why when it stays open (its changes could not be written).
    fn close(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(Ok(()));
        rx
    }
}

/// The most projects open at once, one tab each (the host runs a database server for each).
pub const MAX_TABS: usize = 8;

/// An open project's tab in the bar: its folder and name, and its window state while another tab
/// is shown.
pub struct Tab {
    pub path: std::path::PathBuf,
    pub name: String,
    parked: Option<Box<Parked>>,
    /// The host's answer to closing it, while it closes; the tab stays until it has closed, and
    /// cannot be shown meanwhile, so nothing is changed in a project that is closing.
    closing: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Why its project could not be closed, until its next change is handed over.
    pub close_error: Option<String>,
}

impl Tab {
    /// Whether its project is closing.
    pub fn closing(&self) -> bool {
        self.closing.is_some()
    }
}

/// A save the host works on: its answer, the model's revision it writes, and its ticket.
type Saving = (std::sync::mpsc::Receiver<Result<bool, String>>, u64, u64);

/// How many finished saves the shell remembers for the agent asking how its save went.
const SAVES_KEPT: usize = 16;

/// How a save asked for by its ticket stands.
#[derive(Clone, Debug, PartialEq)]
pub enum SaveState {
    Running,
    /// Done: the model's revision it wrote.
    Saved(u64),
    Failed(String),
    /// Its project was closed before the host answered, so whether it was saved is not known.
    Unknown,
    /// No such save is remembered.
    Forgotten,
}

/// Looks whether the host has answered `saving`; when it has, marks what it wrote saved (or keeps
/// why not) and gives the save's ticket and outcome.
fn poll_save(
    saving: &mut Option<Saving>,
    session: Option<&mut archforge_notations::Session>,
    save_error: &mut Option<String>,
) -> Option<(u64, SaveState)> {
    let (rx, revision, ticket) = saving.as_ref()?;
    let (revision, ticket) = (*revision, *ticket);
    let outcome = match rx.try_recv() {
        Ok(Ok(_)) => {
            // What the save wrote is saved; an edit made while it ran is not.
            if let Some(s) = session
                && let Err(e) = s.mark_saved_at(revision)
            {
                log::warn!("the session could not be marked saved: {e}");
            }
            SaveState::Saved(revision)
        }
        Ok(Err(e)) => {
            *save_error = Some(e.clone());
            SaveState::Failed(e)
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => return None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            let e = crate::i18n::tr("The save stopped without an answer");
            *save_error = Some(e.clone());
            SaveState::Failed(e)
        }
    };
    *saving = None;
    Some((ticket, outcome))
}

/// Everything the window keeps for a project whose tab is not the one shown.
struct Parked {
    project: Opened,
    session: Option<archforge_notations::Session>,
    session_error: Option<SessionError>,
    adopted: Option<std::path::PathBuf>,
    written: Option<u64>,
    refused: Option<u64>,
    saving: Option<Saving>,
    save_error: Option<String>,
    explorer: explorer::State,
    diagrams: diagrams::State,
    canvas: canvas::State,
    panels: panels::State,
    problems: std::collections::BTreeMap<archforge_mof::Obj, Vec<String>>,
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
    /// The diagram kinds whose tools and gestures the canvas offers.
    pub(crate) kinds: &'static [&'static dyn kinds::DiagramKind],
    /// The notations whose elements the explorers, the search, the problems and Properties show.
    pub(crate) notation_uis: &'static [&'static dyn notation_ui::NotationUi],
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
    /// Home's search field and its list.
    pub home_search: home_search::Search,
    /// The New project dialog, while it is open over Home.
    pub new_project: Option<new_project::Dialog>,
    /// The Settings dialog while it is open.
    pub settings: Option<settings::Dialog>,
    /// About Archforge, with how to support it, while it is open.
    pub about: Option<support::Dialog>,
    /// AI Agents as this run knows it: the programs and keys found, the sign-in, what is under way.
    pub agents: ai_agents::State,
    /// Chat's conversations as this window knows them, and whether one holds agent access.
    pub chats: chat::State,
    /// The open projects, one tab each, in the bar's order; the one shown has its state in the
    /// fields below, the others keep theirs parked in their tab.
    pub tabs: Vec<Tab>,
    /// The tab shown; none while Home is.
    pub active: Option<usize>,
    /// The project shown, when a tab is.
    pub project: Option<Opened>,
    /// The open project's model: the ER session over a store with the project's Model root, made
    /// once when the project opens and dropped when it closes. Nothing reaches the project's
    /// database yet; the status bar says so.
    pub session: Option<archforge_notations::Session>,
    /// Why the session could not be made, when it could not.
    pub session_error: Option<SessionError>,
    /// The project the session was made for, so it is made once.
    adopted: Option<std::path::PathBuf>,
    /// The model's revision last handed to the host to write.
    written: Option<u64>,
    /// The model's revision that could not be turned into rows, so it is not tried every frame.
    refused: Option<u64>,
    /// A save the host works on, and the state it writes.
    saving: Option<Saving>,
    /// The last save's ticket, and how the latest finished ones went (the revision written, or
    /// why not), whichever project they saved.
    save_ticket: u64,
    saves: std::collections::VecDeque<(u64, SaveState)>,
    /// An opening the host works on.
    opening: Option<std::sync::mpsc::Receiver<Result<Opened, CreateError>>>,
    /// The look for Dolt the host works on: started at the first frame and by Home's Look again.
    dolt_check: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    dolt_asked: bool,
    /// Why there is no Dolt Archforge can use, in the host's words, while there is none: Home
    /// says so, with the way to install it.
    pub dolt_missing: Option<String>,
    /// Closing and quitting with unsaved changes: the question open, the projects quitting
    /// still asks about, the save a closing waits for (its project and ticket), and whether
    /// quitting was answered for every project, so the window may close.
    pub ask: Option<close_ask::Ask>,
    /// File → Generate…'s handoff to an agent, while it is open.
    pub generate: Option<generate::Generate>,
    quitting: Option<Vec<std::path::PathBuf>>,
    close_after_save: Vec<(std::path::PathBuf, u64)>,
    quit_confirmed: bool,
    /// Why the last save or the last opening failed, for the status bar and Home.
    pub save_error: Option<String>,
    pub open_error: Option<String>,
    /// Why the last folder pick showed no picker, for Home and the status bar; gone with
    /// the next pick that shows one.
    pub picker_error: Option<String>,
    /// The Models explorer's own state: folds, filter, a name being typed, the dialogs.
    pub explorer: explorer::State,
    /// The Diagrams explorer's own state: folds, filter, the New diagram dialog.
    pub diagrams: diagrams::State,
    /// The centre's own state: the open tabs, the view, the tool and the toggles.
    pub canvas: canvas::State,
    /// The right panels' own state: the open editors and the last refusal.
    pub panels: panels::State,
    /// The rendered Markdown pages' cache, for a skill's Preview.
    pub(crate) markdown: egui_commonmark::CommonMarkCache,
    /// The problems the rules find, by element, read once per frame while a session is open.
    pub problems: std::collections::BTreeMap<archforge_mof::Obj, Vec<String>>,
    /// Set by `request_quit`; the next frame closes the window.
    quit_requested: bool,
    /// The control channel's requests, from every opening of agent access.
    pub(crate) control: Option<std::sync::mpsc::Receiver<control::ControlRequest>>,
}

impl Shell {
    pub fn new(services: Services) -> Self {
        Self {
            services,
            state: UiState::default(),
            frames: 0,
            registry: widgets::Registry::default(),
            kinds: kinds::KINDS,
            notation_uis: notation_ui::UIS,
            fonts: fonts::FontState::default(),
            #[cfg(test)]
            font_notices: Vec::new(),
            applied_theme: None,
            last_title: String::new(),
            top_inset: 0.0,
            home_search: home_search::Search::default(),
            new_project: None,
            settings: None,
            about: None,
            agents: ai_agents::State::default(),
            chats: chat::State::default(),
            tabs: Vec::new(),
            active: None,
            project: None,
            session: None,
            session_error: None,
            adopted: None,
            written: None,
            refused: None,
            saving: None,
            save_ticket: 0,
            saves: std::collections::VecDeque::new(),
            opening: None,
            dolt_check: None,
            dolt_asked: false,
            dolt_missing: None,
            ask: None,
            generate: None,
            quitting: None,
            close_after_save: Vec::new(),
            quit_confirmed: false,
            save_error: None,
            open_error: None,
            picker_error: None,
            explorer: explorer::State::default(),
            diagrams: diagrams::State::default(),
            canvas: canvas::State::default(),
            panels: panels::State::default(),
            markdown: Default::default(),
            problems: Default::default(),
            quit_requested: false,
            control: None,
        }
    }

    /// Opens a project in a new tab and shows it: the workspace replaces Home at the next frame. A
    /// project already open in a tab is shown in it instead.
    pub fn open_project(&mut self, opened: Opened) {
        self.open_error = None;
        // A project made or opened is kept in Dolt: Home no longer asks to install it.
        self.dolt_missing = None;
        // Home's recent list puts it first.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.state.remember(&opened.name, &opened.path, now);
        if let Some(i) = self.tabs.iter().position(|t| t.path == opened.path) {
            self.activate(i);
            return;
        }
        self.park();
        self.tabs.push(Tab { path: opened.path.clone(), name: opened.name.clone(), parked: None, closing: None, close_error: None });
        self.active = Some(self.tabs.len() - 1);
        self.clear_workspace();
        self.project = Some(opened);
    }

    /// Shows Home; the open projects stay open in their tabs.
    pub fn go_home(&mut self) {
        self.park();
    }

    /// Shows the project of tab `i`, parking the one shown.
    pub fn activate(&mut self, i: usize) {
        if self.active == Some(i) || i >= self.tabs.len() || self.tabs[i].closing.is_some() {
            return;
        }
        self.park();
        if let Some(parked) = self.tabs[i].parked.take() {
            let p = *parked;
            self.project = Some(p.project);
            self.session = p.session;
            self.session_error = p.session_error;
            self.adopted = p.adopted;
            self.written = p.written;
            self.refused = p.refused;
            self.saving = p.saving;
            self.save_error = p.save_error;
            self.explorer = p.explorer;
            self.diagrams = p.diagrams;
            self.canvas = p.canvas;
            self.panels = p.panels;
            self.problems = p.problems;
            self.active = Some(i);
        }
    }

    /// Closes tab `i`'s project, asking first when it holds unsaved changes (Save, Don't save,
    /// Cancel) or when its changes could not be written (Close without writing, confirmed). A
    /// description being typed is written first; one refused counts as unsaved.
    pub fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() || self.tabs[i].closing.is_some() {
            return;
        }
        if self.active == Some(i) {
            panels::commit_description(self);
        }
        let path = self.tabs[i].path.clone();
        if let Some(why) = self.tabs[i].close_error.clone() {
            self.ask = Some(close_ask::Ask::CannotWrite { path, why });
        } else if self.tab_unsaved(i) {
            self.ask = Some(close_ask::Ask::Unsaved { path });
        } else {
            self.close_tab_now(i);
        }
    }

    /// Whether tab `i`'s project holds changes not yet saved, a description typed but not yet
    /// written, or refused, among them (AR-186).
    pub fn tab_unsaved(&self, i: usize) -> bool {
        if self.active == Some(i) {
            return self.session.as_ref().is_some_and(|s| s.is_dirty()) || panels::description_pending(&self.panels);
        }
        self.tabs
            .get(i)
            .and_then(|t| t.parked.as_ref())
            .is_some_and(|p| p.session.as_ref().is_some_and(|s| s.is_dirty()) || panels::description_pending(&p.panels))
    }

    /// The tab of the project at `path`.
    pub(crate) fn tab_of(&self, path: &std::path::Path) -> Option<usize> {
        self.tabs.iter().position(|t| t.path == path)
    }

    /// Save, then close: the tab's project shows, its save starts, and the tab closes when the
    /// save is done; a project with nothing to write closes at once.
    pub(crate) fn save_and_close(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        if self.active != Some(i) {
            self.activate(i);
        }
        let path = self.tabs[i].path.clone();
        // A save already under way is waited for; what it did not take is asked about again.
        if let Some((_, _, running)) = &self.saving {
            self.close_after_save.push((path, *running));
            return;
        }
        let before = self.save_ticket;
        self.save();
        if self.saving.is_some() && self.save_ticket != before {
            self.close_after_save.push((path, self.save_ticket));
        } else if let Some(why) = self.save_error.clone() {
            // The save could not start: quitting stops, and the question says why.
            self.quitting = None;
            self.ask = Some(close_ask::Ask::CannotWrite { path, why });
        } else {
            self.close_tab_now(i);
        }
    }

    /// Don't save (`reset`: put back to its last save) or Close without writing (what was written
    /// stays, unsaved): tab `i`'s project closes without its last changes being written.
    pub(crate) fn discard_tab(&mut self, i: usize, reset: bool) {
        if i >= self.tabs.len() || self.tabs[i].closing.is_some() {
            return;
        }
        self.tabs[i].close_error = None;
        let path = &self.tabs[i].path;
        let Some(rx) =
            self.services.projects.as_ref().map(|host| if reset { host.discard_and_close(path) } else { host.close_without_writing(path) })
        else {
            self.closed(i);
            return;
        };
        self.tabs[i].closing = Some(rx);
        if self.active == Some(i) {
            // Nothing is handed over: what the project held since its last save goes.
            self.stash();
            if let Some(next) = self.neighbour(i) {
                self.activate(next);
            }
        }
    }

    /// Quitting: asks about each project with unsaved changes in turn, then lets the window close.
    /// The description being typed is written first; one refused counts as unsaved.
    fn start_quit(&mut self, ctx: &egui::Context) {
        if self.active.is_some() {
            panels::commit_description(self);
        }
        let unsaved: Vec<std::path::PathBuf> = (0..self.tabs.len()).filter(|i| self.tab_unsaved(*i)).map(|i| self.tabs[i].path.clone()).collect();
        if unsaved.is_empty() {
            self.quit_confirmed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.quitting = Some(unsaved);
        }
    }

    /// The next step of quitting, once the last question is answered and its closing or save is
    /// done: the next project to ask about, or the window closing.
    fn continue_quit(&mut self, ctx: &egui::Context) {
        if self.ask.is_some() || !self.close_after_save.is_empty() || self.tabs.iter().any(|t| t.closing.is_some()) {
            return;
        }
        let Some(mut queue) = self.quitting.take() else { return };
        queue.retain(|p| self.tab_of(p).is_some_and(|i| self.tab_unsaved(i)));
        match queue.first().cloned() {
            Some(path) => {
                if let Some(i) = self.tab_of(&path)
                    && self.active != Some(i)
                {
                    self.activate(i);
                }
                self.ask = Some(close_ask::Ask::Unsaved { path });
                self.quitting = Some(queue);
            }
            None => {
                self.quit_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Whether quitting was answered for every unsaved project, so the window closes.
    pub fn quit_confirmed(&self) -> bool {
        self.quit_confirmed
    }

    /// Whether quitting is under way, asking about unsaved projects.
    pub fn quitting(&self) -> bool {
        self.quitting.is_some()
    }

    /// Stops quitting: Cancel on its question, or a save that failed.
    pub(crate) fn cancel_quit(&mut self) {
        self.quitting = None;
    }

    /// Closes tab `i`'s project without asking: its last changes are handed over and written, then
    /// it closes.
    fn close_tab_now(&mut self, i: usize) {
        if i >= self.tabs.len() || self.tabs[i].closing.is_some() {
            return;
        }
        self.tabs[i].close_error = None;
        let handed = if self.active == Some(i) {
            // The skill text still being typed goes with the last changes, before the host closes;
            // a text the store refused keeps the project open on its tab, which says why.
            skill_tab::sync(self, 0.0, true, None);
            if let Some((skill, _)) = skill_tab::unkept(&self.canvas).first().copied() {
                canvas::open(self, skill);
                self.tabs[i].close_error = Some(skill_tab::unkept_reason());
                return;
            }
            // So does the description being typed; one refused keeps the project open, its
            // refusal shown in Properties and on the tab.
            if panels::commit_description(self) {
                self.tabs[i].close_error = self.panels.notice.clone();
                self.quitting = None;
                return;
            }
            self.write_changes();
            let current = self.session.as_ref().map(|s| s.revision());
            if current.is_some() && self.refused == current { Err(self.save_error.clone().unwrap_or_default()) } else { Ok(()) }
        } else {
            self.hand_over_parked(i)
        };
        if let Err(e) = handed {
            self.tabs[i].close_error = Some(e);
            return;
        }
        let Some(rx) = self.services.projects.as_ref().map(|host| host.close(&self.tabs[i].path)) else {
            self.closed(i);
            return;
        };
        self.tabs[i].closing = Some(rx);
        // While it closes it steps aside: the tab beside it shows, or Home.
        if self.active == Some(i) {
            self.park();
            if let Some(next) = self.neighbour(i) {
                self.activate(next);
            }
        }
    }

    /// The open tab nearest `i`, after it first, that is not closing.
    fn neighbour(&self, i: usize) -> Option<usize> {
        let open = |j: &usize| *j != i && self.tabs.get(*j).is_some_and(|t| t.closing.is_none());
        (i + 1..self.tabs.len()).find(open).or_else(|| (0..i).rev().find(open))
    }

    /// The model of parked tab `i`, handed to the host when it changed since its last hand-over.
    fn hand_over_parked(&mut self, i: usize) -> Result<(), String> {
        let path = self.tabs[i].path.clone();
        if self.tabs[i].parked.as_ref().is_some_and(|p| !skill_tab::unkept(&p.canvas).is_empty()) {
            return Err(skill_tab::unkept_reason());
        }
        let (Some(host), Some(parked)) = (&self.services.projects, self.tabs[i].parked.as_mut()) else { return Ok(()) };
        let Some(session) = &parked.session else { return Ok(()) };
        let revision = session.revision();
        if parked.written == Some(revision) {
            return Ok(());
        }
        let (tables, junctions) = archforge_notations::storage();
        let rows = archforge_mof::rows::snapshot(session.doc(), &tables, &junctions).map_err(|e| e.to_string())?;
        host.write(&path, rows);
        parked.written = Some(revision);
        Ok(())
    }

    /// Tab `i`'s project has closed: the tab goes, and the one beside it shows when it was shown
    /// (only a tab closed without a host is still shown here).
    fn closed(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        if self.active == Some(i) {
            self.clear_workspace();
            self.project = None;
            self.active = None;
            let next = self.neighbour(i);
            self.tabs.remove(i);
            if let Some(next) = next {
                self.activate(if next > i { next - 1 } else { next });
            }
        } else {
            let tab = self.tabs.remove(i);
            if let Some((_, _, ticket)) = tab.parked.and_then(|p| p.saving) {
                self.finished_save(ticket, SaveState::Unknown);
            }
            if let Some(a) = self.active
                && a > i
            {
                self.active = Some(a - 1);
            }
        }
    }

    /// Closes the project shown, as its tab's close button does.
    pub fn close_project(&mut self) {
        if let Some(i) = self.active {
            self.close_tab(i);
        }
    }

    /// Parks the project shown in its tab, its last changes handed to the host first, the
    /// description being typed among them; Home shows.
    fn park(&mut self) {
        if self.active.is_none() {
            return;
        }
        skill_tab::sync(self, 0.0, true, None);
        panels::commit_description(self);
        self.write_changes();
        self.stash();
    }

    /// Parks the project shown in its tab as it is, handing nothing over; Home shows.
    fn stash(&mut self) {
        let Some(i) = self.active else { return };
        let Some(project) = self.project.take() else { return };
        // A drawing under way never outlives the project it was on (AR-128).
        self.canvas.nary = None;
        self.canvas.nary_menu = None;
        let parked = Parked {
            project,
            session: self.session.take(),
            session_error: self.session_error.take(),
            adopted: self.adopted.take(),
            written: self.written.take(),
            refused: self.refused.take(),
            saving: self.saving.take(),
            save_error: self.save_error.take(),
            explorer: std::mem::take(&mut self.explorer),
            diagrams: std::mem::take(&mut self.diagrams),
            canvas: std::mem::take(&mut self.canvas),
            panels: std::mem::take(&mut self.panels),
            problems: std::mem::take(&mut self.problems),
        };
        if let Some(tab) = self.tabs.get_mut(i) {
            tab.parked = Some(Box::new(parked));
        }
        self.active = None;
    }

    /// Empties the fields of the project shown.
    fn clear_workspace(&mut self) {
        self.session = None;
        self.session_error = None;
        self.adopted = None;
        self.written = None;
        self.refused = None;
        // A save of another project answers for that one, never for this.
        if let Some((_, _, ticket)) = self.saving.take() {
            self.finished_save(ticket, SaveState::Unknown);
        }
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
        self.location_of(&self.project.as_ref()?.path)
    }

    /// Where a project lives, as the status bar and Home's recent list say it.
    pub fn location_of(&self, path: &std::path::Path) -> Option<String> {
        if let Some(folder) = &self.services.projects_folder
            && path.starts_with(&folder.path)
        {
            return Some(folder.label.clone());
        }
        path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned())
    }

    /// The session of the open project, made once per project from its notations, with the
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
                    Some(rows) => archforge_notations::session_from_rows_repaired(rows, ids, editing::fit_tables).map(|mut s| {
                        if project.unsaved {
                            s.mark_unsaved();
                        }
                        s
                    }),
                    None => archforge_notations::new_project_session(&project.name, ids),
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
            Ok(mut s) => {
                // What was read is what the project holds: nothing to write yet.
                if read {
                    self.written = Some(s.revision());
                }
                // A project from before the model's root package: its top-level packages go into
                // the root, named after the project, as one step that is written and can be undone
                // (owner, 2026-11-04).
                let store = s.doc();
                if !archforge_uml::commands::packages(store).is_empty()
                    && archforge_uml::commands::root_package(store).is_none()
                    && let Err(e) = s.execute("uml.model.root", &serde_json::json!({}))
                {
                    log::warn!("the model's packages could not be put under its root package: {e}");
                }
                self.session = Some(s);
                self.resume_view();
            }
            Err(e) => {
                log::error!("the project's model could not be opened: {e:?}");
                self.session_error = Some(e);
            }
        }
    }

    /// Opens the tabs the project was left with, in their order, the one it showed, panned and
    /// zoomed as it was. A tab whose diagram or skill is gone is left out; with none left,
    /// the project opens on no tab, as a new one does. Opening a diagram is a view command, so
    /// nothing is changed and nothing joins the history.
    fn resume_view(&mut self) {
        let (Some(project), Some(session)) = (&self.project, &self.session) else { return };
        let Some(view) = self.state.view_of(&project.path).cloned() else { return };
        let store = session.doc();
        let opens = |id: &archforge_mof::Uuid| {
            store.by_uuid(*id).is_some_and(|o| store.class_of(o).ok() == Some(archforge_diagram::DIAGRAM) || skill_tab::is_skill(store, *id))
        };
        let tabs: Vec<archforge_mof::Uuid> = view.tabs.iter().filter_map(|t| archforge_mof::Uuid::parse(t)).filter(opens).collect();
        let active = view.active.as_deref().and_then(archforge_mof::Uuid::parse).filter(|a| tabs.contains(a)).or_else(|| tabs.first().copied());
        let Some(active) = active else { return };
        canvas::open(self, active);
        self.canvas.tabs = tabs;
        self.canvas.active = Some(active);
        self.canvas.pan = egui::vec2(view.pan[0], view.pan[1]);
        self.canvas.zoom = view.zoom;
        self.canvas.fit_pending = false;
    }

    /// Keeps the view the open project shows as the one to resume it in, once its canvas has
    /// settled (a diagram just opened is fitted first).
    fn keep_view(&mut self) {
        let Some(project) = &self.project else { return };
        if self.session.is_none() || self.adopted.as_ref() != Some(&project.path) || self.canvas.fit_pending {
            return;
        }
        let label = self
            .session
            .as_ref()
            .and_then(|s| self.canvas.active.and_then(|a| s.doc().by_uuid(a)).map(|d| settings::clean(&archforge_core::name_of(s.doc(), d))))
            .unwrap_or_default();
        let view = state::SavedView {
            tabs: self.canvas.tabs.iter().take(state::MAX_VIEW_TABS).map(|t| t.to_string()).collect(),
            active: self.canvas.active.map(|a| a.to_string()),
            pan: [self.canvas.pan.x, self.canvas.pan.y],
            zoom: self.canvas.zoom,
            label,
        };
        let path = project.path.clone();
        self.state.keep_view(&path, view);
    }

    /// Hands the model to the host when it changed since the last hand-over; the host writes it a
    /// moment later. Nothing is handed over in the middle of a drag.
    fn write_changes(&mut self) {
        let (Some(session), Some(host), Some(project)) = (&self.session, &self.services.projects, &self.project) else { return };
        let revision = session.revision();
        if self.written == Some(revision) || self.refused == Some(revision) || session.interacting() {
            return;
        }
        let (tables, junctions) = archforge_notations::storage();
        match archforge_mof::rows::snapshot(session.doc(), &tables, &junctions) {
            Ok(rows) => {
                host.write(&project.path, rows);
                self.written = Some(revision);
                // A change handed over replaces the word on a refused closing.
                if let Some(tab) = self.active.and_then(|i| self.tabs.get_mut(i)) {
                    tab.close_error = None;
                }
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

    /// Save: the host writes what waits and commits it, a skill's text and a description being
    /// typed included. A description whose write is refused stops the save before the host is
    /// asked: it stays in its field with the refusal, and the project stays unsaved.
    pub fn save(&mut self) {
        if self.saving.is_some() || self.session.is_none() {
            return;
        }
        skill_tab::sync(self, 0.0, true, None);
        let refused = panels::commit_description(self);
        self.write_changes();
        if refused {
            return;
        }
        // A state that could not be written cannot be saved: its error stays on the status bar.
        let current = self.session.as_ref().map(|s| s.revision());
        if current.is_some() && self.refused == current {
            return;
        }
        let Some(revision) = self.written else { return };
        if let (Some(host), Some(project)) = (&self.services.projects, &self.project) {
            self.save_ticket += 1;
            self.saving = Some((host.save(&project.path), revision, self.save_ticket));
            self.save_error = None;
            self.open_error = None;
        }
    }

    /// Open project: asks the user for a project, then the host opens it.
    pub fn open_from_picker(&mut self) {
        self.open_from_picker_in(None);
    }

    /// [`Shell::open_from_picker`] with the picker starting in `folder` (a pinned folder).
    pub fn open_from_picker_in(&mut self, folder: Option<&std::path::Path>) {
        // One opening at a time, and none behind the New project dialog.
        if !self.can_open_project() {
            return;
        }
        let Some(pick) = self.services.pick_project.clone() else { return };
        match pick(folder) {
            Some(path) => {
                self.picker_error = None;
                self.open_path(&path);
            }
            None => self.say_why_no_picker(),
        }
    }

    /// Pin a folder: asks the user for a folder and pins it on Home.
    pub fn pin_from_picker(&mut self) {
        let Some(pick) = self.services.pick_folder.clone() else { return };
        match pick(&crate::i18n::tr("Pin a folder")) {
            Some(path) => {
                self.picker_error = None;
                self.state.pin(&path);
            }
            None => self.say_why_no_picker(),
        }
    }

    /// A pick that came back empty: a cancel, or a picker that could not appear, which Home and
    /// the status bar then say.
    fn say_why_no_picker(&mut self) {
        self.picker_error = self.services.picker_unavailable.as_ref().and_then(|probe| probe());
    }

    /// Opens the project at `path` (from the picker or Home's recent list): a project open in a
    /// tab shows there without asking the host; another opens in a tab of its own while one is free.
    pub fn open_path(&mut self, path: &std::path::Path) {
        if let Some(i) = self.tabs.iter().position(|t| t.path == path) {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let name = self.tabs[i].name.clone();
            self.state.remember(&name, path, now);
            self.activate(i);
            return;
        }
        if self.opening.is_some() || self.new_project.is_some() {
            return;
        }
        if self.tabs.len() >= MAX_TABS {
            self.open_error = Some(crate::i18n::tr("Close a project to open another"));
            return;
        }
        // The shown model's newest state goes to the host first, as before every change of tab.
        self.write_changes();
        let Some(host) = &self.services.projects else { return };
        self.opening = Some(host.open(path));
        self.open_error = None;
    }

    /// The answers of a save, of an opening and of closings the host was working on.
    fn poll_host(&mut self) {
        // Closings: a closed tab goes (from the last, so the others keep their places); a refused
        // one stays, and says why.
        for i in (0..self.tabs.len()).rev() {
            let Some(rx) = &self.tabs[i].closing else { continue };
            match rx.try_recv() {
                Ok(Ok(())) => self.closed(i),
                // A project that stays open comes back to the front, saying why.
                Ok(Err(e)) => self.refused_close(i, e),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.refused_close(i, crate::i18n::tr("The closing stopped without an answer"));
                }
            }
        }
        // Saves, the shown project's and those of projects parked in their tabs.
        let mut done: Vec<(u64, SaveState)> = poll_save(&mut self.saving, self.session.as_mut(), &mut self.save_error).into_iter().collect();
        for p in self.tabs.iter_mut().filter_map(|t| t.parked.as_deref_mut()) {
            done.extend(poll_save(&mut p.saving, p.session.as_mut(), &mut p.save_error));
        }
        for (ticket, outcome) in done {
            self.finished_save(ticket, outcome);
        }
        if let Some(rx) = &self.dolt_check {
            match rx.try_recv() {
                Ok(answer) => {
                    self.dolt_missing = answer.err();
                    self.dolt_check = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                // No answer: Home keeps what it said, and Look again asks again.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.dolt_check = None,
            }
        }
        if let Some(rx) = &self.opening {
            match rx.try_recv() {
                Ok(Ok(opened)) => {
                    self.opening = None;
                    self.open_project(opened);
                }
                Ok(Err(e)) => {
                    self.opening = None;
                    // Home says so with the way to install it, as after the first look.
                    if let CreateError::DoltMissing { detail } = &e {
                        self.dolt_missing = Some(detail.clone());
                    }
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

    /// Asks the host to look for Dolt, unless it is looking already; Home shows the answer.
    pub fn look_for_dolt(&mut self) {
        self.dolt_asked = true;
        if self.dolt_check.is_some() {
            return;
        }
        if let Some(host) = &self.services.projects {
            self.dolt_check = Some(host.find_dolt());
        }
    }

    /// Whether Home is looking for Dolt now.
    pub fn looking_for_dolt(&self) -> bool {
        self.dolt_check.is_some()
    }

    /// Whether Save can act now: a model is open and no save is under way.
    pub fn can_save(&self) -> bool {
        self.session.is_some() && self.saving.is_none()
    }

    /// Whether File → Generate… can open: a project is open and the dialog is not.
    pub fn can_generate(&self) -> bool {
        self.session.is_some() && self.generate.is_none() && self.about.is_none()
    }

    /// Opens About Archforge, with how to support its development.
    pub fn open_about(&mut self) {
        support::open(self);
    }

    /// Opens File → Generate…'s handoff to an agent.
    pub fn open_generate(&mut self) {
        generate::open(self);
    }

    /// Whether Open can act now: the host opens projects, no opening is under way, the New
    /// project dialog is closed and a tab is free.
    pub fn can_open_project(&self) -> bool {
        home::can_open(self) && self.opening.is_none() && self.new_project.is_none() && self.about.is_none()
    }

    /// Tab `i`'s project stays open: it shows again, its status bar saying why.
    fn refused_close(&mut self, i: usize, why: String) {
        self.tabs[i].closing = None;
        self.tabs[i].close_error = Some(why);
        self.activate(i);
    }

    /// Why the project shown could not be closed, when it could not.
    pub fn close_error(&self) -> Option<&str> {
        self.active.and_then(|i| self.tabs.get(i)).and_then(|t| t.close_error.as_deref())
    }

    /// Whether an opening is under way.
    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    /// Whether a save is under way.
    pub fn is_saving(&self) -> bool {
        self.saving.is_some()
    }

    /// Keeps how a save went, the latest few, for the agent that asked for it.
    fn finished_save(&mut self, ticket: u64, outcome: SaveState) {
        // A closing that waited for this save goes on, asks again when edits came in meanwhile,
        // or stops with the save's error.
        let waiting: Vec<std::path::PathBuf> = self.close_after_save.iter().filter(|(_, t)| *t == ticket).map(|(p, _)| p.clone()).collect();
        self.close_after_save.retain(|(_, t)| *t != ticket);
        for path in waiting {
            match (&outcome, self.tab_of(&path)) {
                (SaveState::Saved(_), Some(i)) if self.tab_unsaved(i) => self.ask = Some(close_ask::Ask::Unsaved { path }),
                (SaveState::Saved(_), Some(i)) => self.close_tab_now(i),
                _ => self.quitting = None,
            }
        }
        if self.saves.len() == SAVES_KEPT {
            self.saves.pop_front();
        }
        self.saves.push_back((ticket, outcome));
    }

    /// How the save with `ticket` stands, whichever tab its project is in.
    pub fn save_state(&self, ticket: u64) -> SaveState {
        if let Some((_, outcome)) = self.saves.iter().find(|(t, _)| *t == ticket) {
            return outcome.clone();
        }
        let running = |s: &Option<Saving>| s.as_ref().is_some_and(|(_, _, t)| *t == ticket);
        if running(&self.saving) || self.tabs.iter().any(|tab| tab.parked.as_ref().is_some_and(|p| running(&p.saving))) {
            return SaveState::Running;
        }
        SaveState::Forgotten
    }

    /// At exit, before the host writes what waits: the skill text still being typed is kept and
    /// handed over with the rest.
    pub fn before_exit(&mut self) {
        skill_tab::sync(self, 0.0, true, None);
        let parked = self.tabs.iter().filter_map(|t| t.parked.as_ref()).map(|p| &p.canvas);
        for (skill, chars) in std::iter::once(&self.canvas).chain(parked).flat_map(skill_tab::unkept) {
            log::warn!("at exit: the text of skill {skill} ({chars} characters) was refused and is not kept");
        }
        self.write_changes();
    }

    /// Gives the shell the control channel's requests, from every opening (the person's own access
    /// and Chat's): it answers what `requests` brings once per frame, as each request's token
    /// allows.
    pub fn attach_control(&mut self, requests: std::sync::mpsc::Receiver<control::ControlRequest>) {
        self.control = Some(requests);
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
        self.keep_view();
        if !self.dolt_asked {
            self.look_for_dolt();
        }
        self.poll_host();
        // The agent programs and keys are looked for once at start, never by opening Settings.
        ai_agents::start(self);
        ai_agents::poll(self, ctx);
        // Chat's conversations: the host's events, and the opt-in told to the host.
        chat::frame(self, ctx);
        control::drain(self);
        // While the host works on an opening, a save or a write, the window looks again soon, so
        // its answer shows without waiting for the pointer to move.
        // A write that failed waits for the next change or Save, and the status bar already says
        // why: nothing to look for until then.
        let writing = match (&self.services.projects, &self.project) {
            (Some(h), Some(p)) => {
                let w = h.writing(&p.path);
                w.pending && w.error.is_none()
            }
            _ => false,
        };
        let closing = self.tabs.iter().any(Tab::closing);
        if self.opening.is_some() || self.dolt_check.is_some() || self.saving.is_some() || writing || closing || self.agents.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // Cmd+S (Ctrl+S) saves the open project and Cmd+O (Ctrl+O) opens one; on macOS the menu
        // bar takes both first.
        let now = ctx.input(|i| i.time);
        // File → Generate…'s keys, on every platform.
        if self.can_generate()
            && self.ask.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::G))
        {
            self.open_generate();
        }
        if self.session.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        if self.about.is_none() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_from_picker();
        }
        // A skill's text is kept once its typing pauses, and all of it before quitting.
        skill_tab::sync(self, now, self.quit_requested, None);
        if skill_tab::waiting(self) {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        self.write_changes();
        if let Some(session) = &self.session {
            explorer::retain_valid(&mut self.explorer, session.doc());
            diagrams::retain_valid(&mut self.diagrams, session.doc());
            canvas::retain_valid(&mut self.canvas, session.doc());
            skill_tab::retain_valid(&mut self.canvas);
            panels::retain_valid(&mut self.panels, session.doc());
            uml_inline::retain_valid(&mut self.canvas.inline, session.doc());
            self.problems = problems::by_element(self.notation_uis, session.doc());
        }
        // Quitting, by the menu or the window's close button, asks about unsaved projects first;
        // once each is answered the window closes through the normal path, so the exit hook runs.
        let closing_window = ctx.input(|i| i.viewport().close_requested());
        if closing_window && !self.quit_confirmed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if (std::mem::take(&mut self.quit_requested) || closing_window) && !self.quit_confirmed && self.quitting.is_none() && self.ask.is_none() {
            self.start_quit(ctx);
        }
        self.continue_quit(ctx);
    }

    /// The window title: the open project's name and the application's, or the application's alone.
    pub fn title(&self) -> String {
        match &self.project {
            // Unsaved changes mark the title with a dot before the name.
            Some(p) => {
                let name = if self.session.as_ref().is_some_and(|s| s.is_dirty()) { format!("• {}", p.name) } else { p.name.clone() };
                crate::i18n::fmt(&crate::i18n::tr("{name} — Archforge"), &[("name", &name)])
            }
            None => "Archforge".to_string(),
        }
    }

    /// Draws the window: the workspace when a project is open, otherwise Home, and the New project
    /// dialog over Home while it is open.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.registry.begin_frame();
        settings::shortcut(self, ui.ctx());
        // Escape closes About before the window behind it is drawn, so no editor there reads it.
        support::dismiss_key(self, ui.ctx());
        // The OS window title follows the in-app title; on a host with a native title bar it is
        // the only title the user sees.
        let title = self.title();
        if self.last_title != title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        // While About or Settings is over the window, the window behind it takes no keys: its tree,
        // canvas and editors read keys directly, so the frame's keys are held back while it is
        // drawn and given back for the dialogs.
        let held = if self.about.is_some() || self.settings.is_some() { hold_keys(ui.ctx()) } else { Vec::new() };
        if self.project.is_some() {
            workspace::show(self, ui);
        } else {
            home::show(self, ui);
        }
        if !held.is_empty() {
            ui.ctx().input_mut(|i| i.events.extend(held));
        }
        new_project::show(self, ui);
        settings::show(self, ui);
        support::show(self, ui);
        generate::show(self, ui);
        close_ask::show(self, ui);
        // A project shown in this frame (made, opened or recalled from its tab) gets its model on
        // the next: that frame is asked for now, not left to the pointer's next move.
        if self.project.as_ref().is_some_and(|p| self.adopted.as_ref() != Some(&p.path)) {
            ui.ctx().request_repaint();
        }
    }
}

/// Takes the frame's keyboard events out of the input (keys, typed text, clipboard and
/// composition), leaving pointer and window events, and gives them back.
fn hold_keys(ctx: &egui::Context) -> Vec<egui::Event> {
    ctx.input_mut(|i| {
        let (keys, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut i.events).into_iter().partition(|e| {
            matches!(
                e,
                egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_) | egui::Event::Ime(_)
            )
        });
        i.events = rest;
        keys
    })
}

#[cfg(test)]
mod tests;
