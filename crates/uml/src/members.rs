//! A class's members (archforge-3c5.1.3): attributes (visibility, name, type, multiplicity,
//! static) and operations (visibility, name, parameters, return type, static, abstract), added,
//! changed, moved and listed; a class made abstract or not. A type is named by an element's id
//! (a class of the model, or any type it holds) or by a built-in type's name: Integer, String,
//! Boolean, Real or UnlimitedNatural, which the project keeps in a package named Types, made the
//! first time one is used (owner decision). A multiplicity is typed (`1`, `0..1`, `*`, `1..*`,
//! `2..5`) and refused with a reason when it does not read (owner decision).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{Obj, Store, TypeRef, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, id_of, model, name_of, packages, refuse, set_name, short, text, uuid_of};
use crate::{
    CLASS, OPERATION, PACKAGE, PARAMETER, PRIMITIVE_TYPE, PROPERTY, TYPE, behavioral_feature, class, feature, literal_integer,
    literal_unlimited_natural, multiplicity_element, named_element, operation, package, parameter, property, typed_element,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The built-in types, kept in the package [`TYPES`].
pub const BUILT_IN: [&str; 5] = ["Integer", "String", "Boolean", "Real", "UnlimitedNatural"];
/// The package the built-in types are kept in.
pub const TYPES: &str = "Types";
/// The most parameters an operation takes.
pub const MAX_PARAMETERS: usize = 64;
/// The largest bound a multiplicity names.
pub const MAX_BOUND: u64 = 1_000_000;
/// Why a command is not available.
pub const NO_CLASSES: &str = "There is no class yet.";

/// The visibilities, in the metamodel's order, as the commands name them.
const VISIBILITIES: [&str; 4] = ["public", "private", "protected", "package"];

fn has_classes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(CLASS)) { Ok(()) } else { Err(NO_CLASSES) }
}

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// A class's attributes: the properties it owns that are no association's end.
pub fn attributes(store: &Store, c: Obj) -> Vec<Obj> {
    list(store, c, class::OWNED_ATTRIBUTE).into_iter().filter(|a| one(store, *a, property::ASSOCIATION).is_none()).collect()
}

/// A class's operations.
pub fn operations(store: &Store, c: Obj) -> Vec<Obj> {
    list(store, c, class::OWNED_OPERATION)
}

/// An operation's parameters, the return one apart: (the ones it takes, its return parameter).
pub fn parameters(store: &Store, op: Obj) -> (Vec<Obj>, Option<Obj>) {
    let all = list(store, op, operation::OWNED_PARAMETER);
    let is_return = |p: &Obj| matches!(store.get(*p, parameter::DIRECTION), Ok(Value::Enum(3)));
    (all.iter().copied().filter(|p| !is_return(p)).collect(), all.into_iter().find(|p| is_return(p)))
}

/// A multiplicity as typed: none (empty), or the lower bound and the upper one (None: many).
pub fn parse_multiplicity(s: &str) -> Result<Option<(u64, Option<u64>)>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let bound = |b: &str| -> Result<Option<u64>, String> {
        match b.trim() {
            "*" => Ok(None),
            n => n
                .parse::<u64>()
                .ok()
                .filter(|n| *n <= MAX_BOUND)
                .map(Some)
                .ok_or_else(|| format!("{:?} is not a bound: a whole number up to {MAX_BOUND}, or *", short(n))),
        }
    };
    let (lower, upper) = match s.split_once("..") {
        Some((l, u)) => (bound(l)?.ok_or("the lower bound cannot be *")?, bound(u)?),
        None => match bound(s)? {
            None => (0, None),
            Some(n) => (n, Some(n)),
        },
    };
    if upper.is_some_and(|u| u < lower) {
        return Err(format!("the upper bound is below the lower one, {lower}"));
    }
    if upper == Some(0) {
        return Err("the upper bound must be at least 1".into());
    }
    Ok(Some((lower, upper)))
}

