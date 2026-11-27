//! The diagram kinds the shell knows. What a kind does with the canvas's tools and gestures and
//! what it draws, the interface `DiagramKind`, is `archforge-shell-api`'s `kinds`, re-exported here
//! under its old paths with the data types it hands across: a kind brings its toolbar
//! tools, each placing an element or connecting two, and the hooks the canvas's gestures run on
//! its elements: place, connect, a line dropped on empty canvas, renaming in place and where the
//! name typed goes on a card it draws, opening an
//! element's editor at a row of its card, a second adorner, the rows its element menu adds with
//! what each does, the live line while connecting, a press and a right click it reads before the
//! shared gestures, the keys a drawing under way takes, whether its popover's menu takes the keys,
//! what Select all takes in beside the boxes and lines, and where a label of its own figures is
//! let go; and the hooks the canvas runs as it
//! draws one of its diagrams: before and after the drawing, where a line starts, what goes with a
//! line, the figures of no one node or line, a card it draws itself, and the hint bar; and its rows
//! of the Model menu.
//! Select, Pan, Note, Link note, Grid and Arrange stay shared; the status marks toggle shows where
//! a kind has marks.

use archforge_mof::{Obj, Store};

pub(crate) use archforge_shell_api::kinds::{DiagramKind, KindTool, MenuRow, ModelRow, Press, ToolUse, diagram_icon};

use crate::Shell;

/// The kind of the diagram `d`, when the shell knows it: asked of the shell's kinds.
pub(crate) fn kind_of(shell: &Shell, store: &Store, d: Obj) -> Option<&'static dyn DiagramKind> {
    archforge_shell_api::kinds::kind_of(shell.kinds, store, d)
}

/// Whether the canvas's open popover is a menu the keys walk, as the shell's kinds answer
/// (`archforge_shell_api::kinds::popover_takes_keys`).
pub(crate) fn popover_takes_keys(shell: &Shell) -> bool {
    archforge_shell_api::kinds::popover_takes_keys(shell.kinds, &shell.canvas)
}

/// The kind of the diagram the canvas shows, among the shell's kinds.
pub(crate) fn active_kind(shell: &Shell) -> Option<&'static dyn DiagramKind> {
    archforge_shell_api::kinds::active_kind(shell.kinds, &shell.canvas, shell.session.as_ref())
}
