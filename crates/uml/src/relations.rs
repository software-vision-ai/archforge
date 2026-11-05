//! Relating classes and interfaces: associations, generalizations,
//! dependencies and interface realizations drawn between them, an association's ends set (role, multiplicity, navigability, aggregation),
//! any of them reconnected to another class, and the relationships that touch a class listed so
//! removing the class can say them and take them along.
//!
//! An association owns both its ends; navigability is marked by the association listing the end
//! as navigable, so relating classes never changes a class's attributes (owner decision). A new
//! association is a plain line: no roles, no multiplicities, navigable to neither end (owner
//! decision); a directed one is navigable to its second class only. Aggregation and composition sit on the
//! part's end, the line drawn from the whole to the part, and the diamond shows at the whole.
//!
//! What would make the model wrong is refused with a reason before anything changes: an interface
//! that specializes anything but an interface, a class that specializes an interface, a classifier
//! that specializes itself, directly or round a cycle, the same generalization twice, a class depending
//! on itself, both ends of an association aggregating, a composition whose whole's end allows more
//! than one, two ends of one association with the same role, and an interface realization from
//! anything but a class, to anything but an interface, or from a class to an interface it realizes
//! already. A realization is owned by its implementing class, as UML has it.

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, Value};
use serde_json::{Value as Json, json};

use crate::commands::{a_what, classifier_of, fail, id_of, name_of, package_of, refuse, set_name, text, uuid_of, what};
use crate::{
    ASSOCIATION, CLASS, DEPENDENCY, GENERALIZATION, INTERFACE, INTERFACE_REALIZATION, PROPERTY, association, behaviored_classifier, classifier,
    dependency, generalization, interface_realization, package, property, typed_element,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The kinds of association a line draws, as the commands name them.
pub const ASSOCIATION_KINDS: [&str; 4] = ["association", "directed", "aggregation", "composition"];
/// An end's aggregation, in the metamodel's order, as the commands name it.
const AGGREGATIONS: [&str; 3] = ["none", "shared", "composite"];
/// Why a command is not available.
pub const NO_CLASSES: &str = crate::members::NO_CLASSES;

/// Whether a class or an interface exists, which relating needs.
fn has_classes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(crate::is_classifier)) { Ok(()) } else { Err(NO_CLASSES) }
}

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// The classes `c` specializes, directly.
fn generals(store: &Store, c: Obj) -> Vec<Obj> {
    list(store, c, classifier::GENERALIZATION).into_iter().filter_map(|g| one(store, g, generalization::GENERAL)).collect()
}

/// Whether `from` is `to` or specializes it, directly or through other classes.
fn specializes(store: &Store, from: Obj, to: Obj) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![from];
    while let Some(c) = stack.pop() {
        if c == to {
            return true;
        }
        if seen.insert(c) {
            stack.extend(generals(store, c));
        }
    }
    false
}

/// The aggregation of an association's end, as an index into [`AGGREGATIONS`].
fn aggregation(store: &Store, end: Obj) -> usize {
    match store.get(end, property::AGGREGATION) {
        Ok(Value::Enum(i)) => (i as usize).min(AGGREGATIONS.len() - 1),
        _ => 0,
    }
}

/// The upper bound of an end's multiplicity: none for many, one when it names none.
fn upper(store: &Store, end: Obj) -> Option<u64> {
    match one(store, end, crate::multiplicity_element::UPPER_VALUE).map(|u| store.get(u, crate::literal_unlimited_natural::VALUE)) {
        Some(Ok(Value::Unlimited(n))) => n,
        _ => Some(1),
    }
}

/// The relationships that touch class `c`: the associations with an end typed by it, the
/// generalizations to and from it, the dependencies it is a client or supplier of, and the
/// interface realizations it implements or is the contract of; each once.
pub fn relationships_of(store: &Store, c: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    for (src, prop) in store.referrers(c) {
        let r = if prop == typed_element::TYPE && store.class_of(src).ok() == Some(PROPERTY) {
            // Its association, or the association that owns it.
            let owner = store.container(src).ok().flatten().map(|(o, _)| o).filter(|o| store.class_of(*o).ok() == Some(ASSOCIATION));
            one(store, src, property::ASSOCIATION).or(owner)
        } else if [
            generalization::GENERAL,
            dependency::CLIENT,
            dependency::SUPPLIER,
            interface_realization::CONTRACT,
            interface_realization::IMPLEMENTING_CLASSIFIER,
        ]
        .contains(&prop)
        {
            Some(src)
        } else {
            None
        };
        out.extend(r.filter(|r| store.is_alive(*r)));
    }
    out.extend(list(store, c, classifier::GENERALIZATION));
    out.extend(list(store, c, behaviored_classifier::INTERFACE_REALIZATION));
    out.sort();
    out.dedup();
    out
}

