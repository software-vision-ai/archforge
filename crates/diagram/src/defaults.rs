//! The default-diagram bindings a store holds, read without judging them: which name an element
//! as their owner and which name a diagram as its default. What a binding may name, and what is
//! wrong with one, is the notation's to say (UML's `defaults`). A diagram a binding names stays
//! until that binding goes or names another (AR-578).

use archforge_engine::{BindingFault, CommandError, Reason};
use archforge_mof::{Invalid, Obj, PropId, Store};

use crate::commands::name_or_id;
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

/// Why the diagram `d` stays, as a refusal about `key`, while a binding of an element for which
/// `going` is false names it as a default (AR-578): the diagram's own element's binding, whose
/// default another diagram has to replace first, or another element's, which names it unsoundly
/// ([`BindingFault::WrongOwner`]), which is named first; none when no such binding names it.
pub fn diagram_removal_problem(store: &Store, d: Obj, key: &str, going: impl Fn(Obj) -> bool) -> Option<CommandError> {
    let of = |o: Obj, prop: PropId| store.get(o, prop).ok().and_then(|v| v.as_ref());
    let element = of(d, crate::diagram::ELEMENT);
    let named: Vec<(Obj, Obj)> =
        bindings_to(store, d).into_iter().filter_map(|b| of(b, OWNER_ELEMENT).filter(|o| !going(*o)).map(|o| (b, o))).collect();
    // Another element's binding first: choosing another default for the diagram's own leaves it.
    let (binding, owner) = named.iter().find(|(_, o)| Some(*o) != element).or(named.first()).copied()?;
    let id = |o: Obj| store.uuid(o).map(|u| u.to_string()).unwrap_or_default();
    let (package, fault) = (name_or_id(store, owner), (element != Some(owner)).then_some(BindingFault::WrongOwner));
    let message = match fault {
        None => format!("the diagram {} is the default diagram of {package}; choose another default diagram for it first", id(d)),
        Some(_) => format!(
            "the default-diagram binding {} of {package} names the diagram {}, which is another element's; choose a diagram of its own as its default first",
            id(binding),
            id(d)
        ),
    };
    Some(CommandError::about(key, message).because(Reason::DefaultDiagramOf { package: package.into(), fault }))
}

/// Why removing the diagrams of `elements` (of each one itself; a caller lists what they hold
/// too, [`crate::within`]) is refused, as about `key`: one of them is the default of an element
/// for which `going` is false, which keeps it ([`diagram_removal_problem`]); `going` covers the
/// whole removal, whose bindings go with it (AR-577). None when nothing stands in the way.
pub fn views_removal_problem(store: &Store, elements: impl IntoIterator<Item = Obj>, key: &str, going: impl Fn(Obj) -> bool) -> Option<CommandError> {
    let diagrams = elements.into_iter().flat_map(|e| crate::views_of(store, e).0);
    diagrams.into_iter().find_map(|d| diagram_removal_problem(store, d, key, &going))
}

/// Destroys the bindings the live ones of `elements` own, with no replacement, as one step, and
/// gives how many went: a removal of several elements takes all their bindings before any diagram
/// goes, so one's binding naming another's diagram is in no one's way, whatever the order (AR-578).
pub fn remove_bindings_of(store: &mut Store, elements: impl IntoIterator<Item = Obj>) -> Result<usize, Invalid> {
    let bindings: Vec<Obj> = elements.into_iter().filter(|e| store.is_alive(*e)).flat_map(|e| bindings_of(store, e)).collect();
    store.transaction(|st| {
        let mut gone = 0;
        for b in bindings {
            if st.is_alive(b) {
                gone += st.destroy(b)?;
            }
        }
        Ok(gone)
    })
}
