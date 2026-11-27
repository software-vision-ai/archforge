//! The problems a selection shows in Properties, each on the element a click selects, from the
//! frame's map of what each notation found (`NotationUi::problems`).

use std::collections::BTreeMap;

use archforge_mof::{Obj, Store};

use crate::notation_ui::{NotationUi, Problem};

/// The problems a selection shows, from the frame's map: its own (a part's are those of the element
/// it belongs to), or for a notation's top element or a diagram of one those of every element in
/// it, each with the element a click selects.
pub fn of(uis: &[&'static dyn NotationUi], store: &Store, map: &BTreeMap<Obj, Vec<String>>, subject: Obj) -> Vec<Problem> {
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