/// The class or interface under `key`, which must sit in a package.
fn class_in_package(store: &Store, p: &Params, key: &str) -> Result<(Obj, Obj), CommandError> {
    let c = classifier_of(store, p, key)?;
    let pkg = package_of(store, c).ok_or_else(|| refuse(key, format!("the {} is in no package", what(store, c))))?;
    Ok((c, pkg))
}

/// A relationship's name: empty, or a name.
fn check_name(name: &str) -> Result<(), CommandError> {
    if name.is_empty() {
        return Ok(());
    }
    match archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        Some(p) => Err(refuse("name", format!("the name {p}"))),
        None => Ok(()),
    }
}

/// Lists `end` among the association's navigable ends, or takes it off.
fn set_navigable(store: &mut Store, a: Obj, end: Obj, navigable: bool) -> Result<(), CommandError> {
    let listed = list(store, a, association::NAVIGABLE_OWNED_END).contains(&end);
    if navigable && !listed {
        store.push(a, association::NAVIGABLE_OWNED_END, Value::Ref(end)).map_err(fail)?;
    } else if !navigable && listed {
        // Taking an end off the navigable ends takes it off the lists those subset too, the owned
        // and the member ends; it goes back to its place there.
        let at = store.list(a, association::MEMBER_END).map_err(fail)?.iter().position(|v| *v == Value::Ref(end)).unwrap_or(0);
        store.remove_value(a, association::NAVIGABLE_OWNED_END, &Value::Ref(end)).map_err(fail)?;
        if !store.list(a, association::MEMBER_END).map_err(fail)?.contains(&Value::Ref(end)) {
            store.insert(a, association::MEMBER_END, at as u32, Value::Ref(end)).map_err(fail)?;
        }
        if !store.list(a, association::OWNED_END).map_err(fail)?.contains(&Value::Ref(end)) {
            store.push(a, association::OWNED_END, Value::Ref(end)).map_err(fail)?;
        }
    }
    Ok(())
}

/// `uml.association.add`: an association of the kind asked for between two classifiers, owning both ends.
fn association_add(ctx: &mut C<'_>, p: &Params) -> R {
    let (from, pkg) = class_in_package(ctx.doc, p, "from")?;
    let to = classifier_of(ctx.doc, p, "to")?;
    let kind = match text(p, "kind") {
        "" => "association",
        k if ASSOCIATION_KINDS.contains(&k) => k,
        k => return Err(refuse("kind", format!("{k:?} is not one of {}", ASSOCIATION_KINDS.join(", ")))),
    };
    let name = text(p, "name");
    check_name(name)?;
    let a = ctx.doc.create(ASSOCIATION).map_err(fail)?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(a)).map_err(fail)?;
    let mut ends = Vec::new();
    for ty in [from, to] {
        let end = ctx.doc.create(PROPERTY).map_err(fail)?;
        ctx.doc.set(end, typed_element::TYPE, Value::Ref(ty)).map_err(fail)?;
        // A member end first, which tells the end its association, then owned by it (owned
        // ends subset the member ends; pushing only the owned end leaves the end not knowing).
        ctx.doc.push(a, association::MEMBER_END, Value::Ref(end)).map_err(fail)?;
        if !ctx.doc.list(a, association::OWNED_END).map_err(fail)?.contains(&Value::Ref(end)) {
            ctx.doc.push(a, association::OWNED_END, Value::Ref(end)).map_err(fail)?;
        }
        ends.push(end);
    }
    // Navigable to neither end, or only to the second class; the part's end aggregates.
    set_navigable(ctx.doc, a, ends[0], false)?;
    set_navigable(ctx.doc, a, ends[1], kind == "directed")?;
    let aggregation = match kind {
        "aggregation" => 1,
        "composition" => 2,
        _ => 0,
    };
    ctx.doc.set(ends[1], property::AGGREGATION, Value::Enum(aggregation)).map_err(fail)?;
    ctx.view.selection = vec![a];
    Ok(json!({ "id": uuid_of(ctx.doc, a)?, "ends": [uuid_of(ctx.doc, ends[0])?, uuid_of(ctx.doc, ends[1])?] }))
}

