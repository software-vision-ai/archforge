//! The ER notation's commands over the store. Each validates what it is given before it changes
//! anything, keeps the ER rules, and refuses with a message naming the key. Ids in parameters are
//! the elements' UUIDs.

use std::collections::BTreeSet;

use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::rules::{self, MAX_NAME, MAX_TYPE, name_of};
use crate::{
    ATTRIBUTE, Cardinality, ENTITY, ENTITY_SIZE, EntityKind, ErView, FOREIGN_KEY, FOREIGN_KEY_COLUMN, RELATIONSHIP, ReferentialAction, SCHEMA,
    attribute, entity, foreign_key, foreign_key_column, named_element, relationship, schema,
};

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
                    .replace("foreignkeycolumn", "a foreign key column")
                    .replace("foreignkey", "a foreign key")
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

/// The foreign keys outside `obj` that depend on it or on anything it contains: those that
/// reference it, and those with a column on one of its attributes. They go before it.
pub fn keys_on(store: &Store, obj: Obj) -> Result<Vec<Obj>, Invalid> {
    let mut subtree = vec![obj];
    let mut seen = BTreeSet::from([obj]);
    let mut i = 0;
    while let Some(o) = subtree.get(i).copied() {
        for child in store.owned_elements(o)? {
            if seen.insert(child) {
                subtree.push(child);
            }
        }
        i += 1;
    }
    let mut keys = BTreeSet::new();
    for o in subtree {
        for (src, prop) in store.referrers(o) {
            let fk = if prop == foreign_key::REFERENCES {
                Some(src)
            } else if prop == foreign_key_column::ATTRIBUTE || prop == foreign_key_column::TARGET {
                store.container(src)?.map(|(fk, _)| fk)
            } else {
                None
            };
            if let Some(fk) = fk
                && fk != obj
                && !store.contains(obj, fk)
            {
                keys.insert(fk);
            }
        }
    }
    Ok(keys.into_iter().collect())
}

/// Turns associative entities among `entities` that lost what makes them one back into regular
/// entities, so a removal never leaves a broken mapping table.
fn demote_broken(store: &mut Store, entities: &BTreeSet<Obj>) -> Result<(), Invalid> {
    for e in entities {
        if store.is_alive(*e) && rules::kind_of(store, *e) == EntityKind::Associative && rules::associative_problem(store, *e).is_some() {
            store.set(*e, entity::KIND, Value::Enum(EntityKind::Regular.index()))?;
        }
    }
    Ok(())
}

/// Turns identifying relationships among `relationships` that lost their last foreign key into
/// non-identifying ones, so a removal never leaves an identifying relationship without a key.
fn unidentify_keyless(store: &mut Store, relationships: &BTreeSet<Obj>) -> Result<(), Invalid> {
    for r in relationships {
        let keyless = !store.referrers(*r).into_iter().any(|(_, prop)| prop == foreign_key::RELATIONSHIP);
        if store.is_alive(*r) && keyless && store.get(*r, relationship::IDENTIFYING)?.as_bool() == Some(true) {
            store.set(*r, relationship::IDENTIFYING, Value::Bool(false))?;
        }
    }
    Ok(())
}

