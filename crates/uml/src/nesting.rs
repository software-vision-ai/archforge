//! Nested classifiers (AR-132): a class, an interface, an enumeration or a data type owned by a
//! class, an interface or an association class through UML's `nestedClassifier`, which the
//! generated schema already keeps, rather than by a package. A classifier's owner is a package or
//! a classifier; it is changed by `uml.classifier.set-owner`, which keeps the classifier's identity,
//! its members, its relationships and every type reference to it, since the element itself only
//! moves from one list to another.
//!
//! What may own what is UML's, narrowed to what the notation draws: a class, an interface and an
//! association class own classifiers (each has `nestedClassifier`); an enumeration, a data type and
//! a primitive type own none. A class, an interface, an enumeration and a data type may be nested;
//! a primitive type and an association class stay in a package, as the design offers them (page
//! 08, D1). An owner is never the classifier itself nor one nested in it at any depth, and a nested
//! classifier's name is told apart from its owner's other classifiers whatever the case, as a
//! package's classifiers' names are, by the one rule of [`crate::names`]: an attribute, an
//! operation or a package may share it (AR-208). A built-in type is never moved.
//!
//! Ownership is not package membership: a nested classifier's package is its owner's, its home
//! package ([`home_package`]), which places it on its package's diagrams; and it is never what a
//! card overlapping another means, which is presentation alone. Removing an owner removes what it
//! nests, with their relationships and views, in the same step (`commands::remove_elements`).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, Reason, cmd};
use archforge_mof::{ClassId, Obj, PropId, Store, Value};
use serde_json::{Value as Json, json};

use crate::commands::{a_what, check_name, classifier_of, fail, name_of, refuse, set_name, text, uuid_of, what};
use crate::{ASSOCIATION_CLASS, CLASS, DATA_TYPE, ENUMERATION, INTERFACE, PACKAGE, class, interface, package};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The kinds that own nested classifiers: a class, an interface and an association class (each has
/// UML's `nestedClassifier`).
pub const OWNERS: [ClassId; 3] = [CLASS, INTERFACE, ASSOCIATION_CLASS];

/// The kinds that may be nested in a classifier, in the order the menus offer them: a class, an
/// interface, an enumeration and a data type (design page 08, D1).
pub const NESTABLE: [ClassId; 4] = [CLASS, INTERFACE, ENUMERATION, DATA_TYPE];

/// The deepest an owner chain is followed: a damaged file could hold a loop.
const MAX_DEPTH: usize = 256;

/// The kinds `uml.nested.add` takes, by the names the commands use, with their metaclass.
pub const KINDS: [(&str, ClassId); 4] = [("class", CLASS), ("interface", INTERFACE), ("enumeration", ENUMERATION), ("data-type", DATA_TYPE)];

/// Whether a classifier of `class` owns nested classifiers.
pub fn can_own(class: ClassId) -> bool {
    OWNERS.contains(&class)
}

/// Whether a classifier of `class` may be nested in another.
pub fn can_nest(class: ClassId) -> bool {
    NESTABLE.contains(&class)
}

/// The list a classifier of `class` keeps its nested classifiers in, when it has one.
fn list_of(class: ClassId) -> Option<PropId> {
    match class {
        c if c == CLASS || c == ASSOCIATION_CLASS => Some(class::NESTED_CLASSIFIER),
        c if c == INTERFACE => Some(interface::NESTED_CLASSIFIER),
        _ => None,
    }
}

/// The classifiers `owner` nests, in its order: none for anything that owns none.
pub fn nested(store: &Store, owner: Obj) -> Vec<Obj> {
    let Some(prop) = store.class_of(owner).ok().and_then(list_of) else { return Vec::new() };
    store
        .list(owner, prop)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| store.is_alive(*o) && store.class_of(*o).is_ok_and(crate::is_classifier))
        .collect()
}

