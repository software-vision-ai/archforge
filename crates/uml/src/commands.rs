//! UML's commands over the store: packages in the project's Model,
//! nested as deep as needed, and classes, interfaces, enumerations, data types and primitive types
//! in a package, added, renamed, moved and listed; an association class, made between two classes
//! (`relations`), renamed and moved here as the class it is. A package's classifiers share its names: no two
//! of them are named alike, whatever the case, so a type picked by its name is never in doubt, and
//! in the package Types the built-in types' names are kept for them. A built-in type keeps its
//! name and its package. Each
//! validates what it is given before it changes anything and refuses with a message naming the
//! key. Ids in parameters are the elements' UUIDs. Removing and describing go through the shared
//! `element.remove` and `element.describe` (a description is a Comment the element owns,
//! `descriptions`); a nested package removed hands what it holds up to its
//! parent (owner decision, for every notation).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, Reason, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::{ASSOCIATION_CLASS, CLASS, DATA_TYPE, ENUMERATION, INTERFACE, PACKAGE, PRIMITIVE_TYPE, enumeration, named_element, package};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The longest name of a package or a class.
pub const MAX_NAME: usize = 128;

/// Why a command is not available.
pub const NO_PACKAGES: &str = "There is no package yet.";

pub(crate) fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

pub(crate) fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

pub(crate) fn text<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

pub(crate) fn short(s: &str) -> String {
    s.chars().take(40).collect()
}

pub(crate) fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

pub(crate) fn name_of(store: &Store, obj: Obj) -> String {
    archforge_core::name_of(store, obj)
}

pub(crate) fn has_packages(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(PACKAGE)) { Ok(()) } else { Err(NO_PACKAGES) }
}

/// The live element the id under `key` names, of whatever class; never a template parameter's
/// classifier, which is its template's own (AR-130).
fn element_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    let obj = store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))?;
    match crate::templates::parameter_element_problem(store, obj) {
        Some(why) => Err(refuse(key, why)),
        None => Ok(obj),
    }
}

/// The element the id under `key` names, of `class` (`what` names it in a refusal).
pub(crate) fn id_of(store: &Store, p: &Params, key: &str, class: ClassId, what: &str) -> Result<Obj, CommandError> {
    let obj = element_of(store, p, key)?;
    if store.class_of(obj).ok() != Some(class) {
        return Err(refuse(key, format!("{} is not {what}", text(p, key))));
    }
    // A metaclass of the hidden UML library is UML's own, never the model's (AR-134).
    if let Some(why) = crate::profiles::library_problem(store, obj) {
        return Err(refuse(key, why));
    }
    Ok(obj)
}

/// What an element is, with its article: a class, an interface.
pub(crate) fn a_what(store: &Store, o: Obj) -> String {
    let what = what(store, o);
    let article = if what.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
    format!("{article} {what}")
}

/// The class, interface, enumeration, data type, primitive type or association class the id under
/// `key` names.
pub(crate) fn classifier_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let obj = element_of(store, p, key)?;
    if !store.class_of(obj).is_ok_and(crate::is_classifier) {
        return Err(refuse(
            key,
            format!("{} is not a class, an interface, an enumeration, a data type, a primitive type or an association class", text(p, key)),
        ));
    }
    if let Some(why) = crate::profiles::library_problem(store, obj) {
        return Err(refuse(key, why));
    }
    Ok(obj)
}

/// The project's Model, the root that holds the packages.
pub fn model(store: &Store) -> Result<Obj, CommandError> {
    store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_core::MODEL)).ok_or_else(|| CommandError::new("the project has no model root"))
}

/// The members of `owner`'s list `prop` of `class`.
pub(crate) fn members(store: &Store, owner: Obj, prop: archforge_mof::PropId, class: ClassId) -> Vec<Obj> {
    store
        .list(owner, prop)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| store.is_alive(*o) && store.class_of(*o).ok() == Some(class))
        .collect()
}

/// The top-level packages of the model, in its order.
pub fn packages(store: &Store) -> Vec<Obj> {
    model(store).map(|m| members(store, m, archforge_core::model::MEMBER, PACKAGE)).unwrap_or_default()
}

/// The packages a package holds, in its order.
pub fn subpackages(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, PACKAGE)
}

/// Every package of the model, each before the packages it holds.
pub fn all_packages(store: &Store) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut stack: Vec<Obj> = packages(store).into_iter().rev().collect();
    while let Some(p) = stack.pop() {
        out.push(p);
        stack.extend(subpackages(store, p).into_iter().rev());
    }
    out
}

/// The name of the tag that marks the model's root package (owner, 2026-11-04): one plain package,
/// named after the project, that holds every package and diagram without another home.
pub const ROOT_TAG: &str = "archforge.root";

/// The model's root package: the top-level package the root tag marks, if there is one yet.
pub fn root_package(store: &Store) -> Option<Obj> {
    let top = packages(store);
    let m = model(store).ok()?;
    store.list(m, archforge_core::model::TAG).ok()?.iter().filter_map(Value::as_ref).find_map(|tag| {
        let named = store.get(tag, archforge_core::tag::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).as_deref() == Some(ROOT_TAG);
        let element = store.list(tag, archforge_core::tag::ELEMENT).ok()?.iter().filter_map(Value::as_ref).next();
        element.filter(|e| named && top.contains(e))
    })
}

/// The root package, made when there is none: named after the project's model (a top-level
/// package of that name becomes it), marked with the root tag, and holding every other top-level
/// package whose name is free in it. Gives the root.
pub fn ensure_root(store: &mut Store) -> Result<Obj, CommandError> {
    if let Some(root) = root_package(store) {
        return Ok(root);
    }
    let m = model(store)?;
    let wanted = archforge_core::name_of(store, m);
    let wanted = if wanted.trim().is_empty() { "Model".to_string() } else { wanted };
    let top = packages(store);
    let root = match top.iter().copied().find(|p| name_of(store, *p).eq_ignore_ascii_case(&wanted)) {
        Some(existing) => existing,
        None => {
            let pkg = store.create(PACKAGE).map_err(fail)?;
            set_name(store, pkg, &wanted)?;
            place_package(store, pkg, None)?;
            pkg
        }
    };
    let tag = store.create(archforge_core::TAG).map_err(fail)?;
    store.set(tag, archforge_core::tag::NAME, Value::str(ROOT_TAG)).map_err(fail)?;
    store.push(tag, archforge_core::tag::ELEMENT, Value::Ref(root)).map_err(fail)?;
    store.push(m, archforge_core::model::TAG, Value::Ref(tag)).map_err(fail)?;
    for pkg in top.into_iter().filter(|p| *p != root) {
        if crate::names::clash(store, root, PACKAGE, &name_of(store, pkg), None).is_none() {
            detach(store, pkg)?;
            place_package(store, pkg, Some(root))?;
        }
    }
    Ok(root)
}

