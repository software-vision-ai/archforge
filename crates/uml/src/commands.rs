//! UML's commands over the store (archforge-3c5.1.1): packages in the project's Model and classes
//! in a package, added, renamed and listed. Each validates what it is given before it changes
//! anything and refuses with a message naming the key. Ids in parameters are the elements' UUIDs.
//! Removing and describing go through the shared `element.remove` and `element.describe`.

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::{CLASS, PACKAGE, named_element, package};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The longest name of a package or a class.
pub const MAX_NAME: usize = 128;

/// Why a command is not available.
pub const NO_PACKAGES: &str = "There is no package yet.";

fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn text<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

fn short(s: &str) -> String {
    s.chars().take(40).collect()
}

fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

fn name_of(store: &Store, obj: Obj) -> String {
    archforge_core::name_of(store, obj)
}

fn has_packages(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(PACKAGE)) { Ok(()) } else { Err(NO_PACKAGES) }
}

/// The element the id under `key` names, of `class` (`what` names it in a refusal).
fn id_of(store: &Store, p: &Params, key: &str, class: ClassId, what: &str) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    let obj = store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))?;
    if store.class_of(obj).ok() != Some(class) {
        return Err(refuse(key, format!("{id} is not {what}")));
    }
    Ok(obj)
}

/// The project's Model, the root that holds the packages.
fn model(store: &Store) -> Result<Obj, CommandError> {
    store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_core::MODEL)).ok_or_else(|| CommandError::new("the project has no model root"))
}

/// The members of `owner`'s list `prop` of `class`.
fn members(store: &Store, owner: Obj, prop: archforge_mof::PropId, class: ClassId) -> Vec<Obj> {
    store
        .list(owner, prop)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|o| store.is_alive(*o) && store.class_of(*o).ok() == Some(class))
        .collect()
}

/// The packages of the model, in its order.
pub fn packages(store: &Store) -> Vec<Obj> {
    model(store).map(|m| members(store, m, archforge_core::model::MEMBER, PACKAGE)).unwrap_or_default()
}

/// The classes of a package, in its order.
pub fn classes(store: &Store, package: Obj) -> Vec<Obj> {
    members(store, package, package::PACKAGED_ELEMENT, CLASS)
}

/// The package a class sits in.
pub fn package_of(store: &Store, class: Obj) -> Option<Obj> {
    store.container(class).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).ok() == Some(PACKAGE))
}

/// A name a package or class may have, distinct, whatever the case, from its siblings' (a
/// namespace tells its members apart by name).
fn check_name(store: &Store, name: &str, siblings: &[Obj], except: Option<Obj>, what: &str) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")));
    }
    if siblings.iter().any(|s| Some(*s) != except && name_of(store, *s).to_lowercase() == name.to_lowercase()) {
        return Err(refuse("name", format!("there is a {what} named {name} already")));
    }
    Ok(())
}

fn set_name(store: &mut Store, obj: Obj, name: &str) -> Result<(), CommandError> {
    store.set(obj, named_element::NAME, Value::str(name)).map_err(fail)
}

fn package_add(ctx: &mut C<'_>, p: &Params) -> R {
    let name = text(p, "name");
    check_name(ctx.doc, name, &packages(ctx.doc), None, "package")?;
    let m = model(ctx.doc)?;
    let pkg = ctx.doc.create(PACKAGE).map_err(fail)?;
    set_name(ctx.doc, pkg, name)?;
    ctx.doc.push(m, archforge_core::model::MEMBER, Value::Ref(pkg)).map_err(fail)?;
    ctx.view.selection = vec![pkg];
    Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }))
}

fn package_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let name = text(p, "name");
    check_name(ctx.doc, name, &packages(ctx.doc), Some(pkg), "package")?;
    set_name(ctx.doc, pkg, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }))
}

fn class_add(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = id_of(ctx.doc, p, "package", PACKAGE, "a package")?;
    let name = text(p, "name");
    check_name(ctx.doc, name, &classes(ctx.doc, pkg), None, "class")?;
    let c = ctx.doc.create(CLASS).map_err(fail)?;
    set_name(ctx.doc, c, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn class_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx.doc, p, "class", CLASS, "a class")?;
    let name = text(p, "name");
    let siblings = package_of(ctx.doc, c).map(|pkg| classes(ctx.doc, pkg)).unwrap_or_default();
    check_name(ctx.doc, name, &siblings, Some(c), "class")?;
    set_name(ctx.doc, c, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn package_list(ctx: &mut C<'_>, _: &Params) -> R {
    let store = &*ctx.doc;
    let items: Vec<Json> = packages(store)
        .into_iter()
        .map(|pkg| json!({ "id": uuid_of(store, pkg).unwrap_or_default(), "name": name_of(store, pkg), "classes": classes(store, pkg).len() }))
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

/// Removes UML elements, each with what it holds and the diagrams, nodes and lines that show
/// them (`element.remove`, through [`crate::UmlNotation`]).
pub fn remove_elements(store: &mut Store, objs: &[Obj]) -> Result<usize, CommandError> {
    // Every id is checked before anything goes, so a refusal leaves the store as it was.
    for o in objs.iter().filter(|o| store.is_alive(**o)) {
        if ![PACKAGE, CLASS].contains(&store.class_of(*o).map_err(fail)?) {
            return Err(refuse("ids", format!("{} cannot be removed yet", uuid_of(store, *o)?)));
        }
    }
    let mut gone = 0;
    for o in objs {
        if !store.is_alive(*o) {
            continue;
        }
        gone += store
            .transaction(|st| {
                let views = archforge_diagram::remove_views_of(st, *o)?;
                Ok(views + st.destroy(*o)?)
            })
            .map_err(fail)?;
    }
    Ok(gone)
}

/// UML's own commands; the edit area, the shared element commands, the diagrams' and the skills'
/// join them in a registry.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!("uml.package.add", "Add package", &["Package"], None, "name: string; the package's name", always, package_add),
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
            "package: string; the package's id\nname: string; the class's name",
            has_packages,
            class_add
        ),
        cmd!(
            "uml.class.rename",
            "Rename class",
            &["Class"],
            None,
            "class: string; the class's id\nname: string; the new name",
            has_packages,
            class_rename
        ),
        cmd!(query "uml.package.list", "List packages", &["Package"], None, "", always, package_list),
        cmd!(query "uml.class.list", "List classes", &["Class"], None, "package: string; the package's id", has_packages, class_list),
    ]
}
