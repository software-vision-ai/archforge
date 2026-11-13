//! The command engine. Every edit is a registered command: an identifier, a label, a menu place, a
//! shortcut, its parameters documented one line per key, an enabled predicate with its reason, and
//! a handler. A session validates every call before its handler runs, records what the handler
//! changes as a delta, rolls that delta back when the call fails, records one undo step per
//! command, batch or gesture, undoes and redoes by applying inverse deltas, and knows whether the
//! project has unsaved changes.
//!
//! The engine knows no notation: a notation crate brings its document (what undo covers, recording
//! its own changes), its view state (selection, zoom; never undone) and its command areas, and
//! builds a `Registry` from them. A document that is cheaper to copy than to record wraps itself in
//! [`Snapshotted`]. The engine builds for the portable target: no file system, process, clock or
//! environment.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod cmd;
mod error;
pub mod params;
mod session;
mod snapshot;
#[cfg(feature = "testing")]
pub mod testing;

pub use cmd::{CommandInfo, CommandSpec, Ctx, Facts, Handler, ParamInfo, Registry, always};
pub use error::{CommandError, End, EngineError, Parameter, Reason};
pub use params::{ParamDoc, Params, parse_params};
pub use session::{Batch, Dropped, GroupEnd, JournalEntry, MAX_GROUPS, Options, Outcome, Session};
pub use snapshot::{Replace, Snapshot, Snapshotted};

/// What undo covers: the model, its layout, the project's settings. A handler changes it in place;
/// the document records what changed between [`Document::begin`] and [`Document::end`] as a delta
/// with an exact inverse, which the session applies to roll a failed call back and to undo and
/// redo.
pub trait Document: Send + 'static {
    /// One edit's recorded change.
    type Delta: Clone + Send + 'static;
    /// Starts recording; a recording in progress is discarded.
    fn begin(&mut self);
    /// Stops recording and gives what changed since `begin`, or None when nothing did.
    fn end(&mut self) -> Option<Self::Delta>;
    /// Applies a delta outside a recording, all or nothing: on an error the document is as it was.
    /// An error here is internal (a delta applied to a state it does not fit).
    fn apply(&mut self, delta: &Self::Delta) -> Result<(), String>;
    /// The delta that takes the document back.
    fn invert(delta: &Self::Delta) -> Self::Delta;
    /// `first` then `second` as one delta.
    fn compose(first: Self::Delta, second: Self::Delta) -> Self::Delta;
    /// Whether the delta changes nothing in the end (a drag back to where it started).
    fn is_empty(delta: &Self::Delta) -> bool;
    /// An estimate of the delta's size in bytes, for the history's cap.
    fn weight(delta: &Self::Delta) -> usize;
}

/// View state undo never touches: the selection, the zoom, the open diagram.
pub trait View<D>: Clone + PartialEq + Default + Send + 'static {
    /// Drops what no longer exists in `doc`, such as a selected element an undo removed.
    fn retain_valid(&mut self, doc: &D);
}