/// Every classifier nested in `owner` at any depth, each before those it nests; a seen-set bounds
/// the walk.
pub fn nested_within(store: &Store, owner: Obj) -> Vec<Obj> {
    let mut seen = std::collections::BTreeSet::from([owner]);
    let mut out = Vec::new();
    let mut stack: Vec<Obj> = nested(store, owner).into_iter().rev().collect();
    while let Some(c) = stack.pop() {
        if !seen.insert(c) {
            continue;
        }
        out.push(c);
        stack.extend(nested(store, c).into_iter().rev());
    }
    out
}

/// The classifier that nests `c`, when one does.
pub fn classifier_owner(store: &Store, c: Obj) -> Option<Obj> {
    let (owner, prop) = store.container(c).ok().flatten()?;
    let nests = store.class_of(owner).ok().and_then(list_of) == Some(prop);
    nests.then_some(owner)
}

/// Whether `c` is nested in a classifier.
pub fn is_nested(store: &Store, c: Obj) -> bool {
    classifier_owner(store, c).is_some()
}

/// What owns the classifier `c`: its package, or the classifier that nests it.
pub fn owner_of(store: &Store, c: Obj) -> Option<Obj> {
    classifier_owner(store, c).or_else(|| crate::commands::package_of(store, c))
}

/// The package `c` belongs in: its own package, or, nested, its outermost owner's. A nested
/// classifier is shown on its home package's diagrams as a member of it.
pub fn home_package(store: &Store, c: Obj) -> Option<Obj> {
    let mut at = c;
    for _ in 0..MAX_DEPTH {
        match classifier_owner(store, at) {
            Some(owner) => at = owner,
            None => return crate::commands::package_of(store, at),
        }
    }
    None
}

/// The classifiers that nest `c`, outermost first.
pub fn owner_chain(store: &Store, c: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut at = c;
    while let Some(owner) = classifier_owner(store, at).filter(|o| out.len() < MAX_DEPTH && !out.contains(o)) {
        out.push(owner);
        at = owner;
    }
    out.reverse();
    out
}

/// The name `c` reads by where its owner is not in sight: its owners' names and its own joined by
/// `::`, outermost first (`Order::Priority`); a classifier nested in none reads as its name.
pub fn qualified_name(store: &Store, c: Obj) -> String {
    let mut parts: Vec<String> = owner_chain(store, c).into_iter().map(|o| name_of(store, o)).collect();
    parts.push(name_of(store, c));
    parts.join("::")
}

/// Every named element `owner` holds, whatever its kind: what a new nested classifier's default
/// name is kept free of, so it never reads as a member's. Only its classifiers clash with it
/// ([`classifier_clash`]).
pub fn names_under(store: &Store, owner: Obj) -> Vec<Obj> {
    store.owned_elements(owner).unwrap_or_default().into_iter().filter(|o| store.is_alive(*o) && !name_of(store, *o).is_empty()).collect()
}

/// The classifier `owner` holds (nests, or for a package holds), other than `except`, that a
/// classifier named `name` would clash with there, whatever the case: classifiers are one kind
/// ([`crate::names`]), so an attribute, an operation or a package of that name never clashes.
pub fn classifier_clash(store: &Store, owner: Obj, name: &str, except: Option<Obj>) -> Option<Obj> {
    crate::names::clash(store, owner, CLASS, name, except)
}

/// Why a classifier may not go under an owner: each a reason the Owner selector shows beside a
/// disabled owner, and the command's refusal says in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerProblem {
    /// The classifier itself.
    Itself,
    /// A classifier nested in it, at any depth: owning it would make a cycle.
    Inside,
    /// A kind that owns no classifiers: an enumeration, a data type, a primitive type.
    OwnsNone,
    /// The classifier is of a kind that is never nested (a primitive type, an association class),
    /// and the owner is a classifier.
    NotNestable,
    /// The owner has a classifier of that name already, whatever the case.
    NameTaken(Obj),
    /// The package Types keeps that name for a built-in type.
    Reserved,
    /// A built-in type stays in Types.
    BuiltIn,
}