/// Why `specific` may not specialize `general`, when it may not: an interface specializes only an
/// interface, and a class never specializes one.
pub fn generalization_problem(store: &Store, specific: Obj, general: Obj, except: Option<Obj>) -> Option<&'static str> {
    let interface = |o: Obj| store.class_of(o).ok() == Some(INTERFACE);
    match (interface(specific), interface(general)) {
        (true, false) => return Some("an interface specializes only an interface"),
        (false, true) => return Some("a class cannot specialize an interface"),
        _ => {}
    }
    if specific == general {
        return Some(if interface(specific) { "an interface cannot specialize itself" } else { "a class cannot specialize itself" });
    }
    if list(store, specific, classifier::GENERALIZATION)
        .into_iter()
        .any(|g| Some(g) != except && one(store, g, generalization::GENERAL) == Some(general))
    {
        return Some(if interface(specific) {
            "the interface specializes that interface already"
        } else {
            "the class specializes that class already"
        });
    }
    if specializes(store, general, specific) {
        return Some(if interface(specific) {
            "that interface specializes this one already, which would make a cycle"
        } else {
            "that class specializes this one already, which would make a cycle"
        });
    }
    None
}

/// `uml.generalization.add`: `specific` specializes `general`, unless the classifier rules refuse it.
fn generalization_add(ctx: &mut C<'_>, p: &Params) -> R {
    let specific = classifier_of(ctx.doc, p, "specific")?;
    let general = classifier_of(ctx.doc, p, "general")?;
    if let Some(why) = generalization_problem(ctx.doc, specific, general, None) {
        return Err(refuse("general", why));
    }
    let g = ctx.doc.create(GENERALIZATION).map_err(fail)?;
    ctx.doc.set(g, generalization::GENERAL, Value::Ref(general)).map_err(fail)?;
    ctx.doc.push(specific, classifier::GENERALIZATION, Value::Ref(g)).map_err(fail)?;
    ctx.view.selection = vec![g];
    Ok(json!({ "id": uuid_of(ctx.doc, g)? }))
}

/// `uml.dependency.add`: a client depends on a supplier, never on itself.
fn dependency_add(ctx: &mut C<'_>, p: &Params) -> R {
    let (client, pkg) = class_in_package(ctx.doc, p, "client")?;
    let supplier = classifier_of(ctx.doc, p, "supplier")?;
    if client == supplier {
        return Err(refuse("supplier", format!("{} cannot depend on itself", a_what(ctx.doc, client))));
    }
    let name = text(p, "name");
    check_name(name)?;
    let d = ctx.doc.create(DEPENDENCY).map_err(fail)?;
    set_name(ctx.doc, d, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(d)).map_err(fail)?;
    ctx.doc.push(d, dependency::CLIENT, Value::Ref(client)).map_err(fail)?;
    ctx.doc.push(d, dependency::SUPPLIER, Value::Ref(supplier)).map_err(fail)?;
    ctx.view.selection = vec![d];
    Ok(json!({ "id": uuid_of(ctx.doc, d)? }))
}

/// Why a classifier may not realize another: what a realization from anything but a class, to
/// anything but an interface, or a second one between the same two would be refused for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealizationProblem {
    /// Only a class implements: UML's implementing classifier is behaviored.
    NotAClass,
    /// Only an interface is a contract.
    NotAnInterface,
    /// The class realizes that interface already.
    Again,
}

impl RealizationProblem {
    /// The parameter of `uml.realization.add` a refusal names.
    pub fn key(self) -> &'static str {
        match self {
            RealizationProblem::NotAClass => "class",
            RealizationProblem::NotAnInterface | RealizationProblem::Again => "interface",
        }
    }

    /// Why, as a refusal says it.
    pub fn why(self) -> &'static str {
        match self {
            RealizationProblem::NotAClass => "only a class implements an interface",
            RealizationProblem::NotAnInterface => "a class realizes only an interface",
            RealizationProblem::Again => "the class realizes that interface already",
        }
    }
}

