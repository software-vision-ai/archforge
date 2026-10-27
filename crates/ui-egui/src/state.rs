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
}

impl Default for UiState {
    fn default() -> Self {
        Self { theme: ThemeKind::default(), language: "en".to_string(), home_banner_closed: false }
    }
}
