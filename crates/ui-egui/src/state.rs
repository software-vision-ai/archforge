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
        }
    }
}

impl UiState {
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
