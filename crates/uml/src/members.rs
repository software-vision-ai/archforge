//! A class's, an interface's or a data type's members: attributes (visibility, name, type,
//! multiplicity, static) and operations (visibility, name, parameters, return type, static,
//! abstract), added, changed, moved and listed; a class made abstract or not. An interface's
//! members are public, as UML has them; a data type keeps its own as a class does, in
//! DataType's ownedAttribute and ownedOperation; an enumeration takes neither, its members being its
//! literals, which are added, renamed and moved here (`uml.literal.*`), their names told apart
//! within their enumeration whatever the case, and removed through `element.remove`; a primitive
//! type has no members at all. A type is named by an element's id
//! (a class of the model, an interface, an enumeration, a data type, a primitive type, or any type
//! it holds) or by a built-in type's name: Integer, String, Boolean, Real or UnlimitedNatural,
//! which the project keeps as primitive types in a package named Types, made the first time one is
//! used (owner decision). A built-in type's name always names the one in Types, never a type of the
//! model's own named alike, and in a package named Types those names are kept for the built-in
//! types (AR-124). A multiplicity is typed (`1`, `0..1`, `*`, `1..*`, `2..5`) and refused with a
//! reason when it does not read (owner decision).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{Obj, Store, TypeRef, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::commands::{classifier_of, fail, id_of, name_of, packages, refuse, set_name, short, text, uuid_of};
use crate::{
    CLASS, DATA_TYPE, ENUMERATION, ENUMERATION_LITERAL, INTERFACE, OPERATION, PACKAGE, PARAMETER, PRIMITIVE_TYPE, PROPERTY, TYPE, behavioral_feature,
    class, data_type, enumeration, feature, interface, literal_integer, literal_unlimited_natural, multiplicity_element, named_element, operation,
    package, parameter, property, typed_element,
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
pub const NO_CLASSES: &str = "There is no class, interface, enumeration, data type or primitive type yet.";

/// Whether `t` is a built-in type: a primitive type named Integer, String, Boolean, Real or
/// UnlimitedNatural, exactly, kept in a package named [`TYPES`]. No primitive type of the model's own
/// can be one, since those names are kept for the built-in types there ([`reserved_problem`]).
pub fn is_built_in(store: &Store, t: Obj) -> bool {
    let in_types = store.container(t).ok().flatten().is_some_and(|(p, _)| store.class_of(p).ok() == Some(PACKAGE) && name_of(store, p) == TYPES);
    in_types && store.class_of(t).ok() == Some(PRIMITIVE_TYPE) && BUILT_IN.contains(&name_of(store, t).as_str())
}

/// Why a classifier named `name` may not sit in `package`, when it may not: in a package named
/// [`TYPES`] the built-in types' names, whatever the case, are kept for them, made there the first
/// time one is used, so a type of the model's own never stands in for one. The refusal names the
/// name and what may be done instead.
pub(crate) fn reserved_problem(store: &Store, package: Obj, name: &str) -> Option<String> {
    if name_of(store, package) != TYPES {
        return None;
    }
    let builtin = BUILT_IN.iter().find(|b| b.eq_ignore_ascii_case(name.trim()))?;
    Some(format!(
        "{name} is the name of the built-in type {builtin}, kept for it in the package {TYPES}; name it otherwise or put it in another package"
    ))
}

/// Why the built-in type `t` may not be `changed` (renamed, moved), when it is one: it keeps its
/// name and its package. The refusal names its id and name and what may be changed instead.
pub(crate) fn built_in_problem(store: &Store, t: Obj, changed: &str) -> Option<String> {
    is_built_in(store, t).then(|| {
        let id = uuid_of(store, t).unwrap_or_default();
        format!(
            "{id} is the built-in type {}, which is never {changed}: it keeps its name and its package {TYPES}; a primitive type of the model's own is",
            name_of(store, t)
        )
    })
}

/// The most literals an enumeration holds: ER's enum types hold as many
/// (`archforge_er::rules::MAX_LITERALS`), one rule across the notations, and the card, Properties
/// and Models stay small enough to build every frame.
pub const MAX_LITERALS: usize = 256;
/// Why a command for literals is not available.
pub const NO_ENUMERATIONS: &str = "There is no enumeration yet.";

/// The visibilities, in the metamodel's order, as the commands name them.
const VISIBILITIES: [&str; 4] = ["public", "private", "protected", "package"];

/// Whether a class, an interface, an enumeration, a data type or a primitive type exists, which the
/// member commands need.
fn has_classes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(crate::is_classifier)) { Ok(()) } else { Err(NO_CLASSES) }
}