/// Gives `o` the multiplicity typed as `typed` (refused as `key`), replacing the bounds it had.
pub(crate) fn set_multiplicity(store: &mut Store, o: Obj, typed: &str, key: &str) -> Result<(), CommandError> {
    let bounds = parse_multiplicity(typed).map_err(|why| refuse(key, format!("the multiplicity {why}")))?;
    for prop in [multiplicity_element::LOWER_VALUE, multiplicity_element::UPPER_VALUE] {
        if let Some(old) = one(store, o, prop) {
            store.destroy(old).map_err(fail)?;
        }
    }
    let Some((lower, upper)) = bounds else { return Ok(()) };
    let l = store.create(crate::LITERAL_INTEGER).map_err(fail)?;
    store.set(l, literal_integer::VALUE, Value::Int(i64::try_from(lower).unwrap_or(i64::MAX))).map_err(fail)?;
    store.set(o, multiplicity_element::LOWER_VALUE, Value::Ref(l)).map_err(fail)?;
    let u = store.create(crate::LITERAL_UNLIMITED_NATURAL).map_err(fail)?;
    store.set(u, literal_unlimited_natural::VALUE, Value::Unlimited(upper)).map_err(fail)?;
    store.set(o, multiplicity_element::UPPER_VALUE, Value::Ref(u)).map_err(fail)
}

/// The package that keeps the built-in types, made in the model when there is none.
fn types_package(store: &mut Store) -> Result<Obj, CommandError> {
    if let Some(p) = packages(store).into_iter().find(|p| name_of(store, *p) == TYPES) {
        return Ok(p);
    }
    let m = model(store)?;
    let p = store.create(PACKAGE).map_err(fail)?;
    set_name(store, p, TYPES)?;
    store.push(m, archforge_core::model::MEMBER, Value::Ref(p)).map_err(fail)?;
    Ok(p)
}

/// The type `value` names: empty for none, an element's id, or a built-in type's name, which is
/// made in the Types package the first time it is used.
fn resolve_type(store: &mut Store, key: &str, value: &str) -> Result<Option<Obj>, CommandError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if let Some(builtin) = BUILT_IN.iter().find(|b| b.eq_ignore_ascii_case(value)) {
        let pkg = types_package(store)?;
        let held = list(store, pkg, package::PACKAGED_ELEMENT)
            .into_iter()
            .find(|t| store.class_of(*t).ok() == Some(PRIMITIVE_TYPE) && name_of(store, *t) == *builtin);
        if let Some(t) = held {
            return Ok(Some(t));
        }
        let t = store.create(PRIMITIVE_TYPE).map_err(fail)?;
        set_name(store, t, builtin)?;
        store.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(t)).map_err(fail)?;
        return Ok(Some(t));
    }
    let Some(uuid) = Uuid::parse(value) else {
        return Err(refuse(key, format!("{:?} is neither a type's id nor one of {}", short(value), BUILT_IN.join(", "))));
    };
    let t = store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(value))))?;
    let class = store.class_of(t).map_err(fail)?;
    if !store.universe().conforms(class, TypeRef::Class(TYPE)) {
        return Err(refuse(key, format!("{value} is not a type")));
    }
    Ok(Some(t))
}

fn set_type(store: &mut Store, o: Obj, ty: Option<Obj>) -> Result<(), CommandError> {
    store.set(o, typed_element::TYPE, ty.map_or(Value::Null, Value::Ref)).map_err(fail)
}

/// The visibility under `key`, public when none is named: every member has one.
fn visibility(p: &Params, key: &str) -> Result<Value, CommandError> {
    match text(p, key) {
        "" => Ok(Value::Enum(0)),
        v => VISIBILITIES
            .iter()
            .position(|w| *w == v)
            .map(|i| Value::Enum(i as u32))
            .ok_or_else(|| refuse(key, format!("{v:?} is not one of {}", VISIBILITIES.join(", ")))),
    }
}

/// A member's name: a name, told apart from the class's other attributes when it is one.
fn check_name(store: &Store, name: &str, siblings: &[Obj], except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")));
    }
    if siblings.iter().any(|s| Some(*s) != except && name_of(store, *s).to_lowercase() == name.to_lowercase()) {
        return Err(refuse("name", format!("the class has an attribute named {name} already")));
    }
    Ok(())
}

/// Where in the list `at` puts a member: -1 or past the end for the end.
fn place(p: &Params, len: usize) -> u32 {
    match p.integer("at") {
        Some(at) if at >= 0 && (at as usize) < len => at as u32,
        _ => len as u32,
    }
}

fn select(ctx: &mut C<'_>, o: Obj) {
    ctx.view.selection = vec![o];
}

