//! The shell's interfaces (ADR 0003, "Two registries"): what a notation's screens implement, and
//! what they may use from the shell, so a notation's screens never depend on the shell crate
//! (`archforge-ui-egui`) itself.
//!
//! The crate is empty for now. Story AR-780 moves into it, step by step and unchanged: the
//! automation-id registry, the design tokens and shared icons, the shared controls and canvas
//! geometry, the interfaces `NotationUi` and `DiagramKind` with the data types they name, and the
//! translations machinery. It sits on the shell interfaces' layer (9), below the notations'
//! screens (10) and the shell (11), and builds for the portable target.