/// Whether an enumeration, whose literals the literal commands change, exists.
fn has_enumerations(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(ENUMERATION)) { Ok(()) } else { Err(NO_ENUMERATIONS) }
}

/// Whether a class, which `uml.class.set` changes, exists.
fn has_class(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(CLASS)) { Ok(()) } else { Err("There is no class yet.") }
}

/// The lists a class, an interface or a data type keeps its attributes and its operations in; an
/// enumeration, though a DataType in UML's metamodel, keeps none here: its members are its
/// literals.
pub fn member_lists(store: &Store, c: Obj) -> Option<(archforge_mof::PropId, archforge_mof::PropId)> {
    match store.class_of(c).ok()? {
        k if k == CLASS => Some((class::OWNED_ATTRIBUTE, class::OWNED_OPERATION)),
        k if k == INTERFACE => Some((interface::OWNED_ATTRIBUTE, interface::OWNED_OPERATION)),
        k if k == DATA_TYPE => Some((data_type::OWNED_ATTRIBUTE, data_type::OWNED_OPERATION)),
        _ => None,
    }
}

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// A class's, an interface's or a data type's attributes: the properties it owns that are no
/// association's end.
pub fn attributes(store: &Store, c: Obj) -> Vec<Obj> {
    let Some((attributes, _)) = member_lists(store, c) else { return Vec::new() };
    list(store, c, attributes).into_iter().filter(|a| one(store, *a, property::ASSOCIATION).is_none()).collect()
}

/// A class's, an interface's or a data type's operations.
pub fn operations(store: &Store, c: Obj) -> Vec<Obj> {
    member_lists(store, c).map(|(_, operations)| list(store, c, operations)).unwrap_or_default()
}

/// An operation's parameters, the return one apart: (the ones it takes, its return parameter).
pub fn parameters(store: &Store, op: Obj) -> (Vec<Obj>, Option<Obj>) {
    let all = list(store, op, operation::OWNED_PARAMETER);
    let is_return = |p: &Obj| matches!(store.get(*p, parameter::DIRECTION), Ok(Value::Enum(3)));
    (all.iter().copied().filter(|p| !is_return(p)).collect(), all.into_iter().find(|p| is_return(p)))
}

/// Why a typed multiplicity does not read; its text follows "the multiplicity" in a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiplicityError {
    /// Not a whole number, a range `lower..upper` or `*`: the text as typed, cut short.
    Malformed(String),
    /// A bound above [`MAX_BOUND`].
    TooLarge,
    /// A range that starts at `*`.
    LowerMany,
    /// A range whose upper bound is below its lower one, the lower bound named.
    Reversed(u64),
}

impl std::fmt::Display for MultiplicityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiplicityError::Malformed(s) => write!(f, "{s:?} is not a whole number, a range such as 2..5, or *"),
            MultiplicityError::TooLarge => write!(f, "has a bound above {MAX_BOUND}"),
            MultiplicityError::LowerMany => write!(f, "cannot start at *"),
            MultiplicityError::Reversed(lower) => write!(f, "needs an upper bound of at least {lower}"),
        }
    }
}

