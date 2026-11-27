//! Which explorer the workspace's activity rail shows and which panel its right rail shows, as the
//! shell keeps them in its saved interface state (`archforge_ui_egui::state::UiState`) and as the
//! narrow context asks for them ([`crate::cx::Request`]).

use serde::{Deserialize, Serialize};

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
