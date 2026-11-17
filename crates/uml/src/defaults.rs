//! A package's default diagram, read from the one binding that names it (Decision 12, AR-575):
//! the one resolver every reader asks, which gives the bound structure diagram, says the package
//! has none yet, or names what is wrong with its binding. It never falls back to the first diagram
//! found, and it changes nothing. A profile has no default.

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{defaults, diagram};
use archforge_mof::{Obj, Store, Uuid};

use crate::{PACKAGE, STRUCTURE_DIAGRAM};

/// A package's default diagram when its binding is sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultDiagram {
    /// The structure diagram the package's one binding names.
    Bound(Obj),
    /// The package has no structure diagram yet, and so no binding.
    NoDiagramYet,
    /// The element has no default diagram: a profile, or anything else that is not a live package.
    DoesNotApply,
}

/// What is wrong with a package's default-diagram binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingProblem {
    /// The package has structure diagrams, but no binding names one its default.
    Missing,
    /// More than one binding names the package's default: all of them, in id order.
    Duplicate(Vec<Obj>),
    /// The binding names no live diagram.
    DeadTarget { binding: Obj },
    /// The binding names a diagram that is no structure diagram.
    WrongKind { binding: Obj, diagram: Obj },
    /// The binding names a diagram of another element, or its own owner is no package and so has
    /// no default.
    WrongOwner { binding: Obj, diagram: Obj },
}

impl std::fmt::Display for BindingProblem {
    /// The problem in English, for the command line; an interface says it in its own words.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BindingProblem::Missing => "the package has structure diagrams but no default diagram",
            BindingProblem::Duplicate(_) => "the package has more than one default diagram",
            BindingProblem::DeadTarget { .. } => "the package's default diagram is gone",
            BindingProblem::WrongKind { .. } => "the package's default diagram is no structure diagram",
            BindingProblem::WrongOwner { .. } => "the default diagram is not the package's own",
        })
    }
}

impl std::error::Error for BindingProblem {}

/// A binding problem found in a store, with the element whose default it concerns: none for a
/// binding that names no owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub owner: Option<Obj>,
    pub problem: BindingProblem,
}

/// The default diagram of `package` as its binding gives it, or what is wrong with that binding.
pub fn resolve(store: &Store, package: Obj) -> Result<DefaultDiagram, BindingProblem> {
    if store.class_of(package).ok() != Some(PACKAGE) {
        return Ok(DefaultDiagram::DoesNotApply);
    }
    match by_id(store, defaults::bindings_of(store, package))[..] {
        [] if archforge_diagram::views_of(store, package).0.into_iter().any(|d| is_structure(store, d)) => Err(BindingProblem::Missing),
        [] => Ok(DefaultDiagram::NoDiagramYet),
        [binding] => target(store, binding, Some(package)).map(DefaultDiagram::Bound),
        ref many => Err(BindingProblem::Duplicate(many.to_vec())),
    }
}

/// Every binding problem in the store, ordered by the owner's id and then the binding's: each
/// package's, and every binding whose owner is no package.
pub fn problems(store: &Store) -> Vec<Diagnosis> {
    let mut out: Vec<Diagnosis> = store
        .alive()
        .filter(|o| store.class_of(*o).ok() == Some(PACKAGE))
        .filter_map(|package| resolve(store, package).err().map(|problem| Diagnosis { owner: Some(package), problem }))
        .collect();
    for binding in defaults::bindings(store) {
        let owner = store.get(binding, OWNER_ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o));
        // A package's own bindings were judged with it.
        if owner.is_some_and(|o| store.class_of(o).ok() == Some(PACKAGE)) {
            continue;
        }
        if let Err(problem) = target(store, binding, None) {
            out.push(Diagnosis { owner, problem });
        }
    }
    out.sort_by_key(|d| (d.owner.and_then(|o| store.uuid(o).ok()), first_binding(store, &d.problem)));
    out
}

/// The diagram `binding` names when it is a live structure diagram of `owner`, a package; an
/// owner that is none has no default, so any binding of it is wrong.
fn target(store: &Store, binding: Obj, owner: Option<Obj>) -> Result<Obj, BindingProblem> {
    let named = store.get(binding, DEFAULT_DIAGRAM).ok().and_then(|v| v.as_ref());
    let Some(diagram) = named.filter(|d| store.is_alive(*d)) else { return Err(BindingProblem::DeadTarget { binding }) };
    if !is_structure(store, diagram) {
        return Err(BindingProblem::WrongKind { binding, diagram });
    }
    let of = store.get(diagram, diagram::ELEMENT).ok().and_then(|v| v.as_ref());
    if owner.is_none() || of != owner {
        return Err(BindingProblem::WrongOwner { binding, diagram });
    }
    Ok(diagram)
}

/// Whether `d` is a UML structure diagram.
fn is_structure(store: &Store, d: Obj) -> bool {
    store.get(d, diagram::KIND).ok().is_some_and(|v| v.as_str() == Some(STRUCTURE_DIAGRAM))
}

/// `objs` in the order of their ids, so a store read back from its rows lists them alike.
fn by_id(store: &Store, mut objs: Vec<Obj>) -> Vec<Obj> {
    objs.sort_by_key(|o| store.uuid(*o).ok());
    objs
}

/// The id of the first binding `problem` names, none for a missing one.
fn first_binding(store: &Store, problem: &BindingProblem) -> Option<Uuid> {
    let binding = match problem {
        BindingProblem::Missing => None,
        BindingProblem::Duplicate(all) => all.first().copied(),
        BindingProblem::DeadTarget { binding } | BindingProblem::WrongKind { binding, .. } | BindingProblem::WrongOwner { binding, .. } => {
            Some(*binding)
        }
    };
    binding.and_then(|b| store.uuid(b).ok())
}