/// A multiplicity as typed: none (empty), or the lower bound and the upper one (None: many).
/// Whitespace around it is dropped; inside it, only digits, one `..` and `*` read (owner decision):
/// `0`, `2`, `0..1`, `2..5`, `1..*`, and `*` for `0..*`.
pub fn parse_multiplicity(s: &str) -> Result<Option<(u64, Option<u64>)>, MultiplicityError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let malformed = || MultiplicityError::Malformed(short(s));
    let bound = |b: &str| -> Result<Option<u64>, MultiplicityError> {
        if b == "*" {
            return Ok(None);
        }
        if b.is_empty() || !b.bytes().all(|c| c.is_ascii_digit()) {
            return Err(malformed());
        }
        // Digits that overflow are a bound too large, not a typing error.
        b.parse::<u64>().ok().filter(|n| *n <= MAX_BOUND).map(Some).ok_or(MultiplicityError::TooLarge)
    };
    let (lower, upper) = match s.split_once("..") {
        Some((l, u)) => (bound(l)?.ok_or(MultiplicityError::LowerMany)?, bound(u)?),
        None => match bound(s)? {
            None => (0, None),
            Some(n) => (n, Some(n)),
        },
    };
    if upper.is_some_and(|u| u < lower) {
        return Err(MultiplicityError::Reversed(lower));
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
    // Types sits in the model's root package, as every package does; making the root takes in a
    // Types an older project kept at the top level, so its types are used again.
    let root = crate::commands::ensure_root(store)?;
    let held = crate::commands::subpackages(store, root).into_iter().chain(packages(store)).find(|p| name_of(store, *p) == TYPES);
    if let Some(p) = held {
        return Ok(p);
    }
    let p = store.create(PACKAGE).map_err(fail)?;
    set_name(store, p, TYPES)?;
    store.push(root, package::PACKAGED_ELEMENT, Value::Ref(p)).map_err(fail)?;
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

/// The visibility under `key`, public when none is named: every member has one. An interface's
/// (`owner`'s) members are all public.
fn visibility(store: &Store, owner: Obj, p: &Params, key: &str) -> Result<Value, CommandError> {
    let v = match text(p, key) {
        "" => Value::Enum(0),
        v => VISIBILITIES
            .iter()
            .position(|w| *w == v)
            .map(|i| Value::Enum(i as u32))
            .ok_or_else(|| refuse(key, format!("{v:?} is not one of {}", VISIBILITIES.join(", "))))?,
    };
    if v != Value::Enum(0) && store.class_of(owner).ok() == Some(INTERFACE) {
        return Err(refuse(key, "an interface's members are public"));
    }
    Ok(v)
}

/// A member's name: a name, told apart from its owner's other attributes when it is one.
fn check_name(store: &Store, owner: Obj, name: &str, siblings: &[Obj], except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")));
    }
    if siblings.iter().any(|s| Some(*s) != except && name_of(store, *s).to_lowercase() == name.to_lowercase()) {
        return Err(refuse("name", format!("the {} has an attribute named {name} already", crate::commands::what(store, owner))));
    }
    Ok(())
}

/// Why `c` takes no `members` (attributes or operations): its id, what it is (an enumeration,
/// whose members are its literals, or a primitive type, which has none), and what keeps them
/// instead.
fn no_members(store: &Store, c: Obj, members: &str) -> String {
    let id = uuid_of(store, c).unwrap_or_default();
    let because =
        if store.class_of(c).ok() == Some(PRIMITIVE_TYPE) { "a primitive type having no members" } else { "its members being its literals" };
    format!("{id} is {} and keeps no {members}, {because}; a class, an interface or a data type does", crate::commands::a_what(store, c))
}

/// The class, interface or data type that owns `member` in its list of attributes or operations.
fn owner_of(store: &Store, member: Obj) -> Option<Obj> {
    store.container(member).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).is_ok_and(crate::is_classifier))
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

