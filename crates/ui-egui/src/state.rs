//! The serialisable UI state, stored with the preferences.

use serde::{Deserialize, Serialize};

use crate::theme::ThemeKind;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub theme: ThemeKind,
    pub language: String,
    /// Home's community banner, closed by the user and kept closed.
    pub home_banner_closed: bool,
    /// The workspace's explorer and right panel, and their widths in logical pixels.
    pub explorer: Explorer,
    pub panel: Panel,
    pub explorer_width: f32,
    pub panel_width: f32,
    /// The explorer and the panel folded away by the user: their rails stay, the centre widens.
    pub explorer_collapsed: bool,
    pub panel_collapsed: bool,
    /// Home's recent projects, the last opened first. An entry a preferences file holds in a
    /// shape the window does not read is left out, not the whole file.
    #[serde(deserialize_with = "lenient_recent")]
    pub recent: Vec<RecentProject>,
    /// The folders pinned on Home, in the order they were pinned. A path a preferences file
    /// holds that the window does not take is left out by [`UiState::sanitize`].
    #[serde(deserialize_with = "lenient_pinned")]
    pub pinned: Vec<std::path::PathBuf>,
    /// How Home shows the recent projects, and in which order; kept as the user left them.
    pub recent_view: RecentView,
    pub recent_sort: RecentSort,
    /// The user's name as entered in Settings; the bar shows its initials. Empty: JD, for John Doe.
    pub user_name: String,
    /// Settings → AI Agents' choices: the default agent, the models and how Codex signs in. No key
    /// is ever kept here. A section a preferences file
    /// holds in a shape the window does not read (a later version's choice) goes back to the
    /// defaults, not the whole file.
    #[serde(deserialize_with = "lenient_agents")]
    pub agents: crate::ai_agents::AgentPrefs,
}

/// The recent list as a file holds it: the entries that read, the others left out (the next
/// [`UiState::sanitize`] says how many went).
fn lenient_recent<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<RecentProject>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Read(RecentProject),
        Unread(serde::de::IgnoredAny),
    }
    let entries: Vec<Entry> = Vec::deserialize(d)?;
    Ok(entries
        .into_iter()
        .map(|e| match e {
            Entry::Read(r) => r,
            // Left out by sanitize: it has no name.
            Entry::Unread(_) => RecentProject::default(),
        })
        .collect())
}

/// The AI Agents choices as a file holds them, or the defaults when they do not read (said in the
/// log).
fn lenient_agents<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::ai_agents::AgentPrefs, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Read(crate::ai_agents::AgentPrefs),
        Unread(serde::de::IgnoredAny),
    }
    Ok(match Entry::deserialize(d)? {
        Entry::Read(prefs) => prefs,
        Entry::Unread(_) => {
            log::warn!("the AI Agents choices in the preferences do not read; the defaults are used");
            crate::ai_agents::AgentPrefs::default()
        }
    })
}

/// The pinned folders as a file holds them: an entry that is not a path is left out, not the
/// whole file.
fn lenient_pinned<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<std::path::PathBuf>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Read(std::path::PathBuf),
        Unread(serde::de::IgnoredAny),
    }
    let entries: Vec<Entry> = Vec::deserialize(d)?;
    Ok(entries
        .into_iter()
        .map(|e| match e {
            Entry::Read(p) => p,
            // Left out and counted by sanitize: it is not absolute.
            Entry::Unread(_) => std::path::PathBuf::new(),
        })
        .collect())
}

/// The most projects Home's recent list keeps, and the most folders it pins.
pub const MAX_RECENT: usize = 12;
pub const MAX_PINNED: usize = 8;
/// The longest folder path, in bytes, a recent entry may have: longer ones in a preferences file
/// are dropped. Names follow the project name rule.
const MAX_RECENT_PATH: usize = 4096;

/// Whether a recent entry fits the list: a project name and an absolute folder of bounded length.
fn recent_ok(name: &str, path: &std::path::Path) -> bool {
    archforge_names::problem(name).is_none() && path.is_absolute() && path.as_os_str().len() <= MAX_RECENT_PATH && path.to_str().is_some()
}

/// Whether a folder fits Home's pinned list: absolute, of bounded length, with a name, and
/// readable as text, as the preferences file must write it.
fn pin_ok(path: &std::path::Path) -> bool {
    path.is_absolute() && path.as_os_str().len() <= MAX_RECENT_PATH && path.file_name().is_some() && path.to_str().is_some()
}

