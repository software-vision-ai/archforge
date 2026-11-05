//! UML's commands over the store: packages in the project's Model,
//! nested as deep as needed, and classes, interfaces, enumerations, data types and primitive types
//! in a package, added, renamed, moved and listed. A package's classifiers share its names: no two
//! of them are named alike, whatever the case, so a type picked by its name is never in doubt, and
//! in the package Types the built-in types' names are kept for them. A built-in type keeps its
//! name and its package. Each
//! validates what it is given before it changes anything and refuses with a message naming the
//! key. Ids in parameters are the elements' UUIDs. Removing and describing go through the shared
//! `element.remove` and `element.describe`; a nested package removed hands what it holds up to its
//! parent (owner decision, for every notation).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::{CLASS, DATA_TYPE, ENUMERATION, INTERFACE, PACKAGE, PRIMITIVE_TYPE, enumeration, named_element, package};

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

/// The live element the id under `key` names, of whatever class.
fn element_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))
}

/// The element the id under `key` names, of `class` (`what` names it in a refusal).
pub(crate) fn id_of(store: &Store, p: &Params, key: &str, class: ClassId, what: &str) -> Result<Obj, CommandError> {
    let obj = element_of(store, p, key)?;
    if store.class_of(obj).ok() != Some(class) {
        return Err(refuse(key, format!("{} is not {what}", text(p, key))));
    }
    Ok(obj)
}

/// What an element is, with its article: a class, an interface.
pub(crate) fn a_what(store: &Store, o: Obj) -> String {
    let what = what(store, o);
    let article = if what.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
    format!("{article} {what}")
}

/// The class, interface, enumeration, data type or primitive type the id under `key` names.
pub(crate) fn classifier_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let obj = element_of(store, p, key)?;
    if !store.class_of(obj).is_ok_and(crate::is_classifier) {
        return Err(refuse(key, format!("{} is not a class, an interface, an enumeration, a data type or a primitive type", text(p, key))));
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
        let taken = subpackages(store, root).iter().any(|s| name_of(store, *s).eq_ignore_ascii_case(&name_of(store, pkg)));
        if !taken {
            detach(store, pkg)?;
            place_package(store, pkg, Some(root))?;
        }
    }
    Ok(root)
}

/// The packages beside `package`: its parent's, or the model's top level.
fn siblings_of(store: &Store, parent: Option<Obj>) -> Vec<Obj> {
    parent.map_or_else(|| packages(store), |p| subpackages(store, p))
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

/// An enumeration's literals, in their order.
pub fn literals(store: &Store, enumeration: Obj) -> Vec<Obj> {
    members(store, enumeration, enumeration::OWNED_LITERAL, crate::ENUMERATION_LITERAL)
}

/// The classifiers of a package, its classes, interfaces, enumerations, data types and primitive
/// types (the package Types' built-in ones among them), in its order.
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
/// primitive type, a package, a literal.
pub(crate) fn what(store: &Store, o: Obj) -> &'static str {
    match store.class_of(o).ok() {
        Some(c) if c == CLASS => "class",
        Some(c) if c == INTERFACE => "interface",
        Some(c) if c == ENUMERATION => "enumeration",
        Some(c) if c == DATA_TYPE => "data type",
        Some(c) if c == PRIMITIVE_TYPE => "primitive type",
        Some(c) if c == PACKAGE => "package",
        Some(c) if c == crate::ENUMERATION_LITERAL => "literal",
        _ => "element",
    }
}

/// The package an element (a class, or a nested package) sits in.
pub fn package_of(store: &Store, class: Obj) -> Option<Obj> {
    store.container(class).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).ok() == Some(PACKAGE))
}