/// `uml.attribute.add`: an attribute of a class, an interface or a data type, its name free among
/// its attributes.
fn attribute_add(ctx: &mut C<'_>, p: &Params) -> R {
    let c = classifier_of(ctx.doc, p, "class")?;
    let Some((owned, _)) = member_lists(ctx.doc, c) else { return Err(refuse("class", no_members(ctx.doc, c, "attributes"))) };
    let name = text(p, "name");
    check_name(ctx.doc, c, name, &attributes(ctx.doc, c), None)?;
    let vis = visibility(ctx.doc, c, p, "visibility")?;
    let a = ctx.doc.create(PROPERTY).map_err(fail)?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.set(a, named_element::VISIBILITY, vis).map_err(fail)?;
    ctx.doc.set(a, feature::IS_STATIC, Value::Bool(p.bool("static").unwrap_or(false))).map_err(fail)?;
    let ty = resolve_type(ctx.doc, "type", text(p, "type"))?;
    set_type(ctx.doc, a, ty)?;
    set_multiplicity(ctx.doc, a, text(p, "multiplicity"), "multiplicity")?;
    let at = place(p, list(ctx.doc, c, owned).len());
    ctx.doc.insert(c, owned, at, Value::Ref(a)).map_err(fail)?;
    select(ctx, a);
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// `uml.attribute.set`: an attribute's name, visibility, type, multiplicity and static changed.
fn attribute_set(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx.doc, p, "attribute", PROPERTY, "an attribute")?;
    let Some(c) = owner_of(ctx.doc, a).filter(|_| one(ctx.doc, a, property::ASSOCIATION).is_none()) else {
        return Err(refuse("attribute", "it is not a class's, an interface's or a data type's attribute"));
    };
    let name = text(p, "name");
    check_name(ctx.doc, c, name, &attributes(ctx.doc, c), Some(a))?;
    let vis = visibility(ctx.doc, c, p, "visibility")?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.set(a, named_element::VISIBILITY, vis).map_err(fail)?;
    ctx.doc.set(a, feature::IS_STATIC, Value::Bool(p.bool("static").unwrap_or(false))).map_err(fail)?;
    let ty = resolve_type(ctx.doc, "type", text(p, "type"))?;
    set_type(ctx.doc, a, ty)?;
    set_multiplicity(ctx.doc, a, text(p, "multiplicity"), "multiplicity")?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// Moves a member to `at` within its owner's list of members of its kind, attributes (`first`) or
/// operations.
fn member_move(ctx: &mut C<'_>, p: &Params, key: &str, class_of: archforge_mof::ClassId, first: bool, what: &str) -> R {
    let m = id_of(ctx.doc, p, key, class_of, what)?;
    let lists = owner_of(ctx.doc, m).and_then(|c| Some((c, member_lists(ctx.doc, c)?)));
    let Some((c, (attributes, operations))) = lists else { return Err(refuse(key, "it belongs to no class, interface or data type")) };
    let prop = if first { attributes } else { operations };
    let all = list(ctx.doc, c, prop);
    let from = all.iter().position(|x| *x == m).ok_or_else(|| refuse(key, "it belongs to no class, interface or data type"))?;
    let from = u32::try_from(from)
        .map_err(|_| refuse(key, format!("{} sits at {from}, past the places a move reaches; a place from 0 to {}", text(p, key), u32::MAX)))?;
    let to = place(p, all.len()).min(all.len().saturating_sub(1) as u32);
    ctx.doc.move_item(c, prop, from, to).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, m)? }))
}

/// `uml.attribute.move`: an attribute to another place among its owner's attributes.
fn attribute_move(ctx: &mut C<'_>, p: &Params) -> R {
    member_move(ctx, p, "attribute", PROPERTY, true, "an attribute")
}