/// A project opened before: its name, its folder, when it was last opened (seconds since 1970),
/// and the working view it was left in.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentProject {
    pub name: String,
    pub path: std::path::PathBuf,
    pub opened: u64,
    pub view: Option<SavedView>,
    /// The branch it was left on, as Dolt stores the name; none for `main`, or a file from before
    /// branches, which opens `main` all the same.
    pub branch: Option<String>,
}

/// The working view a project was left in: its diagram and skill tabs in order, the one
/// shown, and how the canvas was panned and zoomed. It is the window's, kept with the recent
/// entry, never in the model, so resuming it makes no change and no undo step.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedView {
    pub tabs: Vec<String>,
    pub active: Option<String>,
    pub pan: [f32; 2],
    pub zoom: f32,
    /// The shown diagram's name, for Home's recent row.
    pub label: String,
}

/// The most bytes of a stored branch name kept: more than any branch name can have, so that a
/// planted one goes no further than here.
const MAX_BRANCH_BYTES: usize = 1024;
/// The most tabs a saved view keeps.
pub const MAX_VIEW_TABS: usize = 64;
/// The farthest a saved view's pan reaches, in logical pixels.
const MAX_PAN: f32 = 1.0e7;

impl SavedView {
    /// Whether the view reads as one the window can resume: ids, a pan within reach and a zoom
    /// the canvas takes; one that does not is left out whole, never bent into another.
    fn ok(&self) -> bool {
        let id = |t: &String| archforge_mof::Uuid::parse(t).is_some();
        let (min, max) = crate::canvas::ZOOM_RANGE;
        self.tabs.len() <= MAX_VIEW_TABS
            && self.tabs.iter().all(id)
            && self.active.as_ref().is_none_or(id)
            && self.pan.iter().all(|v| v.is_finite() && v.abs() <= MAX_PAN)
            && self.zoom.is_finite()
            && (min..=max).contains(&self.zoom)
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: ThemeKind::default(),
            language: "en".to_string(),
            home_banner_closed: false,
            explorer: Explorer::Models,
            panel: Panel::Properties,
            explorer_width: crate::workspace::EXPLORER_DEFAULT,
            panel_width: crate::workspace::PANEL_DEFAULT,
            explorer_collapsed: false,
            panel_collapsed: false,
            recent: Vec::new(),
            pinned: Vec::new(),
            recent_view: RecentView::List,
            recent_sort: RecentSort::LastOpened,
            user_name: String::new(),
            agents: crate::ai_agents::AgentPrefs::default(),
        }
    }
}

impl UiState {
    /// Puts the project at `path` at the top of the recent list, opened at `now`.
    pub fn remember(&mut self, name: &str, path: &std::path::Path, now: u64) {
        if !recent_ok(name, path) {
            log::warn!("the project {} is not listed on Home: its name or folder does not fit the list", path.display());
            return;
        }
        // Opened again, it keeps the view it was left in.
        let (view, branch) = self.recent.iter().find(|r| r.path == path).map(|r| (r.view.clone(), r.branch.clone())).unwrap_or_default();
        self.recent.retain(|r| r.path != path);
        self.recent.insert(0, RecentProject { name: name.to_string(), path: path.to_path_buf(), opened: now, view, branch });
        self.recent.truncate(MAX_RECENT);
    }

    /// Keeps `view` as the one the recent project at `path` was left in; a project not listed
    /// keeps none.
    pub fn keep_view(&mut self, path: &std::path::Path, view: SavedView) {
        if let Some(r) = self.recent.iter_mut().find(|r| r.path == path)
            && r.view.as_ref() != Some(&view)
        {
            r.view = Some(view);
        }
    }

    /// The view the recent project at `path` was left in, when one was kept.
    pub fn view_of(&self, path: &std::path::Path) -> Option<&SavedView> {
        self.recent.iter().find(|r| r.path == path).and_then(|r| r.view.as_ref())
    }

    /// Keeps `branch` as the one the recent project at `path` was left on, none being `main`, as
    /// `main` named in any case is.
    pub fn keep_branch(&mut self, path: &std::path::Path, branch: Option<String>) {
        if let Some(r) = self.recent.iter_mut().find(|r| r.path == path) {
            r.branch = branch.filter(|b| !b.eq_ignore_ascii_case("main"));
        }
    }

