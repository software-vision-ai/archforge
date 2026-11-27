//! UML's screens (ADR 0002, "Notations are plugins"): what UML brings to the shell through the
//! shell's interfaces (`archforge-shell-api`), and nothing of the shell itself. Its screens part,
//! [`UML`], gives the shell UML's notation (`NotationUi`: its rows in Models, its search entries,
//! Properties' bodies and editors, its problems, refusals and words, with its own catalogues) and
//! the class diagram's kind (`DiagramKind`: its tools and gestures, its cards, lines and overlays,
//! and package navigation); the app registers it. Each screen reaches the shell through the narrow
//! context, never the whole shell, and draws through the design (`ux`, with the icons only UML
//! draws).
//!
//! Its modules are its own. Behind the `snapshot` feature, `snapshot` lists what the screenshot
//! tool, the app's tests across every catalogue and the shell's tests reach in them. It sits on
//! the notations' screens' layer (10), above the shell's interfaces (9) and below the shell (11),
//! and builds for the portable target.

mod uml_adornments;
mod uml_bindings;
mod uml_classifier;
mod uml_constraint_draw;
mod uml_constraints;
mod uml_inline;
mod uml_nary;
mod uml_nav_route;
mod uml_nesting;
mod uml_package;
mod uml_package_nav;
mod uml_panel;
mod uml_profiles;
mod uml_qualifiers;
mod uml_relate;
mod uml_said;
mod uml_sets;
mod uml_state;
mod uml_stereotypes;
mod uml_templates;
mod uml_ui;
mod ux;

#[cfg(feature = "snapshot")]
pub mod snapshot;

pub use uml_ui::UML;
