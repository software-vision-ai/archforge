//! The problems the shell shows (slice E4 of archforge-gm2.10, from the design's cards "ER ·
//! Canvas · Problems and a note" and "ER · Properties · Problems"): a table without a primary key,
//! a many-to-many relationship still waiting for its mapping table, a foreign key the rules
//! refuse, a key column that may be null, a key whose columns the rules refuse. Each is named by
//! the element it sits on, so the canvas can badge the node or line and Properties can list and
//! select it. Each notation finds its own (`NotationUi::problems`; ER's in `er_ui`). The engine's `rules::problems` stays the rule of record for the whole store; this
//! list is the user's view of it, in the user's words.

use std::collections::BTreeMap;

use archforge_mof::{Obj, Store};

use crate::notation_ui::NotationUi;

/// One problem, on the element it sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub element: Obj,
    pub text: String,
}

/// Every problem of the store, each notation's in turn, on the element the user would fix it on.
pub(crate) fn all(uis: &[&'static dyn NotationUi], store: &Store) -> Vec<Problem> {
    let mut out = Vec::new();
    for ui in uis {
        ui.problems(store, &mut out);
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

/// The problems a selection shows, from the frame's map: its own (a column's are its table's), or
/// for a notation's top element (ER's schema) or a diagram of one those of every element in it,
/// each with the element a click selects.
pub(crate) fn of(uis: &[&'static dyn NotationUi], store: &Store, map: &BTreeMap<Obj, Vec<String>>, subject: Obj) -> Vec<Problem> {
    let ui = crate::notation_ui::of(uis, store, subject);
    let scope = if store.class_of(subject).ok() == Some(archforge_diagram::DIAGRAM) {
        store.get(subject, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref())
    } else if ui.is_some_and(|u| u.is_top(store, subject)) {
        Some(subject)
    } else {
        None
    };
    let root = ui.map_or(subject, |u| u.problem_holder(store, subject));
    map.iter()
        .filter(|(element, _)| match scope {
            Some(s) => **element == s || store.contains(s, **element),
            None => **element == root,
        })
        .flat_map(|(element, texts)| texts.iter().map(move |text| Problem { element: *element, text: text.clone() }))
        .collect()
}