    /// The branch the recent project at `path` was left on, when one was kept.
    pub fn branch_of(&self, path: &std::path::Path) -> Option<&str> {
        self.recent.iter().find(|r| r.path == path).and_then(|r| r.branch.as_deref())
    }

    /// Takes the project at `path` off the recent list; the project itself stays where it is.
    pub fn forget(&mut self, path: &std::path::Path) {
        self.recent.retain(|r| r.path != path);
    }

    /// Pins the folder at `path` on Home, after the folders pinned already; a folder pinned
    /// already, one that does not fit the list or one past the most it keeps is not added.
    pub fn pin(&mut self, path: &std::path::Path) {
        if self.pinned.iter().any(|p| p == path) {
            return;
        }
        if !pin_ok(path) {
            log::warn!("{} is not pinned on Home: the folder does not fit the list", path.display());
            return;
        }
        if self.pinned.len() >= MAX_PINNED {
            log::warn!("{} is not pinned: Home pins at most {MAX_PINNED} folders", path.display());
            return;
        }
        self.pinned.push(path.to_path_buf());
    }

    /// Takes the folder at `path` off Home.
    pub fn unpin(&mut self, path: &std::path::Path) {
        self.pinned.retain(|p| p != path);
    }

    /// Drops what a preferences file may hold that the window does not take: recent entries whose
    /// name breaks the project name rule, whose folder is not absolute or too long, or that are
    /// listed twice; pinned folders that are not absolute, too long or pinned twice; more than
    /// either list keeps. Says in the log how many went.
    pub fn sanitize(&mut self) {
        self.user_name = crate::settings::clean(&self.user_name);
        self.agents.sanitize();
        let pins = self.pinned.len();
        let mut pinned = std::collections::HashSet::new();
        self.pinned.retain(|p| pin_ok(p) && pinned.insert(p.clone()));
        self.pinned.truncate(MAX_PINNED);
        if self.pinned.len() < pins {
            log::warn!("{} of {pins} pinned folders in the preferences were left out", pins - self.pinned.len());
        }
        let before = self.recent.len();
        let mut seen = std::collections::HashSet::new();
        self.recent.retain(|r| recent_ok(&r.name, &r.path) && seen.insert(r.path.clone()));
        self.recent.truncate(MAX_RECENT);
        if self.recent.len() < before {
            log::warn!("{} of {before} recent projects in the preferences were left out", before - self.recent.len());
        }
        for r in &mut self.recent {
            if r.view.as_ref().is_some_and(|v| !v.ok()) {
                log::warn!("the view {} was left in does not read; it opens on no tab", r.path.display());
                r.view = None;
            }
            if let Some(v) = r.view.as_mut() {
                v.label = crate::settings::clean(&v.label);
            }
            r.branch.take_if(|b| b.len() > MAX_BRANCH_BYTES);
        }
    }

    /// Opens the explorer on `which`.
    pub fn show_explorer(&mut self, which: Explorer) {
        self.explorer = which;
        self.explorer_collapsed = false;
    }

    /// Opens the panel on `which`.
    pub fn show_panel(&mut self, which: Panel) {
        self.panel = which;
        self.panel_collapsed = false;
    }

    /// A click on an activity rail icon: the open explorer's own icon folds it away; any other,
    /// or any while it is folded, opens the explorer on that one.
    pub fn rail_explorer(&mut self, which: Explorer) {
        if which == self.explorer && !self.explorer_collapsed {
            self.explorer_collapsed = true;
        } else {
            self.show_explorer(which);
        }
    }

    /// A click on a right rail icon, as [`UiState::rail_explorer`] for the panel.
    pub fn rail_panel(&mut self, which: Panel) {
        if which == self.panel && !self.panel_collapsed {
            self.panel_collapsed = true;
        } else {
            self.show_panel(which);
        }
    }
}

/// Home's recent projects as rows or as a gallery of cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RecentView {
    #[default]
    List,
    Gallery,
}

/// The order of Home's recent projects: the last opened first, or by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RecentSort {
    #[default]
    LastOpened,
    Name,
}

/// The explorers of the workspace's activity rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Explorer {
    #[default]
    Models,
    Diagrams,
    SourceControl,
}

/// The panels of the workspace's right rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Panel {
    Agent,
    Appearance,
    #[default]
    Properties,
    Skills,
}
