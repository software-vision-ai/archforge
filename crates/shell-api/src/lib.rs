//! The shell's interfaces (ADR 0003, "Two registries"): what a notation's screens implement, and
//! what they may use from the shell, so a notation's screens never depend on the shell crate
//! (`archforge-ui-egui`) itself.
//!
//! It holds the automation-id registry (`widgets`), the design (`ux`: the colours, the type, the
//! icons the shell and both notations draw, and the helpers that draw them), the canvas's
//! geometry and state that name no shell (`canvas`, with what one frame's drawing reads), with the
//! editing under way on it (`editing`) and the skills open in its tabs (`skill_tab`), the interfaces
//! `NotationUi` (`notation_ui`) and `DiagramKind` (`kinds`) with the data types they hand across,
//! the narrow context through which they reach the shell (`cx`), how a search or a
//! filter matches what is typed (`search`), and the translations machinery (`i18n`), with which
//! each crate registers its own catalogues. The shared controls the panels and a notation's screens
//! draw are in `panels`, with the problems a selection shows (`problems`) and the key presses they
//! take (`actions`). The screens registry's type is here (`screens`): the notations' screens parts
//! the app registers, with the lists the shell reads made from them; the lists of notations and
//! kinds the shell knows today, which name ER and UML, stay in the shell. It sits
//! on the shell interfaces' layer (9), below the notations' screens (10) and the shell (11), and
//! builds for the portable target.

pub mod actions;
pub mod appearance;
pub mod canvas;
pub mod cx;
pub mod editing;
pub mod engine_said;
pub mod explorer;
pub mod i18n;
pub mod kinds;
pub mod menu_bar;
pub mod notation_ui;
pub mod panels;
pub mod problems;
pub mod screens;
pub mod search;
pub mod skill_tab;
pub mod state;
pub mod ux;
pub mod widgets;
