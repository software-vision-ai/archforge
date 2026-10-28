//! The edit area: undo and redo, which every notation's table includes.

use super::{CommandSpec, Facts, Handler};
use crate::{Document, View};

/// Why Undo and Redo are not available.
pub const NOTHING_TO_UNDO: &str = "There is nothing to undo.";
pub const NOTHING_TO_REDO: &str = "There is nothing to redo.";

fn can_undo<D, V>(f: &Facts<'_, D, V>) -> Result<(), &'static str> {
    if f.can_undo { Ok(()) } else { Err(NOTHING_TO_UNDO) }
}

fn can_redo<D, V>(f: &Facts<'_, D, V>) -> Result<(), &'static str> {
    if f.can_redo { Ok(()) } else { Err(NOTHING_TO_REDO) }
}

/// Undo and Redo: journaled, recording no undo step of their own.
pub fn specs<D: Document, V: View<D>>() -> Vec<CommandSpec<D, V>> {
    vec![
        CommandSpec {
            id: "edit.undo",
            label: "Undo",
            menu: &["Edit"],
            shortcut: Some("Ctrl+Z"),
            params: "",
            enabled: can_undo,
            handler: Handler::Undo,
            journal: true,
            undoable: false,
        },
        CommandSpec {
            id: "edit.redo",
            label: "Redo",
            menu: &["Edit"],
            shortcut: Some("Ctrl+Shift+Z"),
            params: "",
            enabled: can_redo,
            handler: Handler::Redo,
            journal: true,
            undoable: false,
        },
    ]
}
