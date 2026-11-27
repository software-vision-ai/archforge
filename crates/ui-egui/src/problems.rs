//! The problems the shell shows (from the design's cards "ER ·
//! Canvas · Problems and a note" and "ER · Properties · Problems"): what the rules refuse in a
//! notation's elements, or what an element still waits for. Each is named by the element it sits
//! on, so the canvas can badge the node or line and Properties can list and select it. Each
//! notation finds its own (`NotationUi::problems`). The engine's `rules::problems` stays the rule
//! of record for the whole store; this list is the user's view of it, in the user's words. What a
//! selection shows of it is `archforge_shell_api::problems::of`, which the shared Problems section
//! reads.

use std::collections::BTreeMap;

use archforge_mof::{Obj, Store};

pub use archforge_shell_api::notation_ui::Problem;

use crate::notation_ui::NotationUi;

/// Every problem of the store, each notation's in turn, on the element the user would fix it on.
pub(crate) fn all(uis: &[&'static dyn NotationUi], store: &Store) -> Vec<Problem> {
    let mut out = Vec::new();
    for ui in uis {
        ui.problems(uis, store, &mut out);
    }
    out
}

/// The problems by the element they sit on.
pub(crate) fn by_element(uis: &[&'static dyn NotationUi], store: &Store) -> BTreeMap<Obj, Vec<String>> {
    let mut out: BTreeMap<Obj, Vec<String>> = BTreeMap::new();
    for p in all(uis, store) {
        out.entry(p.element).or_default().push(p.text);
    }
    out
}