/// A name a package or a classifier may have, distinct, whatever the case, from its siblings'
/// (a namespace tells its members apart by name); a refusal names what has it already.
fn check_name(store: &Store, name: &str, siblings: &[Obj], except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")));
    }
    if let Some(s) = siblings.iter().find(|s| Some(**s) != except && name_of(store, **s).to_lowercase() == name.to_lowercase()) {
        return Err(refuse("name", format!("there is {} named {name} already", a_what(store, *s))));
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
    check_name(ctx.doc, name, &siblings_of(ctx.doc, parent), None)?;
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
    check_name(ctx.doc, name, &siblings_of(ctx.doc, package_of(ctx.doc, pkg)), Some(pkg))?;
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
        return Err(refuse("to", "a package cannot go into itself or a package it holds"));
    }
    if to == package_of(ctx.doc, pkg) {
        return Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }));
    }
    if let Some(why) = crate::members::types_problem(ctx.doc, pkg, &name_of(ctx.doc, pkg), to) {
        return Err(refuse("to", why));
    }
    check_name(ctx.doc, &name_of(ctx.doc, pkg), &siblings_of(ctx.doc, to), Some(pkg))?;
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
/// of the built-in types'. A built-in type stays in Types.
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
    check_name(ctx.doc, &name, &classifiers(ctx.doc, to), Some(c))?;
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
    check_name(ctx.doc, name, &classifiers(ctx.doc, pkg), None)?;
    let c = ctx.doc.create(kind).map_err(fail)?;
    set_name(ctx.doc, c, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// The classifier under `key` renamed, its new name free among its package's classifiers and, in
/// the package Types, none of the built-in types'. A built-in type keeps its name.
fn classifier_rename(ctx: &mut C<'_>, p: &Params, kind: ClassId, key: &str, what: &str) -> R {
    let c = id_of(ctx.doc, p, key, kind, what)?;
    if let Some(why) = crate::members::built_in_problem(ctx.doc, c, "renamed") {
        return Err(refuse(key, why));
    }
    let name = text(p, "name");
    if let Some(why) = package_of(ctx.doc, c).and_then(|pkg| crate::members::reserved_problem(ctx.doc, pkg, name)) {
        return Err(refuse("name", why));
    }
    let siblings = package_of(ctx.doc, c).map(|pkg| classifiers(ctx.doc, pkg)).unwrap_or_default();
    check_name(ctx.doc, name, &siblings, Some(c))?;
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

/// `uml.package.list`: every package with its parent and how many classes, interfaces,
/// enumerations, data types and primitive types it holds.
fn package_list(ctx: &mut C<'_>, _: &Params) -> R {
    let store = &*ctx.doc;
    let items: Vec<Json> = all_packages(store)
        .into_iter()
        .map(|pkg| {
            let parent = package_of(store, pkg).and_then(|p| uuid_of(store, p).ok());
            json!({
                "id": uuid_of(store, pkg).unwrap_or_default(), "name": name_of(store, pkg), "parent": parent,
                "classes": classes(store, pkg).len(), "interfaces": interfaces(store, pkg).len(),
                "enumerations": enumerations(store, pkg).len(), "data_types": data_types(store, pkg).len(),
                "primitive_types": primitive_types(store, pkg).len(),
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

/// Every classifier inside a package, at any depth.
fn classifiers_within(store: &Store, pkg: Obj) -> Vec<Obj> {
    let mut out = classifiers(store, pkg);
    for sub in subpackages(store, pkg) {
        out.extend(classifiers_within(store, sub));
    }
    out
}

/// Why `parent` cannot take what `pkg` holds, when a name would clash there, a built-in type's name
/// would land in the package Types, or a package handed up would become a second Types or a Types
/// holding such a name.
fn hand_up_problem(store: &Store, pkg: Obj, parent: Obj) -> Option<String> {
    for o in held(store, pkg) {
        // Classifiers share their package's names, and the package Types keeps the built-in
        // types'; a package has its own.
        let n = name_of(store, o);
        let siblings = match store.class_of(o).ok() {
            Some(c) if crate::is_classifier(c) => {
                if let Some(why) = crate::members::reserved_problem(store, parent, &n) {
                    return Some(why);
                }
                classifiers(store, parent)
            }
            Some(c) if c == PACKAGE => {
                if let Some(why) = crate::members::types_problem(store, o, &n, Some(parent)) {
                    return Some(why);
                }
                subpackages(store, parent)
            }
            _ => continue,
        };
        if let Some(s) = siblings.iter().find(|s| !n.is_empty() && name_of(store, **s).to_lowercase() == n.to_lowercase()) {
            return Some(format!("{} has {} named {n} already", name_of(store, parent), a_what(store, *s)));
        }
    }
    None
}

/// Removes UML elements (packages, classes, interfaces, enumerations, data types, attributes,
/// operations, literals, primitive types, associations, generalizations, dependencies and interface
/// realizations), each with what it holds (an enumeration its literals, a data type its attributes
/// and operations) and the diagrams, nodes and lines that show them (`element.remove`, through
/// [`crate::UmlNotation`]). A nested package hands what it holds, and its diagrams, up to its parent
/// first (owner decision); a top-level package takes them along. A classifier takes the
/// relationships that touch it along, and a member's type that goes is dropped from what had it.
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
    ];
    for o in objs.iter().filter(|o| store.is_alive(**o)) {
        let class = store.class_of(*o).map_err(fail)?;
        // An association's end goes with its association.
        let end = class == crate::PROPERTY && store.get(*o, crate::property::ASSOCIATION).ok().and_then(|v| v.as_ref()).is_some();
        if !removable.contains(&class) || end {
            return Err(refuse("ids", format!("{} cannot be removed yet", uuid_of(store, *o)?)));
        }
        if Some(*o) == root_package(store) {
            return Err(refuse("ids", "the model's root package holds the whole model and stays"));
        }
        if class == PACKAGE
            && let Some(why) = crate::members::removal_problem(store, *o)
        {
            return Err(refuse("ids", why));
        }
        if class == PACKAGE
            && let Some(parent) = package_of(store, *o)
            && let Some(why) = hand_up_problem(store, *o, parent)
        {
            return Err(refuse("ids", format!("its contents cannot move up: {why}")));
        }
    }
    let mut gone = 0;
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
    let mut all: Vec<Obj> = Vec::new();
    for o in objs.iter().filter(|o| store.is_alive(**o)) {
        let classes = match store.class_of(*o).ok() {
            Some(c) if crate::is_classifier(c) => vec![*o],
            Some(c) if c == PACKAGE => classifiers_within(store, *o),
            _ => Vec::new(),
        };
        for c in classes {
            all.extend(crate::relations::relationships_of(store, c));
        }
    }
    all.extend(objs);
    for o in all {
        if !store.is_alive(o) {
            continue;
        }
        gone += store
            .transaction(|st| {
                let views = archforge_diagram::remove_views_of(st, o)?;
                Ok(views + archforge_core::destroy_with_links(st, o)?)
            })
            .map_err(fail)?;
    }
    Ok(gone)
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