/// `uml.operation.move`: an operation to another place among its owner's operations.
fn operation_move(ctx: &mut C<'_>, p: &Params) -> R {
    member_move(ctx, p, "operation", OPERATION, false, "an operation")
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

/// Gives operation `op` of `owner` what the parameters say.
fn set_operation(store: &mut Store, owner: Obj, op: Obj, p: &Params) -> Result<(), CommandError> {
    let name = text(p, "name");
    if let Some(problem) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {problem}")));
    }
    let vis = visibility(store, owner, p, "visibility")?;
    set_name(store, op, name)?;
    store.set(op, named_element::VISIBILITY, vis).map_err(fail)?;
    store.set(op, feature::IS_STATIC, Value::Bool(p.bool("static").unwrap_or(false))).map_err(fail)?;
    store.set(op, behavioral_feature::IS_ABSTRACT, Value::Bool(p.bool("abstract").unwrap_or(false))).map_err(fail)?;
    set_signature(store, op, p)
}

/// `uml.operation.add`: an operation of a class, an interface or a data type, at the place asked for.
fn operation_add(ctx: &mut C<'_>, p: &Params) -> R {
    let c = classifier_of(ctx.doc, p, "class")?;
    let Some((_, owned)) = member_lists(ctx.doc, c) else { return Err(refuse("class", no_members(ctx.doc, c, "operations"))) };
    let op = ctx.doc.create(OPERATION).map_err(fail)?;
    set_operation(ctx.doc, c, op, p)?;
    let at = place(p, list(ctx.doc, c, owned).len());
    ctx.doc.insert(c, owned, at, Value::Ref(op)).map_err(fail)?;
    select(ctx, op);
    Ok(json!({ "id": uuid_of(ctx.doc, op)? }))
}

/// `uml.operation.set`: an operation's name, visibility, flags and signature changed.
fn operation_set(ctx: &mut C<'_>, p: &Params) -> R {
    let op = id_of(ctx.doc, p, "operation", OPERATION, "an operation")?;
    let owner = owner_of(ctx.doc, op).unwrap_or(op);
    set_operation(ctx.doc, owner, op, p)?;
    Ok(json!({ "id": uuid_of(ctx.doc, op)? }))
}

/// The enumeration that owns `literal` in its list of literals.
pub fn literal_owner(store: &Store, literal: Obj) -> Option<Obj> {
    store.container(literal).ok().flatten().map(|(e, _)| e).filter(|e| store.class_of(*e).ok() == Some(ENUMERATION))
}

/// The literal of `enumeration`, other than `except`, that a literal named `name` would clash
/// with: literals are told apart within their enumeration whatever the case, so `Paid` and `paid`
/// clash. The commands and the shell ask this one rule.
pub fn literal_clash(store: &Store, enumeration: Obj, name: &str, except: Option<Obj>) -> Option<Obj> {
    let wanted = name.to_lowercase();
    crate::commands::literals(store, enumeration).into_iter().find(|l| Some(*l) != except && name_of(store, *l).to_lowercase() == wanted)
}

/// A literal's name in `enumeration`: a name, none of its other literals' whatever the case. A
/// clash names the name typed, the literal that has it and what is accepted instead.
fn check_literal_name(store: &Store, enumeration: Obj, name: &str, except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")));
    }
    if let Some(l) = literal_clash(store, enumeration, name, except) {
        let owner = name_of(store, enumeration);
        return Err(refuse(
            "name",
            format!(
                "there is a literal named {} already in {owner}, and {:?} matches it whatever the case; choose a name none of {owner}'s other literals has",
                name_of(store, l),
                short(name)
            ),
        ));
    }
    Ok(())
}

/// The literal under `key` with the enumeration that owns it.
fn literal_of(store: &Store, p: &Params, key: &str) -> Result<(Obj, Obj), CommandError> {
    let l = id_of(store, p, key, ENUMERATION_LITERAL, "a literal")?;
    let e = literal_owner(store, l)
        .ok_or_else(|| refuse(key, format!("{} belongs to no enumeration; a literal is one of an enumeration's", text(p, key))))?;
    Ok((l, e))
}

