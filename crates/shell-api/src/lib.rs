//! The shell's interfaces (ADR 0003, "Two registries"): what a notation's screens implement, and
//! what they may use from the shell, so a notation's screens never depend on the shell crate
//! (`archforge-ui-egui`) itself.
//!
//! It holds the automation-id registry (`widgets`) and the design (`ux`: the colours, the type, the
//! icons the shell and both notations draw, and the helpers that draw them). Story AR-780 moves the
//! rest into it, step by step and unchanged: the shared controls and canvas geometry, the
//! interfaces `NotationUi` and `DiagramKind` with the data types they name, and the translations
//! machinery. It sits on the shell interfaces' layer (9), below the notations'
//! screens (10) and the shell (11), and builds for the portable target.

pub mod ux;
pub mod widgets;
