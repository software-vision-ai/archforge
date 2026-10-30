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