/// Why `c` may not go under `owner`, or None when it may (being there already included).
pub fn owner_problem(store: &Store, c: Obj, owner: Obj) -> Option<OwnerProblem> {
    if crate::members::is_built_in(store, c) {
        return Some(OwnerProblem::BuiltIn);
    }
    if owner == c {
        return Some(OwnerProblem::Itself);
    }
    let kind = store.class_of(owner).ok()?;
    if kind != PACKAGE {
        if nested_within(store, c).contains(&owner) {
            return Some(OwnerProblem::Inside);
        }
        if !can_own(kind) {
            return Some(OwnerProblem::OwnsNone);
        }
        if !store.class_of(c).is_ok_and(can_nest) {
            return Some(OwnerProblem::NotNestable);
        }
    }
    let name = name_of(store, c);
    if kind == PACKAGE && crate::members::reserved_problem(store, owner, &name).is_some() {
        return Some(OwnerProblem::Reserved);
    }
    // An association class without a name clashes with none.
    if name.is_empty() {
        return None;
    }
    classifier_clash(store, owner, &name, Some(c)).map(OwnerProblem::NameTaken)
}

/// The refusal `problem` makes of `c` going under `owner`, in full.
pub fn problem_text(store: &Store, c: Obj, owner: Obj, problem: &OwnerProblem) -> String {
    let (cn, on) = (name_of(store, c), name_of(store, owner));
    match problem {
        OwnerProblem::Itself => format!("{cn} cannot own itself"),
        OwnerProblem::Inside => format!("{on} is nested in {cn}: owning {cn} would make a cycle"),
        OwnerProblem::OwnsNone => {
            format!("{on} is {} and owns no classifiers; a package, a class, an interface or an association class does", a_what(store, owner))
        }
        OwnerProblem::NotNestable => format!("{cn} is {}, which stays in a package and is never nested in a classifier", a_what(store, c)),
        OwnerProblem::NameTaken(s) => format!("{on} has {} named {} already", a_what(store, *s), name_of(store, *s)),
        OwnerProblem::Reserved => format!("the package {on} keeps the name {cn} for a built-in type"),
        OwnerProblem::BuiltIn => format!("{cn} is a built-in type, which stays in the package {}", crate::members::TYPES),
    }
}

/// Where `c` may be offered to go: every package, then every classifier that may own, by the
/// Owner selector's order, each with why it may not, when it may not. The classifiers nested in
/// `c` are never offered, since owning `c` would make a cycle; `c` itself is, as the reason.
pub fn candidates(store: &Store, c: Obj) -> Vec<(Obj, Option<OwnerProblem>)> {
    let inside = nested_within(store, c);
    let mut out: Vec<(Obj, Option<OwnerProblem>)> = Vec::new();
    for pkg in crate::commands::all_packages(store) {
        out.push((pkg, owner_problem(store, c, pkg)));
    }
    for pkg in crate::commands::all_packages(store) {
        let mut stack: Vec<Obj> = crate::commands::classifiers(store, pkg).into_iter().rev().collect();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(o) = stack.pop() {
            if !seen.insert(o) || crate::members::is_built_in(store, o) {
                continue;
            }
            if !inside.contains(&o) {
                out.push((o, owner_problem(store, c, o)));
            }
            stack.extend(nested(store, o).into_iter().rev());
        }
    }
    out
}

/// The owner the id under `key` names: a package, or a classifier.
fn owner_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let obj = archforge_mof::Uuid::parse(id)
        .and_then(|u| store.by_uuid(u))
        .filter(|o| store.is_alive(*o))
        .ok_or_else(|| refuse(key, format!("there is no package or classifier {}", crate::commands::short(id))))?;
    if let Some(why) = crate::templates::parameter_element_problem(store, obj) {
        return Err(refuse(key, why));
    }
    match store.class_of(obj).ok() {
        Some(c) if c == PACKAGE || crate::is_classifier(c) => Ok(obj),
        _ => Err(refuse(key, format!("{id} is neither a package nor a classifier"))),
    }
}

