//! The command engine. Every edit is a registered command: an identifier, a label, a menu place, a
//! shortcut, its parameters documented one line per key, an enabled predicate with its reason, and
//! a handler. A session validates every call before its handler runs, applies it to a copy kept only
//! on success, records one undo step per command, batch or gesture, undoes and redoes to identical
//! state, and knows whether the project has unsaved changes.
//!
//! The engine knows no notation: a notation crate brings its document (what undo covers), its view
//! state (selection, zoom; never undone) and its command areas, and builds a `Registry` from them.
//! It builds for the portable target: no file system, process, clock or environment.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod cmd;
mod error;
pub mod params;
mod session;
#[cfg(feature = "testing")]
pub mod testing;

pub use cmd::{CommandInfo, CommandSpec, Ctx, Facts, Handler, ParamInfo, Registry, always};
pub use error::{CommandError, EngineError};
pub use params::{ParamDoc, Params, parse_params};
pub use session::{Batch, Dropped, JournalEntry, Options, Outcome, Session};

/// What undo covers: the model, its layout, the project's settings. Its states are compared for
/// equality, so it never holds a NaN.
pub trait Document: Clone + PartialEq + Send + 'static {
    /// An estimate of its size in bytes, for the history's cap.
    fn weight(&self) -> usize;
}

/// View state undo never touches: the selection, the zoom, the open diagram.
pub trait View<D>: Clone + PartialEq + Default + Send + 'static {
    /// Drops what no longer exists in `doc`, such as a selected element an undo removed.
    fn retain_valid(&mut self, doc: &D);
}
