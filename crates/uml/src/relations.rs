//! Relating classes, interfaces, enumerations, data types and primitive types: associations, generalizations,
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
//! that specializes anything but an interface, a class that specializes an interface, an
//! enumeration that specializes anything but an enumeration or is specialized by anything else, a
//! data type that specializes anything but a data type or is specialized by anything else, a
//! primitive type likewise, a built-in type that would specialize anything, an
//! association to or from an enumeration, a data type or a primitive type (whose values are no
//! instances to link), a classifier
//! that specializes itself, directly or round a cycle, a classifier that specializes a final one (UML's
//! `isFinalSpecialization`, set with `uml.classifier.set`), the same generalization twice, a class depending
//! on itself, both ends of an association aggregating, a composition whose whole's end allows more
//! than one, two ends of one association with the same role, and an interface realization from
//! anything but a class, to anything but an interface, or from a class to an interface it realizes
//! already. A realization is owned by its implementing class, as UML has it.
//!
//! A classifier of any kind may be made final (AR-137): nothing may specialize it then, so a new
//! generalization to it, or one whose general end moves onto it, is refused; it is made final only
//! while nothing specializes it, the refusal naming the classifiers that do. Clearing it changes
//! nothing else. It is UML's `isFinalSpecialization`, apart from a leaf element's `isLeaf`.
//!
//! A classifier, a property or an operation may be made leaf (AR-138), UML's `isLeaf`: nothing
//! may redefine it then. It is made leaf only while nothing redefines it, the refusal naming what
//! does ([`leaf_problem`]); a property that would redefine a leaf one is refused by the modifiers'
//! rules. Clearing it changes nothing else. A classifier's is set with `uml.classifier.set`, a
//! property's with the attribute commands and `uml.association.set-end`, an operation's with the
//! operation commands.
//!
//! An association class (AR-125) is one element, UML's AssociationClass: a class and an
//! association at once. It is made between two classes as a plain association is, owning both its
//! ends, and its ends are set and reconnected as an association's are; it has attributes and
//! operations as a class has (`members`), and it is named among its package's classifiers. Its ends
//! are classes only: an interface, an enumeration, a data type or a primitive type is refused, and
//! so is the association class itself. As the class it is, it implements interfaces and takes
//! part in other relationships; nothing specializes it, since a plain class may not specialize an
//! association class in UML and another association class would bind its ends to these.

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, Value};
use serde_json::{Value as Json, json};

use crate::commands::{a_what, classifier_of, fail, id_of, name_of, refuse, set_name, text, uuid_of, what};
use crate::{
    ASSOCIATION, ASSOCIATION_CLASS, CLASS, DATA_TYPE, DEPENDENCY, ENUMERATION, GENERALIZATION, INTERFACE, INTERFACE_REALIZATION, PRIMITIVE_TYPE,
    PROPERTY, association, behaviored_classifier, classifier, dependency, generalization, interface_realization, package, property, typed_element,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The kinds of association a line draws, as the commands name them.
pub const ASSOCIATION_KINDS: [&str; 4] = ["association", "directed", "aggregation", "composition"];
/// An end's aggregation, in the metamodel's order, as the commands name it.
const AGGREGATIONS: [&str; 3] = ["none", "shared", "composite"];
/// Why a command is not available.
pub const NO_CLASSES: &str = crate::members::NO_CLASSES;

/// Whether a class, an interface, an enumeration, a data type or a primitive type exists, which
/// relating needs.
fn has_classes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(crate::is_classifier)) { Ok(()) } else { Err(NO_CLASSES) }
}

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// The classifiers `c` specializes, directly.
pub fn generals(store: &Store, c: Obj) -> Vec<Obj> {
    list(store, c, classifier::GENERALIZATION).into_iter().filter_map(|g| one(store, g, generalization::GENERAL)).collect()
}

