//! The default-diagram bindings a store holds, read without judging them: which name an element
//! as their owner and which name a diagram as its default. What a binding may name, and what is
//! wrong with one, is the notation's to say (UML's `defaults`).

use archforge_mof::{Obj, PropId, Store};

use crate::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};

/// The bindings whose `ownerElement` is `owner`, in handle order: one for an element with a
/// default, none or several otherwise.
pub fn bindings_of(store: &Store, owner: Obj) -> Vec<Obj> {
    referring(store, owner, OWNER_ELEMENT)
}

/// The bindings whose `defaultDiagram` is `diagram`, in handle order.
pub fn bindings_to(store: &Store, diagram: Obj) -> Vec<Obj> {
    referring(store, diagram, DEFAULT_DIAGRAM)
}

/// Every live binding in the store, in handle order, whatever its ends hold.
pub fn bindings(store: &Store) -> Vec<Obj> {
    store.alive().filter(|o| store.class_of(*o).ok() == Some(crate::DEFAULT_DIAGRAM_BINDING)).collect()
}

/// The objects referring to `target` through the single-valued `prop`, so each once.
fn referring(store: &Store, target: Obj, prop: PropId) -> Vec<Obj> {
    store.referrers(target).into_iter().filter(|(_, p)| *p == prop).map(|(src, _)| src).collect()
}
