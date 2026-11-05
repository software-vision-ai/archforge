//! UML's commands over the store: packages in the project's Model,
//! nested as deep as needed, and classes and interfaces in a package, added, renamed, moved and
//! listed. A package's classes and interfaces share its names: no two of them are named alike,
//! whatever the case, so a type picked by its name is never in doubt. Each
//! validates what it is given before it changes anything and refuses with a message naming the
//! key. Ids in parameters are the elements' UUIDs. Removing and describing go through the shared
//! `element.remove` and `element.describe`; a nested package removed hands what it holds up to its
//! parent (owner decision, for every notation).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::{CLASS, INTERFACE, PACKAGE, named_element, package};

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

/// The class or interface the id under `key` names.
pub(crate) fn classifier_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let obj = element_of(store, p, key)?;
    if !store.class_of(obj).is_ok_and(crate::is_classifier) {
        return Err(refuse(key, format!("{} is not a class or an interface", text(p, key))));
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

/// The classifiers of a package, its classes and interfaces, in its order.
pub fn classifiers(store: &Store, package: Obj) -> Vec<Obj> {
    store
        .list(package, package::PACKAGED_ELEMENT)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| store.is_alive(*o) && store.class_of(*o).is_ok_and(crate::is_classifier))
        .collect()
}

/// What an element is, as a refusal names it: a class, an interface, a package.
pub(crate) fn what(store: &Store, o: Obj) -> &'static str {
    match store.class_of(o).ok() {
        Some(c) if c == CLASS => "class",
        Some(c) if c == INTERFACE => "interface",
        Some(c) if c == PACKAGE => "package",
        _ => "element",
    }
}

/// The package an element (a class, or a nested package) sits in.
pub fn package_of(store: &Store, class: Obj) -> Option<Obj> {
    store.container(class).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).ok() == Some(PACKAGE))
}

/// A name a package, class or interface may have, distinct, whatever the case, from its siblings'
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

fn package_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let name = text(p, "name");
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