/// Why `implementing` may not realize `contract`, when it may not: only a class implements, only an
/// interface is a contract, and a class realizes an interface once. `except` is a realization being
/// reconnected, which never clashes with itself.
pub fn realization_problem(store: &Store, implementing: Obj, contract: Obj, except: Option<Obj>) -> Option<RealizationProblem> {
    if store.class_of(implementing).ok() != Some(CLASS) {
        return Some(RealizationProblem::NotAClass);
    }
    if store.class_of(contract).ok() != Some(INTERFACE) {
        return Some(RealizationProblem::NotAnInterface);
    }
    let again = list(store, implementing, behaviored_classifier::INTERFACE_REALIZATION)
        .into_iter()
        .any(|r| Some(r) != except && one(store, r, interface_realization::CONTRACT) == Some(contract));
    again.then_some(RealizationProblem::Again)
}

/// Keeps a realization's client and supplier, the dependency's ends its implementing classifier and
/// contract subset, holding exactly those two: the store carries a list into the lists it subsets,
/// but not a single reference.
fn mirror_ends(store: &mut Store, r: Obj) -> Result<(), CommandError> {
    let ends = [
        (dependency::CLIENT, one(store, r, interface_realization::IMPLEMENTING_CLASSIFIER)),
        (dependency::SUPPLIER, one(store, r, interface_realization::CONTRACT)),
    ];
    for (prop, want) in ends {
        for old in list(store, r, prop).into_iter().filter(|o| Some(*o) != want) {
            store.remove_value(r, prop, &Value::Ref(old)).map_err(fail)?;
        }
        if let Some(w) = want
            && !list(store, r, prop).contains(&w)
        {
            store.push(r, prop, Value::Ref(w)).map_err(fail)?;
        }
    }
    Ok(())
}

