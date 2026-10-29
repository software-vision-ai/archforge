//! The ER notation's commands over the store. Each validates what it is given before it changes
//! anything, keeps the ER rules, and refuses with a message naming the key. Ids in parameters are
//! the elements' UUIDs.

use std::collections::BTreeSet;

use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::rules::{self, MAX_NAME, MAX_TYPE, name_of};
use crate::{ATTRIBUTE, Cardinality, ENTITY, ErView, RELATIONSHIP, SCHEMA, attribute, entity, named_element, relationship, schema};

type C<'a> = Ctx<'a, Store, ErView>;
type R = Result<Json, CommandError>;

/// Why a command is not available.
pub const NO_SCHEMAS: &str = "There is no schema yet.";
pub const NO_ENTITIES: &str = "There is no entity yet.";

fn any_of(store: &Store, class: ClassId) -> bool {
    store.alive().any(|o| store.class_of(o).ok() == Some(class))
}

fn has_schemas(f: &Facts<'_, Store, ErView>) -> Result<(), &'static str> {
    if any_of(f.doc, SCHEMA) { Ok(()) } else { Err(NO_SCHEMAS) }
}

fn has_entities(f: &Facts<'_, Store, ErView>) -> Result<(), &'static str> {
    if any_of(f.doc, ENTITY) { Ok(()) } else { Err(NO_ENTITIES) }
}

fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

/// A store refusal, as the command's failure.
fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn text<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

/// The element the id under `key` names, of `class`.
fn id_of(ctx: &C<'_>, p: &Params, key: &str, class: ClassId) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{id:?} is not an id"))) };
    let Some(obj) = ctx.doc.by_uuid(uuid) else { return Err(refuse(key, format!("there is no element {id}"))) };
    if ctx.doc.class_of(obj).ok() != Some(class) {
        return Err(refuse(
            key,
            format!(
                "{id} is not {}",
                ctx.doc
                    .universe()
                    .class_name(class)
                    .to_lowercase()
                    .replace("relationship", "a relationship")
                    .replace("entity", "an entity")
                    .replace("attribute", "an attribute")
                    .replace("schema", "a schema")
            ),
        ));
    }
    Ok(obj)
}

fn check_name(key: &str, name: &str, max: usize) -> Result<(), CommandError> {
    match rules::name_problem(name, max) {
        Some(p) => Err(refuse(key, format!("the name {p}"))),
        None => Ok(()),
    }
}

/// The project's Model, the root that holds the schemas.
fn model(ctx: &C<'_>) -> Result<Obj, CommandError> {
    ctx.doc
        .alive()
        .find(|o| ctx.doc.class_of(*o).ok() == Some(archforge_core::MODEL))
        .ok_or_else(|| CommandError::new("the project has no model root"))
}

fn select(ctx: &mut C<'_>, objs: Vec<Obj>) {
    ctx.view.selection = objs;
}

fn set_name(ctx: &mut C<'_>, obj: Obj, prop: archforge_mof::PropId, name: &str) -> Result<(), CommandError> {
    ctx.doc.set(obj, prop, if name.is_empty() { Value::Null } else { Value::str(name) }).map_err(fail)
}

// Schemas.

fn schema_add(ctx: &mut C<'_>, p: &Params) -> R {
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    let m = model(ctx)?;
    let s = ctx.doc.create(SCHEMA).map_err(fail)?;
    set_name(ctx, s, named_element::NAME, name)?;
    ctx.doc.push(m, archforge_core::model::MEMBER, Value::Ref(s)).map_err(fail)?;
    select(ctx, vec![s]);
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

fn schema_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    set_name(ctx, s, named_element::NAME, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

fn schema_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let gone = remove_element(ctx.doc, s).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Entities.

fn entity_add(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    if !rules::entity_name_free(ctx.doc, s, name, None) {
        return Err(refuse("name", format!("the schema has an entity named {name:?} already")));
    }
    let e = ctx.doc.create(ENTITY).map_err(fail)?;
    set_name(ctx, e, named_element::NAME, name)?;
    ctx.doc.push(s, schema::ENTITY, Value::Ref(e)).map_err(fail)?;
    select(ctx, vec![e]);
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
}

fn entity_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    if let Some(s) = rules::schema_of(ctx.doc, e)
        && !rules::entity_name_free(ctx.doc, s, name, Some(e))
    {
        return Err(refuse("name", format!("the schema has an entity named {name:?} already")));
    }
    set_name(ctx, e, named_element::NAME, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
}

/// Removes an ER element with everything it contains, the diagrams of it, the nodes and edges that
/// show any of them, and the trace links on any of them, in one step.
pub fn remove_element(store: &mut Store, obj: Obj) -> Result<usize, Invalid> {
    store.transaction(|st| {
        let views = archforge_diagram::remove_views_of(st, obj)?;
        Ok(views + archforge_core::destroy_with_links(st, obj)?)
    })
}

/// Removes an entity with its attributes, the relationships that reach it, the views of any of
/// them, and the trace links on any of them.
pub fn remove_entity(store: &mut Store, e: Obj) -> Result<usize, Invalid> {
    store.transaction(|st| {
        let mut gone = 0;
        let relationships: BTreeSet<Obj> =
            st.referrers(e).into_iter().filter(|(_, prop)| *prop == relationship::FROM || *prop == relationship::TO).map(|(r, _)| r).collect();
        for r in relationships {
            if st.is_alive(r) {
                gone += remove_element(st, r)?;
            }
        }
        gone += remove_element(st, e)?;
        Ok(gone)
    })
}

fn entity_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let gone = remove_entity(ctx.doc, e).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Attributes.

/// Whether a key may be nullable: `auto` makes a key not nullable and anything else nullable.
fn nullable(p: &Params, primary_key: bool) -> Result<bool, CommandError> {
    match text(p, "nullable") {
        "auto" | "" => Ok(!primary_key),
        "yes" => {
            if primary_key {
                return Err(refuse("nullable", "a key cannot be nullable"));
            }
            Ok(true)
        }
        "no" => Ok(false),
        other => Err(refuse("nullable", format!("{other:?} is not auto, yes or no"))),
    }
}

fn attribute_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    if !rules::attribute_name_free(ctx.doc, e, name, None) {
        return Err(refuse("name", format!("the entity has an attribute named {name:?} already")));
    }
    let kind = text(p, "type");
    if !kind.is_empty() {
        check_name("type", kind, MAX_TYPE)?;
    }
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key)?;
    let len = ctx.doc.list(e, entity::ATTRIBUTE).map_err(fail)?.len();
    let at = match p.integer("at") {
        Some(i) if i < 0 => len,
        Some(i) => usize::try_from(i).ok().filter(|i| *i <= len).ok_or_else(|| refuse("at", format!("the entity has {len} attributes")))?,
        None => len,
    };
    let a = ctx.doc.create(ATTRIBUTE).map_err(fail)?;
    set_name(ctx, a, named_element::NAME, name)?;
    set_name(ctx, a, attribute::TYPE, kind)?;
    ctx.doc.set(a, attribute::PRIMARY_KEY, Value::Bool(key)).map_err(fail)?;
    ctx.doc.set(a, attribute::NULLABLE, Value::Bool(null)).map_err(fail)?;
    ctx.doc.set(a, attribute::UNIQUE, Value::Bool(p.bool("unique").unwrap_or(false))).map_err(fail)?;
    let at = u32::try_from(at).map_err(|_| refuse("at", "too large"))?;
    ctx.doc.insert(e, entity::ATTRIBUTE, at, Value::Ref(a)).map_err(fail)?;
    select(ctx, vec![a]);
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

fn attribute_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    if let Some((e, _)) = ctx.doc.container(a).map_err(fail)?
        && !rules::attribute_name_free(ctx.doc, e, name, Some(a))
    {
        return Err(refuse("name", format!("the entity has an attribute named {name:?} already")));
    }
    set_name(ctx, a, named_element::NAME, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

fn attribute_set_type(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let kind = text(p, "type");
    if !kind.is_empty() {
        check_name("type", kind, MAX_TYPE)?;
    }
    set_name(ctx, a, attribute::TYPE, kind)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

fn attribute_set_flags(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key)?;
    let unique = p.bool("unique").unwrap_or(false);
    ctx.doc.set(a, attribute::PRIMARY_KEY, Value::Bool(key)).map_err(fail)?;
    ctx.doc.set(a, attribute::NULLABLE, Value::Bool(null)).map_err(fail)?;
    ctx.doc.set(a, attribute::UNIQUE, Value::Bool(unique)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

fn attribute_move(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let Some((e, _)) = ctx.doc.container(a).map_err(fail)? else { return Err(refuse("attribute", "the attribute belongs to no entity")) };
    let list = ctx.doc.list(e, entity::ATTRIBUTE).map_err(fail)?;
    let from = list.iter().position(|v| *v == Value::Ref(a)).ok_or_else(|| refuse("attribute", "the attribute is not in its entity's list"))?;
    let to = usize::try_from(p.integer("to").unwrap_or(0))
        .ok()
        .filter(|i| *i < list.len())
        .ok_or_else(|| refuse("to", format!("the entity has {} attributes", list.len())))?;
    let (from, to) = (u32::try_from(from).map_err(|_| refuse("attribute", "too far in"))?, u32::try_from(to).map_err(|_| refuse("to", "too large"))?);
    ctx.doc.move_item(e, entity::ATTRIBUTE, from, to).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)?, "to": to }))
}

fn attribute_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let gone = remove_element(ctx.doc, a).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Relationships.

fn cardinality(p: &Params, key: &str) -> Result<Cardinality, CommandError> {
    let s = text(p, key);
    Cardinality::from_index(match s {
        "zero-or-one" => 0,
        "exactly-one" => 1,
        "zero-or-more" => 2,
        "one-or-more" => 3,
        _ => return Err(refuse(key, format!("{s:?} is not a cardinality"))),
    })
    .ok_or_else(|| refuse(key, "not a cardinality"))
}

/// The cardinality's name in parameters.
pub fn cardinality_name(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ZeroOrOne => "zero-or-one",
        Cardinality::ExactlyOne => "exactly-one",
        Cardinality::ZeroOrMore => "zero-or-more",
        Cardinality::OneOrMore => "one-or-more",
    }
}

fn relationship_name(p: &Params) -> Result<&str, CommandError> {
    let name = text(p, "name");
    if !name.is_empty() {
        check_name("name", name, MAX_NAME)?;
    }
    Ok(name)
}

fn relationship_add(ctx: &mut C<'_>, p: &Params) -> R {
    let from = id_of(ctx, p, "from", ENTITY)?;
    let to = id_of(ctx, p, "to", ENTITY)?;
    let (from_end, to_end) = (cardinality(p, "from_end")?, cardinality(p, "to_end")?);
    let name = relationship_name(p)?.to_string();
    let s = rules::schema_of(ctx.doc, from).ok_or_else(|| refuse("from", "the entity belongs to no schema"))?;
    if rules::schema_of(ctx.doc, to) != Some(s) {
        return Err(refuse("to", "the entities belong to different schemas"));
    }
    let r = ctx.doc.create(RELATIONSHIP).map_err(fail)?;
    ctx.doc.set(r, relationship::FROM, Value::Ref(from)).map_err(fail)?;
    ctx.doc.set(r, relationship::TO, Value::Ref(to)).map_err(fail)?;
    ctx.doc.set(r, relationship::FROM_END, Value::Enum(from_end.index())).map_err(fail)?;
    ctx.doc.set(r, relationship::TO_END, Value::Enum(to_end.index())).map_err(fail)?;
    set_name(ctx, r, relationship::NAME, &name)?;
    ctx.doc.push(s, schema::RELATIONSHIP, Value::Ref(r)).map_err(fail)?;
    select(ctx, vec![r]);
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

fn relationship_set_ends(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    let (from_end, to_end) = (cardinality(p, "from_end")?, cardinality(p, "to_end")?);
    ctx.doc.set(r, relationship::FROM_END, Value::Enum(from_end.index())).map_err(fail)?;
    ctx.doc.set(r, relationship::TO_END, Value::Enum(to_end.index())).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

fn relationship_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    let name = relationship_name(p)?.to_string();
    set_name(ctx, r, relationship::NAME, &name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

fn relationship_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    let gone = remove_element(ctx.doc, r).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

/// The ids of an array parameter: strings naming live elements, none twice.
fn id_list(ctx: &C<'_>, p: &Params, key: &str) -> Result<Vec<Obj>, CommandError> {
    let items = p.get(key).and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > archforge_mof::store::MAX_LIST {
        return Err(refuse(key, format!("at most {} ids", archforge_mof::store::MAX_LIST)));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for v in &items {
        let id = v.as_str().ok_or_else(|| refuse(key, "every id must be a string"))?;
        let uuid = Uuid::parse(id).ok_or_else(|| refuse(key, format!("{id:?} is not an id")))?;
        let obj = ctx.doc.by_uuid(uuid).ok_or_else(|| refuse(key, format!("there is no element {id}")))?;
        if !seen.insert(obj) {
            return Err(refuse(key, format!("{id} is listed twice")));
        }
        out.push(obj);
    }
    Ok(out)
}

/// Removes schemas, entities, attributes and relationships at once: the selection's delete.
/// Relationships and attributes first, then entities, then schemas, so a cascade finds nothing
/// missing; what an earlier removal already took is skipped.
fn remove_many(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(ctx, p, "ids")?;
    if ids.is_empty() {
        return Err(refuse("ids", "nothing to remove"));
    }
    let mut by_class: Vec<(ClassId, Obj)> = Vec::new();
    for o in &ids {
        let class = ctx.doc.class_of(*o).map_err(fail)?;
        if ![RELATIONSHIP, ATTRIBUTE, ENTITY, SCHEMA].contains(&class) {
            return Err(refuse("ids", format!("{} is not an ER element", uuid_of(ctx.doc, *o)?)));
        }
        by_class.push((class, *o));
    }
    let order = [RELATIONSHIP, ATTRIBUTE, ENTITY, SCHEMA];
    let mut gone = 0;
    for class in order {
        for (c, o) in &by_class {
            if *c == class && ctx.doc.is_alive(*o) {
                gone += if class == ENTITY { remove_entity(ctx.doc, *o) } else { remove_element(ctx.doc, *o) }.map_err(fail)?;
            }
        }
    }
    Ok(json!({ "removed": gone }))
}

fn view_select(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(ctx, p, "ids")?;
    ctx.view.selection = ids;
    Ok(Json::Null)
}

// Queries.

fn uuid_json(store: &Store, obj: Obj) -> Json {
    store.uuid(obj).map(|u| Json::String(u.to_string())).unwrap_or(Json::Null)
}

fn attribute_json(store: &Store, a: Obj) -> Json {
    let flag = |p| store.get(a, p).ok().and_then(|v| v.as_bool()).unwrap_or(false);
    json!({
        "id": uuid_json(store, a),
        "name": name_of(store, a),
        "type": store.get(a, attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        "primary_key": flag(attribute::PRIMARY_KEY),
        "nullable": flag(attribute::NULLABLE),
        "unique": flag(attribute::UNIQUE),
    })
}

fn entity_json(store: &Store, e: Obj) -> Json {
    let attributes: Vec<Json> =
        store.list(e, entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(Value::as_ref).map(|a| attribute_json(store, a)).collect();
    json!({ "id": uuid_json(store, e), "name": name_of(store, e), "attributes": attributes })
}

fn relationship_json(store: &Store, r: Obj) -> Json {
    let end = |p| store.get(r, p).ok().and_then(|v| v.as_ref()).map(|e| uuid_json(store, e)).unwrap_or(Json::Null);
    let card = |p| store.get(r, p).ok().and_then(|v| if let Value::Enum(i) = v { Cardinality::from_index(i) } else { None }).map(cardinality_name);
    json!({
        "id": uuid_json(store, r),
        "name": store.get(r, relationship::NAME).ok().and_then(|v| v.as_str().map(str::to_string)),
        "from": end(relationship::FROM),
        "to": end(relationship::TO),
        "from_end": card(relationship::FROM_END),
        "to_end": card(relationship::TO_END),
    })
}

fn schema_list(ctx: &mut C<'_>, _: &Params) -> R {
    let list: Vec<Json> = ctx
        .doc
        .alive()
        .filter(|o| ctx.doc.class_of(*o).ok() == Some(SCHEMA))
        .map(|s| json!({ "id": uuid_json(ctx.doc, s), "name": name_of(ctx.doc, s) }))
        .collect();
    Ok(Json::Array(list))
}

fn entity_list(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let list: Vec<Json> = ctx
        .doc
        .list(s, schema::ENTITY)
        .map_err(fail)?
        .iter()
        .filter_map(Value::as_ref)
        .map(|e| json!({ "id": uuid_json(ctx.doc, e), "name": name_of(ctx.doc, e) }))
        .collect();
    Ok(Json::Array(list))
}

fn entity_get(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    Ok(entity_json(ctx.doc, e))
}

fn schema_get(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let entities: Vec<Json> =
        ctx.doc.list(s, schema::ENTITY).map_err(fail)?.iter().filter_map(Value::as_ref).map(|e| entity_json(ctx.doc, e)).collect();
    let relationships: Vec<Json> =
        ctx.doc.list(s, schema::RELATIONSHIP).map_err(fail)?.iter().filter_map(Value::as_ref).map(|r| relationship_json(ctx.doc, r)).collect();
    Ok(json!({ "id": uuid_json(ctx.doc, s), "name": name_of(ctx.doc, s), "entities": entities, "relationships": relationships }))
}

/// The ER notation's command table, with the edit area.
pub fn specs() -> Vec<CommandSpec<Store, ErView>> {
    let mut s = archforge_engine::cmd::edit::specs();
    s.extend([
        cmd!("er.schema.add", "Add schema", &["Schema"], None, "name: string; the schema's name", always, schema_add),
        cmd!("er.schema.rename", "Rename schema", &["Schema"], None, "schema: string; the schema's id\nname: string; the new name", has_schemas, schema_rename),
        cmd!("er.schema.remove", "Remove schema", &["Schema"], None, "schema: string; the schema's id", has_schemas, schema_remove),
        cmd!("er.entity.add", "Add entity", &["Entity"], Some("Ctrl+E"), "schema: string; the schema's id\nname: string; the entity's name", has_schemas, entity_add),
        cmd!("er.entity.rename", "Rename entity", &["Entity"], None, "entity: string; the entity's id\nname: string; the new name", has_entities, entity_rename),
        cmd!("er.entity.remove", "Remove entity", &["Entity"], None, "entity: string; the entity's id", has_entities, entity_remove),
        cmd!(
            "er.attribute.add",
            "Add attribute",
            &["Entity"],
            None,
            "entity: string; the entity's id\nname: string; the attribute's name\ntype: string = \"\"; its type, free text\nprimary_key: bool = false; part of the key\nnullable: string = \"auto\", auto|yes|no; nullable (auto: a key is not)\nunique: bool = false; unique\nat: integer = -1, -1..1000000; where in the entity, -1 for the end",
            has_entities,
            attribute_add
        ),
        cmd!("er.attribute.rename", "Rename attribute", &["Entity"], None, "attribute: string; the attribute's id\nname: string; the new name", has_entities, attribute_rename),
        cmd!("er.attribute.set-type", "Set attribute type", &["Entity"], None, "attribute: string; the attribute's id\ntype: string = \"\"; its type, free text", has_entities, attribute_set_type),
        cmd!(
            "er.attribute.set-flags",
            "Set attribute flags",
            &["Entity"],
            None,
            "attribute: string; the attribute's id\nprimary_key: bool; part of the key\nnullable: string, auto|yes|no; nullable (auto: a key is not)\nunique: bool; unique",
            has_entities,
            attribute_set_flags
        ),
        cmd!("er.attribute.move", "Move attribute", &["Entity"], None, "attribute: string; the attribute's id\nto: integer, 0..1000000; its new place", has_entities, attribute_move),
        cmd!("er.attribute.remove", "Remove attribute", &["Entity"], None, "attribute: string; the attribute's id", has_entities, attribute_remove),
        cmd!(
            "er.relationship.add",
            "Add relationship",
            &["Relationship"],
            None,
            "from: string; the first entity's id\nto: string; the second entity's id\nfrom_end: string = \"exactly-one\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the first end\nto_end: string = \"zero-or-more\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the second end\nname: string = \"\"; a name, or nothing",
            has_entities,
            relationship_add
        ),
        cmd!(
            "er.relationship.set-ends",
            "Set relationship ends",
            &["Relationship"],
            None,
            "relationship: string; the relationship's id\nfrom_end: string = \"exactly-one\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the first end\nto_end: string = \"zero-or-more\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the second end",
            has_entities,
            relationship_set_ends
        ),
        cmd!("er.relationship.rename", "Rename relationship", &["Relationship"], None, "relationship: string; the relationship's id\nname: string = \"\"; the new name, or nothing", has_entities, relationship_rename),
        cmd!("er.relationship.remove", "Remove relationship", &["Relationship"], None, "relationship: string; the relationship's id", has_entities, relationship_remove),
        cmd!("er.remove", "Remove", &["Edit"], Some("Delete"), "ids: array; the ids of what goes", has_schemas, remove_many),
        cmd!(noundo "view.select", "Select", &["View"], None, "ids: array = []; the ids selected", always, view_select),
        cmd!(query "er.schema.list", "List schemas", &["Schema"], None, "", always, schema_list),
        cmd!(query "er.schema.get", "Get schema", &["Schema"], None, "schema: string; the schema's id", has_schemas, schema_get),
        cmd!(query "er.entity.list", "List entities", &["Entity"], None, "schema: string; the schema's id", has_schemas, entity_list),
        cmd!(query "er.entity.get", "Get entity", &["Entity"], None, "entity: string; the entity's id", has_entities, entity_get),
    ]);
    s
}