/// `uml.literal.add`: a literal of an enumeration, its name free among its literals whatever the
/// case, at the place `at` asks for (from 0 to the count of literals) or, at -1, at the end. A place
/// past the end is refused, not taken as the end; an enumeration that holds [`MAX_LITERALS`]
/// already takes no more.
fn literal_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx.doc, p, "enumeration", ENUMERATION, "an enumeration")?;
    let count = list(ctx.doc, e, enumeration::OWNED_LITERAL).len();
    if count >= MAX_LITERALS {
        return Err(refuse(
            "enumeration",
            format!(
                "{} holds {count} literals already, and an enumeration holds at most {MAX_LITERALS}; remove one before adding another",
                text(p, "enumeration")
            ),
        ));
    }
    let name = text(p, "name");
    check_literal_name(ctx.doc, e, name, None)?;
    let at = match p.integer("at").unwrap_or(-1) {
        -1 => count,
        at => usize::try_from(at)
            .ok()
            .filter(|i| *i <= count)
            .ok_or_else(|| refuse("at", format!("{at} is past the end of {} literals; a place from 0 to {count}, or -1 for the end", count)))?,
    };
    let at = u32::try_from(at).map_err(|_| refuse("at", format!("{at} is too far; a place from 0 to {count}, or -1 for the end")))?;
    let l = ctx.doc.create(ENUMERATION_LITERAL).map_err(fail)?;
    set_name(ctx.doc, l, name)?;
    ctx.doc.insert(e, enumeration::OWNED_LITERAL, at, Value::Ref(l)).map_err(fail)?;
    select(ctx, l);
    Ok(json!({ "id": uuid_of(ctx.doc, l)? }))
}

/// `uml.literal.rename`: a literal renamed, its new name free among its enumeration's other
/// literals whatever the case; it keeps its place and its identity.
fn literal_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let (l, e) = literal_of(ctx.doc, p, "literal")?;
    let name = text(p, "name");
    check_literal_name(ctx.doc, e, name, Some(l))?;
    set_name(ctx.doc, l, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, l)? }))
}

/// `uml.literal.move`: a literal to the place `at` among its enumeration's literals, from 0 to
/// their count less one; a place past the last is refused, not taken as the last.
fn literal_move(ctx: &mut C<'_>, p: &Params) -> R {
    let (l, e) = literal_of(ctx.doc, p, "literal")?;
    let all = list(ctx.doc, e, enumeration::OWNED_LITERAL);
    let last = all.len().saturating_sub(1);
    let from = all
        .iter()
        .position(|x| *x == l)
        .ok_or_else(|| refuse("literal", format!("{} is not among its enumeration's literals", text(p, "literal"))))?;
    let at = p.integer("at").unwrap_or(-1);
    let to = usize::try_from(at)
        .ok()
        .filter(|i| *i <= last)
        .ok_or_else(|| refuse("at", format!("{at} is past the last literal; a place from 0 to {last}")))?;
    let (Ok(from), Ok(to)) = (u32::try_from(from), u32::try_from(to)) else {
        return Err(refuse("at", format!("{at} is too far; a place from 0 to {last}")));
    };
    ctx.doc.move_item(e, enumeration::OWNED_LITERAL, from, to).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, l)? }))
}