/// Removes an ER element with everything it contains, the foreign keys that depend on any of it,
/// the diagrams of it, the nodes and edges that show any of them, and the trace links on any of
/// them, in one step. A mapping table that loses one of its keys becomes a regular entity, and an
/// identifying relationship that loses its key a non-identifying one.
pub fn remove_element(store: &mut Store, obj: Obj) -> Result<usize, Invalid> {
    store.transaction(|st| {
        let mut gone = 0;
        let mut owners: BTreeSet<Obj> = BTreeSet::new();
        let mut relationships: BTreeSet<Obj> = BTreeSet::new();
        let mut note = |st: &Store, fk: Obj| -> Result<(), Invalid> {
            if let Some((e, _)) = st.container(fk)? {
                owners.insert(e);
            }
            if let Some(r) = st.get(fk, foreign_key::RELATIONSHIP)?.as_ref() {
                relationships.insert(r);
            }
            Ok(())
        };
        if st.class_of(obj)? == FOREIGN_KEY {
            note(st, obj)?;
        }
        for fk in keys_on(st, obj)? {
            if st.is_alive(fk) {
                note(st, fk)?;
                gone += st.destroy(fk)?;
            }
        }
        gone += archforge_diagram::remove_views_of(st, obj)?;
        gone += archforge_core::destroy_with_links(st, obj)?;
        demote_broken(st, &owners)?;
        unidentify_keyless(st, &relationships)?;
        Ok(gone)
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
    let at = u32::try_from(at).map_err(|_| refuse("at", "too large"))?;
    let unique = p.bool("unique").unwrap_or(false);
    // A new key column changes the primary key that foreign keys elsewhere target.
    let (keys, entities) = keys_near(ctx.doc, e).map_err(fail)?;
    let a = ctx
        .doc
        .transaction(|st| {
            let a = st.create(ATTRIBUTE)?;
            st.set(a, named_element::NAME, Value::str(name))?;
            if !kind.is_empty() {
                st.set(a, attribute::TYPE, Value::str(kind))?;
            }
            st.set(a, attribute::PRIMARY_KEY, Value::Bool(key))?;
            st.set(a, attribute::NULLABLE, Value::Bool(null))?;
            st.set(a, attribute::UNIQUE, Value::Bool(unique))?;
            st.insert(e, entity::ATTRIBUTE, at, Value::Ref(a))?;
            keys_still_hold(st, &keys, &entities)?;
            Ok(a)
        })
        .map_err(|e| refuse("primary_key", e.to_string()))?;
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

/// Every foreign key that involves entity `e`, and the entities around them: what a change to `e`'s
/// attributes may break.
fn keys_near(store: &Store, e: Obj) -> Result<(Vec<Obj>, BTreeSet<Obj>), Invalid> {
    gather(store, BTreeSet::new(), Some(e))
}

/// Every foreign key that involves `a` or its entity, and the entities around them: what an edit
/// of `a` may break.
fn keys_around(store: &Store, a: Obj) -> Result<(Vec<Obj>, BTreeSet<Obj>), Invalid> {
    gather(store, keys_on(store, a)?.into_iter().collect(), rules::entity_of(store, a))
}

fn gather(store: &Store, mut keys: BTreeSet<Obj>, entity: Option<Obj>) -> Result<(Vec<Obj>, BTreeSet<Obj>), Invalid> {
    let mut entities = BTreeSet::new();
    if let Some(e) = entity {
        entities.insert(e);
        keys.extend(rules::foreign_keys(store, e));
        keys.extend(keys_on(store, e)?);
    }
    for fk in &keys {
        if let Some((e, _)) = store.container(*fk)? {
            entities.insert(e);
        }
    }
    Ok((keys.into_iter().collect(), entities))
}

/// After an edit inside a transaction: the foreign keys and mapping tables around `a` still keep
/// the rules, or the edit is refused.
fn keys_still_hold(store: &Store, keys: &[Obj], entities: &BTreeSet<Obj>) -> Result<(), Invalid> {
    for fk in keys {
        let parts = rules::key_parts(store, *fk);
        if let (Some(e), Some(r)) = (parts.entity, parts.references)
            && let Some((_, problem)) = rules::key_problem(store, e, r, &parts.pairs, [parts.on_delete, parts.on_update], parts.relationship)
        {
            return Err(Invalid::Refused(format!("the foreign key {} would break: {problem}", rules::label(store, *fk))));
        }
    }
    for e in entities {
        if rules::kind_of(store, *e) == EntityKind::Associative
            && let Some(problem) = rules::associative_problem(store, *e)
        {
            return Err(Invalid::Refused(problem));
        }
    }
    Ok(())
}

fn attribute_set_type(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let kind = text(p, "type");
    if !kind.is_empty() {
        check_name("type", kind, MAX_TYPE)?;
    }
    let (keys, entities) = keys_around(ctx.doc, a).map_err(fail)?;
    let value = if kind.is_empty() { Value::Null } else { Value::str(kind) };
    ctx.doc
        .transaction(|st| {
            st.set(a, attribute::TYPE, value)?;
            keys_still_hold(st, &keys, &entities)
        })
        .map_err(|e| refuse("type", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

fn attribute_set_flags(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key)?;
    let unique = p.bool("unique").unwrap_or(false);
    let (keys, entities) = keys_around(ctx.doc, a).map_err(fail)?;
    ctx.doc
        .transaction(|st| {
            st.set(a, attribute::PRIMARY_KEY, Value::Bool(key))?;
            st.set(a, attribute::NULLABLE, Value::Bool(null))?;
            st.set(a, attribute::UNIQUE, Value::Bool(unique))?;
            keys_still_hold(st, &keys, &entities)
        })
        .map_err(|e| refuse("primary_key", e.to_string()))?;
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

/// A name as a column or table name: lower case, anything but letters and digits as `_`.
fn snake(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    s.trim_matches('_').to_string()
}

/// `base`, or `base_2`, `base_3`, … : the first that `free` accepts, cut to the name length.
fn first_free(base: &str, free: impl Fn(&str) -> bool) -> Result<String, CommandError> {
    let base: String = base.chars().take(MAX_NAME - 6).collect();
    let base = if base.is_empty() { "column".to_string() } else { base };
    if free(&base) {
        return Ok(base);
    }
    (2..1000).map(|n| format!("{base}_{n}")).find(|c| free(c)).ok_or_else(|| CommandError::new(format!("no free name like {base:?}")))
}

/// Adds the columns that copy `parent`'s primary key into `child` and the foreign key over them,
/// linked to `relationship`. The columns are named `<parent>_<column>`, typed like their targets,
/// part of the child's key when `in_key`, nullable when `optional`, and unique when `unique` and the
/// parent's key has one column (a pair cannot be marked unique together).
fn add_key_columns(
    store: &mut Store,
    child: Obj,
    parent: Obj,
    relationship: Option<Obj>,
    in_key: bool,
    optional: bool,
    unique: bool,
) -> Result<Obj, Invalid> {
    let targets = rules::primary_key(store, parent);
    if targets.is_empty() {
        return Err(Invalid::Refused(format!("{} has no primary key to refer to", name_of(store, parent))));
    }
    let unique = unique && targets.len() == 1;
    let prefix = snake(&name_of(store, parent));
    let mut pairs = Vec::with_capacity(targets.len());
    for t in targets {
        let base = format!("{prefix}_{}", snake(&name_of(store, t)));
        let name = first_free(&base, |n| rules::attribute_name_free(store, child, n, None)).map_err(|e| Invalid::Refused(e.message))?;
        let a = store.create(ATTRIBUTE)?;
        store.set(a, named_element::NAME, Value::str(&name))?;
        let kind = rules::type_of(store, t);
        if !kind.is_empty() {
            store.set(a, attribute::TYPE, Value::str(&kind))?;
        }
        store.set(a, attribute::PRIMARY_KEY, Value::Bool(in_key))?;
        store.set(a, attribute::NULLABLE, Value::Bool(optional && !in_key))?;
        store.set(a, attribute::UNIQUE, Value::Bool(unique))?;
        store.push(child, entity::ATTRIBUTE, Value::Ref(a))?;
        pairs.push((a, t));
    }
    new_foreign_key(store, child, parent, &pairs, "", [ReferentialAction::NoAction, ReferentialAction::NoAction], relationship)
}

/// Creates a foreign key without checking the rules (the callers do).
fn new_foreign_key(
    store: &mut Store,
    e: Obj,
    references: Obj,
    pairs: &[(Obj, Obj)],
    name: &str,
    actions: [ReferentialAction; 2],
    relationship: Option<Obj>,
) -> Result<Obj, Invalid> {
    let fk = store.create(FOREIGN_KEY)?;
    if !name.is_empty() {
        store.set(fk, foreign_key::NAME, Value::str(name))?;
    }
    store.set(fk, foreign_key::REFERENCES, Value::Ref(references))?;
    store.set(fk, foreign_key::ON_DELETE, Value::Enum(actions[0].index()))?;
    store.set(fk, foreign_key::ON_UPDATE, Value::Enum(actions[1].index()))?;
    if let Some(r) = relationship {
        store.set(fk, foreign_key::RELATIONSHIP, Value::Ref(r))?;
    }
    for (a, t) in pairs {
        let c = store.create(FOREIGN_KEY_COLUMN)?;
        store.set(c, foreign_key_column::ATTRIBUTE, Value::Ref(*a))?;
        store.set(c, foreign_key_column::TARGET, Value::Ref(*t))?;
        store.push(fk, foreign_key::COLUMN, Value::Ref(c))?;
    }
    store.push(e, entity::FOREIGN_KEY, Value::Ref(fk))?;
    Ok(fk)
}

/// Creates a relationship without checking the rules (the callers do).
fn new_relationship(store: &mut Store, s: Obj, from: Obj, to: Obj, ends: [Cardinality; 2], name: &str, identifying: bool) -> Result<Obj, Invalid> {
    let r = store.create(RELATIONSHIP)?;
    store.set(r, relationship::FROM, Value::Ref(from))?;
    store.set(r, relationship::TO, Value::Ref(to))?;
    store.set(r, relationship::FROM_END, Value::Enum(ends[0].index()))?;
    store.set(r, relationship::TO_END, Value::Enum(ends[1].index()))?;
    store.set(r, relationship::IDENTIFYING, Value::Bool(identifying))?;
    if !name.is_empty() {
        store.set(r, relationship::NAME, Value::str(name))?;
    }
    store.push(s, schema::RELATIONSHIP, Value::Ref(r))?;
    Ok(r)
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
    let identifying = p.bool("identifying").unwrap_or(false);
    let with_key = p.bool("create_foreign_key").unwrap_or(false);
    if identifying && !with_key {
        return Err(refuse("identifying", "an identifying relationship needs its foreign key; set create_foreign_key"));
    }
    // The child holds the key: the end with many, or `to` for one to one.
    let (child, parent, parent_end, child_many) = if rules::is_many(to_end) {
        (to, from, from_end, true)
    } else if rules::is_many(from_end) {
        (from, to, to_end, true)
    } else {
        (to, from, from_end, false)
    };
    if with_key {
        if rules::is_many(from_end) && rules::is_many(to_end) {
            return Err(refuse("create_foreign_key", "a many-to-many relationship needs a mapping table; resolve it instead"));
        }
        if child == parent {
            return Err(refuse("create_foreign_key", "a relationship of an entity with itself takes its foreign key by hand"));
        }
        if rules::primary_key(ctx.doc, parent).is_empty() {
            return Err(refuse("create_foreign_key", format!("{} has no primary key to refer to", name_of(ctx.doc, parent))));
        }
    }
    let r = ctx
        .doc
        .transaction(|st| {
            let r = new_relationship(st, s, from, to, [from_end, to_end], &name, identifying)?;
            if with_key {
                add_key_columns(st, child, parent, Some(r), identifying, parent_end == Cardinality::ZeroOrOne, !child_many)?;
                // Identifying key columns join the child's primary key, which other keys may target.
                let (keys, entities) = keys_near(st, child)?;
                keys_still_hold(st, &keys, &entities)?;
            }
            Ok(r)
        })
        .map_err(|e| match (identifying, with_key) {
            (true, _) => refuse("identifying", e.to_string()),
            (false, true) => refuse("create_foreign_key", e.to_string()),
            (false, false) => fail(e),
        })?;
    select(ctx, vec![r]);
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

fn relationship_set_identifying(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    let identifying = p.bool("identifying").unwrap_or(false);
    let keys: Vec<Obj> = ctx.doc.referrers(r).into_iter().filter(|(_, prop)| *prop == foreign_key::RELATIONSHIP).map(|(fk, _)| fk).collect();
    if identifying && keys.is_empty() {
        return Err(refuse("identifying", "the relationship has no foreign key to make part of a primary key"));
    }
    ctx.doc
        .transaction(|st| {
            st.set(r, relationship::IDENTIFYING, Value::Bool(identifying))?;
            keys_still_hold(st, &keys, &BTreeSet::new())
        })
        .map_err(|e| refuse("identifying", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// Replaces a many-to-many relationship by a mapping table: an associative entity with the key
/// columns of both ends as its primary key, a foreign key to each end, and two identifying
/// one-to-many relationships; on every diagram that showed the relationship, the mapping table sits
/// in the middle of the old line with the two new lines. One step.
pub fn resolve(store: &mut Store, r: Obj, name: &str) -> Result<Obj, Invalid> {
    if !rules::is_many_to_many(store, r) {
        return Err(Invalid::Refused("only a many-to-many relationship becomes a mapping table".into()));
    }
    let end = |p| store.get(r, p).ok().and_then(|v| v.as_ref()).ok_or_else(|| Invalid::Refused("the relationship lacks an end".into()));
    let (from, to) = (end(relationship::FROM)?, end(relationship::TO)?);
    let s = rules::schema_of(store, r).ok_or_else(|| Invalid::Refused("the relationship belongs to no schema".into()))?;
    for e in [from, to] {
        if rules::primary_key(store, e).is_empty() {
            return Err(Invalid::Refused(format!("{} has no primary key to refer to", name_of(store, e))));
        }
    }
    let table = if name.is_empty() {
        let base = format!("{}_{}", snake(&name_of(store, from)), snake(&name_of(store, to)));
        first_free(&base, |n| rules::entity_name_free(store, s, n, None)).map_err(|e| Invalid::Refused(e.message))?
    } else if rules::entity_name_free(store, s, name, None) {
        name.to_string()
    } else {
        return Err(Invalid::Refused(format!("the schema has an entity named {name:?} already")));
    };
    // Where the old line was drawn: (diagram, node of `from`, node of `to`).
    let shown: Vec<(Obj, Obj, Obj)> = store
        .referrers(r)
        .into_iter()
        .filter(|(_, prop)| *prop == archforge_diagram::edge::ELEMENT)
        .filter_map(|(edge, _)| {
            let d = archforge_diagram::diagram_of(store, edge)?;
            let src = store.get(edge, archforge_diagram::edge::SOURCE).ok()?.as_ref()?;
            let dst = store.get(edge, archforge_diagram::edge::TARGET).ok()?.as_ref()?;
            Some((d, src, dst))
        })
        .collect();
    store.transaction(|st| {
        let m = st.create(ENTITY)?;
        st.set(m, named_element::NAME, Value::str(&table))?;
        st.push(s, schema::ENTITY, Value::Ref(m))?;
        let many = [Cardinality::ExactlyOne, Cardinality::ZeroOrMore];
        let r1 = new_relationship(st, s, from, m, many, "", true)?;
        let r2 = new_relationship(st, s, to, m, many, "", true)?;
        add_key_columns(st, m, from, Some(r1), true, false, false)?;
        add_key_columns(st, m, to, Some(r2), true, false, false)?;
        st.set(m, entity::KIND, Value::Enum(EntityKind::Associative.index()))?;
        for (d, src, dst) in &shown {
            place_between(st, *d, m, (*src, r1), (*dst, r2))?;
        }
        remove_element(st, r)?;
        Ok(m)
    })
}

/// Places `m` on diagram `d` midway between two nodes, or beside the node when both are one, and
/// draws an edge from each to it.
fn place_between(store: &mut Store, d: Obj, m: Obj, a: (Obj, Obj), b: (Obj, Obj)) -> Result<(), Invalid> {
    use archforge_diagram::{EDGE, NODE, diagram, edge, node};
    let centre = |st: &Store, n: Obj| {
        let r = |p| if let Ok(Value::Real(v)) = st.get(n, p) { v } else { 0.0 };
        (r(node::X) + r(node::WIDTH) / 2.0, r(node::Y) + r(node::HEIGHT) / 2.0)
    };
    let ((ax, ay), (bx, by)) = (centre(store, a.0), centre(store, b.0));
    let (w, h) = ENTITY_SIZE;
    let (bx, by) = if a.0 == b.0 { (ax + 2.0 * w, ay) } else { (bx, by) };
    let n = store.create(NODE)?;
    store.set(n, node::ELEMENT, Value::Ref(m))?;
    store.set(n, node::X, Value::Real((ax + bx) / 2.0 - w / 2.0))?;
    store.set(n, node::Y, Value::Real((ay + by) / 2.0 - h / 2.0))?;
    store.set(n, node::WIDTH, Value::Real(w))?;
    store.set(n, node::HEIGHT, Value::Real(h))?;
    store.push(d, diagram::NODE, Value::Ref(n))?;
    for (source, relationship) in [a, b] {
        let e = store.create(EDGE)?;
        store.set(e, edge::ELEMENT, Value::Ref(relationship))?;
        store.set(e, edge::SOURCE, Value::Ref(source))?;
        store.set(e, edge::TARGET, Value::Ref(n))?;
        store.push(d, diagram::EDGE, Value::Ref(e))?;
    }
    Ok(())
}

fn relationship_resolve(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    let name = text(p, "name");
    if !name.is_empty() {
        check_name("name", name, MAX_NAME)?;
        if let Some(s) = rules::schema_of(ctx.doc, r)
            && !rules::entity_name_free(ctx.doc, s, name, None)
        {
            return Err(refuse("name", format!("the schema has an entity named {name:?} already")));
        }
    }
    let m = resolve(ctx.doc, r, name).map_err(|e| refuse("relationship", e.to_string()))?;
    select(ctx, vec![m]);
    Ok(json!({ "id": uuid_of(ctx.doc, m)? }))
}

// Foreign keys.

fn action(p: &Params, key: &str) -> Result<ReferentialAction, CommandError> {
    Ok(match text(p, key) {
        "no-action" | "" => ReferentialAction::NoAction,
        "restrict" => ReferentialAction::Restrict,
        "cascade" => ReferentialAction::Cascade,
        "set-null" => ReferentialAction::SetNull,
        "set-default" => ReferentialAction::SetDefault,
        other => return Err(refuse(key, format!("{other:?} is not no-action, restrict, cascade, set-null or set-default"))),
    })
}

/// The referential action's name in parameters.
pub fn action_name(a: ReferentialAction) -> &'static str {
    match a {
        ReferentialAction::NoAction => "no-action",
        ReferentialAction::Restrict => "restrict",
        ReferentialAction::Cascade => "cascade",
        ReferentialAction::SetNull => "set-null",
        ReferentialAction::SetDefault => "set-default",
    }
}

fn key_name(p: &Params) -> Result<&str, CommandError> {
    let name = text(p, "name");
    if !name.is_empty() {
        check_name("name", name, MAX_NAME)?;
    }
    Ok(name)
}

/// The column pairs of the `columns` parameter: `[attribute id, target id]` each.
fn column_pairs(ctx: &C<'_>, p: &Params) -> Result<Vec<(Obj, Obj)>, CommandError> {
    let items = p.get("columns").and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > rules::MAX_KEY_COLUMNS {
        return Err(refuse("columns", format!("at most {} column pairs", rules::MAX_KEY_COLUMNS)));
    }
    let lookup = |v: Option<&Json>| -> Result<Obj, CommandError> {
        let id = v.and_then(Json::as_str).ok_or_else(|| refuse("columns", "every pair is [attribute id, target id]"))?;
        let uuid = Uuid::parse(id).ok_or_else(|| refuse("columns", format!("{id:?} is not an id")))?;
        let obj = ctx.doc.by_uuid(uuid).ok_or_else(|| refuse("columns", format!("there is no element {id}")))?;
        if ctx.doc.class_of(obj).ok() != Some(ATTRIBUTE) {
            return Err(refuse("columns", format!("{id} is not an attribute")));
        }
        Ok(obj)
    };
    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        let pair = item.as_array().filter(|a| a.len() == 2).ok_or_else(|| refuse("columns", "every pair is [attribute id, target id]"))?;
        out.push((lookup(pair.first())?, lookup(pair.get(1))?));
    }
    Ok(out)
}

fn foreign_key_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let references = id_of(ctx, p, "references", ENTITY)?;
    let pairs = column_pairs(ctx, p)?;
    let name = key_name(p)?.to_string();
    let actions = [action(p, "on_delete")?, action(p, "on_update")?];
    let rel = if text(p, "relationship").is_empty() { None } else { Some(id_of(ctx, p, "relationship", RELATIONSHIP)?) };
    if let Some((key, problem)) = rules::key_problem(ctx.doc, e, references, &pairs, actions, rel) {
        return Err(refuse(key, problem));
    }
    let fk = ctx.doc.transaction(|st| new_foreign_key(st, e, references, &pairs, &name, actions, rel)).map_err(fail)?;
    select(ctx, vec![fk]);
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

fn foreign_key_set_actions(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    let actions = [action(p, "on_delete")?, action(p, "on_update")?];
    let parts = rules::key_parts(ctx.doc, fk);
    if let (Some(e), Some(r)) = (parts.entity, parts.references)
        && let Some((_, problem)) = rules::key_problem(ctx.doc, e, r, &parts.pairs, actions, parts.relationship)
    {
        return Err(refuse("on_delete", problem));
    }
    ctx.doc
        .transaction(|st| {
            st.set(fk, foreign_key::ON_DELETE, Value::Enum(actions[0].index()))?;
            st.set(fk, foreign_key::ON_UPDATE, Value::Enum(actions[1].index()))
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

fn foreign_key_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    let name = key_name(p)?.to_string();
    set_name(ctx, fk, foreign_key::NAME, &name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

fn foreign_key_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    let gone = remove_element(ctx.doc, fk).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

fn entity_set_kind(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let kind = match text(p, "kind") {
        "regular" => EntityKind::Regular,
        "associative" => EntityKind::Associative,
        other => return Err(refuse("kind", format!("{other:?} is not regular or associative"))),
    };
    if kind == EntityKind::Associative
        && let Some(problem) = rules::associative_problem(ctx.doc, e)
    {
        return Err(refuse("kind", problem));
    }
    ctx.doc.set(e, entity::KIND, Value::Enum(kind.index())).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
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

/// Removes schemas, entities, attributes, relationships and foreign keys at once: the selection's
/// delete. Foreign keys, relationships and attributes first, then entities, then schemas, so a
/// cascade finds nothing missing; what an earlier removal already took is skipped.
fn remove_many(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(ctx, p, "ids")?;
    if ids.is_empty() {
        return Err(refuse("ids", "nothing to remove"));
    }
    let mut by_class: Vec<(ClassId, Obj)> = Vec::new();
    for o in &ids {
        let class = ctx.doc.class_of(*o).map_err(fail)?;
        if ![FOREIGN_KEY, RELATIONSHIP, ATTRIBUTE, ENTITY, SCHEMA].contains(&class) {
            return Err(refuse("ids", format!("{} is not an ER element", uuid_of(ctx.doc, *o)?)));
        }
        by_class.push((class, *o));
    }
    let order = [FOREIGN_KEY, RELATIONSHIP, ATTRIBUTE, ENTITY, SCHEMA];
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
    let in_key = store.referrers(a).iter().any(|(c, prop)| {
        *prop == foreign_key_column::ATTRIBUTE
            && store.container(*c).ok().flatten().and_then(|(fk, _)| store.container(fk).ok().flatten()).map(|(e, _)| e) == rules::entity_of(store, a)
    });
    json!({
        "foreign_key": in_key,
        "id": uuid_json(store, a),
        "name": name_of(store, a),
        "type": store.get(a, attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        "primary_key": flag(attribute::PRIMARY_KEY),
        "nullable": flag(attribute::NULLABLE),
        "unique": flag(attribute::UNIQUE),
    })
}

fn foreign_key_json(store: &Store, fk: Obj) -> Json {
    let parts = rules::key_parts(store, fk);
    let columns: Vec<Json> = parts.pairs.iter().map(|(a, t)| json!({ "attribute": uuid_json(store, *a), "target": uuid_json(store, *t) })).collect();
    json!({
        "id": uuid_json(store, fk),
        "name": store.get(fk, foreign_key::NAME).ok().and_then(|v| v.as_str().map(str::to_string)),
        "references": parts.references.map(|r| uuid_json(store, r)).unwrap_or(Json::Null),
        "columns": columns,
        "on_delete": action_name(parts.on_delete),
        "on_update": action_name(parts.on_update),
        "relationship": parts.relationship.map(|r| uuid_json(store, r)).unwrap_or(Json::Null),
    })
}

fn entity_json(store: &Store, e: Obj) -> Json {
    let attributes: Vec<Json> =
        store.list(e, entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(Value::as_ref).map(|a| attribute_json(store, a)).collect();
    let keys: Vec<Json> = rules::foreign_keys(store, e).into_iter().map(|fk| foreign_key_json(store, fk)).collect();
    let kind = match rules::kind_of(store, e) {
        EntityKind::Regular => "regular",
        EntityKind::Associative => "associative",
    };
    json!({ "id": uuid_json(store, e), "name": name_of(store, e), "kind": kind, "attributes": attributes, "foreign_keys": keys })
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
        "identifying": store.get(r, relationship::IDENTIFYING).ok().and_then(|v| v.as_bool()).unwrap_or(false),
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
    let notices: Vec<Json> = rules::notices(ctx.doc)
        .into_iter()
        .filter(|(r, _)| rules::schema_of(ctx.doc, *r) == Some(s))
        .map(|(r, notice)| json!({ "relationship": uuid_json(ctx.doc, r), "notice": notice }))
        .collect();
    Ok(json!({ "id": uuid_json(ctx.doc, s), "name": name_of(ctx.doc, s), "entities": entities, "relationships": relationships, "notices": notices }))
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
            "from: string; the first entity's id\nto: string; the second entity's id\nfrom_end: string = \"exactly-one\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the first end\nto_end: string = \"zero-or-more\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the second end\nname: string = \"\"; a name, or nothing\ncreate_foreign_key: bool = false; also add the foreign key and its columns to the end with many\nidentifying: bool = false; the foreign key is part of the child's primary key",
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
        cmd!(
            "er.relationship.set-identifying",
            "Set identifying",
            &["Relationship"],
            None,
            "relationship: string; the relationship's id\nidentifying: bool; its foreign key is part of the child's primary key",
            has_entities,
            relationship_set_identifying
        ),
        cmd!(
            "er.relationship.resolve",
            "Resolve into mapping table",
            &["Relationship"],
            None,
            "relationship: string; the many-to-many relationship's id\nname: string = \"\"; the mapping table's name, or nothing for <from>_<to>",
            has_entities,
            relationship_resolve
        ),
        cmd!(
            "er.foreign-key.add",
            "Add foreign key",
            &["Entity"],
            None,
            "entity: string; the id of the entity that holds the key\nreferences: string; the referenced entity's id\ncolumns: array; the column pairs, each [attribute id, target id]\nname: string = \"\"; a constraint name, or nothing\non_delete: string = \"no-action\", no-action|restrict|cascade|set-null|set-default; when the referenced row goes\non_update: string = \"no-action\", no-action|restrict|cascade|set-null|set-default; when the referenced key changes\nrelationship: string = \"\"; the id of the relationship it realises, or nothing",
            has_entities,
            foreign_key_add
        ),
        cmd!(
            "er.foreign-key.set-actions",
            "Set referential actions",
            &["Entity"],
            None,
            "foreign_key: string; the foreign key's id\non_delete: string, no-action|restrict|cascade|set-null|set-default; when the referenced row goes\non_update: string, no-action|restrict|cascade|set-null|set-default; when the referenced key changes",
            has_entities,
            foreign_key_set_actions
        ),
        cmd!("er.foreign-key.rename", "Rename foreign key", &["Entity"], None, "foreign_key: string; the foreign key's id\nname: string = \"\"; the new name, or nothing", has_entities, foreign_key_rename),
        cmd!("er.foreign-key.remove", "Remove foreign key", &["Entity"], None, "foreign_key: string; the foreign key's id", has_entities, foreign_key_remove),
        cmd!("er.entity.set-kind", "Set entity kind", &["Entity"], None, "entity: string; the entity's id\nkind: string, regular|associative; associative for a mapping table", has_entities, entity_set_kind),
        cmd!("er.remove", "Remove", &["Edit"], Some("Delete"), "ids: array; the ids of what goes", has_schemas, remove_many),
        cmd!(noundo "view.select", "Select", &["View"], None, "ids: array = []; the ids selected", always, view_select),
        cmd!(query "er.schema.list", "List schemas", &["Schema"], None, "", always, schema_list),
        cmd!(query "er.schema.get", "Get schema", &["Schema"], None, "schema: string; the schema's id", has_schemas, schema_get),
        cmd!(query "er.entity.list", "List entities", &["Entity"], None, "schema: string; the schema's id", has_schemas, entity_list),
        cmd!(query "er.entity.get", "Get entity", &["Entity"], None, "entity: string; the entity's id", has_entities, entity_get),
    ]);
    s
}