fn attribute_add(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx.doc, p, "class", CLASS, "a class")?;
    let name = text(p, "name");
    check_name(ctx.doc, name, &attributes(ctx.doc, c), None)?;
    let vis = visibility(p, "visibility")?;
    let a = ctx.doc.create(PROPERTY).map_err(fail)?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.set(a, named_element::VISIBILITY, vis).map_err(fail)?;
    ctx.doc.set(a, feature::IS_STATIC, Value::Bool(p.bool("static").unwrap_or(false))).map_err(fail)?;
    let ty = resolve_type(ctx.doc, "type", text(p, "type"))?;
    set_type(ctx.doc, a, ty)?;
    set_multiplicity(ctx.doc, a, text(p, "multiplicity"), "multiplicity")?;
    let at = place(p, list(ctx.doc, c, class::OWNED_ATTRIBUTE).len());
    ctx.doc.insert(c, class::OWNED_ATTRIBUTE, at, Value::Ref(a)).map_err(fail)?;
    select(ctx, a);
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

fn attribute_set(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx.doc, p, "attribute", PROPERTY, "an attribute")?;
    let c = ctx.doc.container(a).map_err(fail)?.map(|(c, _)| c).filter(|c| ctx.doc.class_of(*c).ok() == Some(CLASS));
    let Some(c) = c.filter(|_| one(ctx.doc, a, property::ASSOCIATION).is_none()) else {
        return Err(refuse("attribute", "it is not a class's attribute"));
    };
    let name = text(p, "name");
    check_name(ctx.doc, name, &attributes(ctx.doc, c), Some(a))?;
    let vis = visibility(p, "visibility")?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.set(a, named_element::VISIBILITY, vis).map_err(fail)?;
    ctx.doc.set(a, feature::IS_STATIC, Value::Bool(p.bool("static").unwrap_or(false))).map_err(fail)?;
    let ty = resolve_type(ctx.doc, "type", text(p, "type"))?;
    set_type(ctx.doc, a, ty)?;
    set_multiplicity(ctx.doc, a, text(p, "multiplicity"), "multiplicity")?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// Moves a member to `at` within its class's list of members of its kind.
fn member_move(ctx: &mut C<'_>, p: &Params, key: &str, class_of: archforge_mof::ClassId, prop: archforge_mof::PropId, what: &str) -> R {
    let m = id_of(ctx.doc, p, key, class_of, what)?;
    let Some((c, _)) = ctx.doc.container(m).map_err(fail)? else { return Err(refuse(key, "it belongs to no class")) };
    let all = list(ctx.doc, c, prop);
    let from = all.iter().position(|x| *x == m).ok_or_else(|| refuse(key, "it belongs to no class"))? as u32;
    let to = place(p, all.len()).min(all.len().saturating_sub(1) as u32);
    ctx.doc.move_item(c, prop, from, to).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, m)? }))
}

fn attribute_move(ctx: &mut C<'_>, p: &Params) -> R {
    member_move(ctx, p, "attribute", PROPERTY, class::OWNED_ATTRIBUTE, "an attribute")
}

fn operation_move(ctx: &mut C<'_>, p: &Params) -> R {
    member_move(ctx, p, "operation", OPERATION, class::OWNED_OPERATION, "an operation")
}