/// `uml.realization.add`: a class realizes an interface, the realization owned by the class.
fn realization_add(ctx: &mut C<'_>, p: &Params) -> R {
    let implementing = classifier_of(ctx.doc, p, "class")?;
    let contract = classifier_of(ctx.doc, p, "interface")?;
    if let Some(problem) = realization_problem(ctx.doc, implementing, contract, None) {
        return Err(refuse(problem.key(), problem.why()));
    }
    let r = ctx.doc.create(INTERFACE_REALIZATION).map_err(fail)?;
    ctx.doc.set(r, interface_realization::CONTRACT, Value::Ref(contract)).map_err(fail)?;
    // Owned by the class, which makes the class its implementing classifier and its client.
    ctx.doc.push(implementing, behaviored_classifier::INTERFACE_REALIZATION, Value::Ref(r)).map_err(fail)?;
    mirror_ends(ctx.doc, r)?;
    ctx.view.selection = vec![r];
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// The association or dependency under `key`.
fn named_relationship(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let r = id_of(store, p, key, ASSOCIATION, "an association").or_else(|_| id_of(store, p, key, DEPENDENCY, "a dependency"));
    r.map_err(|_| refuse(key, "it is no association or dependency"))
}

fn relationship_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let r = named_relationship(ctx.doc, p, "relationship")?;
    let name = text(p, "name");
    check_name(name)?;
    set_name(ctx.doc, r, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// An association's two ends, in its member order.
fn ends_of(store: &Store, a: Obj, key: &str) -> Result<[Obj; 2], CommandError> {
    crate::figures::member_ends(store, a).ok_or_else(|| refuse(key, "the association has not two ends"))
}

/// The end an end owned by a class becomes: the association's own, as every end drawn here is.
fn own_end(store: &mut Store, a: Obj, end: Obj) -> Result<(), CommandError> {
    if let Some((owner, prop)) = store.container(end).map_err(fail)?
        && owner != a
    {
        // An end its class owned was navigable; it stays so, now marked on the association.
        store.remove_value(owner, prop, &Value::Ref(end)).map_err(fail)?;
        store.push(a, association::OWNED_END, Value::Ref(end)).map_err(fail)?;
        set_navigable(store, a, end, true)?;
    }
    Ok(())
}

fn association_set_end(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx.doc, p, "association", ASSOCIATION, "an association")?;
    let ends = ends_of(ctx.doc, a, "association")?;
    let i = match p.integer("end") {
        Some(i @ 0..=1) => i as usize,
        _ => return Err(refuse("end", "the end is 0 or 1")),
    };
    let (end, other) = (ends[i], ends[1 - i]);
    let role = text(p, "role");
    if let Some(problem) = (!role.is_empty()).then(|| archforge_core::skills::name_problem(role, crate::commands::MAX_NAME)).flatten() {
        return Err(refuse("role", format!("the role {problem}")));
    }
    if !role.is_empty() && name_of(ctx.doc, other).to_lowercase() == role.to_lowercase() {
        return Err(refuse("role", format!("the other end's role is {role} already")));
    }
    let kind = match text(p, "aggregation") {
        "" => 0,
        k => AGGREGATIONS
            .iter()
            .position(|w| *w == k)
            .ok_or_else(|| refuse("aggregation", format!("{k:?} is not one of {}", AGGREGATIONS.join(", "))))?,
    };
    if kind != 0 && aggregation(ctx.doc, other) != 0 {
        return Err(refuse("aggregation", "the other end aggregates already: only the part's end can"));
    }
    let typed = text(p, "multiplicity");
    let bounds = crate::members::parse_multiplicity(typed).map_err(|why| refuse("multiplicity", format!("the multiplicity {why}")))?;
    // A part belongs to at most one whole: the whole's end allows one at most.
    if kind == 2 && upper(ctx.doc, other).is_none_or(|u| u > 1) {
        return Err(refuse("aggregation", "a composition's whole end allows one at most; set the other end's multiplicity to 1 or 0..1 first"));
    }
    if aggregation(ctx.doc, other) == 2 && bounds.is_some_and(|(_, u)| u.is_none_or(|u| u > 1)) {
        return Err(refuse("multiplicity", "this end is a composition's whole, which allows one at most"));
    }
    // The whole's end, where the diamond sits, is never navigable (owner decision).
    let whole = aggregation(ctx.doc, other) != 0;
    let navigable = p.bool("navigable").unwrap_or(!whole);
    if whole && navigable {
        return Err(refuse("navigable", "the whole's end of an aggregation or composition is not navigable"));
    }
    own_end(ctx.doc, a, end)?;
    set_name(ctx.doc, end, role)?;
    crate::members::set_multiplicity(ctx.doc, end, typed, "multiplicity")?;
    set_navigable(ctx.doc, a, end, navigable)?;
    ctx.doc.set(end, property::AGGREGATION, Value::Enum(kind as u32)).map_err(fail)?;
    // This end aggregating makes the other end the whole's: it stops being navigable.
    if kind != 0 {
        own_end(ctx.doc, a, other)?;
        set_navigable(ctx.doc, a, other, false)?;
    }
    Ok(json!({ "id": uuid_of(ctx.doc, end)? }))
}

/// Moves the drawn lines of `element` to the nodes of its classes on each diagram; a diagram that
/// does not show a class the element now joins loses the line.
fn follow_ends(store: &mut Store, element: Obj) -> Result<(), CommandError> {
    let Some((from, to)) = crate::figures::ends(store, element) else { return Ok(()) };
    let (_, edges, _) = archforge_diagram::views_of(store, element);
    for e in edges {
        let Some((d, _)) = store.container(e).map_err(fail)? else { continue };
        let node_of = |c: Obj| {
            list(store, d, archforge_diagram::diagram::NODE).into_iter().find(|n| one(store, *n, archforge_diagram::node::ELEMENT) == Some(c))
        };
        let (Some(source), Some(target)) = (node_of(from), node_of(to)) else {
            archforge_diagram::destroy_edge_with_links(store, e).map_err(fail)?;
            continue;
        };
        if one(store, e, archforge_diagram::edge::SOURCE) == Some(source) && one(store, e, archforge_diagram::edge::TARGET) == Some(target) {
            continue;
        }
        store.set(e, archforge_diagram::edge::SOURCE, Value::Ref(source)).map_err(fail)?;
        store.set(e, archforge_diagram::edge::TARGET, Value::Ref(target)).map_err(fail)?;
        // Its bends were for the old ends.
        for w in list(store, e, archforge_diagram::edge::WAYPOINT) {
            store.destroy(w).map_err(fail)?;
        }
    }
    Ok(())
}

/// `uml.relationship.reconnect`: one end of a relationship moved to another classifier, its rules kept and its lines following.
fn relationship_reconnect(ctx: &mut C<'_>, p: &Params) -> R {
    let key = "relationship";
    let id = text(p, key);
    let r = crate::RELATIONSHIPS.into_iter().find_map(|k| id_of(ctx.doc, p, key, k, "").ok()).ok_or_else(|| {
        refuse(key, format!("{} is no association, generalization, dependency or interface realization", crate::commands::short(id)))
    })?;
    let source = match text(p, "end") {
        "source" => true,
        "target" => false,
        e => return Err(refuse("end", format!("{e:?} is not source or target"))),
    };
    let c = classifier_of(ctx.doc, p, "class")?;
    let class = ctx.doc.class_of(r).map_err(fail)?;
    if class == ASSOCIATION {
        let ends = ends_of(ctx.doc, r, key)?;
        let end = ends[if source { 0 } else { 1 }];
        own_end(ctx.doc, r, end)?;
        ctx.doc.set(end, typed_element::TYPE, Value::Ref(c)).map_err(fail)?;
    } else if class == GENERALIZATION {
        let (specific, general) = crate::figures::ends(ctx.doc, r).ok_or_else(|| refuse(key, "the generalization joins no two classifiers"))?;
        let (s, g) = if source { (c, general) } else { (specific, c) };
        if let Some(why) = generalization_problem(ctx.doc, s, g, Some(r)) {
            return Err(refuse("class", why));
        }
        if source && s != specific {
            ctx.doc.remove_value(specific, classifier::GENERALIZATION, &Value::Ref(r)).map_err(fail)?;
            ctx.doc.push(s, classifier::GENERALIZATION, Value::Ref(r)).map_err(fail)?;
        }
        ctx.doc.set(r, generalization::GENERAL, Value::Ref(g)).map_err(fail)?;
    } else if class == INTERFACE_REALIZATION {
        let (implementing, contract) =
            crate::figures::ends(ctx.doc, r).ok_or_else(|| refuse(key, "the interface realization joins no class and interface"))?;
        let (i, c) = if source { (c, contract) } else { (implementing, c) };
        // The end that stays may be wrong already (a project edited by hand): that is the
        // realization's fault, not the classifier the moved end was dropped on.
        match realization_problem(ctx.doc, i, c, Some(r)) {
            Some(RealizationProblem::NotAClass) if !source => {
                return Err(refuse(key, "the realization's existing implementing end is not a class"));
            }
            Some(RealizationProblem::NotAnInterface) if source => {
                return Err(refuse(key, "the realization's existing contract end is not an interface"));
            }
            Some(problem) => return Err(refuse("class", problem.why())),
            None => {}
        }
        if i != implementing {
            ctx.doc.remove_value(implementing, behaviored_classifier::INTERFACE_REALIZATION, &Value::Ref(r)).map_err(fail)?;
            ctx.doc.push(i, behaviored_classifier::INTERFACE_REALIZATION, Value::Ref(r)).map_err(fail)?;
        }
        ctx.doc.set(r, interface_realization::CONTRACT, Value::Ref(c)).map_err(fail)?;
        mirror_ends(ctx.doc, r)?;
    } else {
        let (client, supplier) = crate::figures::ends(ctx.doc, r).ok_or_else(|| refuse(key, "the dependency joins no two classifiers"))?;
        let (cl, su) = if source { (c, supplier) } else { (client, c) };
        if cl == su {
            return Err(refuse("class", format!("{} cannot depend on itself", a_what(ctx.doc, cl))));
        }
        let (prop, old) = if source { (dependency::CLIENT, client) } else { (dependency::SUPPLIER, supplier) };
        ctx.doc.remove_value(r, prop, &Value::Ref(old)).map_err(fail)?;
        ctx.doc.push(r, prop, Value::Ref(c)).map_err(fail)?;
    }
    follow_ends(ctx.doc, r)?;
    ctx.view.selection = vec![r];
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// A relationship as the query commands give it.
fn describe(store: &Store, r: Obj) -> Json {
    let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
    let (from, to) = crate::figures::ends(store, r).map_or((Json::Null, Json::Null), |(a, b)| (json!(id(a)), json!(id(b))));
    match store.class_of(r).ok() {
        Some(c) if c == ASSOCIATION => {
            let ends: Vec<Json> = crate::figures::member_ends(store, r)
                .map(|ends| {
                    ends.iter()
                        .map(|e| {
                            json!({
                                "id": id(*e),
                                "class": one(store, *e, typed_element::TYPE).map_or(Json::Null, |t| json!(id(t))),
                                "role": name_of(store, *e),
                                "multiplicity": crate::figures::multiplicity(store, *e),
                                "navigable": crate::figures::navigable(store, r, *e),
                                "aggregation": AGGREGATIONS[aggregation(store, *e)],
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({ "id": id(r), "kind": "association", "name": name_of(store, r), "from": from, "to": to, "ends": ends })
        }
        Some(c) if c == GENERALIZATION => json!({ "id": id(r), "kind": "generalization", "specific": from, "general": to }),
        Some(c) if c == INTERFACE_REALIZATION => json!({ "id": id(r), "kind": "realization", "implementing": from, "contract": to }),
        _ => json!({ "id": id(r), "kind": "dependency", "name": name_of(store, r), "client": from, "supplier": to }),
    }
}

/// `uml.relationship.list`: the relationships that touch a class or an interface.
fn relationship_list(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let c = classifier_of(store, p, "class")?;
    Ok(json!({ "relationships": relationships_of(store, c).into_iter().map(|r| describe(store, r)).collect::<Vec<_>>() }))
}

macro_rules! end_lines {
    () => {
        concat!(
            "association: string; the association's id\n",
            "end: integer, 0..1; which end, 0 at the class or interface it was drawn from, 1 at the other\n",
            "role: string = \"\"; the end's role name, or nothing\n",
            "multiplicity: string = \"\"; 1, 0..1, *, 1..*, 2..5, or nothing\n",
            "navigable: bool = true; the association navigates to this end; never the whole's end of an aggregation or composition (default false there)\n",
            "aggregation: string = \"none\", none|shared|composite; this end is a part of the other end's class, shared or composed"
        )
    };
}

/// Relating classes' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.association.add",
            "Add association",
            &["Association"],
            None,
            concat!(
                "from: string; the id of the class or interface it is drawn from, the whole of an aggregation or composition\n",
                "to: string; the id of the class or interface it is drawn to, the part of an aggregation or composition\n",
                "kind: string = \"association\", association|directed|aggregation|composition; directed navigates only to the second class\n",
                "name: string = \"\"; the association's name, or nothing"
            ),
            has_classes,
            association_add
        ),
        cmd!("uml.association.set-end", "Set association end", &["Association"], None, end_lines!(), has_classes, association_set_end),
        cmd!(
            "uml.generalization.add",
            "Add generalization",
            &["Generalization"],
            None,
            concat!(
                "specific: string; the id of the class or interface that specializes\n",
                "general: string; the id of the class or interface it specializes: an interface specializes only an interface, and a class never specializes one"
            ),
            has_classes,
            generalization_add
        ),
        cmd!(
            "uml.dependency.add",
            "Add dependency",
            &["Dependency"],
            None,
            "client: string; the id of the class or interface that depends\nsupplier: string; the id of the class or interface it depends on\nname: string = \"\"; the dependency's name, or nothing",
            has_classes,
            dependency_add
        ),
        cmd!(
            "uml.realization.add",
            "Add interface realization",
            &["Interface realization"],
            None,
            concat!(
                "class: string; the id of the class that implements the interface, its implementing classifier; only a class implements\n",
                "interface: string; the id of the interface it implements, its contract; a class realizes an interface once"
            ),
            has_classes,
            realization_add
        ),
        cmd!(
            "uml.relationship.rename",
            "Rename relationship",
            &["Association", "Dependency"],
            None,
            "relationship: string; the association's or dependency's id\nname: string; the new name, or nothing",
            has_classes,
            relationship_rename
        ),
        cmd!(
            "uml.relationship.reconnect",
            "Reconnect relationship",
            &["Association", "Generalization", "Dependency", "Interface realization"],
            None,
            concat!(
                "relationship: string; the association's, generalization's, dependency's or interface realization's id\n",
                "end: string, source|target; the end to move: the class or interface it was drawn from (the specific one, the client, the implementing class) or to\n",
                "class: string; the id of the class or interface the end moves to; a generalization's end keeps its rules: an interface specializes only an interface, a class never one; a realization's source is a class and its target an interface, the class realizing it once"
            ),
            has_classes,
            relationship_reconnect
        ),
        cmd!(query "uml.relationship.list", "List relationships", &["Association", "Generalization", "Dependency", "Interface realization"], None, "class: string; the class's or interface's id", has_classes, relationship_list),
    ]
}