/// The class or interface under `key` (of `kind`, `what` naming it) moves into the package under
/// `package`, its name free among that package's classifiers.
fn classifier_move(ctx: &mut C<'_>, p: &Params, kind: ClassId, key: &str, what: &str) -> R {
    let c = id_of(ctx.doc, p, key, kind, what)?;
    let to = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    if package_of(ctx.doc, c) == Some(to) {
        return Ok(json!({ "id": uuid_of(ctx.doc, c)? }));
    }
    check_name(ctx.doc, &name_of(ctx.doc, c), &classifiers(ctx.doc, to), Some(c))?;
    detach(ctx.doc, c)?;
    ctx.doc.push(to, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// A class or an interface (`kind`) named `name` in the package under `package`.
fn classifier_add(ctx: &mut C<'_>, p: &Params, kind: ClassId) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let name = text(p, "name");
    check_name(ctx.doc, name, &classifiers(ctx.doc, pkg), None)?;
    let c = ctx.doc.create(kind).map_err(fail)?;
    set_name(ctx.doc, c, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

/// The class or interface under `key` renamed, its new name free among its package's classifiers.
fn classifier_rename(ctx: &mut C<'_>, p: &Params, kind: ClassId, key: &str, what: &str) -> R {
    let c = id_of(ctx.doc, p, key, kind, what)?;
    let name = text(p, "name");
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

fn package_list(ctx: &mut C<'_>, _: &Params) -> R {
    let store = &*ctx.doc;
    let items: Vec<Json> = all_packages(store)
        .into_iter()
        .map(|pkg| {
            let parent = package_of(store, pkg).and_then(|p| uuid_of(store, p).ok());
            json!({
                "id": uuid_of(store, pkg).unwrap_or_default(), "name": name_of(store, pkg), "parent": parent,
                "classes": classes(store, pkg).len(), "interfaces": interfaces(store, pkg).len(),
            })
        })
        .collect();
    Ok(json!({ "packages": items }))
}

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

/// What a nested package hands up to its parent when it is removed: everything it holds.
fn held(store: &Store, pkg: Obj) -> Vec<Obj> {
    store.list(pkg, package::PACKAGED_ELEMENT).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|o| store.is_alive(*o)).collect()
}

/// Every class and interface inside a package, at any depth.
fn classifiers_within(store: &Store, pkg: Obj) -> Vec<Obj> {
    let mut out = classifiers(store, pkg);
    for sub in subpackages(store, pkg) {
        out.extend(classifiers_within(store, sub));
    }
    out
}

/// Why `parent` cannot take what `pkg` holds, when a name would clash there.
fn hand_up_problem(store: &Store, pkg: Obj, parent: Obj) -> Option<String> {
    for o in held(store, pkg) {
        // A class and an interface share their package's names; a package has its own.
        let siblings = match store.class_of(o).ok() {
            Some(c) if crate::is_classifier(c) => classifiers(store, parent),
            Some(c) if c == PACKAGE => subpackages(store, parent),
            _ => continue,
        };
        let n = name_of(store, o);
        if let Some(s) = siblings.iter().find(|s| !n.is_empty() && name_of(store, **s).to_lowercase() == n.to_lowercase()) {
            return Some(format!("{} has {} named {n} already", name_of(store, parent), a_what(store, *s)));
        }
    }
    None
}

/// Removes UML elements (packages, classes, interfaces, attributes, operations, primitive types,
/// associations, generalizations and dependencies), each with what it holds and the diagrams,
/// nodes and lines that show them (`element.remove`, through [`crate::UmlNotation`]). A nested
/// package hands what it holds, and its diagrams, up to its parent first (owner decision);
/// a top-level package takes them along. A class or an interface takes the relationships that touch it along, and
/// a member's type that goes is dropped from what had it.
pub fn remove_elements(store: &mut Store, objs: &[Obj]) -> Result<usize, CommandError> {
    // Every id is checked before anything goes, so a refusal leaves the store as it was.
    let removable = [
        PACKAGE,
        CLASS,
        INTERFACE,
        crate::PROPERTY,
        crate::OPERATION,
        crate::PRIMITIVE_TYPE,
        crate::ASSOCIATION,
        crate::GENERALIZATION,
        crate::DEPENDENCY,
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
    // The relationships of every class and interface that goes, a top-level package's among them,
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
            "package: string; the package's id\nto: string = \"\"; the id of the package it goes into, or nothing for the model's root package",
            has_packages,
            package_move
        ),
        cmd!(
            "uml.class.move",
            "Move class",
            &["Class"],
            None,
            "class: string; the class's id\npackage: string; the id of the package it goes into, where its name is free among the classes and interfaces",
            has_packages,
            class_move
        ),
        cmd!(
            "uml.package.rename",
            "Rename package",
            &["Package"],
            None,
            "package: string; the package's id\nname: string; the new name",
            has_packages,
            package_rename
        ),
        cmd!(
            "uml.class.add",
            "Add class",
            &["Class"],
            None,
            "package: string; the package's id\nname: string; the class's name, free among the package's classes and interfaces",
            has_packages,
            class_add
        ),
        cmd!(
            "uml.class.rename",
            "Rename class",
            &["Class"],
            None,
            "class: string; the class's id\nname: string; the new name, free among its package's classes and interfaces",
            has_packages,
            class_rename
        ),
        cmd!(
            "uml.interface.add",
            "Add interface",
            &["Interface"],
            None,
            "package: string; the package's id\nname: string; the interface's name, free among the package's classes and interfaces",
            has_packages,
            interface_add
        ),
        cmd!(
            "uml.interface.rename",
            "Rename interface",
            &["Interface"],
            None,
            "interface: string; the interface's id\nname: string; the new name, free among its package's classes and interfaces",
            has_packages,
            interface_rename
        ),
        cmd!(
            "uml.interface.move",
            "Move interface",
            &["Interface"],
            None,
            "interface: string; the interface's id\npackage: string; the id of the package it goes into, where its name is free among the classes and interfaces",
            has_packages,
            interface_move
        ),
        cmd!(query "uml.package.list", "List packages", &["Package"], None, "", always, package_list),
        cmd!(query "uml.class.list", "List classes", &["Class"], None, "package: string; the package's id", has_packages, class_list),
        cmd!(query "uml.interface.list", "List interfaces", &["Interface"], None, "package: string; the package's id", has_packages, interface_list),
    ]
}