/// Gives an operation the parameters under `parameters` and the return type under `returns`,
/// replacing the ones it had.
fn set_signature(store: &mut Store, op: Obj, p: &Params) -> Result<(), CommandError> {
    let items = p.get("parameters").and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > MAX_PARAMETERS {
        return Err(refuse("parameters", format!("at most {MAX_PARAMETERS} parameters")));
    }
    let mut names = Vec::with_capacity(items.len());
    let mut made = Vec::with_capacity(items.len());
    for item in &items {
        let name = item.get("name").and_then(Json::as_str).unwrap_or_default().to_string();
        if let Some(problem) = archforge_core::skills::name_problem(&name, crate::commands::MAX_NAME) {
            return Err(refuse("parameters", format!("a parameter's name {problem}")));
        }
        if names.iter().any(|n: &String| n.eq_ignore_ascii_case(&name)) {
            return Err(refuse("parameters", format!("two parameters are named {name}")));
        }
        names.push(name.clone());
        let ty = resolve_type(store, "parameters", item.get("type").and_then(Json::as_str).unwrap_or_default())?;
        let m = item.get("multiplicity").and_then(Json::as_str).unwrap_or_default().to_string();
        made.push((name, ty, m));
    }
    let returns = resolve_type(store, "returns", text(p, "returns"))?;
    for old in list(store, op, operation::OWNED_PARAMETER) {
        store.destroy(old).map_err(fail)?;
    }
    for (name, ty, m) in made {
        let param = store.create(PARAMETER).map_err(fail)?;
        set_name(store, param, &name)?;
        set_type(store, param, ty)?;
        set_multiplicity(store, param, &m, "parameters")?;
        store.push(op, operation::OWNED_PARAMETER, Value::Ref(param)).map_err(fail)?;
    }
    if let Some(ty) = returns {
        let r = store.create(PARAMETER).map_err(fail)?;
        store.set(r, parameter::DIRECTION, Value::Enum(3)).map_err(fail)?;
        set_type(store, r, Some(ty))?;
        store.push(op, operation::OWNED_PARAMETER, Value::Ref(r)).map_err(fail)?;
    }
    Ok(())
}

fn set_operation(store: &mut Store, op: Obj, p: &Params) -> Result<(), CommandError> {
    let name = text(p, "name");
    if let Some(problem) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {problem}")));
    }
    let vis = visibility(p, "visibility")?;
    set_name(store, op, name)?;
    store.set(op, named_element::VISIBILITY, vis).map_err(fail)?;
    store.set(op, feature::IS_STATIC, Value::Bool(p.bool("static").unwrap_or(false))).map_err(fail)?;
    store.set(op, behavioral_feature::IS_ABSTRACT, Value::Bool(p.bool("abstract").unwrap_or(false))).map_err(fail)?;
    set_signature(store, op, p)
}

fn operation_add(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx.doc, p, "class", CLASS, "a class")?;
    let op = ctx.doc.create(OPERATION).map_err(fail)?;
    set_operation(ctx.doc, op, p)?;
    let at = place(p, list(ctx.doc, c, class::OWNED_OPERATION).len());
    ctx.doc.insert(c, class::OWNED_OPERATION, at, Value::Ref(op)).map_err(fail)?;
    select(ctx, op);
    Ok(json!({ "id": uuid_of(ctx.doc, op)? }))
}

fn operation_set(ctx: &mut C<'_>, p: &Params) -> R {
    let op = id_of(ctx.doc, p, "operation", OPERATION, "an operation")?;
    set_operation(ctx.doc, op, p)?;
    Ok(json!({ "id": uuid_of(ctx.doc, op)? }))
}