/// What holds a package in `parent`: the parent, or the model at the top level.
fn holder_of(store: &Store, parent: Option<Obj>) -> Option<Obj> {
    parent.or_else(|| model(store).ok())
}

/// Whether `package` is `ancestor` or sits inside it, at any depth.
pub fn is_within(store: &Store, package: Obj, ancestor: Obj) -> bool {
    let mut at = Some(package);
    for _ in 0..256 {
        match at {
            Some(p) if p == ancestor => return true,
            Some(p) => at = package_of(store, p),
            None => return false,
        }
    }
    false
}

/// The classes of a package, in its order.
pub fn classes(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, CLASS)
}

/// The interfaces of a package, in its order.
pub fn interfaces(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, INTERFACE)
}

/// The enumerations of a package, in its order.
pub fn enumerations(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, ENUMERATION)
}

/// The data types of a package, in its order: the DataType metaclass alone, neither its
/// enumerations nor its primitive types, which UML makes data types too.
pub fn data_types(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, DATA_TYPE)
}

/// The primitive types of a package, in its order: the PrimitiveType metaclass alone. The package
/// Types holds the built-in ones (`members::is_built_in`).
pub fn primitive_types(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, PRIMITIVE_TYPE)
}

/// The association classes of a package, in its order.
pub fn association_classes(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, ASSOCIATION_CLASS)
}

/// An enumeration's literals, in their order.
pub fn literals(store: &Store, enumeration: Obj) -> Vec<Obj> {
    members(store, enumeration, enumeration::OWNED_LITERAL, crate::ENUMERATION_LITERAL)
}

/// The classifiers of a package, its classes, interfaces, enumerations, data types, primitive types
/// (the package Types' built-in ones among them) and association classes, in its order: the names a
/// classifier's must differ from.
pub fn classifiers(store: &Store, package: Obj) -> Vec<Obj> {
    store
        .list(package, package::PACKAGED_ELEMENT)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| store.is_alive(*o) && store.class_of(*o).is_ok_and(crate::is_classifier))
        .collect()
}

/// What an element is, as a refusal names it: a class, an interface, an enumeration, a data type, a
/// primitive type, an association class, a package, a literal.
pub(crate) fn what(store: &Store, o: Obj) -> &'static str {
    match store.class_of(o).ok() {
        Some(c) if c == CLASS => "class",
        Some(c) if c == ASSOCIATION_CLASS => "association class",
        Some(c) if c == INTERFACE => "interface",
        Some(c) if c == ENUMERATION => "enumeration",
        Some(c) if c == DATA_TYPE => "data type",
        Some(c) if c == PRIMITIVE_TYPE => "primitive type",
        Some(c) if c == PACKAGE => "package",
        Some(c) if c == crate::ENUMERATION_LITERAL => "literal",
        Some(c) if c == crate::GENERALIZATION_SET => "generalization set",
        Some(c) if c == crate::PROPERTY => "attribute",
        Some(c) if c == crate::OPERATION => "operation",
        Some(c) if c == crate::CONSTRAINT => "constraint",
        Some(c) if c == crate::PROFILE => "profile",
        Some(c) if c == crate::STEREOTYPE => "stereotype",
        _ => "element",
    }
}

/// The package an element (a class, or a nested package) sits in directly; a classifier nested in
/// another has none of its own (`nesting::home_package` gives its owner's).
pub fn package_of(store: &Store, class: Obj) -> Option<Obj> {
    store.container(class).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).ok() == Some(PACKAGE))
}

/// A name an element of `class` (a package or a classifier) may have in `owner`: distinct, whatever
/// the case, from the names of the elements of its kind there ([`crate::names`]: a namespace tells
/// its members apart by kind, then by name, so a package and a class may share a name); a refusal
/// names what has it already. No owner, no clash.
pub(crate) fn check_name(store: &Store, owner: Option<Obj>, class: ClassId, name: &str, except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")).because(p.reason));
    }
    if let Some(s) = owner.and_then(|o| crate::names::clash(store, o, class, name, except)) {
        let existing = name_of(store, s);
        let message = format!("there is {} named {existing} already", a_what(store, s));
        return Err(refuse("name", message).because(Reason::NameTaken { kind: what(store, s), name: existing }));
    }
    Ok(())
}

pub(crate) fn set_name(store: &mut Store, obj: Obj, name: &str) -> Result<(), CommandError> {
    store.set(obj, named_element::NAME, Value::str(name)).map_err(fail)
}

/// The package under `key`, or none when it is empty: the model's top level.
/// The package `key` names, or the root package, made when there is none, when it names nothing.
fn parent_param(store: &mut Store, p: &Params, key: &str) -> Result<Option<Obj>, CommandError> {
    if text(p, key).is_empty() { ensure_root(store).map(Some) } else { id_of(store, p, key, PACKAGE, "a package").map(Some) }
}

fn model_root(ctx: &mut C<'_>, _p: &Params) -> R {
    let root = ensure_root(ctx.doc)?;
    Ok(json!({ "id": uuid_of(ctx.doc, root)? }))
}