/// Puts `c` under `owner`: into a package's elements or a classifier's nested classifiers, at the
/// end.
fn place(store: &mut Store, c: Obj, owner: Obj) -> Result<(), CommandError> {
    let prop = match store.class_of(owner).map_err(fail)? {
        k if k == PACKAGE => package::PACKAGED_ELEMENT,
        k => list_of(k).ok_or_else(|| refuse("owner", format!("{} owns no classifiers", name_of(store, owner))))?,
    };
    store.push(owner, prop, Value::Ref(c)).map_err(fail)?;
    Ok(())
}

/// `uml.nested.add`: a class, an interface, an enumeration or a data type nested in a class, an
/// interface or an association class, its name free among the owner's nested classifiers (other
/// kinds may share it, [`crate::names`]).
fn nested_add(ctx: &mut C<'_>, p: &Params) -> R {
    let owner = classifier_of(ctx.doc, p, "owner")?;
    let owner_class = ctx.doc.class_of(owner).map_err(fail)?;
    if !can_own(owner_class) {
        return Err(refuse(
            "owner",
            format!(
                "{} is {} and owns no classifiers; a class, an interface or an association class does",
                name_of(ctx.doc, owner),
                a_what(ctx.doc, owner)
            ),
        ));
    }
    let kind = text(p, "kind");
    let Some((_, metaclass)) = KINDS.iter().find(|(k, _)| *k == kind) else {
        let names: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
        return Err(refuse("kind", format!("{kind:?} is not one of {}", names.join(", "))));
    };
    let name = text(p, "name");
    check_name(ctx.doc, Some(owner), *metaclass, name, None)?;
    let c = ctx.doc.create(*metaclass).map_err(fail)?;
    set_name(ctx.doc, c, name)?;
    place(ctx.doc, c, owner)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// `uml.classifier.set-owner`: a classifier moved under another owner, a package or a classifier,
/// as [`owner_problem`] allows; it keeps its id, its members, its relationships and every type
/// reference to it, and its cards stay on the diagrams that show it.
fn set_owner(ctx: &mut C<'_>, p: &Params) -> R {
    let c = classifier_of(ctx.doc, p, "classifier")?;
    let owner = owner_param(ctx.doc, p, "owner")?;
    if let Some(problem) = owner_problem(ctx.doc, c, owner) {
        let key = if problem == OwnerProblem::BuiltIn || problem == OwnerProblem::NotNestable { "classifier" } else { "owner" };
        let refusal = refuse(key, problem_text(ctx.doc, c, owner, &problem));
        return Err(match problem {
            OwnerProblem::NameTaken(s) => refusal.because(Reason::NameTaken { kind: what(ctx.doc, s), name: name_of(ctx.doc, s) }),
            _ => refusal,
        });
    }
    if owner_of(ctx.doc, c) != Some(owner) {
        // A move that would take stereotype applications out of their profile's scope is refused
        // (AR-134).
        let uses = crate::profiles::moving_uses(ctx.doc, c, owner);
        if !uses.is_empty() {
            return Err(refuse("owner", crate::profiles::stranding(ctx.doc, c, &uses)));
        }
        if let Some((from, prop)) = ctx.doc.container(c).map_err(fail)? {
            ctx.doc.remove_value(from, prop, &Value::Ref(c)).map_err(fail)?;
        }
        place(ctx.doc, c, owner)?;
    }
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// A classifier as the reads list it: its id, kind and name.
fn entry(store: &Store, c: Obj) -> Json {
    json!({ "id": uuid_of(store, c).unwrap_or_default(), "kind": what(store, c).replace(' ', "_"), "name": name_of(store, c) })
}

/// `uml.nested.list`: the classifiers a classifier nests, in its order, each with those it nests.
fn nested_list(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let owner = classifier_of(store, p, "owner")?;
    fn tree(store: &Store, o: Obj, depth: usize) -> Json {
        let mut row = entry(store, o);
        if depth < MAX_DEPTH
            && let Some(fields) = row.as_object_mut()
        {
            fields.insert("nested".into(), Json::Array(nested(store, o).into_iter().map(|n| tree(store, n, depth + 1)).collect()));
        }
        row
    }
    let items: Vec<Json> = nested(store, owner).into_iter().map(|n| tree(store, n, 1)).collect();
    Ok(json!({ "owner": uuid_of(store, owner)?, "nested": items }))
}

/// `uml.classifier.owners`: where a classifier may go, every package and every classifier that
/// may own one, each saying whether it may and, when not, why; those nested in it are left out.
fn owners_list(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let c = classifier_of(store, p, "classifier")?;
    let current = owner_of(store, c);
    let items: Vec<Json> = candidates(store, c)
        .into_iter()
        .map(|(o, problem)| {
            let mut row = entry(store, o);
            if let Some(fields) = row.as_object_mut() {
                fields.insert("current".into(), Json::Bool(Some(o) == current));
                fields.insert("allowed".into(), Json::Bool(problem.is_none()));
                if let Some(problem) = &problem {
                    fields.insert("reason".into(), Json::String(problem_text(store, c, o, problem)));
                }
            }
            row
        })
        .collect();
    let owner = current.map_or(Json::Null, |o| entry(store, o));
    Ok(json!({ "classifier": uuid_of(store, c)?, "owner": owner, "qualified_name": qualified_name(store, c), "owners": items }))
}

/// Whether a classifier that owns nested ones exists.
fn has_owners(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(can_own)) { Ok(()) } else { Err("There is no class or interface yet.") }
}

/// Whether a classifier exists.
fn has_classifiers(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(crate::is_classifier)) { Ok(()) } else { Err(crate::members::NO_CLASSES) }
}

