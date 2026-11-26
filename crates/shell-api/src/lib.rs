//! The shell's interfaces (ADR 0003, "Two registries"): what a notation's screens implement, and
//! what they may use from the shell, so a notation's screens never depend on the shell crate
//! (`archforge-ui-egui`) itself.
//!
//! It holds the automation-id registry (`widgets`), the design (`ux`: the colours, the type, the
//! icons the shell and both notations draw, and the helpers that draw them), the canvas's
//! geometry that names no shell (`canvas`), and the data types the interfaces `NotationUi`
//! (`notation_ui`) and `DiagramKind` (`kinds`) hand across that name no shell. Story AR-780 moves
//! the rest into it, step by step and unchanged: the shared controls, the interfaces themselves
//! with the data types that still name the shell, and the translations machinery. It sits on the shell interfaces' layer (9),
//! below the notations' screens (10) and the shell (11), and builds for the portable target.

pub mod canvas;
pub mod kinds;
pub mod notation_ui;
pub mod ux;
pub mod widgets;