/// The class diagram a package leads to (AR-95): the first of the package's class diagrams.
pub fn diagram_of(store: &Store, package: Obj) -> Option<Obj> {
    use archforge_diagram::diagram;
    store.alive().find(|d| {
        store.class_of(*d).ok() == Some(archforge_diagram::DIAGRAM)
            && store.get(*d, diagram::KIND).ok().and_then(|v| v.as_str().map(|k| k == crate::CLASS_DIAGRAM)).unwrap_or(false)
            && store.get(*d, diagram::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(package)
    })
}

/// The package's diagram, made when it has none: a class diagram of the package named like it,
/// beside the model's other diagrams. `made` says which.
fn package_diagram(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "package")?;
    if let Some(d) = diagram_of(ctx.doc, pkg) {
        return Ok(json!({ "id": uuid_of(ctx.doc, d)?, "made": false }));
    }
    let m = model(ctx.doc)?;
    let name: String = name_of(ctx.doc, pkg).chars().take(archforge_diagram::rules::MAX_NAME).collect();
    // Made as diagram.add makes one, its lines at right angles around the classes.
    let d = archforge_diagram::commands::new_diagram(ctx.doc, crate::CLASS_DIAGRAM, &name, Some(pkg), Some((m, archforge_core::model::MEMBER)))
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, d)?, "made": true }))
}

/// Puts `pkg` into `parent`, or at the model's top level.
fn place_package(store: &mut Store, pkg: Obj, parent: Option<Obj>) -> Result<(), CommandError> {
    match parent {
        Some(parent) => store.push(parent, package::PACKAGED_ELEMENT, Value::Ref(pkg)).map_err(fail)?,
        None => store.push(model(store)?, archforge_core::model::MEMBER, Value::Ref(pkg)).map_err(fail)?,
    };
    Ok(())
}

/// `uml.package.add`: a package in its parent, or in the root package, its name free there.
fn package_add(ctx: &mut C<'_>, p: &Params) -> R {
    let parent = parent_param(ctx.doc, p, "parent")?;
    let name = text(p, "name");
    check_name(ctx.doc, holder_of(ctx.doc, parent), PACKAGE, name, None)?;
    let pkg = ctx.doc.create(PACKAGE).map_err(fail)?;
    set_name(ctx.doc, pkg, name)?;
    place_package(ctx.doc, pkg, parent)?;
    ctx.view.selection = vec![pkg];
    Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }))
}

/// `uml.package.rename`: a package renamed, the diagrams still named like it following. The
/// package Types keeps its name while it holds built-in types, and a package becomes Types only as
/// `members::types_problem` allows.
fn package_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let name = text(p, "name");
    if let Some(why) = crate::members::types_problem(ctx.doc, pkg, name, package_of(ctx.doc, pkg)) {
        return Err(refuse("name", why));
    }
    check_name(ctx.doc, holder_of(ctx.doc, package_of(ctx.doc, pkg)), PACKAGE, name, Some(pkg))?;
    let old = name_of(ctx.doc, pkg);
    set_name(ctx.doc, pkg, name)?;
    // Its class diagrams still named like it follow the new name (AR-95: a package's own diagram).
    let follows: Vec<Obj> = ctx
        .doc
        .alive()
        .filter(|d| ctx.doc.class_of(*d).ok() == Some(archforge_diagram::DIAGRAM))
        .filter(|d| ctx.doc.get(*d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(pkg))
        .filter(|d| {
            ctx.doc.get(*d, archforge_diagram::diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).as_deref() == Some(old.as_str())
        })
        .collect();
    let diagram_name: String = name.chars().take(archforge_diagram::rules::MAX_NAME).collect();
    for d in follows {
        ctx.doc.set(d, archforge_diagram::diagram::NAME, Value::str(&diagram_name)).map_err(fail)?;
    }
    Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }))
}

/// Takes `obj` out of whatever holds it, so it can be put elsewhere.
fn detach(store: &mut Store, obj: Obj) -> Result<(), CommandError> {
    if let Some((owner, prop)) = store.container(obj).map_err(fail)? {
        store.remove_value(owner, prop, &Value::Ref(obj)).map_err(fail)?;
    }
    Ok(())
}

/// `uml.package.move`: a package into another, never into itself, its cards leaving its old home's
/// diagrams. The package Types stays under the root while it holds built-in types, and a package
/// named Types goes there only as `members::types_problem` allows.
fn package_move(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    if Some(pkg) == root_package(ctx.doc) {
        return Err(refuse("package", "the model's root package holds the others and stays where it is"));
    }
    let to = parent_param(ctx.doc, p, "to")?;
    if to.is_some_and(|t| is_within(ctx.doc, t, pkg)) {
        return Err(refuse("to", "a package cannot go into itself or a package it holds").because(Reason::PackageIntoItself));
    }
    if to == package_of(ctx.doc, pkg) {
        return Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }));
    }
    if let Some(why) = crate::members::types_problem(ctx.doc, pkg, &name_of(ctx.doc, pkg), to) {
        return Err(refuse("to", why));
    }
    check_name(ctx.doc, holder_of(ctx.doc, to), PACKAGE, &name_of(ctx.doc, pkg), Some(pkg))?;
    if let Some(to) = to {
        let uses = crate::profiles::moving_uses(ctx.doc, pkg, to);
        if !uses.is_empty() {
            return Err(refuse("to", crate::profiles::stranding(ctx.doc, pkg, &uses)));
        }
    }
    detach(ctx.doc, pkg)?;
    place_package(ctx.doc, pkg, to)?;
    // Its cards go from the diagrams of a package that no longer holds it (AR-102): a class
    // diagram shows the packages its own package holds. Its own diagram and what it holds stay.
    let parent = package_of(ctx.doc, pkg);
    let (_, _, nodes) = archforge_diagram::views_of(ctx.doc, pkg);
    for n in nodes {
        let shown_on = ctx.doc.container(n).map_err(fail)?.map(|(d, _)| d);
        let of = shown_on.and_then(|d| ctx.doc.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()));
        if of != parent && ctx.doc.is_alive(n) {
            archforge_diagram::destroy_node_with_edges(ctx.doc, n).map_err(fail)?;
        }
    }
    ctx.view.selection = vec![pkg];
    Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }))
}