/// The nested classifiers' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.nested.add",
            "Add nested classifier",
            &["Class", "Interface", "Enumeration", "Data type"],
            None,
            concat!(
                "owner: string; the id of the class, interface or association class that owns it (an enumeration, a data type or a primitive type owns none)\n",
                "kind: string = \"class\", class|interface|enumeration|data-type; what it is\n",
                "name: string; its name, free among the owner's nested classifiers whatever the case; an attribute or an operation of the owner may share it"
            ),
            has_owners,
            nested_add
        ),
        cmd!(
            "uml.classifier.set-owner",
            "Change owner",
            &["Class", "Interface", "Enumeration", "Data type", "Primitive type", "Association class"],
            None,
            concat!(
                "classifier: string; the classifier's id; it keeps its id, members, relationships and every type reference to it; a built-in type stays in Types\n",
                "owner: string; the id of a package, or of a class, an interface or an association class it is nested in; never the classifier itself nor one nested in it; only a class, an interface, an enumeration or a data type is nested in a classifier; its name must be free among the classifiers there, whatever the case (an attribute, an operation or a package may share it)"
            ),
            has_classifiers,
            set_owner
        ),
        cmd!(query "uml.nested.list", "List nested classifiers", &["Class", "Interface"], None, "owner: string; the id of the class, interface or association class", has_owners, nested_list),
        cmd!(
            query "uml.classifier.owners",
            "List possible owners",
            &["Class", "Interface", "Enumeration", "Data type", "Primitive type", "Association class"],
            None,
            "classifier: string; the classifier's id; its owner, its qualified name and every package and classifier it may be offered to go under, each allowed or with the reason it may not, those nested in it left out",
            has_classifiers,
            owners_list
        ),
    ]
}