/// Whether `from` is `to` or specializes it, directly or through other classifiers; a seen-set
/// bounds the walk.
pub fn specializes(store: &Store, from: Obj, to: Obj) -> bool {
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

/// Whether nothing may specialize the classifier `c`: UML's `isFinalSpecialization` (AR-137).
pub fn is_final(store: &Store, c: Obj) -> bool {
    store.get(c, classifier::IS_FINAL_SPECIALIZATION).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Whether nothing may redefine the classifier, property or operation `o`: UML's `isLeaf`
/// (AR-138), apart from a classifier's `isFinalSpecialization`.
pub fn is_leaf(store: &Store, o: Obj) -> bool {
    store.get(o, crate::redefinable_element::IS_LEAF).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// The elements that redefine `o` directly: the properties that list it among the properties they
/// redefine, the operations among the operations, the classifiers among the classifiers; each
/// once, in a stable order.
pub fn redefiners(store: &Store, o: Obj) -> Vec<Obj> {
    let by = [property::REDEFINED_PROPERTY, crate::operation::REDEFINED_OPERATION, classifier::REDEFINED_CLASSIFIER];
    let mut out: Vec<Obj> =
        store.referrers(o).into_iter().filter(|(r, prop)| by.contains(prop) && *r != o && store.is_alive(*r)).map(|(r, _)| r).collect();
    out.sort();
    out.dedup();
    out
}

/// An element that redefines another, as a refusal names it: a property and an operation by
/// their classifier and name (`CardPayment::amount`, `CardPayment::authorise()`), a classifier by
/// its name.
pub fn redefiner_name(store: &Store, o: Obj) -> String {
    let own = name_of(store, o);
    if store.class_of(o).is_ok_and(crate::is_classifier) {
        return own;
    }
    let owner = match store.class_of(o).ok() {
        Some(k) if k == PROPERTY => crate::modifiers::context_of(store, o).first().copied(),
        _ => store.container(o).ok().flatten().map(|(c, _)| c),
    };
    let own = if store.class_of(o).ok() == Some(crate::OPERATION) { format!("{own}()") } else { own };
    match owner {
        Some(c) => format!("{}::{own}", name_of(store, c)),
        None => own,
    }
}

/// Why `o`, a classifier, a property or an operation, may not be made leaf, when it may not: a
/// built-in type is never changed, and an element something redefines already would break the
/// rule at once; the refusal names those elements and asks for their redefinitions to go first.
pub fn leaf_problem(store: &Store, o: Obj) -> Option<String> {
    let id = uuid_of(store, o).unwrap_or_default();
    if crate::members::is_built_in(store, o) {
        return Some(format!("{id} is the built-in type {}, which is never made leaf", name_of(store, o)));
    }
    let by = redefiners(store, o);
    if by.is_empty() {
        return None;
    }
    let names: Vec<String> = by.iter().map(|r| format!("{} ({})", redefiner_name(store, *r), uuid_of(store, *r).unwrap_or_default())).collect();
    Some(format!(
        "{id} is redefined by {}; remove those redefinitions (a property's redefines, with uml.attribute.set or uml.association.set-end) before making it leaf",
        names.join(", ")
    ))
}

/// The generalizations that specialize the classifier `c` directly, each with its specific
/// classifier, in the order they were made.
pub fn specializations(store: &Store, c: Obj) -> Vec<(Obj, Obj)> {
    let mut out: Vec<(Obj, Obj)> = store
        .referrers(c)
        .into_iter()
        .filter(|(g, prop)| *prop == generalization::GENERAL && store.is_alive(*g) && store.class_of(*g).ok() == Some(GENERALIZATION))
        .filter_map(|(g, _)| {
            let specific = store.container(g).ok().flatten().filter(|(_, p)| *p == classifier::GENERALIZATION).map(|(o, _)| o)?;
            store.is_alive(specific).then_some((g, specific))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Why the classifier `c` may not be made final, when it may not: a built-in type is never changed,
/// and a classifier something specializes already would break the rule at once; the refusal names
/// those classifiers and asks for their generalizations to go first.
pub fn final_problem(store: &Store, c: Obj) -> Option<String> {
    let id = uuid_of(store, c).unwrap_or_default();
    if crate::members::is_built_in(store, c) {
        return Some(format!(
            "{id} is the built-in type {}, which is never made final; a primitive type of the model's own may specialize it",
            name_of(store, c)
        ));
    }
    let below = specializations(store, c);
    if below.is_empty() {
        return None;
    }
    let names: Vec<String> = below.iter().map(|(_, s)| format!("{} ({})", name_of(store, *s), uuid_of(store, *s).unwrap_or_default())).collect();
    Some(format!("{id} is specialized by {}; remove those generalizations (element.remove) before making it final", names.join(", ")))
}

/// `uml.classifier.set`: a classifier of any kind made final (AR-137) or leaf (AR-138) or not; a
/// field left out keeps its value. Made final only while nothing specializes it, and leaf only
/// while nothing redefines it, both checked before anything changes; cleared, either changes
/// nothing else.
fn classifier_set(ctx: &mut C<'_>, p: &Params) -> R {
    let c = classifier_of(ctx.doc, p, "classifier")?;
    let final_on = p.bool("final");
    let leaf_on = p.bool("leaf");
    if final_on == Some(true)
        && !is_final(ctx.doc, c)
        && let Some(why) = final_problem(ctx.doc, c)
    {
        return Err(refuse("final", why));
    }
    if leaf_on == Some(true)
        && !is_leaf(ctx.doc, c)
        && let Some(why) = leaf_problem(ctx.doc, c)
    {
        return Err(refuse("leaf", why));
    }
    if let Some(on) = final_on.filter(|on| *on != is_final(ctx.doc, c)) {
        ctx.doc.set(c, classifier::IS_FINAL_SPECIALIZATION, Value::Bool(on)).map_err(fail)?;
    }
    if let Some(on) = leaf_on.filter(|on| *on != is_leaf(ctx.doc, c)) {
        ctx.doc.set(c, crate::redefinable_element::IS_LEAF, Value::Bool(on)).map_err(fail)?;
    }
    Ok(json!({ "id": uuid_of(ctx.doc, c)?, "final": is_final(ctx.doc, c), "leaf": is_leaf(ctx.doc, c) }))
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

/// The relationships that touch class `c`: the associations and association classes with an end
/// typed by it, the generalizations to and from it, the dependencies it is a client or supplier of,
/// and the interface realizations it implements or is the contract of; each once. An association
/// class's own ends are not among an association class's relationships: they touch its classes.
pub fn relationships_of(store: &Store, c: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    for (src, prop) in store.referrers(c) {
        let r = if prop == typed_element::TYPE && store.class_of(src).ok() == Some(PROPERTY) {
            // Its association, or the association that owns it.
            let owner = store
                .container(src)
                .ok()
                .flatten()
                .filter(|(o, p)| *p == association::OWNED_END && store.class_of(*o).is_ok_and(crate::is_association))
                .map(|(o, _)| o);
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

/// The classifier under `key`, which must sit in a package, its owner's when it is nested
/// (AR-132).
fn class_in_package(store: &Store, p: &Params, key: &str) -> Result<(Obj, Obj), CommandError> {
    let c = classifier_of(store, p, key)?;
    let pkg = crate::nesting::home_package(store, c).ok_or_else(|| refuse(key, format!("the {} is in no package", what(store, c))))?;
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

/// Why an association may not have an end at `c`, when it may not: a data type, an enumeration
/// and a primitive type among them, has values without identity, and an association joins classes
/// and interfaces, whose instances it links. The refusal names `c`'s id, what it is and what an end
/// may be.
pub fn association_end_problem(store: &Store, c: Obj) -> Option<String> {
    let is = match store.class_of(c).ok() {
        Some(k) if k == ENUMERATION => "an enumeration, a data type,",
        Some(k) if k == DATA_TYPE => "a data type",
        Some(k) if k == PRIMITIVE_TYPE => "a primitive type",
        _ => return None,
    };
    let id = uuid_of(store, c).unwrap_or_default();
    Some(format!("{id} is {is} and joins no association; an association joins classes and interfaces"))
}

/// `uml.association.add`: an association of the kind asked for between two classifiers, owning
/// both ends; never to or from an enumeration, a data type or a primitive type.
fn association_add(ctx: &mut C<'_>, p: &Params) -> R {
    let (from, pkg) = class_in_package(ctx.doc, p, "from")?;
    let to = classifier_of(ctx.doc, p, "to")?;
    for (key, c) in [("from", from), ("to", to)] {
        if let Some(why) = association_end_problem(ctx.doc, c) {
            return Err(refuse(key, why));
        }
    }
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

/// Why association class `ac` (none while it is being made) may not have an end at `c`, when it
/// may not: its ends are classes, plain or association classes, never an interface, an
/// enumeration, a data type or a primitive type, and never the association class itself. The
/// refusal names `c`'s id, what it is and what an end may be.
pub fn association_class_end_problem(store: &Store, c: Obj, ac: Option<Obj>) -> Option<String> {
    let id = uuid_of(store, c).unwrap_or_default();
    if Some(c) == ac {
        return Some(format!("{id} is the association class itself; its ends are other classes"));
    }
    match store.class_of(c).ok() {
        Some(k) if k == CLASS || k == ASSOCIATION_CLASS => None,
        _ => Some(format!("{id} is {}; an association class joins only classes (a class or an association class)", a_what(store, c))),
    }
}

/// `uml.association-class.add`: one association class between two classes, in the first class's
/// package, owning both its ends as an association does: no roles, no multiplicities, navigable to
/// neither end. Its name, when it has one, is free among the package's classifiers.
fn association_class_add(ctx: &mut C<'_>, p: &Params) -> R {
    let (from, pkg) = class_in_package(ctx.doc, p, "from")?;
    let to = classifier_of(ctx.doc, p, "to")?;
    for (key, c) in [("from", from), ("to", to)] {
        if let Some(why) = association_class_end_problem(ctx.doc, c, None) {
            return Err(refuse(key, why));
        }
    }
    let name = text(p, "name");
    if !name.is_empty() {
        if let Some(why) = crate::members::reserved_problem(ctx.doc, pkg, name) {
            return Err(refuse("name", why));
        }
        crate::commands::check_name(ctx.doc, Some(pkg), ASSOCIATION_CLASS, name, None)?;
    }
    let a = ctx.doc.create(ASSOCIATION_CLASS).map_err(fail)?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(a)).map_err(fail)?;
    let mut ends = Vec::new();
    for ty in [from, to] {
        let end = ctx.doc.create(PROPERTY).map_err(fail)?;
        ctx.doc.set(end, typed_element::TYPE, Value::Ref(ty)).map_err(fail)?;
        // A member end first, which tells the end its association, then owned by it.
        ctx.doc.push(a, association::MEMBER_END, Value::Ref(end)).map_err(fail)?;
        if !ctx.doc.list(a, association::OWNED_END).map_err(fail)?.contains(&Value::Ref(end)) {
            ctx.doc.push(a, association::OWNED_END, Value::Ref(end)).map_err(fail)?;
        }
        ends.push(end);
    }
    for end in &ends {
        set_navigable(ctx.doc, a, *end, false)?;
    }
    ctx.view.selection = vec![a];
    Ok(json!({ "id": uuid_of(ctx.doc, a)?, "ends": [uuid_of(ctx.doc, ends[0])?, uuid_of(ctx.doc, ends[1])?] }))
}

/// Why a generalization from `specific` to `general` is refused: a rule between the two kinds, or
/// one between the two classifiers themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralizationProblem {
    /// An interface specializes only an interface, and the general one is none.
    NotAnInterface,
    /// A class specializes no interface: the specific one is a class.
    ClassOfInterface,
    /// An enumeration specializes only an enumeration, and the general one is none.
    NotAnEnumeration,
    /// Only an enumeration specializes an enumeration, and the specific one is none.
    NoEnumeration,
    /// A data type specializes only a data type, and the general one is none.
    NotADataType,
    /// Only a data type specializes a data type, and the specific one is none.
    NoDataType,
    /// A primitive type specializes only a primitive type, and the general one is none.
    NotAPrimitiveType,
    /// Only a primitive type specializes a primitive type, and the specific one is none.
    NoPrimitiveType,
    /// A built-in type is never changed, so it specializes nothing: the specific one is one.
    BuiltIn,
    /// Nothing specializes an association class: the general one is one. A plain class may not in
    /// UML (`maySpecializeType`), and another association class would bind its ends to these.
    AssociationClass,
    /// A classifier specializes another, never itself.
    Itself,
    /// The general classifier is final (`isFinalSpecialization`): nothing may specialize it.
    Final,
    /// The specific classifier specializes the general one already.
    Again,
    /// The general classifier specializes the specific one already: a cycle.
    Cycle,
}

impl GeneralizationProblem {
    /// The parameter of `uml.generalization.add` a refusal names: the end that breaks the rule.
    pub fn key(self) -> &'static str {
        match self {
            GeneralizationProblem::ClassOfInterface
            | GeneralizationProblem::NoEnumeration
            | GeneralizationProblem::NoDataType
            | GeneralizationProblem::NoPrimitiveType
            | GeneralizationProblem::BuiltIn => "specific",
            _ => "general",
        }
    }

    /// Why, as a refusal says it: the offending classifier's id and what is accepted instead.
    pub fn why(self, store: &Store, specific: Obj, general: Obj) -> String {
        let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
        let (s, g) = (id(specific), id(general));
        let (kind, a_kind) = (what(store, specific), a_what(store, specific));
        match self {
            GeneralizationProblem::NotAnInterface => {
                format!("{g} is {}; an interface specializes only an interface", a_what(store, general))
            }
            GeneralizationProblem::ClassOfInterface => {
                format!("{s} is {a_kind}; a class cannot specialize an interface, only a class")
            }
            GeneralizationProblem::AssociationClass => {
                format!("{g} is an association class, which nothing specializes; {s} specializes a class instead")
            }
            GeneralizationProblem::NotAnEnumeration => {
                format!("{g} is {}; an enumeration specializes only an enumeration", a_what(store, general))
            }
            GeneralizationProblem::NoEnumeration => {
                format!("{s} is {a_kind}; only an enumeration specializes an enumeration")
            }
            GeneralizationProblem::NotADataType => {
                format!("{g} is {}; a data type specializes only a data type", a_what(store, general))
            }
            GeneralizationProblem::NoDataType => {
                format!("{s} is {a_kind}; only a data type specializes a data type")
            }
            GeneralizationProblem::NotAPrimitiveType => {
                format!("{g} is {}; a primitive type specializes only a primitive type", a_what(store, general))
            }
            GeneralizationProblem::NoPrimitiveType => {
                format!("{s} is {a_kind}; only a primitive type specializes a primitive type")
            }
            GeneralizationProblem::BuiltIn => {
                format!(
                    "{s} is the built-in type {}, which specializes nothing; a primitive type of the model's own may specialize it",
                    name_of(store, specific)
                )
            }
            GeneralizationProblem::Itself => format!("{s}: {a_kind} cannot specialize itself; it specializes another {kind}"),
            GeneralizationProblem::Final => {
                format!("{g} is final (isFinalSpecialization), so nothing may specialize it; clear final with uml.classifier.set first")
            }
            GeneralizationProblem::Again => format!("{s}: the {kind} specializes that {kind} already, {g}; one generalization joins two"),
            GeneralizationProblem::Cycle => {
                format!("{g}: that {kind} specializes this one already, which would make a cycle; {s} specializes only what is not below it")
            }
        }
    }
}

/// Why `specific` may not specialize `general`, when it may not: an interface specializes only an
/// interface, and a class never specializes one; nothing specializes an association class; an
/// enumeration specializes only an enumeration,
/// and only an enumeration specializes one; a data type likewise only a data type, and a primitive
/// type only a primitive type; a built-in type specializes nothing, though one of the model's own
/// may specialize it; no classifier specializes itself, a final one, another twice, or round a
/// cycle. `except` is a generalization being reconnected, which never clashes with itself.
///
/// The kinds are checked in a fixed order, each by its own metaclass (an enumeration and a
/// primitive type are DataTypes in UML's metamodel, but only the DataType metaclass is a data type
/// here): an enumeration's rule first, then a data type's, then a primitive type's, then an
/// interface's. So a refusal always names the kind whose rule is broken, and
/// [`GeneralizationProblem::ClassOfInterface`] is said only of a class.
pub fn generalization_problem(store: &Store, specific: Obj, general: Obj, except: Option<Obj>) -> Option<GeneralizationProblem> {
    let is = |o: Obj, k| store.class_of(o).ok() == Some(k);
    match (is(specific, ENUMERATION), is(general, ENUMERATION)) {
        (true, false) => return Some(GeneralizationProblem::NotAnEnumeration),
        (false, true) => return Some(GeneralizationProblem::NoEnumeration),
        _ => {}
    }
    match (is(specific, DATA_TYPE), is(general, DATA_TYPE)) {
        (true, false) => return Some(GeneralizationProblem::NotADataType),
        (false, true) => return Some(GeneralizationProblem::NoDataType),
        _ => {}
    }
    match (is(specific, PRIMITIVE_TYPE), is(general, PRIMITIVE_TYPE)) {
        (true, false) => return Some(GeneralizationProblem::NotAPrimitiveType),
        (false, true) => return Some(GeneralizationProblem::NoPrimitiveType),
        _ => {}
    }
    if crate::members::is_built_in(store, specific) {
        return Some(GeneralizationProblem::BuiltIn);
    }
    match (is(specific, INTERFACE), is(general, INTERFACE)) {
        (true, false) => return Some(GeneralizationProblem::NotAnInterface),
        (false, true) => return Some(GeneralizationProblem::ClassOfInterface),
        _ => {}
    }
    if is(general, ASSOCIATION_CLASS) {
        return Some(GeneralizationProblem::AssociationClass);
    }
    if specific == general {
        return Some(GeneralizationProblem::Itself);
    }
    if is_final(store, general) {
        return Some(GeneralizationProblem::Final);
    }
    if list(store, specific, classifier::GENERALIZATION)
        .into_iter()
        .any(|g| Some(g) != except && one(store, g, generalization::GENERAL) == Some(general))
    {
        return Some(GeneralizationProblem::Again);
    }
    specializes(store, general, specific).then_some(GeneralizationProblem::Cycle)
}

/// `uml.generalization.add`: `specific` specializes `general`, unless the classifier rules refuse it.
fn generalization_add(ctx: &mut C<'_>, p: &Params) -> R {
    let specific = classifier_of(ctx.doc, p, "specific")?;
    let general = classifier_of(ctx.doc, p, "general")?;
    if let Some(problem) = generalization_problem(ctx.doc, specific, general, None) {
        return Err(refuse(problem.key(), problem.why(ctx.doc, specific, general)));
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

/// Why a classifier may not realize another: what a realization from anything but a class (an
/// association class is one), to anything but an interface, or a second one between the same two
/// would be refused for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealizationProblem {
    /// Only a class implements, an association class among them: UML's implementing classifier is
    /// behaviored.
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

    /// Why, as a refusal says it with the offending classifier: its id and what it is (the
    /// implementing one when it is no class, else the contract), then the rule.
    pub fn said(self, store: &Store, implementing: Obj, contract: Obj) -> String {
        let offending = if self == RealizationProblem::NotAClass { implementing } else { contract };
        format!("{} is {}; {}", uuid_of(store, offending).unwrap_or_default(), a_what(store, offending), self.why())
    }
}

/// Why `implementing` may not realize `contract`, when it may not: only a class implements (an
/// association class is a class), only an interface is a contract, and a class realizes an
/// interface once. `except` is a realization being reconnected, which never clashes with itself.
pub fn realization_problem(store: &Store, implementing: Obj, contract: Obj, except: Option<Obj>) -> Option<RealizationProblem> {
    if !store.class_of(implementing).is_ok_and(|k| k == CLASS || k == ASSOCIATION_CLASS) {
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
        return Err(refuse(problem.key(), problem.said(ctx.doc, implementing, contract)));
    }
    let r = ctx.doc.create(INTERFACE_REALIZATION).map_err(fail)?;
    ctx.doc.set(r, interface_realization::CONTRACT, Value::Ref(contract)).map_err(fail)?;
    // Owned by the class, which makes the class its implementing classifier and its client.
    ctx.doc.push(implementing, behaviored_classifier::INTERFACE_REALIZATION, Value::Ref(r)).map_err(fail)?;
    mirror_ends(ctx.doc, r)?;
    ctx.view.selection = vec![r];
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// The association or dependency under `key`; an association class is named as the classifier it
/// is, and the refusal says so.
fn named_relationship(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    if let Ok(ac) = id_of(store, p, key, ASSOCIATION_CLASS, "") {
        return Err(refuse(
            key,
            format!("{} is an association class; rename it with uml.association-class.rename, as the classifier it is", uuid_of(store, ac)?),
        ));
    }
    let r = id_of(store, p, key, ASSOCIATION, "an association").or_else(|_| id_of(store, p, key, DEPENDENCY, "a dependency"));
    r.map_err(|_| refuse(key, format!("{} is no association or dependency", crate::commands::short(text(p, key)))))
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

/// The association or association class under `key`: what has ends to set.
fn association_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    id_of(store, p, key, ASSOCIATION, "")
        .or_else(|_| id_of(store, p, key, ASSOCIATION_CLASS, ""))
        .map_err(|_| refuse(key, format!("{} is no association or association class", crate::commands::short(text(p, key)))))
}

/// `uml.association.set-end`: one end of an association or an association class set: its role,
/// multiplicity, navigability and aggregation, with the rules between the two ends kept, and its
/// static and modifiers (AR-126), its context the class at the other end. A field the call leaves
/// out keeps what the end has; clearing is said explicitly.
fn association_set_end(ctx: &mut C<'_>, p: &Params) -> R {
    let a = association_of(ctx.doc, p, "association")?;
    let ends = ends_of(ctx.doc, a, "association")?;
    let i = match p.integer("end") {
        Some(i @ 0..=1) => i as usize,
        _ => return Err(refuse("end", "the end is 0 or 1")),
    };
    let (end, other) = (ends[i], ends[1 - i]);
    // A field left out keeps what the end has.
    let kept_role = name_of(ctx.doc, end);
    let role = p.str("role").unwrap_or(&kept_role);
    if let Some(problem) = (!role.is_empty()).then(|| archforge_core::skills::name_problem(role, crate::commands::MAX_NAME)).flatten() {
        return Err(refuse("role", format!("the role {problem}")));
    }
    if !role.is_empty() && name_of(ctx.doc, other).to_lowercase() == role.to_lowercase() {
        return Err(refuse("role", format!("the other end's role is {role} already")));
    }
    // An association class's ends' roles and its attributes share its names.
    if let Some(attribute) = crate::members::attribute_clash(ctx.doc, a, role) {
        return Err(refuse(
            "role",
            format!(
                "the association class {} has an attribute {} named {}; its ends' roles and its attributes share its names, so the role {role} takes another",
                uuid_of(ctx.doc, a)?,
                uuid_of(ctx.doc, attribute)?,
                name_of(ctx.doc, attribute)
            ),
        ));
    }
    let kind = match p.str("aggregation") {
        None => aggregation(ctx.doc, end),
        Some(k) => AGGREGATIONS
            .iter()
            .position(|w| *w == k)
            .ok_or_else(|| refuse("aggregation", format!("{k:?} is not one of {}", AGGREGATIONS.join(", "))))?,
    };
    if kind != 0 && aggregation(ctx.doc, other) != 0 {
        return Err(refuse("aggregation", "the other end aggregates already: only the part's end can"));
    }
    // Left out, the multiplicity is the stored one as it is, never read again.
    let bounds = match p.str("multiplicity") {
        Some(typed) => crate::members::parse_multiplicity(typed).map_err(|why| refuse("multiplicity", format!("the multiplicity {why}")))?,
        None => Some(crate::modifiers::bounds(ctx.doc, end)),
    };
    // A part belongs to at most one whole: the whole's end allows one at most.
    if kind == 2 && upper(ctx.doc, other).is_none_or(|u| u > 1) {
        return Err(refuse("aggregation", "a composition's whole end allows one at most; set the other end's multiplicity to 1 or 0..1 first"));
    }
    if aggregation(ctx.doc, other) == 2 && bounds.is_some_and(|(_, u)| u.is_none_or(|u| u > 1)) {
        return Err(refuse("multiplicity", "this end is a composition's whole, which allows one at most"));
    }
    // The whole's end, where the diamond sits, is never navigable (owner decision); left out, an
    // end keeps whether it is.
    let whole = aggregation(ctx.doc, other) != 0;
    let navigable = p.bool("navigable").unwrap_or(!whole && crate::figures::navigable(ctx.doc, a, end));
    if whole && navigable {
        return Err(refuse("navigable", "the whole's end of an aggregation or composition is not navigable"));
    }
    // Its modifiers, checked against the end as it will be: its role, bounds and aggregation.
    let (lower, upper) = bounds.unwrap_or((1, Some(1)));
    let shape = crate::modifiers::Shape {
        itself: Some(end),
        context: one(ctx.doc, other, typed_element::TYPE).into_iter().collect(),
        ty: one(ctx.doc, end, typed_element::TYPE),
        lower,
        upper,
        composite: kind == 2,
        name: role.to_string(),
    };
    let modifiers = crate::modifiers::read(ctx.doc, p, &shape, Some(end))?;
    let role = role.to_string();
    own_end(ctx.doc, a, end)?;
    set_name(ctx.doc, end, &role)?;
    if let Some(typed) = p.str("multiplicity") {
        crate::members::set_multiplicity(ctx.doc, end, typed, "multiplicity")?;
    }
    if let Some(on) = p.bool("static") {
        ctx.doc.set(end, crate::feature::IS_STATIC, Value::Bool(on)).map_err(fail)?;
    }
    crate::modifiers::apply(ctx.doc, end, &modifiers)?;
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

/// Why a realization's end that stays, `end`, named `which`, is wrong already: its id, the kind it
/// is, and the kind it needs (`needs`, with its article).
fn wrong_end(store: &Store, which: &str, end: Obj, needs: &str) -> String {
    let id = uuid_of(store, end).unwrap_or_default();
    let is = store.class_of(end).map(|c| store.universe().class_name(c)).unwrap_or_else(|_| "nothing".into());
    format!("the realization's existing {which} end {id} is of kind {is}; it must be {needs}")
}

/// `uml.relationship.reconnect`: one end of a relationship moved to another classifier, its rules kept and its lines following;
/// an association's ends drop, in the same step, what they subset or redefine that the rules no
/// longer allow, each listed under `dropped` (AR-126).
fn relationship_reconnect(ctx: &mut C<'_>, p: &Params) -> R {
    let key = "relationship";
    let id = text(p, key);
    let r = crate::RELATIONSHIPS.into_iter().find_map(|k| id_of(ctx.doc, p, key, k, "").ok()).ok_or_else(|| {
        refuse(
            key,
            format!("{} is no association, generalization, dependency, interface realization or association class", crate::commands::short(id)),
        )
    })?;
    let source = match text(p, "end") {
        "source" => true,
        "target" => false,
        e => return Err(refuse("end", format!("{e:?} is not source or target"))),
    };
    let c = classifier_of(ctx.doc, p, "class")?;
    let class = ctx.doc.class_of(r).map_err(fail)?;
    let mut dropped: Vec<Json> = Vec::new();
    if crate::is_association(class) {
        let why = if class == ASSOCIATION_CLASS { association_class_end_problem(ctx.doc, c, Some(r)) } else { association_end_problem(ctx.doc, c) };
        if let Some(why) = why {
            return Err(refuse("class", why));
        }
        let ends = ends_of(ctx.doc, r, key)?;
        let end = ends[if source { 0 } else { 1 }];
        own_end(ctx.doc, r, end)?;
        ctx.doc.set(end, typed_element::TYPE, Value::Ref(c)).map_err(fail)?;
        // Moved, an end's type and the other end's context changed: what either subsets or
        // redefines that the rules no longer allow goes, in the same step, and is said.
        for e in ends {
            for (list, t) in crate::modifiers::drop_stale(ctx.doc, e)? {
                dropped.push(json!({ "end": uuid_of(ctx.doc, e)?, "list": list, "id": uuid_of(ctx.doc, t)?, "name": name_of(ctx.doc, t) }));
            }
        }
    } else if class == GENERALIZATION {
        let (specific, general) = crate::figures::ends(ctx.doc, r).ok_or_else(|| refuse(key, "the generalization joins no two classifiers"))?;
        let (s, g) = if source { (c, general) } else { (specific, c) };
        // A member of a generalization set keeps its ends, so every set stays one of a single
        // general classifier (AR-129).
        if (s, g) != (specific, general)
            && let Some(why) = crate::generalization_sets::reconnect_problem(ctx.doc, r)
        {
            return Err(refuse(key, why));
        }
        if let Some(problem) = generalization_problem(ctx.doc, s, g, Some(r)) {
            return Err(refuse("class", problem.why(ctx.doc, s, g)));
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
                return Err(refuse(key, wrong_end(ctx.doc, "implementing", implementing, "a Class")));
            }
            Some(RealizationProblem::NotAnInterface) if source => {
                return Err(refuse(key, wrong_end(ctx.doc, "contract", contract, "an Interface")));
            }
            Some(problem) => return Err(refuse("class", problem.said(ctx.doc, i, c))),
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
    Ok(json!({ "id": uuid_of(ctx.doc, r)?, "dropped": dropped }))
}

/// An association's or an association class's two ends as the query commands give them: each with
/// its class, role, multiplicity, navigability, aggregation, static, modifiers and qualifiers.
pub(crate) fn ends_json(store: &Store, r: Obj) -> Json {
    let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
    let ends: Vec<Json> = crate::figures::member_ends(store, r)
        .map(|ends| {
            ends.iter()
                .map(|e| {
                    let mut end = json!({
                        "id": id(*e),
                        "class": one(store, *e, typed_element::TYPE).map_or(Json::Null, |t| json!(id(t))),
                        "role": name_of(store, *e),
                        "multiplicity": crate::figures::multiplicity(store, *e),
                        "navigable": crate::figures::navigable(store, r, *e),
                        "aggregation": AGGREGATIONS[aggregation(store, *e)],
                        "static": store.get(*e, crate::feature::IS_STATIC).ok().and_then(|v| v.as_bool()).unwrap_or(false),
                    });
                    crate::modifiers::merge_into(store, *e, &mut end);
                    // Its qualifiers, in order (AR-127).
                    if let Some(fields) = end.as_object_mut() {
                        fields.insert("qualifiers".to_string(), crate::qualifiers::qualifiers_json(store, *e));
                    }
                    end
                })
                .collect()
        })
        .unwrap_or_default();
    Json::Array(ends)
}

/// A relationship as the query commands give it, with its description or nothing (AR-186).
fn describe(store: &Store, r: Obj) -> Json {
    let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
    let (from, to) = crate::figures::ends(store, r).map_or((Json::Null, Json::Null), |(a, b)| (json!(id(a)), json!(id(b))));
    let mut out = match store.class_of(r).ok() {
        Some(c) if c == ASSOCIATION => {
            json!({ "id": id(r), "kind": "association", "name": name_of(store, r), "from": from, "to": to, "ends": ends_json(store, r) })
        }
        Some(c) if c == ASSOCIATION_CLASS => {
            json!({ "id": id(r), "kind": "association_class", "name": name_of(store, r), "from": from, "to": to, "ends": ends_json(store, r) })
        }
        Some(c) if c == GENERALIZATION => {
            let sets: Vec<Json> = crate::generalization_sets::sets_of(store, r)
                .into_iter()
                .map(|s| json!({ "id": id(s), "name": name_of(store, s), "label": crate::generalization_sets::label(store, s) }))
                .collect();
            json!({ "id": id(r), "kind": "generalization", "specific": from, "general": to, "sets": sets })
        }
        Some(c) if c == INTERFACE_REALIZATION => json!({ "id": id(r), "kind": "realization", "implementing": from, "contract": to }),
        _ => json!({ "id": id(r), "kind": "dependency", "name": name_of(store, r), "client": from, "supplier": to }),
    };
    if let Some(fields) = out.as_object_mut() {
        fields.insert("description".to_string(), Json::String(crate::descriptions::description_of(store, r)));
    }
    out
}

/// `uml.relationship.list`: the relationships that touch a class, an interface, an enumeration, a
/// data type or a primitive type.
fn relationship_list(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let c = classifier_of(store, p, "class")?;
    Ok(json!({ "relationships": relationships_of(store, c).into_iter().map(|r| describe(store, r)).collect::<Vec<_>>() }))
}

macro_rules! end_lines {
    () => {
        concat!(
            "association: string; the association's or association class's id\n",
            "end: integer, 0..1; which end, 0 at the class or interface it was drawn from, 1 at the other; every other field left out keeps what the end has, and clearing is said explicitly\n",
            "role: string?; the end's role name, \"\" for none; an association class's end never takes one of its attributes' names; left out, kept\n",
            "multiplicity: string?; 1, 0..1, *, 1..*, 2..5, \"\" for none; left out, kept\n",
            "navigable: bool?; the association navigates to this end; never the whole's end of an aggregation or composition; left out, kept\n",
            "aggregation: string?, none|shared|composite; this end is a part of the other end's class, shared or composed; left out, kept\n",
            "static: bool?; belongs to the class, not its instances; left out, kept\n",
            kept_modifier_lines!()
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
                "from: string; the id of the class or interface it is drawn from, the whole of an aggregation or composition; never an enumeration, a data type or a primitive type\n",
                "to: string; the id of the class or interface it is drawn to, the part of an aggregation or composition; never an enumeration, a data type or a primitive type\n",
                "kind: string = \"association\", association|directed|aggregation|composition; directed navigates only to the second class\n",
                "name: string = \"\"; the association's name, or nothing"
            ),
            has_classes,
            association_add
        ),
        cmd!(
            "uml.association.set-end",
            "Set association end",
            &["Association", "Association class"],
            None,
            end_lines!(),
            has_classes,
            association_set_end
        ),
        cmd!(
            "uml.association-class.add",
            "Add association class",
            &["Association class"],
            None,
            concat!(
                "from: string; the id of the class it is drawn from, in whose package it goes; a class or an association class, never an interface, an enumeration, a data type or a primitive type\n",
                "to: string; the id of the class it is drawn to; a class or an association class, never an interface, an enumeration, a data type or a primitive type\n",
                "name: string = \"\"; its name, free among the package's classifiers (classes, interfaces, enumerations, data types, primitive types and association classes), or nothing; its ends have no roles or multiplicities and navigate to neither class until set with uml.association.set-end, and its attributes and operations are added as a class's"
            ),
            has_classes,
            association_class_add
        ),
        cmd!(
            "uml.generalization.add",
            "Add generalization",
            &["Generalization"],
            None,
            concat!(
                "specific: string; the id of the class, interface, enumeration, data type or primitive type that specializes; never a built-in type\n",
                "general: string; the id of the class, interface, enumeration, data type or primitive type it specializes: an interface specializes only an interface, a class never specializes one, an enumeration specializes only an enumeration and is specialized only by one, and a data type and a primitive type likewise only their own kind; never a final one"
            ),
            has_classes,
            generalization_add
        ),
        cmd!(
            "uml.classifier.set",
            "Set classifier",
            &["Class", "Interface", "Enumeration", "Data type", "Primitive type", "Association class"],
            None,
            concat!(
                "classifier: string; the id of the class, interface, enumeration, data type, primitive type or association class; every field left out keeps its value\n",
                "final: bool?; nothing may specialize it (UML's isFinalSpecialization, not the leaf setting): made final only while no generalization specializes it and never for a built-in type; cleared, other classifiers may specialize it again; left out, kept\n",
                "leaf: bool?; nothing may redefine it (UML's isLeaf), written {leaf} after its name: made leaf only while nothing redefines it and never for a built-in type; cleared, it may be redefined again; left out, kept"
            ),
            has_classes,
            classifier_set
        ),
        cmd!(
            "uml.dependency.add",
            "Add dependency",
            &["Dependency"],
            None,
            "client: string; the id of the class, interface, enumeration, data type or primitive type that depends\nsupplier: string; the id of the class, interface, enumeration, data type or primitive type it depends on\nname: string = \"\"; the dependency's name, or nothing",
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
            &["Association", "Generalization", "Dependency", "Interface realization", "Association class"],
            None,
            concat!(
                "relationship: string; the association's, generalization's, dependency's, interface realization's or association class's id\n",
                "end: string, source|target; the end to move: the classifier it was drawn from (the specific one, the client, the implementing class) or to\n",
                "class: string; the id of the class, interface, enumeration, data type or primitive type the end moves to; an association's end never moves to an enumeration, a data type or a primitive type; a generalization in a generalization set keeps its ends, refused naming its sets; a generalization's end keeps its rules: an interface specializes only an interface, a class never one, an enumeration only an enumeration, a data type only a data type, a primitive type only a primitive type, a built-in type nothing, and the general end never moves onto a final classifier; a realization's source is a class and its target an interface, the class realizing it once; an association class's ends move only onto a class or another association class; a moved association's ends drop what they subset or redefine that the rules no longer allow, listed under dropped in the result"
            ),
            has_classes,
            relationship_reconnect
        ),
        cmd!(query "uml.relationship.list", "List relationships", &["Association", "Generalization", "Dependency", "Interface realization", "Association class"], None, "class: string; the class's, interface's, enumeration's, data type's, primitive type's or association class's id", has_classes, relationship_list),
    ]
}