/// The classifier under `key` (of `kind`, `what` naming it) moves into the package under
/// `package`, its name free among that package's classifiers and, in the package Types, none
/// of the built-in types'. A built-in type stays in Types; an association class without a name
/// clashes with none.
fn classifier_move(ctx: &mut C<'_>, p: &Params, kind: ClassId, key: &str, what: &str) -> R {
    let c = id_of(ctx.doc, p, key, kind, what)?;
    let to = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    if package_of(ctx.doc, c) == Some(to) {
        return Ok(json!({ "id": uuid_of(ctx.doc, c)? }));
    }
    if let Some(why) = crate::members::built_in_problem(ctx.doc, c, "moved") {
        return Err(refuse(key, why));
    }
    let name = name_of(ctx.doc, c);
    if let Some(why) = crate::members::reserved_problem(ctx.doc, to, &name) {
        return Err(refuse("package", format!("{}: {why}", uuid_of(ctx.doc, c)?)));
    }
    if !(name.is_empty() && kind == ASSOCIATION_CLASS) {
        check_name(ctx.doc, Some(to), kind, &name, Some(c))?;
    }
    // A move that would take stereotype applications out of their profile's scope is refused
    // (AR-134).
    let uses = crate::profiles::moving_uses(ctx.doc, c, to);
    if !uses.is_empty() {
        return Err(refuse("package", crate::profiles::stranding(ctx.doc, c, &uses)));
    }
    detach(ctx.doc, c)?;
    ctx.doc.push(to, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// A class, an interface, an enumeration, a data type or a primitive type (`kind`) named `name` in
/// the package under `package`; in the package Types, never a built-in type's name.
fn classifier_add(ctx: &mut C<'_>, p: &Params, kind: ClassId) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let name = text(p, "name");
    if let Some(why) = crate::members::reserved_problem(ctx.doc, pkg, name) {
        return Err(refuse("name", why));
    }
    check_name(ctx.doc, Some(pkg), kind, name, None)?;
    let c = ctx.doc.create(kind).map_err(fail)?;
    set_name(ctx.doc, c, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// The classifier under `key` renamed, its new name free among its package's classifiers and, in
/// the package Types, none of the built-in types'. A built-in type keeps its name. An association
/// class may go without a name, as an association may.
fn classifier_rename(ctx: &mut C<'_>, p: &Params, kind: ClassId, key: &str, what: &str) -> R {
    let c = id_of(ctx.doc, p, key, kind, what)?;
    if let Some(why) = crate::members::built_in_problem(ctx.doc, c, "renamed") {
        return Err(refuse(key, why));
    }
    let name = text(p, "name");
    if name.is_empty() && kind == ASSOCIATION_CLASS {
        set_name(ctx.doc, c, name)?;
        return Ok(json!({ "id": uuid_of(ctx.doc, c)? }));
    }
    if let Some(why) = package_of(ctx.doc, c).and_then(|pkg| crate::members::reserved_problem(ctx.doc, pkg, name)) {
        return Err(refuse("name", why));
    }
    // A nested classifier's name is told apart from its owner's other classifiers (AR-132).
    check_name(ctx.doc, crate::nesting::owner_of(ctx.doc, c), kind, name, Some(c))?;
    set_name(ctx.doc, c, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// `uml.class.move`: a class into another package.
fn class_move(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_move(ctx, p, CLASS, "class", "a class")
}

/// `uml.class.add`: a class in a package.
fn class_add(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_add(ctx, p, CLASS)
}

/// `uml.class.rename`: a class renamed.
fn class_rename(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_rename(ctx, p, CLASS, "class", "a class")
}

/// `uml.interface.move`: an interface into another package.
fn interface_move(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_move(ctx, p, INTERFACE, "interface", "an interface")
}

/// `uml.interface.add`: an interface in a package.
fn interface_add(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_add(ctx, p, INTERFACE)
}

/// `uml.interface.rename`: an interface renamed.
fn interface_rename(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_rename(ctx, p, INTERFACE, "interface", "an interface")
}

/// `uml.enumeration.move`: an enumeration into another package.
fn enumeration_move(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_move(ctx, p, ENUMERATION, "enumeration", "an enumeration")
}

/// `uml.enumeration.add`: an enumeration in a package, with no literals yet.
fn enumeration_add(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_add(ctx, p, ENUMERATION)
}

/// `uml.enumeration.rename`: an enumeration renamed; what it types keeps it.
fn enumeration_rename(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_rename(ctx, p, ENUMERATION, "enumeration", "an enumeration")
}

/// `uml.data-type.move`: a data type into another package; what it types keeps it.
fn data_type_move(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_move(ctx, p, DATA_TYPE, "data_type", "a data type")
}

/// `uml.data-type.add`: a data type in a package, with no attributes or operations yet.
fn data_type_add(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_add(ctx, p, DATA_TYPE)
}

/// `uml.data-type.rename`: a data type renamed; what it types keeps it.
fn data_type_rename(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_rename(ctx, p, DATA_TYPE, "data_type", "a data type")
}

/// `uml.primitive-type.move`: a primitive type of the model's own into another package; what it
/// types keeps it. A built-in type stays in Types.
fn primitive_type_move(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_move(ctx, p, PRIMITIVE_TYPE, "primitive_type", "a primitive type")
}

/// `uml.primitive-type.add`: a primitive type of the model's own in a package; it has no members.
fn primitive_type_add(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_add(ctx, p, PRIMITIVE_TYPE)
}

/// `uml.primitive-type.rename`: a primitive type of the model's own renamed; what it types keeps
/// it. A built-in type keeps its name.
fn primitive_type_rename(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_rename(ctx, p, PRIMITIVE_TYPE, "primitive_type", "a primitive type")
}

/// `uml.association-class.rename`: an association class renamed as the class it is, its name free
/// among its package's classifiers, or taken away.
fn association_class_rename(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_rename(ctx, p, ASSOCIATION_CLASS, "association_class", "an association class")
}

/// `uml.association-class.move`: an association class into another package; its ends stay at the
/// classes they join.
fn association_class_move(ctx: &mut C<'_>, p: &Params) -> R {
    classifier_move(ctx, p, ASSOCIATION_CLASS, "association_class", "an association class")
}

/// `uml.package.list`: every package with its parent, its description (AR-186) and how many
/// classes, interfaces, enumerations, data types, primitive types and association classes it holds.
fn package_list(ctx: &mut C<'_>, _: &Params) -> R {
    let store = &*ctx.doc;
    let items: Vec<Json> = all_packages(store)
        .into_iter()
        .map(|pkg| {
            let parent = package_of(store, pkg).and_then(|p| uuid_of(store, p).ok());
            json!({
                "id": uuid_of(store, pkg).unwrap_or_default(), "name": name_of(store, pkg), "parent": parent,
                "description": crate::descriptions::description_of(store, pkg),
                "classes": classes(store, pkg).len(), "interfaces": interfaces(store, pkg).len(),
                "enumerations": enumerations(store, pkg).len(), "data_types": data_types(store, pkg).len(),
                "primitive_types": primitive_types(store, pkg).len(), "association_classes": association_classes(store, pkg).len(),
            })
        })
        .collect();
    Ok(json!({ "packages": items }))
}

/// `uml.class.list`: a package's classes, in its order.
fn class_list(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let store = &*ctx.doc;
    let items: Vec<Json> =
        classes(store, pkg).into_iter().map(|c| json!({ "id": uuid_of(store, c).unwrap_or_default(), "name": name_of(store, c) })).collect();
    Ok(json!({ "classes": items }))
}

/// `uml.interface.list`: a package's interfaces, in its order.
fn interface_list(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let store = &*ctx.doc;
    let items: Vec<Json> =
        interfaces(store, pkg).into_iter().map(|c| json!({ "id": uuid_of(store, c).unwrap_or_default(), "name": name_of(store, c) })).collect();
    Ok(json!({ "interfaces": items }))
}

/// `uml.enumeration.list`: a package's enumerations, in its order, each with its literals' names.
fn enumeration_list(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let store = &*ctx.doc;
    let items: Vec<Json> = enumerations(store, pkg)
        .into_iter()
        .map(|e| {
            let literals: Vec<String> = literals(store, e).into_iter().map(|l| name_of(store, l)).collect();
            json!({ "id": uuid_of(store, e).unwrap_or_default(), "name": name_of(store, e), "literals": literals })
        })
        .collect();
    Ok(json!({ "enumerations": items }))
}

/// `uml.data-type.list`: a package's data types, in its order, each with how many attributes and
/// operations it has; `uml.class.get` reads one with its members.
fn data_type_list(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let store = &*ctx.doc;
    let items: Vec<Json> = data_types(store, pkg)
        .into_iter()
        .map(|t| {
            json!({
                "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t),
                "attributes": crate::members::attributes(store, t).len(), "operations": crate::members::operations(store, t).len(),
            })
        })
        .collect();
    Ok(json!({ "data_types": items }))
}

/// `uml.primitive-type.list`: a package's primitive types, in its order, each saying whether it is
/// a built-in type (the package Types holds those).
fn primitive_type_list(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let store = &*ctx.doc;
    let items: Vec<Json> = primitive_types(store, pkg)
        .into_iter()
        .map(|t| json!({ "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t), "built_in": crate::members::is_built_in(store, t) }))
        .collect();
    Ok(json!({ "primitive_types": items }))
}

/// What a nested package hands up to its parent when it is removed: everything it holds.
fn held(store: &Store, pkg: Obj) -> Vec<Obj> {
    store.list(pkg, package::PACKAGED_ELEMENT).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|o| store.is_alive(*o)).collect()
}

/// Every classifier inside a package, at any depth, those nested in its classifiers among them
/// (AR-132).
pub(crate) fn classifiers_within(store: &Store, pkg: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    for c in classifiers(store, pkg) {
        out.push(c);
        out.extend(crate::nesting::nested_within(store, c));
    }
    for sub in subpackages(store, pkg) {
        out.extend(classifiers_within(store, sub));
    }
    out
}

/// Why `parent` cannot take what `pkg` holds, when a name would clash there with one of its kind
/// ([`crate::names`]), a built-in type's name would land in the package Types, or a package handed
/// up would become a second Types or a Types holding such a name. A clash is typed, naming the
/// element of `parent` that has the name.
fn hand_up_problem(store: &Store, pkg: Obj, parent: Obj) -> Result<(), CommandError> {
    let up = |why: String| refuse("ids", format!("its contents cannot move up: {why}"));
    for o in held(store, pkg) {
        let n = name_of(store, o);
        let Ok(class) = store.class_of(o) else { continue };
        // The package Types keeps the built-in types' names, and one package is Types.
        if crate::is_classifier(class)
            && let Some(why) = crate::members::reserved_problem(store, parent, &n)
        {
            return Err(up(why));
        }
        if class == PACKAGE
            && let Some(why) = crate::members::types_problem(store, o, &n, Some(parent))
        {
            return Err(up(why));
        }
        // The package itself goes, so its own name is free for what it hands up.
        if let Some(s) = crate::names::clash(store, parent, class, &n, Some(pkg)) {
            let existing = name_of(store, s);
            let why = format!("{} has {} named {existing} already", name_of(store, parent), a_what(store, s));
            return Err(up(why).because(Reason::NameTaken { kind: what(store, s), name: existing }));
        }
    }
    Ok(())
}

/// Removes UML elements (packages, classes, interfaces, enumerations, data types, attributes,
/// operations, literals, primitive types, associations, generalizations, dependencies, interface
/// realizations, association classes, constraints, generalization sets, template parameters,
/// profiles, stereotypes and template bindings), each with what it holds (an enumeration its literals, a
/// data type or an association class its attributes and operations, an association its ends) and the
/// diagrams, nodes and lines that show them (`element.remove`, through [`crate::UmlNotation`]). A
/// nested package hands what it holds, and its diagrams, up to its parent first (owner decision); a
/// top-level package takes them along. A classifier takes the relationships that touch it along,
/// and an association class among them takes its own ([`going_with`]); a member's type that goes is
/// dropped from what had it, and so is a default that is a literal that goes (AR-126), as the
/// property's own type would be. An element a constraint names leaves the constraint's elements, and
/// a constraint left naming none goes too, in the same step (AR-133,
/// [`crate::constraints::dropped_by`]). A generalization set goes alone, keeping its generalizations;
/// a generalization leaves its sets, and a set left with no member goes too, in the same step
/// (AR-129, [`crate::generalization_sets::emptied_by`]). A template binding goes with its
/// substitutions, and with its bound element or its template; a classifier a binding that stays
/// uses as an argument is refused, naming the binding (AR-131,
/// [`crate::bindings::argument_removal_problem`]).
pub fn remove_elements(store: &mut Store, objs: &[Obj]) -> Result<usize, CommandError> {
    // Every id is checked before anything goes, so a refusal leaves the store as it was.
    let removable = [
        PACKAGE,
        CLASS,
        INTERFACE,
        ENUMERATION,
        DATA_TYPE,
        crate::PROPERTY,
        crate::OPERATION,
        crate::ENUMERATION_LITERAL,
        crate::PRIMITIVE_TYPE,
        crate::ASSOCIATION,
        crate::GENERALIZATION,
        crate::DEPENDENCY,
        crate::INTERFACE_REALIZATION,
        ASSOCIATION_CLASS,
        crate::CONSTRAINT,
        crate::GENERALIZATION_SET,
        crate::CLASSIFIER_TEMPLATE_PARAMETER,
        crate::PROFILE,
        crate::STEREOTYPE,
        crate::TEMPLATE_BINDING,
    ];
    // Template parameters a binding stands in the way of stay: one it substitutes, or a template's
    // last while a binding names its signature (AR-130). A template going too takes its parameters
    // and the bindings to it along, so its own are not asked.
    let params = removed_parameters(store, objs);
    if let Some(why) = crate::templates::removals_problem(store, &params) {
        return Err(refuse("ids", why));
    }
    for o in objs.iter().filter(|o| store.is_alive(**o)) {
        // A template parameter's classifier is its parameter's: the parameter is what goes (AR-130).
        if let Some(why) = crate::templates::parameter_element_problem(store, *o) {
            return Err(refuse("ids", why));
        }
        let class = store.class_of(*o).map_err(fail)?;
        // An association's end goes with its association.
        let end = class == crate::PROPERTY && store.get(*o, crate::property::ASSOCIATION).ok().and_then(|v| v.as_ref()).is_some();
        if end && crate::nary::association_of_end(store, *o).is_some() {
            return Err(refuse(
                "ids",
                format!("{} is an end of an n-ary association; remove it with uml.association.remove-end", uuid_of(store, *o)?),
            ));
        }
        // A profile, a stereotype or a property in use, a package whose applications its contents
        // need, and UML's own metaclasses stay, the uses named (AR-134).
        if let Some(refused) = crate::profiles::removal_problem(store, *o) {
            return Err(refused);
        }
        if !removable.contains(&class) || end {
            return Err(refuse("ids", format!("{} cannot be removed yet", uuid_of(store, *o)?)));
        }
        if Some(*o) == root_package(store) {
            return Err(refuse("ids", "the model's root package holds the whole model and stays"));
        }
        // A built-in type, and a package holding one, stay.
        if let Some(why) = crate::members::removal_problem(store, *o) {
            return Err(refuse("ids", why));
        }
        // Only a classifier's template parameter is removed (AR-130).
        if class == crate::CLASSIFIER_TEMPLATE_PARAMETER && !crate::templates::is_parameter(store, *o) {
            return Err(refuse("ids", format!("{} cannot be removed yet", uuid_of(store, *o)?)));
        }
        if class == PACKAGE
            && let Some(parent) = package_of(store, *o)
        {
            hand_up_problem(store, *o, parent)?;
        }
    }
    // A surviving template parameter's default, or a binding's argument, that would no longer
    // specialize its constraint once what goes has gone is refused before anything goes (AR-130).
    let (gone_classes, gone_generalizations) = going_for_constraints(store, objs);
    if let Some(why) = crate::templates::constraint_problem(store, &gone_classes, &gone_generalizations, &params) {
        return Err(refuse("ids", why));
    }
    // A classifier a binding that stays uses as an argument stays too (AR-131): the binding would
    // be left naming nothing for its parameter.
    let classes: Vec<Obj> = gone_classes.iter().copied().collect();
    let going: Vec<Obj> = going_with(store, &classes).into_iter().chain(objs.iter().copied()).collect();
    if let Some(why) = crate::bindings::argument_removal_problem(store, &classes, &going) {
        return Err(refuse("ids", why));
    }
    // A literal a stereotype's value names stays while the element carrying that value stays
    // (AR-135): the value would go with it unseen.
    let going: Vec<Obj> = gone_classes.iter().chain(&gone_generalizations).copied().chain(going_with(store, &classes)).collect();
    if let Some(why) = crate::applications::literal_problem(store, |o| goes_by(store, objs, &going, o)) {
        return Err(refuse("ids", why));
    }
    // A classifier's nested classifiers go with it (AR-132), named among what goes so their
    // relationships, constraints and sets are worked out as theirs.
    let mut objs: Vec<Obj> = objs.to_vec();
    let nested: Vec<Obj> = objs
        .iter()
        .filter(|o| store.is_alive(**o) && store.class_of(**o).is_ok_and(crate::is_classifier))
        .flat_map(|o| crate::nesting::nested_within(store, *o))
        .collect();
    for n in nested {
        if !objs.contains(&n) {
            objs.push(n);
        }
    }
    // Template parameters go first, each signature left with none going with its last (AR-130);
    // those of a template that goes too go with it.
    let params = removed_parameters(store, &objs);
    let mut gone = crate::templates::remove(store, &params)?;
    objs.retain(|o| !params.contains(o) && !crate::templates::is_parameter(store, *o));
    let objs = objs.as_slice();
    // A constraint left constraining nothing goes with what it constrained (AR-133), worked out
    // before anything moves; the others lose the elements that go as the core destroys them.
    let orphans = crate::constraints::orphans(&crate::constraints::dropped_by(store, objs));
    // A nested package's contents and diagrams move up to its parent before it goes.
    let nested: Vec<Obj> = objs.iter().copied().filter(|o| store.is_alive(*o) && store.class_of(*o).ok() == Some(PACKAGE)).collect();
    for o in nested {
        let Some(parent) = package_of(store, o) else { continue };
        let (diagrams, _, _) = archforge_diagram::views_of(store, o);
        store
            .transaction(|st| {
                for child in held(st, o) {
                    st.remove_value(o, package::PACKAGED_ELEMENT, &Value::Ref(child))?;
                    st.push(parent, package::PACKAGED_ELEMENT, Value::Ref(child))?;
                }
                for d in diagrams {
                    st.set(d, archforge_diagram::diagram::ELEMENT, Value::Ref(parent))?;
                }
                Ok(())
            })
            .map_err(fail)?;
    }
    // The relationships of every classifier that goes, a top-level package's among them,
    // go first: a generalization requires its general classifier.
    let mut classes: Vec<Obj> = Vec::new();
    for o in objs.iter().filter(|o| store.is_alive(**o)) {
        match store.class_of(*o).ok() {
            Some(c) if crate::is_classifier(c) => classes.push(*o),
            Some(c) if c == PACKAGE => classes.extend(classifiers_within(store, *o)),
            _ => {}
        }
    }
    let mut all = going_with(store, &classes);
    all.extend(objs);
    all.extend(orphans);
    // A generalization set left with no member goes with its last one (AR-129), in the same step;
    // removing a set keeps its generalizations.
    let emptied = crate::generalization_sets::emptied_by(store, &all);
    all.extend(emptied);
    // A stereotype application goes with the element it applies to (AR-134).
    let applications = crate::profiles::applications_on(store, &all);
    all.extend(applications);
    // A profile a package with no parent takes along has its applications taken away first, as a
    // profile removed on its own has; its uses were refused above (AR-134).
    let taken: Vec<Obj> = objs
        .iter()
        .filter(|o| store.is_alive(**o) && store.class_of(**o).ok() == Some(PACKAGE) && package_of(store, **o).is_none())
        .flat_map(|o| crate::profiles::profiles_within(store, *o))
        .collect();
    for profile in taken {
        crate::profiles::before_profile_removal(store, profile)?;
    }
    // A default that is a literal goes with the literal: an InstanceValue needs its instance.
    let defaults = crate::modifiers::literal_defaults(store);
    for o in all {
        if !store.is_alive(o) {
            continue;
        }
        // A profile's applications, and a stereotype's extensions, go before it (AR-134).
        match store.class_of(o).ok() {
            Some(c) if c == crate::PROFILE => crate::profiles::before_profile_removal(store, o)?,
            Some(c) if c == crate::STEREOTYPE => crate::profiles::before_stereotype_removal(store, o)?,
            _ => {}
        }
        gone += store
            .transaction(|st| {
                for (value, literal) in &defaults {
                    if st.is_alive(*value) && (*literal == o || st.contains(o, *literal)) {
                        st.destroy(*value)?;
                    }
                }
                let views = archforge_diagram::remove_views_of(st, o)?;
                Ok(views + archforge_core::destroy_with_links(st, o)?)
            })
            .map_err(fail)?;
    }
    Ok(gone)
}

/// What removing `objs` takes away that a specialization path may run through, worked out before
/// anything changes (AR-130): the classifiers that go (those named, what they nest, and what a
/// top-level package holds; a nested package hands its contents up, so they stay), and the
/// generalizations that go (those named, and those that go with the classifiers).
fn going_for_constraints(store: &Store, objs: &[Obj]) -> (std::collections::BTreeSet<Obj>, std::collections::BTreeSet<Obj>) {
    let mut classes = std::collections::BTreeSet::new();
    for o in objs.iter().copied().filter(|o| store.is_alive(*o)) {
        match store.class_of(o).ok() {
            Some(c) if crate::is_classifier(c) => {
                classes.insert(o);
                classes.extend(crate::nesting::nested_within(store, o));
            }
            Some(c) if c == PACKAGE && package_of(store, o).is_none() => classes.extend(classifiers_within(store, o)),
            _ => {}
        }
    }
    let list: Vec<Obj> = classes.iter().copied().collect();
    let generalizations = going_with(store, &list)
        .into_iter()
        .chain(objs.iter().copied())
        .filter(|g| store.is_alive(*g) && store.class_of(*g).ok() == Some(crate::GENERALIZATION))
        .collect();
    (classes, generalizations)
}

/// Whether `o` goes when `objs` are removed, `going` being the classifiers, generalizations and
/// relationships that go with them ([`going_for_constraints`], [`going_with`]): it is one of them, or
/// inside one; what a nested package holds is handed up and stays.
fn goes_by(store: &Store, objs: &[Obj], going: &[Obj], o: Obj) -> bool {
    let handed_up = |x: Obj| store.class_of(x).ok() == Some(PACKAGE) && package_of(store, x).is_some();
    objs.iter().any(|x| *x == o || (store.contains(*x, o) && !handed_up(*x))) || going.iter().any(|x| *x == o || store.contains(*x, o))
}

/// The template parameters among `objs` whose template does not go with them: a template that
/// goes, named among `objs` or inside one of them, takes its parameters along (AR-130).
fn removed_parameters(store: &Store, objs: &[Obj]) -> Vec<Obj> {
    objs.iter()
        .copied()
        .filter(|o| crate::templates::template_of(store, *o).is_some_and(|t| !objs.iter().any(|x| *x == t || store.contains(*x, t))))
        .collect()
}

/// The relationships that go when `classes` go: those that touch any of them or a classifier
/// nested in one (AR-132), and, since an association class is a classifier too, those that touch
/// an association class going with them, and so on, each once. A seen-set bounds the walk.
pub fn going_with(store: &Store, classes: &[Obj]) -> Vec<Obj> {
    let mut seen: std::collections::BTreeSet<Obj> = classes.iter().copied().collect();
    let mut queue: Vec<Obj> = classes.to_vec();
    let mut out = Vec::new();
    while let Some(c) = queue.pop() {
        for n in crate::nesting::nested(store, c) {
            if seen.insert(n) {
                queue.push(n);
            }
        }
        // The bindings to a template go with it (AR-130): a binding needs its signature.
        let bindings = crate::templates::bindings_to(store, c);
        for r in crate::relations::relationships_of(store, c).into_iter().chain(bindings) {
            if seen.insert(r) {
                out.push(r);
                if store.class_of(r).ok() == Some(ASSOCIATION_CLASS) {
                    queue.push(r);
                }
            }
        }
    }
    out
}

/// UML's own commands; the edit area, the shared element commands, the diagrams' and the skills'
/// join them in a registry.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.package.add",
            "Add package",
            &["Package"],
            None,
            "name: string; the package's name\nparent: string = \"\"; the id of the package it goes into, or nothing for the model's root package (made when there is none)",
            always,
            package_add
        ),
        cmd!("uml.model.root", "Make the root package", &["Package"], None, "", always, model_root),
        cmd!(
            "uml.package.diagram",
            "Package's diagram",
            &["Package", "Diagram"],
            None,
            "package: string; the package's id",
            has_packages,
            package_diagram
        ),
        cmd!(
            "uml.package.move",
            "Move package",
            &["Package"],
            None,
            "package: string; the package's id; the package Types stays directly under the root package while it holds the built-in types\nto: string = \"\"; the id of the package it goes into, or nothing for the model's root package; a package named Types goes directly under the root only while there is no package Types and it holds no classifier with a built-in type's name",
            has_packages,
            package_move
        ),
        cmd!(
            "uml.class.move",
            "Move class",
            &["Class"],
            None,
            "class: string; the class's id\npackage: string; the id of the package it goes into, where its name is free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            class_move
        ),
        cmd!(
            "uml.package.rename",
            "Rename package",
            &["Package"],
            None,
            "package: string; the package's id; the package Types keeps its name while it holds the built-in types\nname: string; the new name; a package directly under the root is named Types only while there is no package Types and it holds no classifier with a built-in type's name",
            has_packages,
            package_rename
        ),
        cmd!(
            "uml.class.add",
            "Add class",
            &["Class"],
            None,
            "package: string; the package's id\nname: string; the class's name, free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            class_add
        ),
        cmd!(
            "uml.class.rename",
            "Rename class",
            &["Class"],
            None,
            "class: string; the class's id\nname: string; the new name, free among its package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            class_rename
        ),
        cmd!(
            "uml.interface.add",
            "Add interface",
            &["Interface"],
            None,
            "package: string; the package's id\nname: string; the interface's name, free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            interface_add
        ),
        cmd!(
            "uml.interface.rename",
            "Rename interface",
            &["Interface"],
            None,
            "interface: string; the interface's id\nname: string; the new name, free among its package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            interface_rename
        ),
        cmd!(
            "uml.interface.move",
            "Move interface",
            &["Interface"],
            None,
            "interface: string; the interface's id\npackage: string; the id of the package it goes into, where its name is free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            interface_move
        ),
        cmd!(
            "uml.enumeration.add",
            "Add enumeration",
            &["Enumeration"],
            None,
            "package: string; the package's id\nname: string; the enumeration's name, free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            enumeration_add
        ),
        cmd!(
            "uml.enumeration.rename",
            "Rename enumeration",
            &["Enumeration"],
            None,
            "enumeration: string; the enumeration's id\nname: string; the new name, free among its package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            enumeration_rename
        ),
        cmd!(
            "uml.enumeration.move",
            "Move enumeration",
            &["Enumeration"],
            None,
            "enumeration: string; the enumeration's id\npackage: string; the id of the package it goes into, where its name is free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            enumeration_move
        ),
        cmd!(
            "uml.data-type.add",
            "Add data type",
            &["Data type"],
            None,
            "package: string; the package's id\nname: string; the data type's name, free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            data_type_add
        ),
        cmd!(
            "uml.data-type.rename",
            "Rename data type",
            &["Data type"],
            None,
            "data_type: string; the data type's id\nname: string; the new name, free among its package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            data_type_rename
        ),
        cmd!(
            "uml.data-type.move",
            "Move data type",
            &["Data type"],
            None,
            "data_type: string; the data type's id\npackage: string; the id of the package it goes into, where its name is free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            data_type_move
        ),
        cmd!(
            "uml.primitive-type.add",
            "Add primitive type",
            &["Primitive type"],
            None,
            "package: string; the package's id\nname: string; the primitive type's name, free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            primitive_type_add
        ),
        cmd!(
            "uml.primitive-type.rename",
            "Rename primitive type",
            &["Primitive type"],
            None,
            "primitive_type: string; the primitive type's id, one of the model's own: a built-in type keeps its name\nname: string; the new name, free among its package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            primitive_type_rename
        ),
        cmd!(
            "uml.primitive-type.move",
            "Move primitive type",
            &["Primitive type"],
            None,
            "primitive_type: string; the primitive type's id, one of the model's own: a built-in type stays in Types\npackage: string; the id of the package it goes into, where its name is free among the package's classifiers (classes, interfaces, enumerations, data types and primitive types); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            primitive_type_move
        ),
        cmd!(
            "uml.association-class.rename",
            "Rename association class",
            &["Association class"],
            None,
            "association_class: string; the association class's id\nname: string; the new name, free among its package's classifiers (classes, interfaces, enumerations, data types, primitive types and association classes), or nothing for none; in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural)",
            has_packages,
            association_class_rename
        ),
        cmd!(
            "uml.association-class.move",
            "Move association class",
            &["Association class"],
            None,
            "association_class: string; the association class's id\npackage: string; the id of the package it goes into, where its name is free among the package's classifiers (classes, interfaces, enumerations, data types, primitive types and association classes); in the package Types (directly under the root package) never a built-in type's name (Integer, String, Boolean, Real, UnlimitedNatural); its ends stay at their classes",
            has_packages,
            association_class_move
        ),
        cmd!(query "uml.package.list", "List packages", &["Package"], None, "", always, package_list),
        cmd!(query "uml.class.list", "List classes", &["Class"], None, "package: string; the package's id", has_packages, class_list),
        cmd!(query "uml.interface.list", "List interfaces", &["Interface"], None, "package: string; the package's id", has_packages, interface_list),
        cmd!(
            query "uml.enumeration.list",
            "List enumerations",
            &["Enumeration"],
            None,
            "package: string; the package's id",
            has_packages,
            enumeration_list
        ),
        cmd!(query "uml.data-type.list", "List data types", &["Data type"], None, "package: string; the package's id", has_packages, data_type_list),
        cmd!(
            query "uml.primitive-type.list",
            "List primitive types",
            &["Primitive type"],
            None,
            "package: string; the package's id; the package Types lists the built-in types made so far",
            has_packages,
            primitive_type_list
        ),
    ]
}
