//! ER's screens (ADR 0002, "Notations are plugins"): what ER brings to the shell through the
//! shell's interfaces (`archforge-shell-api`), and nothing of the shell itself. Its screens part,
//! [`ER`], gives the shell ER's notation (`NotationUi`: its rows in Models, its search entries,
//! Properties' bodies and editors, its problems, refusals and words, with its own catalogues) and
//! the ER diagram's kind (`DiagramKind`: its tools and gestures, its tables, its relationship lines
//! with their crow's-foot ends, the cardinality popover and the mapping offer); the app registers
//! it. Each screen reaches the shell through the narrow context, never the whole shell, and draws
//! through the design (`ux`, with the icons only ER draws).
//!
//! Its modules are its own. Behind the `snapshot` feature, `snapshot` lists what the app's
//! screenshot tool and the app's tests reach in them; only the app links this crate (ADR 0004). It
//! sits on the notations' screens' layer (10), above the shell's interfaces (9) and below the shell
//! (11), and builds for the portable target.

mod er_checks;
mod er_connect;
mod er_indexes;
mod er_keys;
mod er_kind;
mod er_objects;
mod er_place;
mod er_relation;
mod er_said;
mod er_state;
mod er_subtypes;
mod er_table;
mod er_types;
mod er_ui;
mod ux;

#[cfg(feature = "snapshot")]
pub mod snapshot;

#[cfg(test)]
mod tests;

pub use er_ui::ER;