/// `uml.class.set`: a class made abstract or not.
fn class_set(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx.doc, p, "class", CLASS, "a class")?;
    ctx.doc.set(c, class::IS_ABSTRACT, Value::Bool(p.bool("abstract").unwrap_or(false))).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn type_ref(store: &Store, o: Obj) -> Json {
    one(store, o, typed_element::TYPE).map_or(Json::Null, |t| json!({ "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t) }))
}

/// A member's visibility as the commands name it, public when it has none.
fn visibility_word(store: &Store, o: Obj) -> &'static str {
    match store.get(o, named_element::VISIBILITY) {
        Ok(Value::Enum(i)) => VISIBILITIES.get(i as usize).copied().unwrap_or("public"),
        _ => "public",
    }
}

/// The boolean `prop` of `o`, false when unset.
fn flag(store: &Store, o: Obj, prop: archforge_mof::PropId) -> bool {
    store.get(o, prop).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// A class, an interface, an enumeration, a data type or a primitive type with its members, as an
/// agent reads it: an enumeration's are its literals, in their order; a primitive type has none and
/// says whether it is a built-in type. Its `kind` is a word without spaces (`class`, `interface`,
/// `enumeration`, `data_type`, `primitive_type`).
fn class_get(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let c = classifier_of(store, p, "class")?;
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
    let is_class = store.class_of(c).ok() == Some(CLASS);
    let mut out = json!({
        "id": uuid_of(store, c)?, "kind": crate::commands::what(store, c).replace(' ', "_"), "name": name_of(store, c),
        "abstract": is_class && flag(store, c, class::IS_ABSTRACT), "attributes": attrs, "operations": ops,
    });
    if store.class_of(c).ok() == Some(PRIMITIVE_TYPE)
        && let Some(fields) = out.as_object_mut()
    {
        fields.insert("built_in".to_string(), Json::Bool(is_built_in(store, c)));
    }
    if store.class_of(c).ok() == Some(crate::ENUMERATION) {
        let literals: Vec<Json> = crate::commands::literals(store, c)
            .into_iter()
            .map(|l| json!({ "id": uuid_of(store, l).unwrap_or_default(), "name": name_of(store, l) }))
            .collect();
        if let Some(fields) = out.as_object_mut() {
            fields.insert("literals".to_string(), Json::Array(literals));
        }
    }
    Ok(out)
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
        "type: string = \"\"; a type's id, or Integer, String, Boolean, Real or UnlimitedNatural, the built-in types kept in the package Types (never a type of the model's own named alike), or nothing"
    };
}
macro_rules! multiplicity_line {
    () => {
        "multiplicity: string = \"\"; 1, 0..1, *, 1..*, 2..5, or nothing"
    };
}
macro_rules! visibility_line {
    () => {
        "visibility: string = \"public\", public|private|protected|package; the visibility, public for an interface's member"
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
            has_class,
            class_set
        ),
        cmd!(
            "uml.attribute.add",
            "Add attribute",
            &["Class"],
            None,
            concat!(
                "class: string; the class's, interface's or data type's id\nname: string; the attribute's name\n",
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
                "class: string; the class's, interface's or data type's id\n",
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
        cmd!(
            "uml.literal.add",
            "Add literal",
            &["Enumeration"],
            None,
            "enumeration: string; the enumeration's id\nname: string; the literal's name, free among the enumeration's literals whatever the case; an enumeration holds at most 256 literals\nat: integer = -1, -1..1000000; where among the literals, from 0 to their count, -1 for the end",
            has_enumerations,
            literal_add
        ),
        cmd!(
            "uml.literal.rename",
            "Rename literal",
            &["Enumeration"],
            None,
            "literal: string; the literal's id\nname: string; the new name, free among its enumeration's other literals whatever the case",
            has_enumerations,
            literal_rename
        ),
        cmd!(
            "uml.literal.move",
            "Move literal",
            &["Enumeration"],
            None,
            "literal: string; the literal's id\nat: integer, 0..1000000; its new place among the enumeration's literals, from 0 to their count less one",
            has_enumerations,
            literal_move
        ),
        cmd!(
            query "uml.class.get",
            "Get class",
            &["Class", "Interface", "Enumeration", "Data type", "Primitive type"],
            None,
            "class: string; the class's, interface's, enumeration's, data type's or primitive type's id; an enumeration comes with its literals, and a primitive type has no members",
            has_classes,
            class_get
        ),
        cmd!(query "uml.type.list", "List types", &["Class"], None, "", always, type_list),
    ]
}
