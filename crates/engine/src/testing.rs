//! Helpers for the tests of crates that bring commands (the `testing` feature; never shipped).

use serde_json::Value;

use crate::{Document, Session, View};

/// Runs a command, then undoes and redoes it, and says whether each step gave the identical
/// document: before the command after the undo, and after it after the redo.
pub fn round_trip<D: Document + std::fmt::Debug, V: View<D>>(session: &mut Session<D, V>, id: &str, params: &Value) -> Result<(), String> {
    let before = session.doc().clone();
    session.execute(id, params).map_err(|e| format!("{id}: {e}"))?;
    let after = session.doc().clone();
    if after == before {
        return Ok(());
    }
    session.undo().map_err(|e| format!("undo of {id}: {e}"))?;
    if *session.doc() != before {
        return Err(format!("undo of {id} gave {:?}, not {before:?}", session.doc()));
    }
    session.redo().map_err(|e| format!("redo of {id}: {e}"))?;
    if *session.doc() != after {
        return Err(format!("redo of {id} gave {:?}, not {after:?}", session.doc()));
    }
    Ok(())
}
