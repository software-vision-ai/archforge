//! A package's default diagram, read from the one binding that names it (Decision 12, AR-575):
//! the one resolver every reader asks, which gives the bound structure diagram, says the package
//! has none yet, or names what is wrong with its binding. It never falls back to the first diagram
//! found, and it changes nothing. A profile has no default. `uml.package.default.get` answers it to
//! agents (AR-576), and `uml.package.default.set` chooses it, the one explicit way to make or mend
//! a binding (AR-580). `uml.package.diagram` gives the diagram a package opens to from it, and
//! makes none (AR-582).

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DEFAULT_DIAGRAM_BINDING, DIAGRAM, ProjectView, defaults, diagram};
use archforge_engine::{BindingFault, CommandError, Ctx, EXPECTED_REVISION, Params, Reason, expected_revision};
use archforge_mof::{Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::applications::{kind_param, name_or_id};
use crate::commands::{fail, id_of, uuid_of};
use crate::{PACKAGE, PROFILE, STRUCTURE_DIAGRAM};

/// The most bindings a refusal of a duplicate names by their ids; it counts the others.
const NAMED: usize = 5;

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

/// `uml.package.default.get`: the default diagram of the package or profile under `package`, with
/// the ids of the package, its binding and the diagram (`state` `bound`), or that the package has
/// no structure diagram yet (`no_diagram_yet`) or, a profile, no default (`does_not_apply`). Given
/// an `expected_revision`, it answers only on that revision of this session, refused as
/// [`Reason::StaleRevision`] on another. An unsound binding is refused as
/// [`Reason::InvalidBinding`], naming the ids concerned and what to do. It only reads.
pub(crate) fn default_get(ctx: &mut Ctx<'_, Store, ProjectView>, p: &Params) -> Result<Json, CommandError> {
    if p.get(EXPECTED_REVISION).is_some() {
        expected_revision(ctx, p)?;
    }
    let store = &*ctx.doc;
    let package = id_of(store, p, "package", PROFILE, "a package").or_else(|_| id_of(store, p, "package", PACKAGE, "a package"))?;
    let (state, binding, diagram) = match resolve(store, package) {
        Ok(DefaultDiagram::Bound(d)) => {
            let binding = defaults::bindings_of(store, package).first().map(|b| uuid_of(store, *b)).transpose()?;
            ("bound", binding, Some(uuid_of(store, d)?))
        }
        Ok(DefaultDiagram::NoDiagramYet) => ("no_diagram_yet", None, None),
        Ok(DefaultDiagram::DoesNotApply) => ("does_not_apply", None, None),
        Err(problem) => return Err(invalid(store, package, &problem)),
    };
    Ok(json!({ "package": uuid_of(store, package)?, "state": state, "binding": binding, "diagram": diagram }))
}

/// `uml.package.diagram`: the id of the class diagram the package under `package` opens to, its
/// default as [`resolve`] reads it from its one binding, the same target Open, keys and zoom
/// reach. It only reads and never makes a diagram: a package with no structure diagram yet is
/// refused as [`Reason::NoDiagramYet`], an unsound binding as [`Reason::InvalidBinding`] (AR-582).
pub(crate) fn entry(ctx: &mut Ctx<'_, Store, ProjectView>, p: &Params) -> Result<Json, CommandError> {
    let store = &*ctx.doc;
    let package = id_of(store, p, "package", PACKAGE, "package")?;
    match resolve(store, package) {
        Ok(DefaultDiagram::Bound(d)) => Ok(json!({ "id": uuid_of(store, d)? })),
        // A live package's default always applies, so `DoesNotApply` never comes here.
        Ok(DefaultDiagram::NoDiagramYet | DefaultDiagram::DoesNotApply) => {
            let named = name_or_id(store, package);
            let said = format!("the package {named} has no class diagram yet, so none opens; make one for it with diagram.add first");
            Err(CommandError::about("package", said).because(Reason::NoDiagramYet { package: named.into() }))
        }
        Err(problem) => Err(invalid(store, package, &problem)),
    }
}

/// `uml.package.default.set`: makes the class diagram under `diagram` the default of the package
/// under `package`, only on the `expected_revision` its caller read, refused as
/// [`Reason::StaleRevision`] on another or none. The diagram is a live structure diagram of that
/// package, each refusal typed. The package's one binding is made, as a member of the project's
/// Model, or keeps its identity and names the diagram; several are repaired, the first by id kept
/// and the others removed, in the same undo step. The diagram it names already changes nothing and
/// records no undo step. Gives the ids of the package, the binding and the diagram, and those of
/// the bindings removed.
pub(crate) fn default_set(ctx: &mut Ctx<'_, Store, ProjectView>, p: &Params) -> Result<Json, CommandError> {
    expected_revision(ctx, p)?;
    let store = &*ctx.doc;
    let package = kind_param(store, p, "package", "package", |o| store.class_of(o).ok() == Some(PACKAGE))?;
    let chosen = kind_param(store, p, "diagram", "class diagram", |d| store.class_of(d).ok() == Some(DIAGRAM) && is_structure(store, d))?;
    if store.get(chosen, diagram::ELEMENT).ok().and_then(|v| v.as_ref()) != Some(package) {
        let (d, pkg) = (name_or_id(store, chosen), name_or_id(store, package));
        let said = format!("the diagram {d} is not one of {pkg}'s own; choose one of its own structure diagrams as its default");
        return Err(CommandError::about("diagram", said).because(Reason::NotOwnDiagram { diagram: d.into(), package: pkg.into() }));
    }
    let mut extra = by_id(store, defaults::bindings_of(store, package));
    let kept = (!extra.is_empty()).then(|| extra.remove(0));
    let removed = extra.iter().map(|b| uuid_of(store, *b)).collect::<Result<Vec<_>, _>>()?;
    let binding = match kept {
        Some(b) => b,
        None => {
            let model = crate::commands::model(ctx.doc)?;
            let b = ctx.doc.create(DEFAULT_DIAGRAM_BINDING).map_err(fail)?;
            ctx.doc.set(b, OWNER_ELEMENT, Value::Ref(package)).map_err(fail)?;
            ctx.doc.push(model, archforge_core::model::MEMBER, Value::Ref(b)).map_err(fail)?;
            b
        }
    };
    // Naming the diagram it names already is no change, so it records no undo step.
    ctx.doc.set(binding, DEFAULT_DIAGRAM, Value::Ref(chosen)).map_err(fail)?;
    for b in extra {
        ctx.doc.destroy(b).map_err(fail)?;
    }
    let store = &*ctx.doc;
    Ok(json!({ "package": uuid_of(store, package)?, "binding": uuid_of(store, binding)?, "diagram": uuid_of(store, chosen)?, "removed": removed }))
}

/// `problem` with `package`'s binding as a refusal about `package`, typed by its fault: the problem,
/// the ids of the package, its bindings and the diagram concerned, and the choice that mends it,
/// made with `uml.package.default.set`.
fn invalid(store: &Store, package: Obj, problem: &BindingProblem) -> CommandError {
    let id = |o: &Obj| store.uuid(*o).map(|u| u.to_string()).unwrap_or_default();
    let mut named = format!("package {}", id(&package));
    match problem {
        BindingProblem::Missing => {}
        BindingProblem::Duplicate(all) => {
            let ids: Vec<String> = all.iter().take(NAMED).map(id).collect();
            named.push_str(&format!(", bindings {}", ids.join(", ")));
            if let Some(more) = all.len().checked_sub(NAMED).filter(|n| *n > 0) {
                named.push_str(&format!(" and {more} more"));
            }
        }
        BindingProblem::DeadTarget { binding } => named.push_str(&format!(", binding {}", id(binding))),
        BindingProblem::WrongKind { binding, diagram } | BindingProblem::WrongOwner { binding, diagram } => {
            named.push_str(&format!(", binding {}, diagram {}", id(binding), id(diagram)));
        }
    }
    let fault = fault_of(problem);
    let mend = match fault {
        BindingFault::Duplicate => {
            "choose which of its structure diagrams stays its default with uml.package.default.set, which removes the other bindings"
        }
        _ => "choose one of its own structure diagrams as its default with uml.package.default.set",
    };
    CommandError::about("package", format!("{problem} ({named}); {mend}")).because(Reason::InvalidBinding(fault))
}

/// The engine's [`BindingFault`] for `problem`.
fn fault_of(problem: &BindingProblem) -> BindingFault {
    match problem {
        BindingProblem::Missing => BindingFault::Missing,
        BindingProblem::Duplicate(_) => BindingFault::Duplicate,
        BindingProblem::DeadTarget { .. } => BindingFault::DeadTarget,
        BindingProblem::WrongKind { .. } => BindingFault::WrongKind,
        BindingProblem::WrongOwner { .. } => BindingFault::WrongOwner,
    }
}

/// Why the nested package `pkg` cannot hand its structure diagrams up to `parent`, the package that
/// takes them as `pkg` is removed (AR-673): `parent`'s default is unsound, or it has no structure
/// diagram yet, so they would land in a package whose default no one binding gives. Refused about
/// `ids`, typed as [`Reason::ParentDefaultUnsound`] with `parent`'s name or id and its fault, none
/// for no diagram yet. None when `pkg` hands up no structure diagram, or `parent`'s default is
/// sound; `parent`'s binding and `pkg`'s are judged, never changed.
pub(crate) fn handoff_problem(store: &Store, pkg: Obj, parent: Obj) -> Option<CommandError> {
    if !archforge_diagram::views_of(store, pkg).0.into_iter().any(|d| is_structure(store, d)) {
        return None;
    }
    let named = name_or_id(store, parent);
    let (fault, said) = match resolve(store, parent) {
        Ok(DefaultDiagram::Bound(_) | DefaultDiagram::DoesNotApply) => return None,
        Ok(DefaultDiagram::NoDiagramYet) => (
            None,
            format!("its diagrams cannot move up: {named} has no structure diagram yet to be its default; make one for it with diagram.add first"),
        ),
        Err(problem) => (
            Some(fault_of(&problem)),
            format!(
                "its diagrams cannot move up: the default diagram of {named} is unsound ({problem}); choose one of its own structure diagrams as its default with uml.package.default.set first"
            ),
        ),
    };
    Some(CommandError::about("ids", said).because(Reason::ParentDefaultUnsound { parent: named.into(), fault }))
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