fn class_set(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx.doc, p, "class", CLASS, "a class")?;
    ctx.doc.set(c, class::IS_ABSTRACT, Value::Bool(p.bool("abstract").unwrap_or(false))).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn type_ref(store: &Store, o: Obj) -> Json {
    one(store, o, typed_element::TYPE).map_or(Json::Null, |t| json!({ "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t) }))
}

fn visibility_word(store: &Store, o: Obj) -> &'static str {
    match store.get(o, named_element::VISIBILITY) {
        Ok(Value::Enum(i)) => VISIBILITIES.get(i as usize).copied().unwrap_or("public"),
        _ => "public",
    }
}

fn flag(store: &Store, o: Obj, prop: archforge_mof::PropId) -> bool {
    store.get(o, prop).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// A class with its members, as an agent reads it.
fn class_get(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let c = id_of(store, p, "class", CLASS, "a class")?;
    let attrs: Vec<Json> = attributes(store, c)
        .into_iter()
        .map(|a| {
            json!({
                "id": uuid_of(store, a).unwrap_or_default(),
                "name": name_of(store, a),
                "visibility": visibility_word(store, a),
                "type": type_ref(store, a),
                "multiplicity": crate::figures::multiplicity(store, a),
                "static": flag(store, a, feature::IS_STATIC),
            })
        })
        .collect();
    let ops: Vec<Json> = operations(store, c)
        .into_iter()
        .map(|op| {
            let (params, ret) = parameters(store, op);
            json!({
                "id": uuid_of(store, op).unwrap_or_default(),
                "name": name_of(store, op),
                "visibility": visibility_word(store, op),
                "static": flag(store, op, feature::IS_STATIC),
                "abstract": flag(store, op, behavioral_feature::IS_ABSTRACT),
                "parameters": params.into_iter().map(|p| json!({ "name": name_of(store, p), "type": type_ref(store, p), "multiplicity": crate::figures::multiplicity(store, p) })).collect::<Vec<_>>(),
                "returns": ret.map_or(Json::Null, |r| type_ref(store, r)),
            })
        })
        .collect();
    Ok(
        json!({ "id": uuid_of(store, c)?, "name": name_of(store, c), "abstract": flag(store, c, class::IS_ABSTRACT), "attributes": attrs, "operations": ops }),
    )
}

/// The types a member may have: the built-in ones (made when first used) and the model's own.
fn type_list(ctx: &mut C<'_>, _: &Params) -> R {
    let store = &*ctx.doc;
    let held: Vec<Json> = store
        .alive()
        .filter(|o| store.class_of(*o).is_ok_and(|c| store.universe().conforms(c, TypeRef::Class(TYPE))))
        .map(|t| json!({ "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t) }))
        .collect();
    Ok(json!({ "built_in": BUILT_IN, "types": held }))
}

macro_rules! type_line {
    () => {
        "type: string = \"\"; a type's id, or Integer, String, Boolean, Real or UnlimitedNatural (kept in the package Types), or nothing"
    };
}
macro_rules! multiplicity_line {
    () => {
        "multiplicity: string = \"\"; 1, 0..1, *, 1..*, 2..5, or nothing"
    };
}
macro_rules! visibility_line {
    () => {
        "visibility: string = \"public\", public|private|protected|package; the visibility"
    };
}
macro_rules! operation_lines {
    () => {
        concat!(
            "name: string; the operation's name\n",
            visibility_line!(),
            "\nparameters: array = []; its parameters, each {name, type, multiplicity}\n",
            "returns: string = \"\"; its return type's id or a built-in type's name, or nothing\n",
            "static: bool = false; belongs to the class, not its instances\n",
            "abstract: bool = false; has no implementation here"
        )
    };
}

/// The members' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.class.set",
            "Set class",
            &["Class"],
            None,
            "class: string; the class's id\nabstract: bool = false; has no instances of its own",
            has_classes,
            class_set
        ),
        cmd!(
            "uml.attribute.add",
            "Add attribute",
            &["Class"],
            None,
            concat!(
                "class: string; the class's id\nname: string; the attribute's name\n",
                type_line!(),
                "\n",
                multiplicity_line!(),
                "\n",
                visibility_line!(),
                "\nstatic: bool = false; belongs to the class, not its instances\nat: integer = -1, -1..1000000; where among the attributes, -1 for the end"
            ),
            has_classes,
            attribute_add
        ),
        cmd!(
            "uml.attribute.set",
            "Set attribute",
            &["Class"],
            None,
            concat!(
                "attribute: string; the attribute's id\nname: string; the attribute's name\n",
                type_line!(),
                "\n",
                multiplicity_line!(),
                "\n",
                visibility_line!(),
                "\nstatic: bool = false; belongs to the class, not its instances"
            ),
            has_classes,
            attribute_set
        ),
        cmd!(
            "uml.attribute.move",
            "Move attribute",
            &["Class"],
            None,
            "attribute: string; the attribute's id\nat: integer, 0..1000000; its new place among the class's attributes",
            has_classes,
            attribute_move
        ),
        cmd!(
            "uml.operation.add",
            "Add operation",
            &["Class"],
            None,
            concat!(
                "class: string; the class's id\n",
                operation_lines!(),
                "\nat: integer = -1, -1..1000000; where among the operations, -1 for the end"
            ),
            has_classes,
            operation_add
        ),
        cmd!(
            "uml.operation.set",
            "Set operation",
            &["Class"],
            None,
            concat!("operation: string; the operation's id\n", operation_lines!()),
            has_classes,
            operation_set
        ),
        cmd!(
            "uml.operation.move",
            "Move operation",
            &["Class"],
            None,
            "operation: string; the operation's id\nat: integer, 0..1000000; its new place among the class's operations",
            has_classes,
            operation_move
        ),
        cmd!(query "uml.class.get", "Get class", &["Class"], None, "class: string; the class's id", has_classes, class_get),
        cmd!(query "uml.type.list", "List types", &["Class"], None, "", always, type_list),
    ]
}
