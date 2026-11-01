//! The ER notation's commands over the store. Each validates what it is given before it changes
//! anything, keeps the ER rules, and refuses with a message naming the key. Ids in parameters are
//! the elements' UUIDs.

use std::collections::BTreeSet;

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, Store, TypeRef, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::rules::{self, IndexColumnSpec, MAX_NAME, MAX_TEXT, MAX_TYPE, name_of};
use crate::{
    ATTRIBUTE, CHECK_CONSTRAINT, Cardinality, DOMAIN, DOMAIN_CHECK, Deferrability, ELEMENT, ENTITY, ENTITY_SIZE, ENUM_TYPE, EntityKind, FOREIGN_KEY,
    GENERALIZATION, Generation, INDEX_COLUMN, MatchType, NullsOrder, PRIMARY_KEY, RELATIONSHIP, ReferentialAction, SCHEMA, SEQUENCE, SortDirection,
    SubtypeMapping, TABLE_INDEX, UNIQUE_CONSTRAINT, VIEW, attribute, check_constraint, domain, domain_check, element, entity, enum_type, foreign_key,
    generalization, index_column, key, named_element, relationship, schema, sequence, table_index, unique_constraint, view,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// Why a command is not available.
pub const NO_SCHEMAS: &str = "There is no schema yet.";
pub const NO_ENTITIES: &str = "There is no entity yet.";

fn any_of(store: &Store, class: ClassId) -> bool {
    store.alive().any(|o| store.class_of(o).ok() == Some(class))
}

fn has_schemas(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if any_of(f.doc, SCHEMA) { Ok(()) } else { Err(NO_SCHEMAS) }
}

fn has_entities(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
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

/// A store refusal as a refusal of `key`; anything else as the command's failure.
fn refused_as(key: &'static str) -> impl Fn(Invalid) -> CommandError {
    move |e| match e {
        Invalid::Refused(why) => refuse(key, why),
        other => fail(other),
    }
}

/// At most 40 characters of a value for an error text.
fn short(s: &str) -> String {
    s.chars().take(40).collect()
}

/// The generalization that owns relationship `r` as a subtype's key (slice S6).
fn subtype_key_owner(store: &Store, r: Obj) -> Option<Obj> {
    store.referrers(r).into_iter().find(|(g, p)| *p == generalization::KEY_RELATIONSHIP && store.is_alive(*g)).map(|(g, _)| g)
}

/// The generalization whose subtype key `obj` is a piece of: the key relationship, its foreign
/// key, one of its columns, or the subtype's primary key that holds them.
fn subtype_key_piece(store: &Store, obj: Obj) -> Option<Obj> {
    let owned_fk = |fk: Obj| rules::key_parts(store, fk).relationship.and_then(|r| subtype_key_owner(store, r));
    match store.class_of(obj).ok()? {
        c if c == RELATIONSHIP => subtype_key_owner(store, obj),
        c if c == FOREIGN_KEY => owned_fk(obj),
        c if c == ATTRIBUTE => store.referrers(obj).into_iter().filter(|(_, p)| *p == foreign_key::COLUMN).find_map(|(fk, _)| owned_fk(fk)),
        c if c == PRIMARY_KEY => {
            let e = rules::entity_of(store, obj)?;
            let pk = rules::key_columns(store, obj);
            rules::foreign_keys(store, e)
                .into_iter()
                .filter(|fk| rules::key_parts(store, *fk).columns.iter().any(|a| pk.contains(a)))
                .find_map(owned_fk)
        }
        _ => None,
    }
}

fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

/// The element the id under `key` names, of `class`.
fn id_of(ctx: &C<'_>, p: &Params, key: &str, class: ClassId) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    let Some(obj) = ctx.doc.by_uuid(uuid) else { return Err(refuse(key, format!("there is no element {}", short(id)))) };
    if ctx.doc.class_of(obj).ok() != Some(class) {
        return Err(refuse(
            key,
            format!(
                "{id} is not {}",
                ctx.doc
                    .universe()
                    .class_name(class)
                    .to_lowercase()
                    .replace("foreignkey", "a foreign key")
                    .replace("primarykey", "a primary key")
                    .replace("uniqueconstraint", "a unique constraint")
                    .replace("relationship", "a relationship")
                    .replace("entity", "an entity")
                    .replace("attribute", "an attribute")
                    .replace("schema", "a schema")
                    .replace("generalization", "a generalization")
                    .replace("sequence", "a sequence")
                    .replace("view", "a view")
            ),
        ));
    }
    Ok(obj)
}

/// Refuses with `key` when a text problem is found.
fn check_text(key: &str, problem: Option<String>) -> Result<(), CommandError> {
    match problem {
        Some(p) => Err(refuse(key, format!("the text {p}"))),
        None => Ok(()),
    }
}

/// A type name: free text, but its size goes into the size parameters, never into the name.
fn check_type(kind: &str) -> Result<(), CommandError> {
    if kind.is_empty() {
        return Ok(());
    }
    check_name("type", kind, MAX_TYPE)?;
    if kind.contains('(') {
        return Err(refuse("type", "carries its own size; give length, precision or scale instead"));
    }
    Ok(())
}

/// The generation word under `generation` and the expression under `expression`.
fn generation_params(p: &Params) -> Result<(Generation, &str), CommandError> {
    let word = text(p, "generation");
    let generation = GENERATION_WORDS.iter().find(|(_, w)| *w == word).map(|(g, _)| *g).ok_or_else(|| {
        refuse("generation", format!("{word:?} is not one of {}", GENERATION_WORDS.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(", ")))
    })?;
    let expression = text(p, "expression");
    check_text("expression", rules::text_problem(expression, MAX_TEXT))?;
    Ok((generation, expression))
}

fn text_or_null(s: &str) -> Value {
    if s.is_empty() { Value::Null } else { Value::str(s) }
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
    let gone = remove_schema(ctx.doc, s).map_err(refused_as("schema"))?;
    Ok(json!({ "removed": gone }))
}

/// Removes a schema with everything in it. Its tables go one by one first, so the relationships of
/// other schemas that reach them go too (slice S7: relationships may cross schemas).
pub fn remove_schema(store: &mut Store, s: Obj) -> Result<usize, Invalid> {
    store.transaction(|st| {
        let mut gone = 0;
        let tables: Vec<Obj> = st.list(s, schema::ENTITY)?.iter().filter_map(Value::as_ref).collect();
        for e in tables {
            if st.is_alive(e) {
                gone += remove_entity(st, e)?;
            }
        }
        Ok(gone + remove_element(st, s)?)
    })
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

/// How deep a removal may cascade through keys built on each other: four nested removals per level
/// of the longest chain the commands build.
const REMOVAL_DEPTH: u32 = 4 * rules::MAX_KEY_COLUMNS as u32;

/// What outside `obj` depends on it or on anything it contains: the keys that list one of its
/// attributes, and the foreign keys that point at one of its keys or at one of those, or list one
/// of its attributes. As in SQL, a key loses all of itself with any of its columns.
pub fn dependents(store: &Store, obj: Obj) -> Result<(Vec<Obj>, Vec<Obj>), Invalid> {
    let (subtree, seen) = subtree_of(store, obj)?;
    let outside = |x: Obj| x != obj && !seen.contains(&x);
    let mut keys = BTreeSet::new();
    let mut fks = BTreeSet::new();
    for o in &subtree {
        for (src, prop) in store.referrers(*o) {
            if prop == key::COLUMN && outside(src) {
                keys.insert(src);
            } else if (prop == foreign_key::COLUMN || prop == foreign_key::REFERENCES) && outside(src) {
                fks.insert(src);
            }
        }
    }
    for k in subtree.iter().copied().filter(|o| rules::is_key(store, *o)).chain(keys.iter().copied()) {
        fks.extend(rules::keys_onto(store, k).into_iter().filter(|fk| outside(*fk)));
    }
    Ok((keys.into_iter().collect(), fks.into_iter().collect()))
}

/// `obj` with everything it contains, in discovery order, and the same as a set.
fn subtree_of(store: &Store, obj: Obj) -> Result<(Vec<Obj>, BTreeSet<Obj>), Invalid> {
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
    Ok((subtree, seen))
}

/// What of the indexes outside `obj` goes with it: the index columns on any attribute it contains,
/// or the whole index when every column of it goes (slice S3).
pub fn index_dependents(store: &Store, obj: Obj) -> Result<Vec<Obj>, Invalid> {
    let (subtree, seen) = subtree_of(store, obj)?;
    let mut columns: BTreeSet<Obj> = BTreeSet::new();
    for o in &subtree {
        for (src, prop) in store.referrers(*o) {
            if prop == index_column::ATTRIBUTE && !seen.contains(&src) {
                columns.insert(src);
            }
        }
    }
    let mut out: Vec<Obj> = Vec::new();
    let mut indexes: BTreeSet<Obj> = BTreeSet::new();
    for c in &columns {
        let Some((ix, _)) = store.container(*c)? else { continue };
        if !indexes.insert(ix) {
            continue;
        }
        if rules::index_columns(store, ix).iter().all(|c| columns.contains(c)) {
            out.push(ix);
        } else {
            out.extend(rules::index_columns(store, ix).into_iter().filter(|c| columns.contains(c)));
        }
    }
    Ok(out)
}

/// The foreign keys outside `obj` that depend on it or on anything it contains.
pub fn keys_on(store: &Store, obj: Obj) -> Result<Vec<Obj>, Invalid> {
    Ok(dependents(store, obj)?.1)
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

/// Turns identifying relationships among `relationships` that lost their foreign key, or whose key
/// columns left the child's primary key, into non-identifying ones.
fn unidentify_broken(store: &mut Store, relationships: &BTreeSet<Obj>) -> Result<(), Invalid> {
    for r in relationships {
        if !store.is_alive(*r) || store.get(*r, relationship::IDENTIFYING)?.as_bool() != Some(true) {
            continue;
        }
        let fks: Vec<Obj> = store.referrers(*r).into_iter().filter(|(_, prop)| *prop == foreign_key::RELATIONSHIP).map(|(fk, _)| fk).collect();
        let holds = !fks.is_empty()
            && fks.iter().all(|fk| {
                let parts = rules::key_parts(store, *fk);
                let pk = parts.entity.map(|e| rules::primary_key(store, e)).unwrap_or_default();
                parts.columns.iter().all(|a| pk.contains(a))
            });
        if !holds {
            store.set(*r, relationship::IDENTIFYING, Value::Bool(false))?;
        }
    }
    Ok(())
}

/// The relationships an entity's foreign keys realise.
fn relationships_of(store: &Store, e: Obj) -> Vec<Obj> {
    rules::foreign_keys(store, e).into_iter().filter_map(|fk| rules::key_parts(store, fk).relationship).collect()
}

/// Removes an ER element with everything it contains, the keys and foreign keys that depend on any
/// of it, the diagrams of it, the nodes and edges that show any of them, and the trace links on any
/// of them, in one step. A mapping table that loses one of its keys becomes a regular entity, and an
/// identifying relationship that loses its key, or whose key leaves the primary key, a
/// non-identifying one.
pub fn remove_element(store: &mut Store, obj: Obj) -> Result<usize, Invalid> {
    // A removal cascades through the keys built on what goes (a chain of subtype keys, level by
    // level); its nesting has a budget, so a deep chain in a loaded document is refused, never a
    // stack overflow.
    thread_local! { static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    struct Deeper;
    impl Drop for Deeper {
        fn drop(&mut self) {
            DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }
    if DEPTH.with(|d| d.get()) >= REMOVAL_DEPTH {
        return Err(Invalid::Refused(format!("the removal reaches deeper than {REMOVAL_DEPTH} levels of keys built on each other")));
    }
    DEPTH.with(|d| d.set(d.get() + 1));
    let _deeper = Deeper;
    store.transaction(|st| {
        let mut gone = 0;
        let mut owners: BTreeSet<Obj> = BTreeSet::new();
        let mut relationships: BTreeSet<Obj> = BTreeSet::new();
        let (keys, fks) = dependents(st, obj)?;
        let index_parts = index_dependents(st, obj)?;
        // A subtype key that goes as a dependent takes its relationship along, and its generalization
        // forgets it (slice S6).
        let mut orphaned: BTreeSet<Obj> = BTreeSet::new();
        let mut orphan_columns: Vec<Obj> = Vec::new();
        for x in fks.iter().copied().chain([obj]) {
            let (rel, columns) = match st.class_of(x)? {
                c if c == FOREIGN_KEY => (rules::key_parts(st, x).relationship, rules::key_parts(st, x).columns),
                c if c == RELATIONSHIP => (Some(x), Vec::new()),
                _ => (None, Vec::new()),
            };
            if let Some(g) = rel.and_then(|r| subtype_key_owner(st, r)) {
                orphaned.insert(g);
                orphan_columns.extend(columns);
            }
        }
        // A follower loses its link when its target goes; a column loses the type that goes.
        for o in subtree_of(st, obj)?.0 {
            for c in rules::linked_from(st, o) {
                st.set(c, attribute::LINKED_TO, Value::Null)?;
            }
            for (a, prop) in st.referrers(o) {
                if (prop == attribute::DOMAIN || prop == attribute::ENUM_TYPE || prop == sequence::OWNED_BY) && st.is_alive(a) {
                    // A column of another schema keeps a type of its own (slice S7): the domain's
                    // base type, size and not null, or text for an enum type.
                    if prop == attribute::DOMAIN {
                        st.set(a, attribute::TYPE, text_or_null(&rules::text_of(st, o, domain::BASE_TYPE)))?;
                        set_size(st, a, rules::own_size_of(st, o))?;
                        if st.get(o, domain::NOT_NULL)?.as_bool() == Some(true) {
                            st.set(a, attribute::NULLABLE, Value::Bool(false))?;
                        }
                    } else if prop == attribute::ENUM_TYPE {
                        st.set(a, attribute::TYPE, Value::str("TEXT"))?;
                    }
                    st.set(a, prop, Value::Null)?;
                }
            }
        }
        for x in fks.iter().chain(&keys).copied().chain([obj]) {
            let class = st.class_of(x)?;
            if class == FOREIGN_KEY {
                if let Some(e) = rules::entity_of(st, x) {
                    owners.insert(e);
                }
                if let Some(r) = rules::key_parts(st, x).relationship {
                    relationships.insert(r);
                }
            } else if class == PRIMARY_KEY
                && let Some(e) = rules::entity_of(st, x)
            {
                owners.insert(e);
                relationships.extend(relationships_of(st, e));
            }
        }
        for x in fks.into_iter().chain(keys).chain(index_parts) {
            if st.is_alive(x) {
                gone += archforge_core::destroy_with_links(st, x)?;
            }
        }
        gone += archforge_diagram::remove_views_of(st, obj)?;
        gone += archforge_core::destroy_with_links(st, obj)?;
        for g in orphaned {
            if st.is_alive(g)
                && let (_, _, key_rel) = rules::generalization_parts(st, g)
            {
                st.set(g, generalization::KEY_RELATIONSHIP, Value::Null)?;
                if let Some(r) = key_rel.filter(|r| st.is_alive(*r)) {
                    gone += remove_element(st, r)?;
                }
            }
        }
        // The columns that followed an orphaned subtype key and that nothing else holds go too.
        for c in orphan_columns {
            let held = st.referrers(c).iter().any(|(_, p)| *p == foreign_key::COLUMN);
            if st.is_alive(c) && rules::linked_to(st, c).is_some() && !held && rules::unique_on(st, c).is_empty() {
                gone += remove_follower(st, c)?;
            }
        }
        demote_broken(st, &owners)?;
        unidentify_broken(st, &relationships)?;
        Ok(gone)
    })
}

/// Removes an entity with its attributes, the relationships that reach it, the views of any of
/// them, and the trace links on any of them.
pub fn remove_entity(store: &mut Store, e: Obj) -> Result<usize, Invalid> {
    store.transaction(|st| {
        let mut gone = 0;
        // Its generalizations go first, with the subtype keys they made.
        let links: Vec<Obj> = rules::supertype_links(st, e).into_iter().chain(rules::subtype_links(st, e)).collect();
        for g in links {
            if st.is_alive(g) {
                gone += remove_generalization(st, g)?;
            }
        }
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
    let gone = remove_entity(ctx.doc, e).map_err(refused_as("entity"))?;
    Ok(json!({ "removed": gone }))
}

// Attributes.

/// Whether a column's value is generated as an identity, which keeps it not null.
fn is_identity(g: Generation) -> bool {
    matches!(g, Generation::IdentityAlways | Generation::IdentityByDefault)
}

/// The nullable parameter: `auto` follows the column (a key or identity column is not nullable).
fn nullable(p: &Params, held_not_null: bool) -> Result<bool, CommandError> {
    match text(p, "nullable") {
        "auto" | "" => Ok(!held_not_null),
        "yes" => {
            if held_not_null {
                return Err(refuse("nullable", "a key or identity column cannot be nullable"));
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
    check_type(kind)?;
    let typed = column_type(ctx, p, e, kind)?;
    let size = size_params(p, kind)?;
    size_allowed(typed, size)?;
    let default = text(p, "default");
    check_text("default", rules::text_problem(default, MAX_TEXT))?;
    if let Typed::Enum(en) = typed
        && let Some(problem) = rules::enum_default_check(ctx.doc, en, default)
    {
        return Err(refuse("default", problem));
    }
    let (generation, expression) = generation_params(p)?;
    let collation = text(p, "collation");
    if !collation.is_empty() {
        check_name("collation", collation, MAX_NAME)?;
    }
    if matches!(typed, Typed::Domain(_)) {
        if !default.is_empty() {
            return Err(refuse("default", "a column on a domain takes the domain's default"));
        }
        if !collation.is_empty() {
            return Err(refuse("collation", "a column on a domain takes the domain's collation"));
        }
    }
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key || is_identity(generation))?;
    let len = ctx.doc.list(e, entity::ATTRIBUTE).map_err(fail)?.len();
    let at = match p.integer("at") {
        Some(i) if i < 0 => len,
        Some(i) => usize::try_from(i).ok().filter(|i| *i <= len).ok_or_else(|| refuse("at", format!("the entity has {len} attributes")))?,
        None => len,
    };
    let at = u32::try_from(at).map_err(|_| refuse("at", "too large"))?;
    let unique = p.bool("unique").unwrap_or(false);
    let (keys, entities) = keys_near(ctx.doc, e).map_err(fail)?;
    let a = ctx
        .doc
        .transaction(|st| {
            let a = st.create(ATTRIBUTE)?;
            st.set(a, named_element::NAME, Value::str(name))?;
            set_column_type(st, a, kind, typed, size)?;
            st.set(a, attribute::NULLABLE, Value::Bool(null))?;
            st.set(a, attribute::DEFAULT, text_or_null(default))?;
            st.set(a, attribute::GENERATION, Value::Enum(generation.index()))?;
            st.set(a, attribute::EXPRESSION, text_or_null(expression))?;
            st.set(a, attribute::COLLATION, text_or_null(collation))?;
            if let Some(problem) = rules::generation_problem(st, a).or_else(|| rules::enum_default_problem(st, a)) {
                return Err(Invalid::Refused(problem));
            }
            st.insert(e, entity::ATTRIBUTE, at, Value::Ref(a))?;
            if key {
                // A new key column grows the primary key that foreign keys elsewhere point at.
                join_primary_key(st, e, a)?;
            }
            if unique {
                new_unique(st, e, &[a], "", true)?;
            }
            keys_still_hold(st, &keys, &entities)?;
            subtypes_hold(st, e)?;
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
    let old = name_of(ctx.doc, a);
    ctx.doc
        .transaction(|st| {
            st.set(a, named_element::NAME, Value::str(name))?;
            if old != name {
                clear_link(st, a)?;
                follow_rename(st, a, &old, name)?;
            }
            Ok(())
        })
        .map_err(|e| refuse("name", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

// Linked key columns (slice S4): a column a relationship made follows the name and type of the
// column it points at until the user edits it.

/// Drops the link of `a`, so an edit of its own stays.
fn clear_link(st: &mut Store, a: Obj) -> Result<(), Invalid> {
    if rules::linked_to(st, a).is_some() {
        st.set(a, attribute::LINKED_TO, Value::Null)?;
    }
    Ok(())
}

/// Renames the columns that follow `a` from `old` to `new`: a follower named `<prefix>_<old>` becomes
/// `<prefix>_<new>`, stepping aside with a number when that name is taken; another name stays.
fn follow_rename(st: &mut Store, a: Obj, old: &str, new: &str) -> Result<(), Invalid> {
    let mut seen: BTreeSet<Obj> = BTreeSet::from([a]);
    let mut queue: Vec<(Obj, String, String)> = vec![(a, old.to_string(), new.to_string())];
    while let Some((a, old, new)) = queue.pop() {
        let suffix = format!("_{}", snake(&old));
        for c in rules::linked_from(st, a) {
            if !seen.insert(c) {
                continue;
            }
            let current = name_of(st, c);
            let Some(e) = rules::entity_of(st, c) else { continue };
            let base = if current == snake(&old) {
                // A follower that took the target's own name, as a subtype's key does.
                snake(&new)
            } else {
                let Some(prefix) = current.strip_suffix(&suffix) else { continue };
                format!("{prefix}_{}", snake(&new))
            };
            let name = first_free(&base, |n| rules::attribute_name_free(st, e, n, Some(c))).map_err(|e| Invalid::Refused(e.message))?;
            st.set(c, named_element::NAME, Value::str(&name))?;
            // A follower's own followers (an identifying chain) follow on.
            queue.push((c, current, name));
        }
    }
    Ok(())
}

/// Gives the columns that follow `a`, and theirs, its type and size.
fn follow_type(st: &mut Store, a: Obj) -> Result<(), Invalid> {
    let kind = rules::own_type_of(st, a);
    let size = rules::own_size_of(st, a);
    let (d, en) = (rules::domain_of(st, a), rules::enum_of(st, a));
    let mut seen: BTreeSet<Obj> = BTreeSet::from([a]);
    let mut queue = vec![a];
    while let Some(a) = queue.pop() {
        for c in rules::linked_from(st, a) {
            if seen.insert(c) {
                st.set(c, attribute::TYPE, text_or_null(&kind))?;
                set_size(st, c, size)?;
                st.set(c, attribute::DOMAIN, d.map_or(Value::Null, Value::Ref))?;
                st.set(c, attribute::ENUM_TYPE, en.map_or(Value::Null, Value::Ref))?;
                if d.is_some() {
                    // The domain brings the default and the collation.
                    st.set(c, attribute::DEFAULT, Value::Null)?;
                    st.set(c, attribute::COLLATION, Value::Null)?;
                }
                queue.push(c);
            }
        }
    }
    Ok(())
}

/// Whether a key column that followed its target can go with it: it is linked, in no unique
/// constraint, and in no foreign key but `fk`.
fn removable_follower(st: &Store, c: Obj, fk: Obj) -> bool {
    rules::linked_to(st, c).is_some()
        && rules::unique_on(st, c).is_empty()
        && !st.referrers(c).iter().any(|(src, prop)| *prop == foreign_key::COLUMN && *src != fk)
}

/// Removes a follower column from its table: out of the primary key first, then with everything
/// that depended on it (index columns, views, links), as any removal does.
fn remove_follower(st: &mut Store, c: Obj) -> Result<usize, Invalid> {
    if let Some(e) = rules::entity_of(st, c) {
        leave_primary_key(st, e, c)?;
    }
    if !st.is_alive(c) {
        return Ok(0);
    }
    remove_element(st, c)
}

/// What a foreign key's new columns take from its relationship: in the child's primary key when it
/// identifies, nullable when the end it points at is optional, named after the role.
fn follower_shape(st: &Store, fk: Obj) -> (bool, bool, String) {
    let parts = rules::key_parts(st, fk);
    match parts.relationship {
        Some(r) => {
            let identifying = st.get(r, relationship::IDENTIFYING).ok().and_then(|v| v.as_bool()).unwrap_or(false);
            let (end, role) = parent_end(st, r, parts.key.and_then(|k| rules::entity_of(st, k)));
            (identifying, rules::cardinality_of(st, r, end) == Some(Cardinality::ZeroOrOne), rules::role_of(st, r, role))
        }
        None => (false, true, String::new()),
    }
}

/// After key `k` changed from `old` to `new` columns: every foreign key onto it gets a matching
/// column for each new key column, keeps its column for each kept one in the key's new order, and
/// loses its column for each dropped one, removing a follower that nothing else holds.
fn grow_foreign_keys(st: &mut Store, k: Obj, old: &[Obj], new: &[Obj]) -> Result<(), Invalid> {
    // Growing a key grows the keys built on it; a cycle of identifying keys would never end, so the
    // depth has a budget (the cycle itself is refused where it would be made).
    thread_local! { static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    struct Deeper;
    impl Drop for Deeper {
        fn drop(&mut self) {
            DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }
    if DEPTH.with(|d| d.get()) >= rules::MAX_KEY_COLUMNS as u32 {
        return Err(Invalid::Refused(format!("the keys built on this key run deeper than {} levels or form a cycle", rules::MAX_KEY_COLUMNS)));
    }
    DEPTH.with(|d| d.set(d.get() + 1));
    let _deeper = Deeper;
    for fk in rules::keys_onto(st, k) {
        let parts = rules::key_parts(st, fk);
        let Some(child) = parts.entity else { continue };
        let (in_key, optional, role) = follower_shape(st, fk);
        let subtype_key = parts.relationship.is_some_and(|r| subtype_key_owner(st, r).is_some());
        let parent = rules::entity_of(st, k).map(|e| name_of(st, e)).unwrap_or_default();
        let prefix = if subtype_key {
            String::new()
        } else if role.is_empty() {
            snake(&parent)
        } else {
            snake(&role)
        };
        let mut columns = Vec::with_capacity(new.len());
        for t in new {
            match old.iter().position(|o| o == t).and_then(|i| parts.columns.get(i)) {
                Some(c) => columns.push(*c),
                None => columns.push(new_key_column(st, child, *t, &prefix, in_key, optional)?),
            }
        }
        for i in (0..parts.columns.len()).rev() {
            st.remove_at(fk, foreign_key::COLUMN, u32::try_from(i).map_err(|_| Invalid::Refused("too many columns".into()))?)?;
        }
        for c in &columns {
            st.push(fk, foreign_key::COLUMN, Value::Ref(*c))?;
        }
        for (i, t) in old.iter().enumerate() {
            if !new.contains(t)
                && let Some(c) = parts.columns.get(i).copied()
                && removable_follower(st, c, fk)
            {
                remove_follower(st, c)?;
            }
        }
    }
    Ok(())
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

/// After an edit inside a transaction: the keys, foreign keys and mapping tables around it still keep
/// the rules, or the edit is refused.
fn keys_still_hold(store: &Store, fks: &[Obj], entities: &BTreeSet<Obj>) -> Result<(), Invalid> {
    for e in entities {
        for k in rules::keys_of(store, *e) {
            if let Some(problem) = rules::key_columns_problem(store, *e, &rules::key_columns(store, k), store.class_of(k)? == PRIMARY_KEY) {
                return Err(Invalid::Refused(format!("the key {} would break: {problem}", rules::label(store, k))));
            }
        }
    }
    for fk in fks {
        let parts = rules::key_parts(store, *fk);
        if let (Some(e), Some(k)) = (parts.entity, parts.key)
            && let Some((_, problem)) = rules::key_problem(store, e, k, &parts.columns, [parts.on_delete, parts.on_update], parts.relationship)
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

/// Refuses under `param` when foreign keys point at key `k`: their columns follow the key's, so the
/// key's columns stay as they are while one does.
fn key_is_free(store: &Store, k: Obj, param: &str) -> Result<(), CommandError> {
    match rules::keys_onto(store, k).first() {
        Some(fk) => Err(refuse(
            param,
            format!(
                "the foreign key {} points at the key of {}; change or remove it first",
                rules::label(store, *fk),
                rules::entity_of(store, k).map(|e| rules::label(store, e)).unwrap_or_default()
            ),
        )),
        None => Ok(()),
    }
}

/// `e`'s primary key, made when it has none.
fn ensure_primary_key(store: &mut Store, e: Obj) -> Result<Obj, Invalid> {
    if let Some(pk) = rules::primary_key_of(store, e) {
        return Ok(pk);
    }
    let pk = store.create(PRIMARY_KEY)?;
    store.set(e, entity::PRIMARY_KEY, Value::Ref(pk))?;
    Ok(pk)
}

/// Adds `a` to the end of its entity's primary key and makes it not nullable.
/// Puts `a` into its entity's primary key, made when there is none; the foreign keys onto the key
/// grow with it.
fn join_primary_key(store: &mut Store, e: Obj, a: Obj) -> Result<(), Invalid> {
    let pk = ensure_primary_key(store, e)?;
    let old = rules::key_columns(store, pk);
    if !old.contains(&a) {
        store.push(pk, key::COLUMN, Value::Ref(a))?;
        let new = rules::key_columns(store, pk);
        grow_foreign_keys(store, pk, &old, &new)?;
    }
    store.set(a, attribute::NULLABLE, Value::Bool(false))?;
    // A supertype that has a key now gives it to its subtypes under table per type.
    ensure_subtype_keys(store, e)
}

/// Takes `a` out of its entity's primary key; a key left without columns goes.
fn leave_primary_key(store: &mut Store, e: Obj, a: Obj) -> Result<(), Invalid> {
    let Some(pk) = rules::primary_key_of(store, e) else { return Ok(()) };
    let old = rules::key_columns(store, pk);
    store.remove_value(pk, key::COLUMN, &Value::Ref(a))?;
    let new = rules::key_columns(store, pk);
    if new.is_empty() {
        // An emptied key goes whole, with the foreign keys onto it (a subtype's below, too).
        remove_element(store, pk)?;
    } else {
        grow_foreign_keys(store, pk, &old, &new)?;
    }
    Ok(())
}

/// Replaces a key's columns.
fn set_key_columns(store: &mut Store, k: Obj, columns: &[Obj]) -> Result<(), Invalid> {
    for i in (0..rules::key_columns(store, k).len()).rev() {
        store.remove_at(k, key::COLUMN, u32::try_from(i).map_err(|_| Invalid::Refused("too many columns".into()))?)?;
    }
    for a in columns {
        store.push(k, key::COLUMN, Value::Ref(*a))?;
    }
    Ok(())
}

/// Creates a unique constraint over `columns` without checking the rules (the callers do).
fn new_unique(store: &mut Store, e: Obj, columns: &[Obj], name: &str, nulls_distinct: bool) -> Result<Obj, Invalid> {
    let u = store.create(UNIQUE_CONSTRAINT)?;
    if !name.is_empty() {
        store.set(u, key::NAME, Value::str(name))?;
    }
    store.set(u, unique_constraint::NULLS_DISTINCT, Value::Bool(nulls_distinct))?;
    for a in columns {
        store.push(u, key::COLUMN, Value::Ref(*a))?;
    }
    store.push(e, entity::UNIQUE_CONSTRAINT, Value::Ref(u))?;
    Ok(u)
}

fn attribute_set_type(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let kind = text(p, "type");
    check_type(kind)?;
    let e = rules::entity_of(ctx.doc, a).ok_or_else(|| refuse("attribute", "the attribute belongs to no entity"))?;
    let typed = column_type(ctx, p, e, kind)?;
    let size = size_params(p, kind)?;
    size_allowed(typed, size)?;
    let (keys, entities) = keys_around(ctx.doc, a).map_err(fail)?;
    let old = (rules::type_of(ctx.doc, a), rules::size_of(ctx.doc, a), rules::domain_of(ctx.doc, a), rules::enum_of(ctx.doc, a));
    ctx.doc
        .transaction(|st| {
            set_column_type(st, a, kind, typed, size)?;
            if (rules::type_of(st, a), rules::size_of(st, a), rules::domain_of(st, a), rules::enum_of(st, a)) != old {
                clear_link(st, a)?;
                follow_type(st, a)?;
            }
            if let Some(problem) = rules::generation_problem(st, a).or_else(|| rules::enum_default_problem(st, a)) {
                return Err(Invalid::Refused(problem));
            }
            keys_still_hold(st, &keys, &entities)
        })
        .map_err(|e| refuse("type", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// The size parameters of a type command, checked against the type: 0 means none.
fn size_params(p: &Params, kind: &str) -> Result<(Option<i64>, Option<i64>, Option<i64>), CommandError> {
    let read = |key: &str| p.integer(key).filter(|v| *v != 0);
    let size = (read("length"), read("precision"), read("scale"));
    if let Some((key, problem)) = rules::size_problem(kind, size.0, size.1, size.2) {
        return Err(refuse(key, problem));
    }
    Ok(size)
}

fn set_size(st: &mut Store, a: Obj, size: (Option<i64>, Option<i64>, Option<i64>)) -> Result<(), Invalid> {
    // The generated integers take no null: 0 is none.
    for (prop, value) in [(attribute::LENGTH, size.0), (attribute::PRECISION, size.1), (attribute::SCALE, size.2)] {
        st.set(a, prop, Value::Int(value.unwrap_or(0)))?;
    }
    Ok(())
}

// Domains and enum types (slice S5).

/// What a column is typed by besides a type name: a domain or an enum type of its schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typed {
    Name,
    Domain(Obj),
    Enum(Obj),
}

/// The `domain` or `enum` parameter beside `type`: exactly one of the three, of any schema of the
/// model (slice S7: a key column in one schema follows its target's type from another).
fn column_type(ctx: &C<'_>, p: &Params, _e: Obj, kind: &str) -> Result<Typed, CommandError> {
    let given = |key: &str, class: ClassId| -> Result<Option<Obj>, CommandError> {
        if text(p, key).is_empty() {
            return Ok(None);
        }
        let t = id_of(ctx, p, key, class)?;
        if rules::schema_of(ctx.doc, t).is_none() {
            return Err(refuse(key, format!("{} belongs to no schema", name_of(ctx.doc, t))));
        }
        Ok(Some(t))
    };
    let (d, en) = (given("domain", DOMAIN)?, given("enum", ENUM_TYPE)?);
    match (kind.is_empty(), d, en) {
        (_, Some(_), Some(_)) => Err(refuse("enum", "a column takes a domain or an enum type, not both")),
        (false, Some(_), _) => Err(refuse("type", "a column on a domain takes no type name of its own")),
        (false, _, Some(_)) => Err(refuse("type", "a column of an enum type takes no type name of its own")),
        (_, Some(d), None) => Ok(Typed::Domain(d)),
        (_, None, Some(en)) => Ok(Typed::Enum(en)),
        (_, None, None) => Ok(Typed::Name),
    }
}

/// A size given beside a domain or an enum type is refused up front, under its own key.
fn size_allowed(typed: Typed, size: (Option<i64>, Option<i64>, Option<i64>)) -> Result<(), CommandError> {
    if typed == Typed::Name || size == (None, None, None) {
        return Ok(());
    }
    let key = if size.0.is_some() {
        "length"
    } else if size.1.is_some() {
        "precision"
    } else {
        "scale"
    };
    Err(refuse(
        key,
        match typed {
            Typed::Domain(_) => "a column on a domain takes the domain's size",
            _ => "a column of an enum type takes no size",
        },
    ))
}

/// Writes a column's type: its own name and size, or the domain or enum type it takes instead.
fn set_column_type(st: &mut Store, a: Obj, kind: &str, typed: Typed, size: (Option<i64>, Option<i64>, Option<i64>)) -> Result<(), Invalid> {
    match typed {
        Typed::Name => {
            st.set(a, attribute::TYPE, text_or_null(kind))?;
            set_size(st, a, size)?;
            st.set(a, attribute::DOMAIN, Value::Null)?;
            st.set(a, attribute::ENUM_TYPE, Value::Null)
        }
        Typed::Domain(d) => {
            if size != (None, None, None) {
                return Err(Invalid::Refused("a column on a domain takes the domain's size".into()));
            }
            st.set(a, attribute::TYPE, Value::Null)?;
            set_size(st, a, (None, None, None))?;
            // The domain's default and collation count: the column's own go.
            st.set(a, attribute::DEFAULT, Value::Null)?;
            st.set(a, attribute::COLLATION, Value::Null)?;
            st.set(a, attribute::DOMAIN, Value::Ref(d))?;
            st.set(a, attribute::ENUM_TYPE, Value::Null)
        }
        Typed::Enum(e) => {
            if size != (None, None, None) {
                return Err(Invalid::Refused("a column of an enum type takes no size".into()));
            }
            st.set(a, attribute::TYPE, Value::Null)?;
            set_size(st, a, (None, None, None))?;
            st.set(a, attribute::DOMAIN, Value::Null)?;
            st.set(a, attribute::ENUM_TYPE, Value::Ref(e))
        }
    }
}

/// The name of a type the schema names once: free among its entities, domains and enum types.
fn type_name(ctx: &C<'_>, p: &Params, s: Obj, except: Option<Obj>) -> Result<String, CommandError> {
    let name = text(p, "name");
    check_name("name", name, MAX_NAME)?;
    if !rules::entity_name_free(ctx.doc, s, name, except) {
        return Err(refuse("name", format!("the schema has a table, type, sequence or view named {name:?} already")));
    }
    Ok(name.to_string())
}

/// A domain's fields from the parameters: base type, size, default, not null and checks.
struct DomainFields {
    base: String,
    size: (Option<i64>, Option<i64>, Option<i64>),
    default: String,
    not_null: bool,
    checks: Vec<(String, String)>,
}

fn domain_fields(p: &Params) -> Result<DomainFields, CommandError> {
    let base = text(p, "base_type");
    check_name("base_type", base, MAX_TYPE)?;
    if base.contains('(') {
        return Err(refuse("base_type", "carries its own size; give length, precision or scale instead"));
    }
    let size = size_params(p, base)?;
    let default = text(p, "default");
    check_text("default", rules::text_problem(default, MAX_TEXT))?;
    let not_null = p.bool("not_null").unwrap_or(false);
    let mut checks = Vec::new();
    match p.get("checks") {
        None | Some(Json::Null) => {}
        Some(Json::Array(items)) => {
            if items.len() > rules::MAX_KEY_COLUMNS {
                return Err(refuse("checks", format!("at most {} checks", rules::MAX_KEY_COLUMNS)));
            }
            for item in items {
                let (expression, name) = match item {
                    Json::String(s) => (s.clone(), String::new()),
                    Json::Object(fields) => {
                        if let Some(unknown) = fields.keys().find(|k| k.as_str() != "expression" && k.as_str() != "name") {
                            let unknown: String = unknown.chars().take(40).collect();
                            return Err(refuse("checks", format!("a check takes no {unknown:?}; it takes expression, name")));
                        }
                        let expression = match item.get("expression") {
                            Some(Json::String(s)) => s.clone(),
                            None | Some(Json::Null) => String::new(),
                            Some(other) => return Err(refuse("checks", format!("expression is {}, not text", shown(other)))),
                        };
                        let name = match item.get("name") {
                            Some(Json::String(s)) => s.clone(),
                            None | Some(Json::Null) => String::new(),
                            Some(other) => return Err(refuse("checks", format!("name is {}, not text", shown(other)))),
                        };
                        (expression, name)
                    }
                    other => return Err(refuse("checks", format!("{} is not a check", shown(other)))),
                };
                if let Some(problem) = rules::check_problem(&expression) {
                    return Err(refuse("checks", problem));
                }
                if !name.is_empty() {
                    check_name("checks", &name, MAX_NAME)?;
                }
                checks.push((expression, name));
            }
        }
        Some(_) => return Err(refuse("checks", "an array of checks")),
    }
    Ok(DomainFields { base: base.to_string(), size, default: default.to_string(), not_null, checks })
}

/// Writes a domain's fields and checks; the old checks go.
fn set_domain(st: &mut Store, d: Obj, name: &str, f: &DomainFields) -> Result<(), Invalid> {
    st.set(d, named_element::NAME, Value::str(name))?;
    st.set(d, domain::BASE_TYPE, Value::str(&f.base))?;
    for (prop, value) in [(domain::LENGTH, f.size.0), (domain::PRECISION, f.size.1), (domain::SCALE, f.size.2)] {
        st.set(d, prop, Value::Int(value.unwrap_or(0)))?;
    }
    st.set(d, domain::DEFAULT, text_or_null(&f.default))?;
    st.set(d, domain::NOT_NULL, Value::Bool(f.not_null))?;
    for old in rules::domain_checks(st, d) {
        archforge_core::destroy_with_links(st, old)?;
    }
    for (expression, check_name) in &f.checks {
        let c = st.create(DOMAIN_CHECK)?;
        st.set(c, domain_check::EXPRESSION, Value::str(expression))?;
        st.set(c, domain_check::NAME, text_or_null(check_name))?;
        st.push(d, domain::CHECK, Value::Ref(c))?;
    }
    Ok(())
}

/// After a domain changed: the columns on it keep every rule (keys pair types, identity needs an
/// integer).
fn columns_still_hold(st: &Store, t: Obj) -> Result<(), Invalid> {
    let columns = rules::type_columns(st, t);
    let mut fks: BTreeSet<Obj> = BTreeSet::new();
    let mut entities: BTreeSet<Obj> = BTreeSet::new();
    for a in &columns {
        if let Some(problem) = rules::generation_problem(st, *a).or_else(|| rules::enum_default_problem(st, *a)) {
            return Err(Invalid::Refused(format!("{}: {problem}", rules::label(st, *a))));
        }
        let (k, e) = keys_around(st, *a)?;
        fks.extend(k);
        entities.extend(e);
    }
    let fks: Vec<Obj> = fks.into_iter().collect();
    keys_still_hold(st, &fks, &entities)
}

fn domain_add(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let name = type_name(ctx, p, s, None)?;
    let f = domain_fields(p)?;
    let d = ctx
        .doc
        .transaction(|st| {
            let d = st.create(DOMAIN)?;
            set_domain(st, d, &name, &f)?;
            st.push(s, schema::DOMAIN, Value::Ref(d))?;
            Ok(d)
        })
        .map_err(fail)?;
    select(ctx, vec![d]);
    Ok(json!({ "id": uuid_of(ctx.doc, d)? }))
}

fn domain_set(ctx: &mut C<'_>, p: &Params) -> R {
    let d = id_of(ctx, p, "domain", DOMAIN)?;
    let s = rules::schema_of(ctx.doc, d).ok_or_else(|| refuse("domain", "the domain belongs to no schema"))?;
    let name = type_name(ctx, p, s, Some(d))?;
    let f = domain_fields(p)?;
    ctx.doc
        .transaction(|st| {
            set_domain(st, d, &name, &f)?;
            columns_still_hold(st, d)
        })
        .map_err(|e| match e {
            Invalid::Refused(why) => refuse("base_type", why),
            other => fail(other),
        })?;
    Ok(json!({ "id": uuid_of(ctx.doc, d)? }))
}

/// Refuses to remove a type that columns still take.
fn type_unused(store: &Store, t: Obj, key: &str) -> Result<(), CommandError> {
    let columns = rules::type_columns(store, t);
    if columns.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = columns
        .iter()
        .take(5)
        .map(|a| match rules::entity_of(store, *a) {
            Some(e) => format!("{}.{}", name_of(store, e), name_of(store, *a)),
            None => rules::label(store, *a),
        })
        .collect();
    Err(refuse(key, format!("{} columns take {}: {}; retype them first", columns.len(), name_of(store, t), names.join(", "))))
}

fn domain_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let d = id_of(ctx, p, "domain", DOMAIN)?;
    type_unused(ctx.doc, d, "domain")?;
    let gone = remove_element(ctx.doc, d).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

/// The `literals` array parameter: names, none twice.
fn literals_param(p: &Params) -> Result<Vec<String>, CommandError> {
    let items = match p.get("literals") {
        Some(Json::Array(items)) => items,
        _ => return Err(refuse("literals", "an array of literals")),
    };
    if items.len() > rules::MAX_LITERALS {
        return Err(refuse("literals", format!("at most {} literals", rules::MAX_LITERALS)));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Json::String(s) => out.push(s.clone()),
            other => return Err(refuse("literals", format!("{} is not a literal", shown(other)))),
        }
    }
    if let Some(problem) = rules::literals_problem(&out) {
        return Err(refuse("literals", problem));
    }
    Ok(out)
}

fn set_literals(st: &mut Store, e: Obj, literals: &[String]) -> Result<(), Invalid> {
    for i in (0..rules::literals_of(st, e).len()).rev() {
        st.remove_at(e, enum_type::LITERAL, u32::try_from(i).map_err(|_| Invalid::Refused("too many literals".into()))?)?;
    }
    for l in literals {
        st.push(e, enum_type::LITERAL, Value::str(l))?;
    }
    Ok(())
}

fn enum_add(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let name = type_name(ctx, p, s, None)?;
    let literals = literals_param(p)?;
    let e = ctx
        .doc
        .transaction(|st| {
            let e = st.create(ENUM_TYPE)?;
            st.set(e, named_element::NAME, Value::str(&name))?;
            set_literals(st, e, &literals)?;
            st.push(s, schema::ENUM_TYPE, Value::Ref(e))?;
            Ok(e)
        })
        .map_err(fail)?;
    select(ctx, vec![e]);
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
}

fn enum_set(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "enum", ENUM_TYPE)?;
    let s = rules::schema_of(ctx.doc, e).ok_or_else(|| refuse("enum", "the enum type belongs to no schema"))?;
    let name = type_name(ctx, p, s, Some(e))?;
    let literals = literals_param(p)?;
    ctx.doc
        .transaction(|st| {
            st.set(e, named_element::NAME, Value::str(&name))?;
            set_literals(st, e, &literals)?;
            columns_still_hold(st, e)
        })
        .map_err(|e| match e {
            Invalid::Refused(why) => refuse("literals", why),
            other => fail(other),
        })?;
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
}

fn enum_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "enum", ENUM_TYPE)?;
    type_unused(ctx.doc, e, "enum")?;
    let gone = remove_element(ctx.doc, e).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Sequences and views (slice S7).

/// A sequence's numbers, owner and flag from the parameters; the bounds kept as their numbers.
/// `current` is the owner the sequence has: kept as it is, it needs no whole-number type again.
fn sequence_params(ctx: &C<'_>, p: &Params, s: Obj, current: Option<Obj>) -> Result<(rules::SequenceNumbers, Option<Obj>), CommandError> {
    let mut n = rules::SequenceNumbers {
        start: p.integer("start").unwrap_or(1),
        increment: p.integer("increment").unwrap_or(1),
        minimum: text(p, "minimum").to_string(),
        maximum: text(p, "maximum").to_string(),
        cycle: p.bool("cycle").unwrap_or(false),
    };
    if let Some((key, why)) = rules::sequence_problem(&n) {
        return Err(refuse(key, why));
    }
    // A bound is kept as its number: "+5" and "05" become "5".
    for bound in [&mut n.minimum, &mut n.maximum] {
        if let Ok(Some(v)) = rules::bound_of(bound) {
            *bound = v.to_string();
        }
    }
    let owned_by = if text(p, "owned_by").is_empty() {
        None
    } else {
        let a = id_of(ctx, p, "owned_by", ATTRIBUTE)?;
        let new_owner = Some(a) != current;
        if let Some(why) = rules::owned_by_problem(ctx.doc, s, a).or_else(|| if new_owner { rules::owner_type_problem(ctx.doc, a) } else { None }) {
            return Err(refuse("owned_by", why));
        }
        Some(a)
    };
    Ok((n, owned_by))
}

fn set_sequence(st: &mut Store, q: Obj, name: &str, n: &rules::SequenceNumbers, owned_by: Option<Obj>) -> Result<(), Invalid> {
    st.set(q, named_element::NAME, Value::str(name))?;
    st.set(q, sequence::START, Value::Int(n.start))?;
    st.set(q, sequence::INCREMENT, Value::Int(n.increment))?;
    st.set(q, sequence::MINIMUM, text_or_null(&n.minimum))?;
    st.set(q, sequence::MAXIMUM, text_or_null(&n.maximum))?;
    st.set(q, sequence::CYCLE, Value::Bool(n.cycle))?;
    st.set(q, sequence::OWNED_BY, owned_by.map_or(Value::Null, Value::Ref))
}

fn sequence_add(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let name = type_name(ctx, p, s, None)?;
    let (n, owned_by) = sequence_params(ctx, p, s, None)?;
    let q = ctx
        .doc
        .transaction(|st| {
            let q = st.create(SEQUENCE)?;
            set_sequence(st, q, &name, &n, owned_by)?;
            st.push(s, schema::SEQUENCE, Value::Ref(q))?;
            Ok(q)
        })
        .map_err(fail)?;
    select(ctx, vec![q]);
    Ok(json!({ "id": uuid_of(ctx.doc, q)? }))
}

fn sequence_set(ctx: &mut C<'_>, p: &Params) -> R {
    let q = id_of(ctx, p, "sequence", SEQUENCE)?;
    let s = rules::schema_of(ctx.doc, q).ok_or_else(|| refuse("sequence", "the sequence belongs to no schema"))?;
    let name = type_name(ctx, p, s, Some(q))?;
    let (n, owned_by) = sequence_params(ctx, p, s, rules::owner_of_sequence(ctx.doc, q))?;
    ctx.doc.transaction(|st| set_sequence(st, q, &name, &n, owned_by)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, q)? }))
}

fn sequence_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let q = id_of(ctx, p, "sequence", SEQUENCE)?;
    let gone = remove_element(ctx.doc, q).map_err(refused_as("sequence"))?;
    Ok(json!({ "removed": gone }))
}

/// A view's query from the parameters: SQL text, not parsed.
fn query_param(p: &Params) -> Result<&str, CommandError> {
    let query = text(p, "query");
    match rules::query_problem(query) {
        Some(why) => Err(refuse("query", why)),
        None => Ok(query),
    }
}

fn set_view(st: &mut Store, v: Obj, name: &str, query: &str, materialized: bool) -> Result<(), Invalid> {
    st.set(v, named_element::NAME, Value::str(name))?;
    st.set(v, view::QUERY, Value::str(query))?;
    st.set(v, view::MATERIALIZED, Value::Bool(materialized))
}

fn view_add(ctx: &mut C<'_>, p: &Params) -> R {
    let s = id_of(ctx, p, "schema", SCHEMA)?;
    let name = type_name(ctx, p, s, None)?;
    let query = query_param(p)?;
    let materialized = p.bool("materialized").unwrap_or(false);
    let v = ctx
        .doc
        .transaction(|st| {
            let v = st.create(VIEW)?;
            set_view(st, v, &name, query, materialized)?;
            st.push(s, schema::VIEW, Value::Ref(v))?;
            Ok(v)
        })
        .map_err(refused_as("query"))?;
    select(ctx, vec![v]);
    Ok(json!({ "id": uuid_of(ctx.doc, v)? }))
}

fn view_set(ctx: &mut C<'_>, p: &Params) -> R {
    let v = id_of(ctx, p, "view", VIEW)?;
    let s = rules::schema_of(ctx.doc, v).ok_or_else(|| refuse("view", "the view belongs to no schema"))?;
    let name = type_name(ctx, p, s, Some(v))?;
    let query = query_param(p)?;
    let materialized = p.bool("materialized").unwrap_or(false);
    ctx.doc.transaction(|st| set_view(st, v, &name, query, materialized)).map_err(refused_as("query"))?;
    Ok(json!({ "id": uuid_of(ctx.doc, v)? }))
}

fn view_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let v = id_of(ctx, p, "view", VIEW)?;
    let gone = remove_element(ctx.doc, v).map_err(refused_as("view"))?;
    Ok(json!({ "removed": gone }))
}

/// Sets a column's default: an SQL expression, or nothing.
fn attribute_set_default(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let default = text(p, "default");
    check_text("default", rules::text_problem(default, MAX_TEXT))?;
    if rules::domain_of(ctx.doc, a).is_some() && !default.is_empty() {
        return Err(refuse("default", "the column takes its domain's default"));
    }
    let old = rules::text_of(ctx.doc, a, attribute::DEFAULT);
    ctx.doc
        .transaction(|st| {
            st.set(a, attribute::DEFAULT, text_or_null(default))?;
            if old != default {
                clear_link(st, a)?;
            }
            match rules::generation_problem(st, a).or_else(|| rules::enum_default_problem(st, a)) {
                Some(problem) => Err(Invalid::Refused(problem)),
                None => Ok(()),
            }
        })
        .map_err(|e| refuse("default", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// The generation words, in the enumeration's order.
pub const GENERATION_WORDS: [(Generation, &str); 5] = [
    (Generation::NoneKind, "none"),
    (Generation::IdentityAlways, "identity-always"),
    (Generation::IdentityByDefault, "identity-by-default"),
    (Generation::ComputedStored, "computed-stored"),
    (Generation::ComputedVirtual, "computed-virtual"),
];

/// Sets how a column's value comes about, with the expression of a computed column.
fn attribute_set_generation(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let (generation, expression) = generation_params(p)?;
    let old = (rules::generation_of(ctx.doc, a), rules::text_of(ctx.doc, a, attribute::EXPRESSION));
    ctx.doc
        .transaction(|st| {
            st.set(a, attribute::GENERATION, Value::Enum(generation.index()))?;
            st.set(a, attribute::EXPRESSION, text_or_null(expression))?;
            if (generation, expression.to_string()) != old {
                clear_link(st, a)?;
            }
            match rules::generation_problem(st, a) {
                Some(problem) => Err(Invalid::Refused(problem)),
                None => Ok(()),
            }
        })
        .map_err(|e| refuse("generation", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// Sets a column's collation, or clears it.
fn attribute_set_collation(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let collation = text(p, "collation");
    if !collation.is_empty() {
        check_name("collation", collation, MAX_NAME)?;
    }
    if rules::domain_of(ctx.doc, a).is_some() && !collation.is_empty() {
        return Err(refuse("collation", "the column takes its domain's collation"));
    }
    let old = rules::text_of(ctx.doc, a, attribute::COLLATION);
    ctx.doc
        .transaction(|st| {
            st.set(a, attribute::COLLATION, text_or_null(collation))?;
            if old != collation { clear_link(st, a) } else { Ok(()) }
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// Sets the description of an ER element, the text DDL writes as its comment, or nothing
/// (`element.describe`, through [`crate::ErNotation`]).
pub fn describe_element(store: &mut Store, o: Obj, description: &str) -> Result<(), CommandError> {
    let class = store.class_of(o).map_err(fail)?;
    if !store.universe().conforms(class, TypeRef::Class(ELEMENT)) {
        return Err(refuse("element", format!("{} is not an element of the ER model", uuid_of(store, o)?)));
    }
    check_text("text", rules::text_problem(description, MAX_TEXT))?;
    store.set(o, element::DESCRIPTION, if description.is_empty() { Value::Null } else { Value::str(description) }).map_err(fail)
}

fn attribute_set_flags(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    let key = p.bool("primary_key").unwrap_or(false);
    let null = nullable(p, key || is_identity(rules::generation_of(ctx.doc, a)))?;
    let unique = p.bool("unique").unwrap_or(false);
    let e = rules::entity_of(ctx.doc, a).ok_or_else(|| refuse("attribute", "the attribute belongs to no entity"))?;
    let was_key = rules::in_primary_key(ctx.doc, a);
    // Leaving the key is fine while the key keeps a column; the last column cannot leave a key that
    // foreign keys point at, since the key would go with it.
    if was_key
        && !key
        && let Some(pk) = rules::primary_key_of(ctx.doc, e)
        && rules::key_columns(ctx.doc, pk).len() == 1
    {
        key_is_free(ctx.doc, pk, "primary_key")?;
    }
    let was_null = rules::is_nullable(ctx.doc, a);
    let uniques = rules::unique_on(ctx.doc, a);
    if !unique {
        for u in &uniques {
            key_is_free(ctx.doc, *u, "unique")?;
        }
    }
    let (fks, entities) = keys_around(ctx.doc, a).map_err(fail)?;
    let was_unique = !uniques.is_empty();
    ctx.doc
        .transaction(|st| {
            match (key, was_key) {
                (true, false) => join_primary_key(st, e, a)?,
                (false, true) => leave_primary_key(st, e, a)?,
                _ => {}
            }
            st.set(a, attribute::NULLABLE, Value::Bool(null))?;
            if null != was_null {
                follow_nullability(st, a, null)?;
            }
            if key != was_key || null != was_null || unique != was_unique {
                clear_link(st, a)?;
            }
            if unique && uniques.is_empty() {
                new_unique(st, e, &[a], "", true)?;
            }
            if !unique {
                for u in &uniques {
                    archforge_core::destroy_with_links(st, *u)?;
                }
            }
            if let Some(problem) = rules::generation_problem(st, a) {
                return Err(Invalid::Refused(problem));
            }
            keys_still_hold(st, &fks, &entities)?;
            subtypes_hold(st, e)
        })
        .map_err(|e| refuse("attribute", e.to_string()))?;
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
    not_piece(ctx.doc, a, "attribute")?;
    let gone = remove_element(ctx.doc, a).map_err(refused_as("attribute"))?;
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

/// A role name under `key`, or empty for none.
fn role(p: &Params, key: &str) -> Result<String, CommandError> {
    let role = text(p, key);
    if !role.is_empty() {
        if let Some(problem) = rules::name_problem(role, MAX_NAME) {
            return Err(refuse(key, format!("the role {problem}")));
        }
        if snake(role).is_empty() {
            return Err(refuse(key, "the role needs a letter or a digit, since it names key columns"));
        }
    }
    Ok(role.to_string())
}

/// The end of relationship `r` that points at `parent`, as its cardinality and role properties.
/// For a relationship of an entity with itself, as for drawing: the parent is the end opposite the
/// many, or `from` for one to one.
fn parent_end(store: &Store, r: Obj, parent: Option<Obj>) -> (archforge_mof::PropId, archforge_mof::PropId) {
    let end = |p| store.get(r, p).ok().and_then(|v| v.as_ref());
    let (from, to) = (end(relationship::FROM), end(relationship::TO));
    let from_is_parent = if from == to {
        let many = |p| rules::cardinality_of(store, r, p).is_some_and(rules::is_many);
        many(relationship::TO_END) || !many(relationship::FROM_END)
    } else {
        from == parent
    };
    if from_is_parent { (relationship::FROM_END, relationship::FROM_ROLE) } else { (relationship::TO_END, relationship::TO_ROLE) }
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

/// Adds the columns that copy key `k` into `child` and the foreign key over them, linked to
/// `relationship`. The columns are named `<role>_<column>`, or `<parent>_<column>` without a role,
/// typed like their targets, part of the
/// child's primary key when `in_key`, nullable when `optional`; when `unique`, a unique constraint
/// covers them together, unless they are the child's whole primary key, which is unique already.
#[allow(clippy::too_many_arguments)]
fn add_key_columns(
    store: &mut Store,
    child: Obj,
    k: Obj,
    role: &str,
    relationship: Option<Obj>,
    in_key: bool,
    optional: bool,
    unique: bool,
) -> Result<Obj, Invalid> {
    let columns = new_key_columns(store, child, k, role, in_key, optional)?;
    if unique && rules::primary_key(store, child) != columns {
        new_unique(store, child, &columns, "", true)?;
    }
    let actions = [ReferentialAction::NoAction, ReferentialAction::NoAction];
    new_foreign_key(store, child, k, &columns, "", actions, relationship)
}

/// Adds one column to `child` for each column of key `k`, named `<role>_<column>`, or
/// `<parent>_<column>` without a role, and typed like it; in the child's primary key when `in_key`,
/// nullable when `optional` and not in the key.
fn new_key_columns(store: &mut Store, child: Obj, k: Obj, role: &str, in_key: bool, optional: bool) -> Result<Vec<Obj>, Invalid> {
    let targets = rules::key_columns(store, k);
    let parent = rules::entity_of(store, k).ok_or_else(|| Invalid::Refused("the key belongs to no entity".into()))?;
    if targets.is_empty() {
        return Err(Invalid::Refused(format!("{} has no key columns to refer to", name_of(store, parent))));
    }
    let prefix = if role.is_empty() { snake(&name_of(store, parent)) } else { snake(role) };
    let mut columns = Vec::with_capacity(targets.len());
    for t in targets {
        columns.push(new_key_column(store, child, t, &prefix, in_key, optional)?);
    }
    Ok(columns)
}

/// One column of `child` that copies key column `t`: named `<prefix>_<column>`, typed and sized like
/// it, linked to it, in the child's primary key when `in_key`, nullable when `optional` and not in
/// the key.
fn new_key_column(store: &mut Store, child: Obj, t: Obj, prefix: &str, in_key: bool, optional: bool) -> Result<Obj, Invalid> {
    // An empty prefix keeps the key column's own name (a subtype's key is its supertype's).
    let base = if prefix.is_empty() { snake(&name_of(store, t)) } else { format!("{prefix}_{}", snake(&name_of(store, t))) };
    let name = first_free(&base, |n| rules::attribute_name_free(store, child, n, None)).map_err(|e| Invalid::Refused(e.message))?;
    let a = store.create(ATTRIBUTE)?;
    store.set(a, named_element::NAME, Value::str(&name))?;
    let kind = rules::own_type_of(store, t);
    if !kind.is_empty() {
        store.set(a, attribute::TYPE, Value::str(&kind))?;
    }
    set_size(store, a, rules::own_size_of(store, t))?;
    store.set(a, attribute::DOMAIN, rules::domain_of(store, t).map_or(Value::Null, Value::Ref))?;
    store.set(a, attribute::ENUM_TYPE, rules::enum_of(store, t).map_or(Value::Null, Value::Ref))?;
    store.set(a, attribute::NULLABLE, Value::Bool(optional && !in_key))?;
    store.set(a, attribute::LINKED_TO, Value::Ref(t))?;
    store.push(child, entity::ATTRIBUTE, Value::Ref(a))?;
    if in_key {
        join_primary_key(store, child, a)?;
    }
    Ok(a)
}

/// Creates a foreign key without checking the rules (the callers do).
fn new_foreign_key(
    store: &mut Store,
    e: Obj,
    k: Obj,
    columns: &[Obj],
    name: &str,
    actions: [ReferentialAction; 2],
    relationship: Option<Obj>,
) -> Result<Obj, Invalid> {
    let fk = store.create(FOREIGN_KEY)?;
    if !name.is_empty() {
        store.set(fk, foreign_key::NAME, Value::str(name))?;
    }
    store.set(fk, foreign_key::REFERENCES, Value::Ref(k))?;
    store.set(fk, foreign_key::ON_DELETE, Value::Enum(actions[0].index()))?;
    store.set(fk, foreign_key::ON_UPDATE, Value::Enum(actions[1].index()))?;
    if let Some(r) = relationship {
        store.set(fk, foreign_key::RELATIONSHIP, Value::Ref(r))?;
    }
    for a in columns {
        store.push(fk, foreign_key::COLUMN, Value::Ref(*a))?;
    }
    store.push(e, entity::FOREIGN_KEY, Value::Ref(fk))?;
    Ok(fk)
}

/// Creates a relationship without checking the rules (the callers do).
#[allow(clippy::too_many_arguments)]
fn new_relationship(
    store: &mut Store,
    s: Obj,
    from: Obj,
    to: Obj,
    ends: [Cardinality; 2],
    roles: [&str; 2],
    name: &str,
    identifying: bool,
) -> Result<Obj, Invalid> {
    let r = store.create(RELATIONSHIP)?;
    store.set(r, relationship::FROM, Value::Ref(from))?;
    store.set(r, relationship::TO, Value::Ref(to))?;
    store.set(r, relationship::FROM_END, Value::Enum(ends[0].index()))?;
    store.set(r, relationship::TO_END, Value::Enum(ends[1].index()))?;
    store.set(r, relationship::IDENTIFYING, Value::Bool(identifying))?;
    for (prop, role) in [(relationship::FROM_ROLE, roles[0]), (relationship::TO_ROLE, roles[1])] {
        if !role.is_empty() {
            store.set(r, prop, Value::str(role))?;
        }
    }
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
    let from_schema = rules::schema_of(ctx.doc, from).ok_or_else(|| refuse("from", "the entity belongs to no schema"))?;
    let to_schema = rules::schema_of(ctx.doc, to).ok_or_else(|| refuse("to", "the entity belongs to no schema"))?;
    let roles = [role(p, "from_role")?, role(p, "to_role")?];
    let identifying = p.bool("identifying").unwrap_or(false);
    let with_key = p.bool("create_foreign_key").unwrap_or(false);
    if identifying && !with_key {
        return Err(refuse("identifying", "an identifying relationship needs its foreign key; set create_foreign_key"));
    }
    // The child holds the key: the end with many, or `to` for one to one. The role at the parent's
    // end names the key columns.
    let (child, parent, parent_end, child_many, parent_role) = if rules::is_many(to_end) {
        (to, from, from_end, true, roles[0].clone())
    } else if rules::is_many(from_end) {
        (from, to, to_end, true, roles[1].clone())
    } else {
        (to, from, from_end, false, roles[0].clone())
    };
    // The relationship may join two schemas (slice S7); it belongs to its child's, as its foreign
    // key does.
    let s = if child == from { from_schema } else { to_schema };
    if !rules::relationship_name_free(ctx.doc, s, &name, None) {
        return Err(refuse("name", format!("the schema has a relationship named {name:?} already")));
    }
    let points_to = if text(p, "points_to").is_empty() { None } else { Some(key_id(ctx, p, "points_to")?) };
    if points_to.is_some() && !with_key {
        return Err(refuse("points_to", "only a relationship that brings its foreign key points at a key; set create_foreign_key"));
    }
    let mut target = None;
    if with_key {
        if rules::is_many(from_end) && rules::is_many(to_end) {
            return Err(refuse("create_foreign_key", "a many-to-many relationship needs a mapping table; resolve it instead"));
        }
        if child == parent && parent_role.is_empty() {
            return Err(refuse(
                "create_foreign_key",
                "a relationship of an entity with itself needs a role at the end its key points to, such as parent; the role names the key columns",
            ));
        }
        if child == parent && identifying && points_to.is_none() {
            return Err(refuse("identifying", "an entity cannot identify itself through its own primary key"));
        }
        target = match points_to {
            Some(k) if rules::entity_of(ctx.doc, k) != Some(parent) => {
                return Err(refuse("points_to", format!("the key is not one of {}'s", name_of(ctx.doc, parent))));
            }
            Some(k) => Some(k),
            None => Some(
                rules::primary_key_of(ctx.doc, parent)
                    .ok_or_else(|| refuse("create_foreign_key", format!("{} has no primary key to refer to", name_of(ctx.doc, parent))))?,
            ),
        };
        if identifying
            && let Some(k) = target
            && rules::identifying_cycle(ctx.doc, child, k)
        {
            return Err(refuse("identifying", cycle_text(ctx.doc, child, k)));
        }
    }
    let r = ctx
        .doc
        .transaction(|st| {
            let r = new_relationship(st, s, from, to, [from_end, to_end], [&roles[0], &roles[1]], &name, identifying)?;
            if let Some(k) = target {
                add_key_columns(st, child, k, &parent_role, Some(r), identifying, parent_end == Cardinality::ZeroOrOne, !child_many)?;
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
    not_subtype_key(ctx.doc, Some(r), "relationship")?;
    let identifying = p.bool("identifying").unwrap_or(false);
    let keys: Vec<Obj> = ctx.doc.referrers(r).into_iter().filter(|(_, prop)| *prop == foreign_key::RELATIONSHIP).map(|(fk, _)| fk).collect();
    if identifying && keys.is_empty() {
        return Err(refuse("identifying", "the relationship has no foreign key to make part of a primary key"));
    }
    if identifying {
        for fk in &keys {
            let parts = rules::key_parts(ctx.doc, *fk);
            if let (Some(child), Some(k)) = (parts.entity, parts.key)
                && rules::identifying_cycle(ctx.doc, child, k)
            {
                return Err(refuse("identifying", cycle_text(ctx.doc, child, k)));
            }
        }
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
    let pk =
        |e: Obj| rules::primary_key_of(store, e).ok_or_else(|| Invalid::Refused(format!("{} has no primary key to refer to", name_of(store, e))));
    let (from_key, to_key) = (pk(from)?, pk(to)?);
    // Each end's role names its columns in the mapping table and goes to the new relationship.
    let (from_role, to_role) = (rules::role_of(store, r, relationship::FROM_ROLE), rules::role_of(store, r, relationship::TO_ROLE));
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
        let r1 = new_relationship(st, s, from, m, many, [&from_role, ""], "", true)?;
        let r2 = new_relationship(st, s, to, m, many, [&to_role, ""], "", true)?;
        add_key_columns(st, m, from_key, &from_role, Some(r1), true, false, false)?;
        add_key_columns(st, m, to_key, &to_role, Some(r2), true, false, false)?;
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

/// Drops a key column's link to the column it followed.
fn attribute_unlink(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx, p, "attribute", ATTRIBUTE)?;
    if rules::linked_to(ctx.doc, a).is_none() {
        return Err(refuse("attribute", format!("{} follows no column", name_of(ctx.doc, a))));
    }
    ctx.doc.set(a, attribute::LINKED_TO, Value::Null).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// Replaces a pure mapping table by the many-to-many relationship it stood for: one relationship
/// between the two parents with the roles and the description carried over, drawn on every diagram
/// between the two parents where the table was; the table, its two relationships and its views go.
/// One step.
pub fn collapse(store: &mut Store, m: Obj) -> Result<Obj, Invalid> {
    if let Some(problem) = rules::collapse_problem(store, m) {
        return Err(Invalid::Refused(problem));
    }
    let here = rules::schema_of(store, m).ok_or_else(|| Invalid::Refused("the table belongs to no schema".into()))?;
    let fks = rules::foreign_keys(store, m);
    let mut ends: Vec<(Obj, Obj, String)> = Vec::with_capacity(2);
    for fk in &fks {
        let parts = rules::key_parts(store, *fk);
        let (Some(r), Some(parent)) = (parts.relationship, parts.key.and_then(|k| rules::entity_of(store, k))) else {
            return Err(Invalid::Refused("a foreign key lacks its relationship or its key".into()));
        };
        let (_, role) = parent_end(store, r, Some(parent));
        ends.push((r, parent, rules::role_of(store, r, role)));
    }
    let [(r1, from, from_role), (r2, to, to_role)] = <[_; 2]>::try_from(ends).map_err(|_| Invalid::Refused("two foreign keys".into()))?;
    // The relationship stays in the table's schema when an end is there, else goes to its `to` end's.
    let s = if [from, to].iter().any(|e| rules::schema_of(store, *e) == Some(here)) {
        here
    } else {
        rules::schema_of(store, to).ok_or_else(|| Invalid::Refused("the table belongs to no schema".into()))?
    };
    // Where the table is drawn: (diagram, node of `from`, node of `to`) through its two lines.
    let other_end = |edge: Obj, n: Obj| {
        let src = store.get(edge, archforge_diagram::edge::SOURCE).ok()?.as_ref()?;
        let dst = store.get(edge, archforge_diagram::edge::TARGET).ok()?.as_ref()?;
        if src == n {
            Some(dst)
        } else if dst == n {
            Some(src)
        } else {
            None
        }
    };
    let mut shown: Vec<(Obj, Obj, Obj)> = Vec::new();
    for n in archforge_diagram::views_of(store, m).2 {
        let Some(d) = archforge_diagram::diagram_of(store, n) else { continue };
        let edges: Vec<Obj> = store
            .referrers(n)
            .into_iter()
            .filter(|(_, p)| *p == archforge_diagram::edge::SOURCE || *p == archforge_diagram::edge::TARGET)
            .map(|(e, _)| e)
            .collect();
        let of = |r: Obj| {
            edges
                .iter()
                .find(|e| store.get(**e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(r))
                .and_then(|e| other_end(*e, n))
        };
        if let (Some(a), Some(b)) = (of(r1), of(r2)) {
            shown.push((d, a, b));
        }
    }
    let description = rules::text_of(store, m, element::DESCRIPTION);
    store.transaction(|st| {
        let r = new_relationship(st, s, from, to, [Cardinality::ZeroOrMore, Cardinality::ZeroOrMore], [&from_role, &to_role], "", false)?;
        st.set(r, element::DESCRIPTION, text_or_null(&description))?;
        for (d, a, b) in &shown {
            let e = st.create(archforge_diagram::EDGE)?;
            st.set(e, archforge_diagram::edge::ELEMENT, Value::Ref(r))?;
            st.set(e, archforge_diagram::edge::SOURCE, Value::Ref(*a))?;
            st.set(e, archforge_diagram::edge::TARGET, Value::Ref(*b))?;
            st.push(*d, archforge_diagram::diagram::EDGE, Value::Ref(e))?;
        }
        remove_entity(st, m)?;
        Ok(r)
    })
}

fn relationship_collapse(ctx: &mut C<'_>, p: &Params) -> R {
    let m = id_of(ctx, p, "entity", ENTITY)?;
    let r = collapse(ctx.doc, m).map_err(|e| refuse("entity", e.to_string()))?;
    select(ctx, vec![r]);
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

// Subtypes (slice S6).

/// The mapping words, in the enumeration's order.
pub const MAPPING_WORDS: [(SubtypeMapping, &str); 3] = [
    (SubtypeMapping::TablePerType, "table-per-type"),
    (SubtypeMapping::SingleTable, "single-table"),
    (SubtypeMapping::TablePerConcreteType, "table-per-concrete-type"),
];

/// A mapping's name in parameters.
pub fn mapping_name(m: SubtypeMapping) -> &'static str {
    MAPPING_WORDS.iter().find(|(x, _)| *x == m).map_or("table-per-type", |(_, w)| *w)
}

/// Makes the key a subtype takes from its supertype under table per type, when the supertype has a
/// primary key and the generalization has none yet: an identifying one to zero-or-one relationship
/// from the supertype to the subtype with the foreign key and its columns, named like the
/// supertype's key columns.
fn ensure_subtype_key(st: &mut Store, g: Obj) -> Result<(), Invalid> {
    let (Some(sup), Some(sub), key_rel) = rules::generalization_parts(st, g) else { return Ok(()) };
    if key_rel.is_some() || rules::mapping_of(st, sup) != SubtypeMapping::TablePerType {
        return Ok(());
    }
    let Some(pk) = rules::primary_key_of(st, sup) else { return Ok(()) };
    if rules::key_columns(st, pk).is_empty() {
        return Ok(());
    }
    // A chain of subtype keys is made level by level; its depth has a budget, and a circle of
    // generalizations or of identifying keys is refused.
    thread_local! { static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    struct Deeper;
    impl Drop for Deeper {
        fn drop(&mut self) {
            DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }
    if DEPTH.with(|d| d.get()) >= rules::MAX_KEY_COLUMNS as u32 {
        return Err(Invalid::Refused(format!("the subtypes run deeper than {} levels", rules::MAX_KEY_COLUMNS)));
    }
    if rules::is_supertype_or_self(st, sub, sup) {
        return Err(Invalid::Refused(format!("{} and {} are subtypes of each other", name_of(st, sub), name_of(st, sup))));
    }
    if rules::identifying_cycle(st, sub, pk) {
        return Err(Invalid::Refused(cycle_text(st, sub, pk)));
    }
    DEPTH.with(|d| d.set(d.get() + 1));
    let _deeper = Deeper;
    let Some(s) = rules::schema_of(st, sup) else { return Ok(()) };
    let r = new_relationship(st, s, sup, sub, [Cardinality::ExactlyOne, Cardinality::ZeroOrOne], ["", ""], "", true)?;
    // Recorded before the columns, so a key appearing on the way does not make a second one.
    st.set(g, generalization::KEY_RELATIONSHIP, Value::Ref(r))?;
    let mut columns = Vec::new();
    for t in rules::key_columns(st, pk) {
        columns.push(new_key_column(st, sub, t, "", true, false)?);
    }
    let actions = [ReferentialAction::Cascade, ReferentialAction::NoAction];
    new_foreign_key(st, sub, pk, &columns, "", actions, Some(r))?;
    // One to zero-or-one: the inherited columns are unique even when the subtype's key is wider.
    if rules::primary_key(st, sub) != columns {
        new_unique(st, sub, &columns, "", true)?;
    }
    Ok(())
}

/// The keys of every subtype of `e` that has none yet.
fn ensure_subtype_keys(st: &mut Store, e: Obj) -> Result<(), Invalid> {
    for g in rules::subtype_links(st, e) {
        ensure_subtype_key(st, g)?;
    }
    Ok(())
}

/// After a change to `e`'s subtyping or key: the keys around `e` and around each of its subtypes
/// keep the rules, the new subtype keys included.
fn subtypes_hold(st: &Store, e: Obj) -> Result<(), Invalid> {
    let mut around = vec![e];
    around.extend(rules::subtype_links(st, e).into_iter().filter_map(|g| rules::generalization_parts(st, g).1));
    for x in around {
        let (fks, entities) = keys_near(st, x)?;
        keys_still_hold(st, &fks, &entities)?;
    }
    Ok(())
}

/// How many levels the generalizations above `e` and below `sub` would reach together.
fn chain_depth(store: &Store, sup: Obj, sub: Obj) -> usize {
    let mut up = 0;
    let mut seen: BTreeSet<Obj> = BTreeSet::new();
    let mut cur = Some(sup);
    while let Some(c) = cur {
        if !seen.insert(c) {
            break;
        }
        up += 1;
        cur = rules::supertype_of(store, c);
    }
    let mut down = 0;
    let mut level = vec![sub];
    let mut seen: BTreeSet<Obj> = BTreeSet::new();
    while !level.is_empty() && down <= rules::MAX_KEY_COLUMNS {
        down += 1;
        let mut next = Vec::new();
        for x in level {
            if seen.insert(x) {
                next.extend(rules::subtype_links(store, x).into_iter().filter_map(|g| rules::generalization_parts(store, g).1));
            }
        }
        level = next;
    }
    up + down
}

/// Removes the key a generalization made: its foreign key, the columns that followed the
/// supertype's key and nothing else holds, and the relationship.
fn remove_subtype_key(st: &mut Store, g: Obj) -> Result<usize, Invalid> {
    let (_, _, Some(r)) = rules::generalization_parts(st, g) else { return Ok(0) };
    st.set(g, generalization::KEY_RELATIONSHIP, Value::Null)?;
    let mut gone = 0;
    let fks: Vec<Obj> = st.referrers(r).into_iter().filter(|(_, p)| *p == foreign_key::RELATIONSHIP).map(|(fk, _)| fk).collect();
    for fk in fks {
        let parts = rules::key_parts(st, fk);
        let columns = parts.columns;
        // The unique constraint the key made over its columns goes with it, unless a key points at it.
        if let Some(child) = parts.entity {
            for u in rules::unique_constraints(st, child) {
                if rules::key_columns(st, u) == columns && rules::keys_onto(st, u).is_empty() {
                    gone += archforge_core::destroy_with_links(st, u)?;
                }
            }
        }
        let keep: Vec<bool> = columns.iter().map(|c| !removable_follower(st, *c, fk)).collect();
        gone += archforge_core::destroy_with_links(st, fk)?;
        for (c, keep) in columns.into_iter().zip(keep) {
            if !keep && st.is_alive(c) {
                gone += remove_follower(st, c)?;
            }
        }
    }
    if st.is_alive(r) {
        gone += remove_element(st, r)?;
    }
    Ok(gone)
}

/// Removes a generalization with the key it made and its views.
fn remove_generalization(st: &mut Store, g: Obj) -> Result<usize, Invalid> {
    let gone = remove_subtype_key(st, g)?;
    Ok(gone + remove_element(st, g)?)
}

fn generalization_add(ctx: &mut C<'_>, p: &Params) -> R {
    let sup = id_of(ctx, p, "supertype", ENTITY)?;
    let sub = id_of(ctx, p, "subtype", ENTITY)?;
    if let Some((key, problem)) = rules::generalization_problem(ctx.doc, sup, sub, None) {
        return Err(refuse(key, problem));
    }
    let s = rules::schema_of(ctx.doc, sup).ok_or_else(|| refuse("supertype", "the table belongs to no schema"))?;
    // Levels are links: the tables of the chain less one.
    if chain_depth(ctx.doc, sup, sub).saturating_sub(1) >= rules::MAX_KEY_COLUMNS {
        return Err(refuse("subtype", format!("the subtypes would run deeper than {} levels", rules::MAX_KEY_COLUMNS)));
    }
    let g = ctx
        .doc
        .transaction(|st| {
            let g = st.create(GENERALIZATION)?;
            st.set(g, generalization::SUPERTYPE, Value::Ref(sup))?;
            st.set(g, generalization::SUBTYPE, Value::Ref(sub))?;
            st.push(s, schema::GENERALIZATION, Value::Ref(g))?;
            ensure_subtype_key(st, g)?;
            subtypes_hold(st, sup)?;
            Ok(g)
        })
        .map_err(|e| match e {
            Invalid::Refused(why) => refuse("subtype", why),
            other => fail(other),
        })?;
    select(ctx, vec![g]);
    Ok(json!({ "id": uuid_of(ctx.doc, g)? }))
}

fn generalization_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let g = id_of(ctx, p, "generalization", GENERALIZATION)?;
    let gone = ctx.doc.transaction(|st| remove_generalization(st, g)).map_err(refused_as("generalization"))?;
    Ok(json!({ "removed": gone }))
}

/// Sets how a supertype's subtypes hold together: complete, disjoint, the discriminator and the
/// mapping; a mapping change makes or removes every subtype's key in the same step.
fn entity_set_subtyping(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let complete = p.bool("complete").unwrap_or(false);
    let disjoint = p.bool("disjoint").unwrap_or(true);
    let discriminator = if text(p, "discriminator").is_empty() {
        None
    } else {
        let a = id_of(ctx, p, "discriminator", ATTRIBUTE)?;
        if rules::entity_of(ctx.doc, a) != Some(e) {
            return Err(refuse("discriminator", format!("{} is not a column of {}", name_of(ctx.doc, a), name_of(ctx.doc, e))));
        }
        Some(a)
    };
    let word = text(p, "mapping");
    let mapping = MAPPING_WORDS.iter().find(|(_, w)| *w == word).map(|(m, _)| *m).ok_or_else(|| {
        let shown: String = word.chars().take(40).collect();
        refuse("mapping", format!("{shown:?} is not one of {}", MAPPING_WORDS.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(", ")))
    })?;
    let was = rules::mapping_of(ctx.doc, e);
    ctx.doc
        .transaction(|st| {
            st.set(e, entity::SUBTYPES_COMPLETE, Value::Bool(complete))?;
            st.set(e, entity::SUBTYPES_DISJOINT, Value::Bool(disjoint))?;
            st.set(e, entity::DISCRIMINATOR, discriminator.map_or(Value::Null, Value::Ref))?;
            st.set(e, entity::SUBTYPE_MAPPING, Value::Enum(mapping.index()))?;
            if mapping != was {
                for g in rules::subtype_links(st, e) {
                    if mapping == SubtypeMapping::TablePerType {
                        ensure_subtype_key(st, g)?;
                    } else {
                        remove_subtype_key(st, g)?;
                    }
                }
            }
            subtypes_hold(st, e)
        })
        .map_err(|e| match e {
            Invalid::Refused(why) => refuse("mapping", why),
            other => fail(other),
        })?;
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
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

/// The attributes the array parameter `key` names, none twice.
fn attribute_list(ctx: &C<'_>, p: &Params, key: &str) -> Result<Vec<Obj>, CommandError> {
    let n = p.get(key).and_then(Json::as_array).map_or(0, Vec::len);
    if n > rules::MAX_KEY_COLUMNS {
        return Err(refuse(key, format!("at most {} columns", rules::MAX_KEY_COLUMNS)));
    }
    let out = id_list(ctx, p, key)?;
    if let Some(o) = out.iter().find(|o| ctx.doc.class_of(**o).ok() != Some(ATTRIBUTE)) {
        return Err(refuse(key, format!("{} is not an attribute", uuid_of(ctx.doc, *o)?)));
    }
    Ok(out)
}

/// The key the id under `param` names: a primary key or a unique constraint.
fn key_id(ctx: &C<'_>, p: &Params, param: &str) -> Result<Obj, CommandError> {
    let id = text(p, param);
    let obj = Uuid::parse(id).and_then(|u| ctx.doc.by_uuid(u)).ok_or_else(|| refuse(param, format!("there is no element {:?}", short(id))))?;
    if !rules::is_key(ctx.doc, obj) {
        return Err(refuse(param, format!("{id} is neither a primary key nor a unique constraint")));
    }
    Ok(obj)
}

/// A constraint name that is free in `obj`'s schema, or empty.
fn constraint_name(ctx: &C<'_>, p: &Params, obj: Obj, except: Option<Obj>) -> Result<String, CommandError> {
    let name = key_name(p)?.to_string();
    if !name.is_empty()
        && let Some(s) = rules::schema_of(ctx.doc, obj)
        && !rules::constraint_name_free(ctx.doc, s, &name, except)
    {
        return Err(refuse("name", format!("the schema has a constraint named {name:?} already")));
    }
    Ok(name)
}

fn match_type(p: &Params) -> Result<MatchType, CommandError> {
    Ok(match text(p, "match") {
        "simple" | "" => MatchType::Simple,
        "full" => MatchType::Full,
        "partial" => MatchType::Partial,
        other => return Err(refuse("match", format!("{other:?} is not simple, full or partial"))),
    })
}

/// The match type's name in parameters.
pub fn match_name(m: MatchType) -> &'static str {
    match m {
        MatchType::Simple => "simple",
        MatchType::Full => "full",
        MatchType::Partial => "partial",
    }
}

fn deferrability(p: &Params) -> Result<Deferrability, CommandError> {
    Ok(match text(p, "deferrable") {
        "not-deferrable" | "" => Deferrability::NotDeferrable,
        "initially-immediate" => Deferrability::InitiallyImmediate,
        "initially-deferred" => Deferrability::InitiallyDeferred,
        other => return Err(refuse("deferrable", format!("{other:?} is not not-deferrable, initially-immediate or initially-deferred"))),
    })
}

/// The deferrability's name in parameters.
pub fn deferrability_name(d: Deferrability) -> &'static str {
    match d {
        Deferrability::NotDeferrable => "not-deferrable",
        Deferrability::InitiallyImmediate => "initially-immediate",
        Deferrability::InitiallyDeferred => "initially-deferred",
    }
}

/// The key a foreign key command names: `key`, or the primary key of the entity `references`.
fn target_key(ctx: &C<'_>, p: &Params) -> Result<Obj, CommandError> {
    if !text(p, "key").is_empty() {
        let k = key_id(ctx, p, "key")?;
        if !text(p, "references").is_empty() && rules::entity_of(ctx.doc, k) != Some(id_of(ctx, p, "references", ENTITY)?) {
            return Err(refuse("references", "the key is not one of the referenced entity's"));
        }
        return Ok(k);
    }
    let e = id_of(ctx, p, "references", ENTITY)?;
    rules::primary_key_of(ctx.doc, e).ok_or_else(|| refuse("references", format!("{} has no primary key to refer to", name_of(ctx.doc, e))))
}

/// Adds a foreign key: with `columns`, those columns hold it; without, new columns are added for
/// the key's columns, named and typed as drawing a relationship names and types them.
fn foreign_key_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let k = target_key(ctx, p)?;
    let given = attribute_list(ctx, p, "columns")?;
    let name = constraint_name(ctx, p, e, None)?;
    let actions = [action(p, "on_delete")?, action(p, "on_update")?];
    let (matching, deferrable) = (match_type(p)?, deferrability(p)?);
    let rel = if text(p, "relationship").is_empty() { None } else { Some(id_of(ctx, p, "relationship", RELATIONSHIP)?) };
    if !given.is_empty()
        && let Some((param, problem)) = rules::key_problem(ctx.doc, e, k, &given, actions, rel)
    {
        return Err(refuse(param, problem));
    }
    // New columns follow the relationship: in the key when it identifies, nullable when the end it
    // points at is optional.
    let (in_key, optional, parent_role) = match rel {
        Some(r) => {
            let identifying = ctx.doc.get(r, relationship::IDENTIFYING).ok().and_then(|v| v.as_bool()).unwrap_or(false);
            if identifying && rules::identifying_cycle(ctx.doc, e, k) {
                return Err(refuse("relationship", cycle_text(ctx.doc, e, k)));
            }
            let (end, role) = parent_end(ctx.doc, r, rules::entity_of(ctx.doc, k));
            (identifying, rules::cardinality_of(ctx.doc, r, end) == Some(Cardinality::ZeroOrOne), rules::role_of(ctx.doc, r, role))
        }
        None => (false, true, String::new()),
    };
    let fk = ctx
        .doc
        .transaction(|st| {
            let columns = if given.is_empty() { new_key_columns(st, e, k, &parent_role, in_key, optional)? } else { given.clone() };
            if let Some((_, problem)) = rules::key_problem(st, e, k, &columns, actions, rel) {
                return Err(Invalid::Refused(problem));
            }
            let fk = new_foreign_key(st, e, k, &columns, &name, actions, rel)?;
            st.set(fk, foreign_key::MATCH, Value::Enum(matching.index()))?;
            st.set(fk, foreign_key::DEFERRABLE, Value::Enum(deferrable.index()))?;
            Ok(fk)
        })
        .map_err(fail)?;
    select(ctx, vec![fk]);
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

fn foreign_key_set_actions(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    let actions = [action(p, "on_delete")?, action(p, "on_update")?];
    let parts = rules::key_parts(ctx.doc, fk);
    if let (Some(e), Some(k)) = (parts.entity, parts.key)
        && let Some((_, problem)) = rules::key_problem(ctx.doc, e, k, &parts.columns, actions, parts.relationship)
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

fn foreign_key_set_options(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    let (matching, deferrable) = (match_type(p)?, deferrability(p)?);
    ctx.doc
        .transaction(|st| {
            st.set(fk, foreign_key::MATCH, Value::Enum(matching.index()))?;
            st.set(fk, foreign_key::DEFERRABLE, Value::Enum(deferrable.index()))
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

/// Points a foreign key at another key: with `columns`, those columns hold it; without, new
/// columns are added for the key's columns, as drawing a relationship adds them. The old columns
/// stay.
fn foreign_key_set_key(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    not_subtype_key(ctx.doc, rules::key_parts(ctx.doc, fk).relationship, "foreign_key")?;
    let k = key_id(ctx, p, "key")?;
    let given = attribute_list(ctx, p, "columns")?;
    let parts = rules::key_parts(ctx.doc, fk);
    let e = parts.entity.ok_or_else(|| refuse("foreign_key", "the foreign key belongs to no entity"))?;
    let actions = [parts.on_delete, parts.on_update];
    if given.is_empty() && parts.key == Some(k) {
        return Err(refuse("key", "the foreign key points at this key already; name the columns to change them"));
    }
    if !given.is_empty()
        && let Some((param, problem)) = rules::key_problem(ctx.doc, e, k, &given, actions, parts.relationship)
    {
        return Err(refuse(param, problem));
    }
    // New columns follow the relationship: in the key when it identifies, nullable when the end it
    // points at is optional.
    let (in_key, optional, parent_role) = match parts.relationship {
        Some(r) => {
            let identifying = ctx.doc.get(r, relationship::IDENTIFYING).ok().and_then(|v| v.as_bool()).unwrap_or(false);
            if identifying && rules::identifying_cycle(ctx.doc, e, k) {
                return Err(refuse("key", cycle_text(ctx.doc, e, k)));
            }
            let (end, role) = parent_end(ctx.doc, r, rules::entity_of(ctx.doc, k));
            (identifying, rules::cardinality_of(ctx.doc, r, end) == Some(Cardinality::ZeroOrOne), rules::role_of(ctx.doc, r, role))
        }
        None => (false, true, String::new()),
    };
    let (mut fks, entities) = keys_near(ctx.doc, e).map_err(fail)?;
    fks.push(fk);
    ctx.doc
        .transaction(|st| {
            let columns = if given.is_empty() { new_key_columns(st, e, k, &parent_role, in_key, optional)? } else { given.clone() };
            st.set(fk, foreign_key::REFERENCES, Value::Ref(k))?;
            for i in (0..parts.columns.len()).rev() {
                st.remove_at(fk, foreign_key::COLUMN, u32::try_from(i).map_err(|_| Invalid::Refused("too many columns".into()))?)?;
            }
            for a in &columns {
                st.push(fk, foreign_key::COLUMN, Value::Ref(*a))?;
            }
            // The old followers that nothing else holds go with the old key.
            for c in &parts.columns {
                if !columns.contains(c) && removable_follower(st, *c, fk) {
                    remove_follower(st, *c)?;
                }
            }
            keys_still_hold(st, &fks, &entities)
        })
        .map_err(|e| refuse("key", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

fn foreign_key_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    let name = constraint_name(ctx, p, fk, Some(fk))?;
    set_name(ctx, fk, foreign_key::NAME, &name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, fk)? }))
}

fn foreign_key_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let fk = id_of(ctx, p, "foreign_key", FOREIGN_KEY)?;
    not_subtype_key(ctx.doc, rules::key_parts(ctx.doc, fk).relationship, "foreign_key")?;
    let gone = remove_element(ctx.doc, fk).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Keys.

/// Sets an entity's primary key: its columns in key order, made not nullable, and its name. A key
/// that foreign keys point at keeps its columns.
fn primary_key_set(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let columns = attribute_list(ctx, p, "columns")?;
    let existing = rules::primary_key_of(ctx.doc, e);
    let name = constraint_name(ctx, p, e, existing)?;
    if columns.is_empty() {
        return Err(refuse("columns", "a primary key needs at least one column; remove it instead"));
    }
    if let Some(problem) = rules::key_columns_problem(ctx.doc, e, &columns, false) {
        return Err(refuse("columns", problem));
    }
    let (fks, entities) = keys_near(ctx.doc, e).map_err(fail)?;
    let pk = ctx
        .doc
        .transaction(|st| {
            let pk = ensure_primary_key(st, e)?;
            let old = rules::key_columns(st, pk);
            set_key_columns(st, pk, &columns)?;
            grow_foreign_keys(st, pk, &old, &columns)?;
            for a in &columns {
                st.set(*a, attribute::NULLABLE, Value::Bool(false))?;
            }
            st.set(pk, key::NAME, if name.is_empty() { Value::Null } else { Value::str(&name) })?;
            keys_still_hold(st, &fks, &entities)?;
            // A supertype that has a key now gives it to its subtypes.
            ensure_subtype_keys(st, e)?;
            subtypes_hold(st, e)?;
            Ok(pk)
        })
        .map_err(|e| refuse("columns", e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, pk)? }))
}

fn primary_key_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    if let Some(pk) = rules::primary_key_of(ctx.doc, e) {
        not_piece(ctx.doc, pk, "entity")?;
    }
    let pk = rules::primary_key_of(ctx.doc, e).ok_or_else(|| refuse("entity", format!("{} has no primary key", name_of(ctx.doc, e))))?;
    let gone = remove_element(ctx.doc, pk).map_err(refused_as("entity"))?;
    Ok(json!({ "removed": gone }))
}

fn nulls_distinct(p: &Params) -> bool {
    p.bool("nulls_distinct").unwrap_or(true)
}

fn unique_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let columns = attribute_list(ctx, p, "columns")?;
    let name = constraint_name(ctx, p, e, None)?;
    if let Some(problem) = rules::key_columns_problem(ctx.doc, e, &columns, false) {
        return Err(refuse("columns", problem));
    }
    let distinct = nulls_distinct(p);
    let u = ctx.doc.transaction(|st| new_unique(st, e, &columns, &name, distinct)).map_err(fail)?;
    select(ctx, vec![u]);
    Ok(json!({ "id": uuid_of(ctx.doc, u)? }))
}

fn unique_set(ctx: &mut C<'_>, p: &Params) -> R {
    let u = id_of(ctx, p, "unique", UNIQUE_CONSTRAINT)?;
    let e = rules::entity_of(ctx.doc, u).ok_or_else(|| refuse("unique", "the constraint belongs to no entity"))?;
    let columns = attribute_list(ctx, p, "columns")?;
    let name = constraint_name(ctx, p, u, Some(u))?;
    if let Some(problem) = rules::key_columns_problem(ctx.doc, e, &columns, false) {
        return Err(refuse("columns", problem));
    }
    let distinct = nulls_distinct(p);
    let (fks, entities) = keys_near(ctx.doc, e).map_err(fail)?;
    ctx.doc
        .transaction(|st| {
            let old = rules::key_columns(st, u);
            set_key_columns(st, u, &columns)?;
            grow_foreign_keys(st, u, &old, &columns)?;
            keys_still_hold(st, &fks, &entities)?;
            st.set(u, key::NAME, if name.is_empty() { Value::Null } else { Value::str(&name) })?;
            st.set(u, unique_constraint::NULLS_DISTINCT, Value::Bool(distinct))
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, u)? }))
}

fn unique_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let u = id_of(ctx, p, "unique", UNIQUE_CONSTRAINT)?;
    let gone = remove_element(ctx.doc, u).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Checks and indexes (slice S3).

/// The expression under `key`, which a check needs.
fn check_expression<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    let expression = text(p, key);
    match rules::check_problem(expression) {
        Some(problem) => Err(refuse(key, problem)),
        None => Ok(expression),
    }
}

fn check_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let expression = check_expression(p, "expression")?;
    let name = constraint_name(ctx, p, e, None)?;
    let c = ctx
        .doc
        .transaction(|st| {
            let c = st.create(CHECK_CONSTRAINT)?;
            st.set(c, check_constraint::NAME, text_or_null(&name))?;
            st.set(c, check_constraint::EXPRESSION, Value::str(expression))?;
            let at = u32::try_from(st.list(e, entity::CHECK_CONSTRAINT)?.len()).unwrap_or(u32::MAX);
            st.insert(e, entity::CHECK_CONSTRAINT, at, Value::Ref(c))?;
            Ok(c)
        })
        .map_err(fail)?;
    select(ctx, vec![c]);
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn check_set(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx, p, "check", CHECK_CONSTRAINT)?;
    let expression = check_expression(p, "expression")?;
    let name = constraint_name(ctx, p, c, Some(c))?;
    ctx.doc
        .transaction(|st| {
            st.set(c, check_constraint::NAME, text_or_null(&name))?;
            st.set(c, check_constraint::EXPRESSION, Value::str(expression))
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn check_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let c = id_of(ctx, p, "check", CHECK_CONSTRAINT)?;
    let gone = remove_element(ctx.doc, c).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

/// The direction words, in the enumeration's order.
pub const DIRECTION_WORDS: [(SortDirection, &str); 2] = [(SortDirection::Ascending, "ascending"), (SortDirection::Descending, "descending")];

/// The nulls-order words, in the enumeration's order.
pub const NULLS_WORDS: [(NullsOrder, &str); 3] = [(NullsOrder::Default, "default"), (NullsOrder::First, "first"), (NullsOrder::Last, "last")];

/// A direction's name in parameters.
pub fn direction_name(d: SortDirection) -> &'static str {
    DIRECTION_WORDS.iter().find(|(x, _)| *x == d).map_or("ascending", |(_, w)| *w)
}

/// A nulls order's name in parameters.
pub fn nulls_name(n: NullsOrder) -> &'static str {
    NULLS_WORDS.iter().find(|(x, _)| *x == n).map_or("default", |(_, w)| *w)
}

/// A JSON value for an error text: its kind, with a short string's text.
fn shown(value: &Json) -> String {
    match value {
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        Json::String(s) if s.chars().count() <= 40 => format!("{s:?}"),
        Json::String(_) => "a long text".into(),
        Json::Array(_) => "an array".into(),
        Json::Object(_) => "an object".into(),
    }
}

/// The keys an index column object takes.
const INDEX_COLUMN_KEYS: [&str; 4] = ["attribute", "expression", "direction", "nulls"];

/// A word from a table, or the default when the field is missing.
fn word_of<T: Copy>(item: &Json, field: &str, words: &[(T, &str)], default: T, key: &str) -> Result<T, CommandError> {
    match item.get(field) {
        None | Some(Json::Null) => Ok(default),
        Some(Json::String(s)) => words
            .iter()
            .find(|(_, w)| w == s)
            .map(|(v, _)| *v)
            .ok_or_else(|| refuse(key, format!("{s:?} is not one of {}", words.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(", ")))),
        Some(other) => Err(refuse(key, format!("{field} is {}, not a word", shown(other)))),
    }
}

/// The index columns the array parameter `key` gives: objects with `attribute` (an id) or
/// `expression`, `direction` (ascending unless given) and `nulls` (default unless given), or bare
/// attribute ids with both defaults; an unknown key in an object is refused; checked against
/// `entity`.
fn index_columns_param(ctx: &C<'_>, p: &Params, key: &str, entity: Obj) -> Result<Vec<IndexColumnSpec>, CommandError> {
    let items = match p.get(key) {
        Some(Json::Array(items)) => items,
        _ => return Err(refuse(key, "an array of columns")),
    };
    if items.len() > rules::MAX_KEY_COLUMNS {
        return Err(refuse(key, format!("at most {} columns", rules::MAX_KEY_COLUMNS)));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let (attribute, expression, direction, nulls) = match item {
            Json::String(id) => (Some(id.as_str()), String::new(), SortDirection::Ascending, NullsOrder::Default),
            Json::Object(fields) => {
                if let Some(unknown) = fields.keys().find(|k| !INDEX_COLUMN_KEYS.contains(&k.as_str())) {
                    let unknown: String = unknown.chars().take(40).collect();
                    return Err(refuse(key, format!("a column takes no {unknown:?}; it takes {}", INDEX_COLUMN_KEYS.join(", "))));
                }
                (
                    match item.get("attribute") {
                        None | Some(Json::Null) => None,
                        Some(Json::String(s)) if s.is_empty() => None,
                        Some(Json::String(s)) => Some(s.as_str()),
                        Some(other) => return Err(refuse(key, format!("attribute is {}, not an id", shown(other)))),
                    },
                    match item.get("expression") {
                        None | Some(Json::Null) => String::new(),
                        Some(Json::String(s)) => s.clone(),
                        Some(other) => return Err(refuse(key, format!("expression is {}, not text", shown(other)))),
                    },
                    word_of(item, "direction", &DIRECTION_WORDS, SortDirection::Ascending, key)?,
                    word_of(item, "nulls", &NULLS_WORDS, NullsOrder::Default, key)?,
                )
            }
            other => return Err(refuse(key, format!("{} is not a column", shown(other)))),
        };
        let attribute = match attribute {
            Some(id) => {
                let uuid = Uuid::parse(id).ok_or_else(|| refuse(key, format!("{:?} is not an id", short(id))))?;
                let a = ctx
                    .doc
                    .by_uuid(uuid)
                    .filter(|a| ctx.doc.is_alive(*a))
                    .ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))?;
                Some(a)
            }
            None => None,
        };
        out.push(IndexColumnSpec { attribute, expression, direction, nulls });
    }
    if let Some(problem) = rules::index_columns_problem(ctx.doc, entity, &out) {
        return Err(refuse(key, problem));
    }
    Ok(out)
}

/// An index's `where` and `method`, checked.
fn index_options(p: &Params) -> Result<(bool, &str, &str), CommandError> {
    let unique = p.bool("unique").unwrap_or(false);
    let where_ = text(p, "where");
    check_text("where", rules::text_problem(where_, MAX_TEXT))?;
    let method = text(p, "method");
    if !method.is_empty() {
        check_name("method", method, MAX_NAME)?;
    }
    Ok((unique, where_, method))
}

/// Writes an index's fields and columns; the old columns go.
fn set_index(st: &mut Store, ix: Obj, name: &str, unique: bool, where_: &str, method: &str, columns: &[IndexColumnSpec]) -> Result<(), Invalid> {
    st.set(ix, table_index::NAME, text_or_null(name))?;
    st.set(ix, table_index::UNIQUE, Value::Bool(unique))?;
    st.set(ix, table_index::WHERE, text_or_null(where_))?;
    st.set(ix, table_index::METHOD, text_or_null(method))?;
    for old in rules::index_columns(st, ix) {
        archforge_core::destroy_with_links(st, old)?;
    }
    for (i, spec) in columns.iter().enumerate() {
        let c = st.create(INDEX_COLUMN)?;
        st.set(c, index_column::ATTRIBUTE, spec.attribute.map_or(Value::Null, Value::Ref))?;
        st.set(c, index_column::EXPRESSION, text_or_null(&spec.expression))?;
        st.set(c, index_column::DIRECTION, Value::Enum(spec.direction.index()))?;
        st.set(c, index_column::NULLS, Value::Enum(spec.nulls.index()))?;
        st.insert(ix, table_index::COLUMN, u32::try_from(i).unwrap_or(u32::MAX), Value::Ref(c))?;
    }
    Ok(())
}

fn index_add(ctx: &mut C<'_>, p: &Params) -> R {
    let e = id_of(ctx, p, "entity", ENTITY)?;
    let columns = index_columns_param(ctx, p, "columns", e)?;
    let name = constraint_name(ctx, p, e, None)?;
    let (unique, where_, method) = index_options(p)?;
    let ix = ctx
        .doc
        .transaction(|st| {
            let ix = st.create(TABLE_INDEX)?;
            set_index(st, ix, &name, unique, where_, method, &columns)?;
            let at = u32::try_from(st.list(e, entity::TABLE_INDEX)?.len()).unwrap_or(u32::MAX);
            st.insert(e, entity::TABLE_INDEX, at, Value::Ref(ix))?;
            Ok(ix)
        })
        .map_err(fail)?;
    select(ctx, vec![ix]);
    Ok(json!({ "id": uuid_of(ctx.doc, ix)? }))
}

fn index_set(ctx: &mut C<'_>, p: &Params) -> R {
    let ix = id_of(ctx, p, "index", TABLE_INDEX)?;
    let e = rules::entity_of(ctx.doc, ix).ok_or_else(|| refuse("index", "the index belongs to no entity"))?;
    let columns = index_columns_param(ctx, p, "columns", e)?;
    let name = constraint_name(ctx, p, ix, Some(ix))?;
    let (unique, where_, method) = index_options(p)?;
    ctx.doc.transaction(|st| set_index(st, ix, &name, unique, where_, method, &columns)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, ix)? }))
}

fn index_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let ix = id_of(ctx, p, "index", TABLE_INDEX)?;
    let gone = remove_element(ctx.doc, ix).map_err(fail)?;
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

/// Why key `k` cannot identify `child`: the identifying keys would run in a circle.
fn cycle_text(store: &Store, child: Obj, k: Obj) -> String {
    let parent = rules::entity_of(store, k).map(|p| name_of(store, p)).unwrap_or_default();
    format!("the identifying relationships would form a cycle between {} and {parent}", name_of(store, child))
}

/// Sets a relationship's ends; the key columns of its foreign key follow the parent end: nullable
/// for zero-or-one, not null for exactly-one, a column in the child's primary key staying not null.
fn relationship_set_ends(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    not_subtype_key(ctx.doc, Some(r), "relationship")?;
    let (from_end, to_end) = (cardinality(p, "from_end")?, cardinality(p, "to_end")?);
    let (was_from, was_to) = (rules::cardinality_of(ctx.doc, r, relationship::FROM_END), rules::cardinality_of(ctx.doc, r, relationship::TO_END));
    let (was_from, was_to) = (was_from.unwrap_or(from_end), was_to.unwrap_or(to_end));
    let fks: Vec<Obj> = ctx.doc.referrers(r).into_iter().filter(|(_, prop)| *prop == foreign_key::RELATIONSHIP).map(|(fk, _)| fk).collect();
    let (fks, entities) = gather(ctx.doc, fks.into_iter().collect(), None).map_err(fail)?;
    ctx.doc
        .transaction(|st| {
            st.set(r, relationship::FROM_END, Value::Enum(from_end.index()))?;
            st.set(r, relationship::TO_END, Value::Enum(to_end.index()))?;
            for fk in &fks {
                let parts = rules::key_parts(st, *fk);
                if parts.relationship != Some(r) {
                    continue;
                }
                let Some(child) = parts.entity else { continue };
                let (end, _) = parent_end(st, r, parts.key.and_then(|k| rules::entity_of(st, k)));
                let nullable = match rules::cardinality_of(st, r, end) {
                    Some(Cardinality::ZeroOrOne) => true,
                    Some(Cardinality::ExactlyOne) => false,
                    _ => continue,
                };
                let pk = rules::primary_key(st, child);
                for a in &parts.columns {
                    // A column another foreign key also holds keeps that key's say.
                    let shared = st.referrers(*a).iter().any(|(src, prop)| *prop == foreign_key::COLUMN && *src != *fk);
                    if !pk.contains(a) && !shared {
                        st.set(*a, attribute::NULLABLE, Value::Bool(nullable))?;
                    }
                }
            }
            keys_still_hold(st, &fks, &entities)
        })
        .map_err(|e| refuse(if to_end != was_to && from_end == was_from { "to_end" } else { "from_end" }, e.to_string()))?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// A key column made nullable or not moves the parent end of its relationship to zero-or-one or
/// exactly-one, so the picture keeps agreeing with itself.
fn follow_nullability(st: &mut Store, a: Obj, nullable: bool) -> Result<(), Invalid> {
    let fks: Vec<Obj> = st.referrers(a).into_iter().filter(|(_, prop)| *prop == foreign_key::COLUMN).map(|(fk, _)| fk).collect();
    for fk in fks {
        let parts = rules::key_parts(st, fk);
        let (Some(r), Some(child)) = (parts.relationship, parts.entity) else { continue };
        if rules::is_many_to_many(st, r) {
            continue;
        }
        // The end is optional when every column of the key outside the child's primary key is.
        let pk = rules::primary_key(st, child);
        let outside: Vec<Obj> = parts.columns.iter().copied().filter(|c| !pk.contains(c)).collect();
        if outside.is_empty() {
            continue;
        }
        let optional = outside.iter().all(|c| if *c == a { nullable } else { rules::is_nullable(st, *c) });
        let (end, _) = parent_end(st, r, parts.key.and_then(|k| rules::entity_of(st, k)));
        let card = if optional { Cardinality::ZeroOrOne } else { Cardinality::ExactlyOne };
        st.set(r, end, Value::Enum(card.index()))?;
    }
    Ok(())
}

fn relationship_set_roles(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    not_subtype_key(ctx.doc, Some(r), "relationship")?;
    let roles = [role(p, "from_role")?, role(p, "to_role")?];
    ctx.doc
        .transaction(|st| {
            for (prop, role) in [(relationship::FROM_ROLE, &roles[0]), (relationship::TO_ROLE, &roles[1])] {
                st.set(r, prop, if role.is_empty() { Value::Null } else { Value::str(role) })?;
            }
            Ok(())
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

fn relationship_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    let name = relationship_name(p)?.to_string();
    if let Some((s, _)) = ctx.doc.container(r).ok().flatten()
        && !rules::relationship_name_free(ctx.doc, s, &name, Some(r))
    {
        return Err(refuse("name", format!("the schema has a relationship named {name:?} already")));
    }
    set_name(ctx, r, relationship::NAME, &name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, r)? }))
}

/// Refuses to remove or reshape a piece of a subtype's key on its own: the generalization owns it.
fn not_piece(store: &Store, obj: Obj, key: &str) -> Result<(), CommandError> {
    if breaks_subtype_key(store, obj).is_some() {
        return Err(refuse(key, "it is part of a subtype's key; remove the generalization or change the mapping instead"));
    }
    Ok(())
}

/// The generalization whose subtype key `obj` is a piece of, or whose subtype's primary key goes
/// with `obj` (a key goes whole with any of its columns).
fn breaks_subtype_key(store: &Store, obj: Obj) -> Option<Obj> {
    subtype_key_piece(store, obj).or_else(|| dependents(store, obj).ok()?.0.into_iter().find_map(|k| subtype_key_piece(store, k)))
}

/// Refuses to remove a relationship or foreign key that is a subtype's key: the generalization owns it.
fn not_subtype_key(store: &Store, r: Option<Obj>, key: &str) -> Result<(), CommandError> {
    let Some(r) = r else { return Ok(()) };
    let owned = store.referrers(r).into_iter().any(|(g, p)| p == generalization::KEY_RELATIONSHIP && store.is_alive(g));
    if owned {
        return Err(refuse(key, "it is a subtype's key; remove the generalization instead"));
    }
    Ok(())
}

fn relationship_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let r = id_of(ctx, p, "relationship", RELATIONSHIP)?;
    not_subtype_key(ctx.doc, Some(r), "relationship")?;
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
        let uuid = Uuid::parse(id).ok_or_else(|| refuse(key, format!("{:?} is not an id", short(id))))?;
        let obj = ctx.doc.by_uuid(uuid).ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))?;
        if !seen.insert(obj) {
            return Err(refuse(key, format!("{id} is listed twice")));
        }
        out.push(obj);
    }
    Ok(out)
}

/// Removes schemas, entities, attributes, relationships, keys and foreign keys at once: the
/// selection's delete (`element.remove`, through [`crate::ErNotation`]). Foreign keys, keys,
/// relationships and attributes first, then entities, then schemas, so a cascade finds nothing
/// missing; what an earlier removal already took is skipped. How many went.
pub fn remove_elements(store: &mut Store, ids: &[Obj]) -> Result<usize, CommandError> {
    let mut by_class: Vec<(ClassId, Obj)> = Vec::new();
    for o in ids {
        let class = store.class_of(*o).map_err(fail)?;
        if ![GENERALIZATION, FOREIGN_KEY, PRIMARY_KEY, UNIQUE_CONSTRAINT, RELATIONSHIP, ATTRIBUTE, ENTITY, SEQUENCE, VIEW, SCHEMA].contains(&class) {
            return Err(refuse("ids", format!("{} is not an ER element", uuid_of(store, *o)?)));
        }
        by_class.push((class, *o));
    }
    // A piece of a subtype key goes only with its own generalization, one of its two tables, or
    // their schema in the same call.
    for (_, o) in &by_class {
        if let Some(g) = breaks_subtype_key(store, *o) {
            let (sup, sub, _) = rules::generalization_parts(store, g);
            let schema = rules::schema_of(store, g);
            let covered = ids.iter().any(|x| Some(*x) == Some(g) || Some(*x) == sup || Some(*x) == sub || Some(*x) == schema);
            if !covered {
                return Err(refuse("ids", "a part of a subtype's key goes only with its generalization"));
            }
        }
    }
    let order = [GENERALIZATION, FOREIGN_KEY, UNIQUE_CONSTRAINT, PRIMARY_KEY, RELATIONSHIP, ATTRIBUTE, ENTITY, SEQUENCE, VIEW, SCHEMA];
    let mut gone = 0;
    for class in order {
        for (c, o) in &by_class {
            if *c == class && store.is_alive(*o) {
                gone += match class {
                    ENTITY => remove_entity(store, *o),
                    GENERALIZATION => store.transaction(|st| remove_generalization(st, *o)),
                    SCHEMA => remove_schema(store, *o),
                    _ => remove_element(store, *o),
                }
                .map_err(refused_as("ids"))?;
            }
        }
    }
    Ok(gone)
}

// Queries.

fn uuid_json(store: &Store, obj: Obj) -> Json {
    store.uuid(obj).map(|u| Json::String(u.to_string())).unwrap_or(Json::Null)
}

fn attribute_json(store: &Store, a: Obj) -> Json {
    let in_foreign_key = store.referrers(a).iter().any(|(_, prop)| *prop == foreign_key::COLUMN);
    json!({
        "foreign_key": in_foreign_key,
        "id": uuid_json(store, a),
        "name": name_of(store, a),
        "type": rules::type_of(store, a),
        "primary_key": rules::in_primary_key(store, a),
        "nullable": rules::is_nullable(store, a),
        "unique": rules::is_unique(store, a),
        "linked_to": rules::linked_to(store, a).map(|t| uuid_json(store, t)).unwrap_or(Json::Null),
        "domain": rules::domain_of(store, a).map(|d| uuid_json(store, d)).unwrap_or(Json::Null),
        "enum": rules::enum_of(store, a).map(|e| uuid_json(store, e)).unwrap_or(Json::Null),
    })
}

fn ids_json(store: &Store, objs: &[Obj]) -> Json {
    Json::Array(objs.iter().map(|o| uuid_json(store, *o)).collect())
}

fn optional_name(store: &Store, obj: Obj) -> Json {
    let name = name_of(store, obj);
    if name.is_empty() { Json::Null } else { Json::String(name) }
}

fn key_json(store: &Store, k: Obj) -> Json {
    let mut j = json!({ "id": uuid_json(store, k), "name": optional_name(store, k), "columns": ids_json(store, &rules::key_columns(store, k)) });
    if store.class_of(k).ok() == Some(UNIQUE_CONSTRAINT) {
        j["nulls_distinct"] = Json::Bool(rules::nulls_distinct(store, k));
    }
    j
}

fn check_json(store: &Store, c: Obj) -> Json {
    json!({ "id": uuid_json(store, c), "name": optional_name(store, c), "expression": rules::text_of(store, c, check_constraint::EXPRESSION) })
}

fn index_json(store: &Store, ix: Obj) -> Json {
    let columns: Vec<Json> = rules::index_specs(store, ix)
        .into_iter()
        .map(|s| {
            json!({
                "attribute": s.attribute.map(|a| uuid_json(store, a)).unwrap_or(Json::Null),
                "expression": if s.expression.is_empty() { Json::Null } else { Json::String(s.expression) },
                "direction": direction_name(s.direction),
                "nulls": nulls_name(s.nulls),
            })
        })
        .collect();
    let text = |prop| {
        let t = rules::text_of(store, ix, prop);
        if t.is_empty() { Json::Null } else { Json::String(t) }
    };
    json!({
        "id": uuid_json(store, ix),
        "name": optional_name(store, ix),
        "unique": rules::is_unique_index(store, ix),
        "columns": columns,
        "where": text(table_index::WHERE),
        "method": text(table_index::METHOD),
    })
}

fn foreign_key_json(store: &Store, fk: Obj) -> Json {
    let parts = rules::key_parts(store, fk);
    json!({
        "id": uuid_json(store, fk),
        "name": optional_name(store, fk),
        "key": parts.key.map(|k| uuid_json(store, k)).unwrap_or(Json::Null),
        "references": parts.references.map(|r| uuid_json(store, r)).unwrap_or(Json::Null),
        "columns": ids_json(store, &parts.columns),
        "targets": ids_json(store, &parts.targets),
        "on_delete": action_name(parts.on_delete),
        "on_update": action_name(parts.on_update),
        "match": match_name(rules::match_of(store, fk)),
        "deferrable": deferrability_name(rules::deferrability_of(store, fk)),
        "relationship": parts.relationship.map(|r| uuid_json(store, r)).unwrap_or(Json::Null),
    })
}

fn entity_json(store: &Store, e: Obj) -> Json {
    let attributes: Vec<Json> =
        store.list(e, entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(Value::as_ref).map(|a| attribute_json(store, a)).collect();
    let uniques: Vec<Json> = rules::unique_constraints(store, e).into_iter().map(|u| key_json(store, u)).collect();
    let keys: Vec<Json> = rules::foreign_keys(store, e).into_iter().map(|fk| foreign_key_json(store, fk)).collect();
    let checks: Vec<Json> = rules::checks_of(store, e).into_iter().map(|c| check_json(store, c)).collect();
    let indexes: Vec<Json> = rules::indexes_of(store, e).into_iter().map(|ix| index_json(store, ix)).collect();
    let kind = match rules::kind_of(store, e) {
        EntityKind::Regular => "regular",
        EntityKind::Associative => "associative",
    };
    json!({
        "id": uuid_json(store, e),
        "name": name_of(store, e),
        "kind": kind,
        "attributes": attributes,
        "primary_key": rules::primary_key_of(store, e).map(|k| key_json(store, k)).unwrap_or(Json::Null),
        "unique_constraints": uniques,
        "foreign_keys": keys,
        "checks": checks,
        "indexes": indexes,
        "subtyping": {
            "complete": rules::subtyping_flags(store, e).0,
            "disjoint": rules::subtyping_flags(store, e).1,
            "discriminator": rules::discriminator_of(store, e).map(|a| uuid_json(store, a)).unwrap_or(Json::Null),
            "mapping": mapping_name(rules::mapping_of(store, e)),
        },
    })
}

fn optional_role(store: &Store, r: Obj, prop: archforge_mof::PropId) -> Json {
    let role = rules::role_of(store, r, prop);
    if role.is_empty() { Json::Null } else { Json::String(role) }
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
        "from_role": optional_role(store, r, relationship::FROM_ROLE),
        "to_role": optional_role(store, r, relationship::TO_ROLE),
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
    let generalizations: Vec<Json> = ctx
        .doc
        .list(s, schema::GENERALIZATION)
        .map_err(fail)?
        .iter()
        .filter_map(Value::as_ref)
        .map(|g| {
            let (sup, sub, key) = rules::generalization_parts(ctx.doc, g);
            let id = |o: Option<Obj>| o.map(|o| uuid_json(ctx.doc, o)).unwrap_or(Json::Null);
            json!({ "id": uuid_json(ctx.doc, g), "supertype": id(sup), "subtype": id(sub), "key_relationship": id(key) })
        })
        .collect();
    let domains: Vec<Json> = rules::domains_of(ctx.doc, s).into_iter().map(|d| domain_json(ctx.doc, d)).collect();
    let enums: Vec<Json> = rules::enums_of(ctx.doc, s).into_iter().map(|e| enum_json(ctx.doc, e)).collect();
    let sequences: Vec<Json> = rules::sequences_of(ctx.doc, s).into_iter().map(|q| sequence_json(ctx.doc, q)).collect();
    let views: Vec<Json> = rules::views_of(ctx.doc, s).into_iter().map(|v| view_json(ctx.doc, v)).collect();
    Ok(json!({
        "id": uuid_json(ctx.doc, s),
        "name": name_of(ctx.doc, s),
        "entities": entities,
        "relationships": relationships,
        "generalizations": generalizations,
        "domains": domains,
        "enums": enums,
        "sequences": sequences,
        "views": views,
        "notices": notices,
    }))
}

fn domain_json(store: &Store, d: Obj) -> Json {
    let (l, p, s) = rules::own_size_of(store, d);
    let checks: Vec<Json> = rules::domain_checks(store, d)
        .into_iter()
        .map(|c| {
            json!({ "id": uuid_json(store, c), "name": optional_name_of(store, c, domain_check::NAME), "expression": rules::text_of(store, c, domain_check::EXPRESSION) })
        })
        .collect();
    json!({
        "id": uuid_json(store, d),
        "name": name_of(store, d),
        "base_type": rules::text_of(store, d, domain::BASE_TYPE),
        "length": l,
        "precision": p,
        "scale": s,
        "default": optional_name_of(store, d, domain::DEFAULT),
        "not_null": store.get(d, domain::NOT_NULL).ok().and_then(|v| v.as_bool()).unwrap_or(false),
        "checks": checks,
    })
}

fn sequence_json(store: &Store, q: Obj) -> Json {
    let n = rules::sequence_numbers(store, q);
    // A bound a loaded document holds that is no number shows as its text, never as no bound.
    let bound = |t: &str| match rules::bound_of(t) {
        Ok(b) => b.map_or(Json::Null, Json::from),
        Err(_) => Json::String(t.chars().take(rules::MAX_NAME).collect()),
    };
    json!({
        "id": uuid_json(store, q),
        "name": name_of(store, q),
        "start": n.start,
        "increment": n.increment,
        "minimum": bound(&n.minimum),
        "maximum": bound(&n.maximum),
        "cycle": n.cycle,
        "owned_by": rules::owner_of_sequence(store, q).map_or(Json::Null, |a| uuid_json(store, a)),
    })
}

fn view_json(store: &Store, v: Obj) -> Json {
    json!({
        "id": uuid_json(store, v),
        "name": name_of(store, v),
        "query": rules::text_of(store, v, view::QUERY),
        "materialized": store.get(v, view::MATERIALIZED).ok().and_then(|x| x.as_bool()).unwrap_or(false),
    })
}

fn enum_json(store: &Store, e: Obj) -> Json {
    json!({ "id": uuid_json(store, e), "name": name_of(store, e), "literals": rules::literals_of(store, e) })
}

/// A text property as JSON: null when empty.
fn optional_name_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Json {
    let t = rules::text_of(store, o, prop);
    if t.is_empty() { Json::Null } else { Json::String(t) }
}

/// The ER notation's command table, with the edit area.
/// ER's own commands; the edit area, the shared element commands, the diagrams' and the skills'
/// join them in a registry.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    let mut s = Vec::new();
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
            "entity: string; the entity's id\nname: string; the attribute's name\ntype: string = \"\"; its type, free text\ndomain: string = \"\"; a domain of the schema instead of a type\nenum: string = \"\"; an enum type of the schema instead of a type\nlength: integer = 0, 0..65535; a length for a type that takes one, 0 for none\nprecision: integer = 0, 0..65535; a precision for a type that takes one, 0 for none\nscale: integer = 0, 0..65535; a scale beside the precision, 0 for none\ndefault: string = \"\"; an SQL expression, or nothing\ngeneration: string = \"none\", none|identity-always|identity-by-default|computed-stored|computed-virtual; how the value comes about\nexpression: string = \"\"; the expression of a computed column, or nothing\ncollation: string = \"\"; a collation name, or nothing\nprimary_key: bool = false; joins the primary key\nnullable: string = \"auto\", auto|yes|no; nullable (auto: a key or identity column is not)\nunique: bool = false; a unique constraint on it alone\nat: integer = -1, -1..1000000; where in the entity, -1 for the end",
            has_entities,
            attribute_add
        ),
        cmd!("er.attribute.rename", "Rename attribute", &["Entity"], None, "attribute: string; the attribute's id\nname: string; the new name", has_entities, attribute_rename),
        cmd!(
            "er.attribute.set-type",
            "Set attribute type",
            &["Entity"],
            None,
            "attribute: string; the attribute's id\ntype: string = \"\"; its type, free text\ndomain: string = \"\"; a domain of the schema instead of a type\nenum: string = \"\"; an enum type of the schema instead of a type\nlength: integer = 0, 0..65535; a length for a type that takes one, 0 for none\nprecision: integer = 0, 0..65535; a precision for a type that takes one, 0 for none\nscale: integer = 0, 0..65535; a scale beside the precision, 0 for none",
            has_entities,
            attribute_set_type
        ),
        cmd!(
            "er.attribute.set-default",
            "Set attribute default",
            &["Entity"],
            None,
            "attribute: string; the attribute's id\ndefault: string = \"\"; an SQL expression, or nothing",
            has_entities,
            attribute_set_default
        ),
        cmd!(
            "er.attribute.set-generation",
            "Set attribute generation",
            &["Entity"],
            None,
            "attribute: string; the attribute's id\ngeneration: string, none|identity-always|identity-by-default|computed-stored|computed-virtual; how the value comes about\nexpression: string = \"\"; the expression of a computed column, or nothing",
            has_entities,
            attribute_set_generation
        ),
        cmd!(
            "er.attribute.set-collation",
            "Set attribute collation",
            &["Entity"],
            None,
            "attribute: string; the attribute's id\ncollation: string = \"\"; a collation name, or nothing",
            has_entities,
            attribute_set_collation
        ),
        cmd!(
            "er.attribute.set-flags",
            "Set attribute flags",
            &["Entity"],
            None,
            "attribute: string; the attribute's id\nprimary_key: bool; in the primary key\nnullable: string, auto|yes|no; nullable (auto: a key or identity column is not)\nunique: bool; a unique constraint on it alone",
            has_entities,
            attribute_set_flags
        ),
        cmd!(
            "er.domain.add",
            "Add domain",
            &["Schema"],
            None,
            "schema: string; the schema's id\nname: string; the domain's name\nbase_type: string; the type it is built on\nlength: integer = 0, 0..65535; a length for a base type that takes one, 0 for none\nprecision: integer = 0, 0..65535; a precision for a base type that takes one, 0 for none\nscale: integer = 0, 0..65535; a scale beside the precision, 0 for none\ndefault: string = \"\"; an SQL expression, or nothing\nnot_null: bool = false; its columns take no null\nchecks: array = []; SQL conditions, each text or {expression, name}",
            has_schemas,
            domain_add
        ),
        cmd!(
            "er.domain.set",
            "Set domain",
            &["Schema"],
            None,
            "domain: string; the domain's id\nname: string; the domain's name\nbase_type: string; the type it is built on\nlength: integer = 0, 0..65535; a length for a base type that takes one, 0 for none\nprecision: integer = 0, 0..65535; a precision for a base type that takes one, 0 for none\nscale: integer = 0, 0..65535; a scale beside the precision, 0 for none\ndefault: string = \"\"; an SQL expression, or nothing\nnot_null: bool = false; its columns take no null\nchecks: array = []; SQL conditions, each text or {expression, name}",
            has_schemas,
            domain_set
        ),
        cmd!("er.domain.remove", "Remove domain", &["Schema"], None, "domain: string; the domain's id", has_schemas, domain_remove),
        cmd!(
            "er.enum.add",
            "Add enum type",
            &["Schema"],
            None,
            "schema: string; the schema's id\nname: string; the enum type's name\nliterals: array; its literals, in order",
            has_schemas,
            enum_add
        ),
        cmd!(
            "er.enum.set",
            "Set enum type",
            &["Schema"],
            None,
            "enum: string; the enum type's id\nname: string; the enum type's name\nliterals: array; its literals, in order",
            has_schemas,
            enum_set
        ),
        cmd!("er.enum.remove", "Remove enum type", &["Schema"], None, "enum: string; the enum type's id", has_schemas, enum_remove),
        cmd!("er.sequence.add", "Add sequence", &["Schema"], None, "schema: string; the schema's id\nname: string; the sequence's name\nstart: integer = 1; its first number\nincrement: integer = 1; the step, negative to count down, never 0\nminimum: string = \"\"; the least number as text, or nothing for the type's\nmaximum: string = \"\"; the greatest number as text, or nothing for the type's\ncycle: bool = false; it starts over past its bound\nowned_by: string = \"\"; the id of a column of the schema it belongs to, or nothing", has_schemas, sequence_add),
        cmd!("er.sequence.set", "Set sequence", &["Schema"], None, "sequence: string; the sequence's id\nname: string; the sequence's name\nstart: integer = 1; its first number\nincrement: integer = 1; the step, negative to count down, never 0\nminimum: string = \"\"; the least number as text, or nothing for the type's\nmaximum: string = \"\"; the greatest number as text, or nothing for the type's\ncycle: bool = false; it starts over past its bound\nowned_by: string = \"\"; the id of a column of the schema it belongs to, or nothing", has_schemas, sequence_set),
        cmd!("er.sequence.remove", "Remove sequence", &["Schema"], None, "sequence: string; the sequence's id", has_schemas, sequence_remove),
        cmd!("er.view.add", "Add view", &["Schema"], None, "schema: string; the schema's id\nname: string; the view's name\nquery: string; its SQL query, not parsed\nmaterialized: bool = false; its rows are stored", has_schemas, view_add),
        cmd!("er.view.set", "Set view", &["Schema"], None, "view: string; the view's id\nname: string; the view's name\nquery: string; its SQL query, not parsed\nmaterialized: bool = false; its rows are stored", has_schemas, view_set),
        cmd!("er.view.remove", "Remove view", &["Schema"], None, "view: string; the view's id", has_schemas, view_remove),
        cmd!("er.attribute.unlink", "Unlink attribute", &["Entity"], None, "attribute: string; the id of a key column that follows the column it points at", has_entities, attribute_unlink),
        cmd!("er.attribute.move", "Move attribute", &["Entity"], None, "attribute: string; the attribute's id\nto: integer, 0..1000000; its new place", has_entities, attribute_move),
        cmd!("er.attribute.remove", "Remove attribute", &["Entity"], None, "attribute: string; the attribute's id", has_entities, attribute_remove),
        cmd!(
            "er.relationship.add",
            "Add relationship",
            &["Relationship"],
            None,
            "from: string; the first entity's id\nto: string; the second entity's id\nfrom_end: string = \"exactly-one\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the first end\nto_end: string = \"zero-or-more\", zero-or-one|exactly-one|zero-or-more|one-or-more; the cardinality at the second end\nname: string = \"\"; a name, or nothing\ncreate_foreign_key: bool = false; also add the foreign key and its columns to the end with many\nidentifying: bool = false; the foreign key is part of the child's primary key\npoints_to: string = \"\"; the id of the parent's key it points at, or nothing for its primary key\nfrom_role: string = \"\"; the role at the first end, such as parent or billing_address; it names the key columns that point there\nto_role: string = \"\"; the role at the second end",
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
        cmd!(
            "er.relationship.set-roles",
            "Set relationship roles",
            &["Relationship"],
            None,
            "relationship: string; the relationship's id\nfrom_role: string; the role at the first end, or empty for none\nto_role: string; the role at the second end, or empty for none",
            has_entities,
            relationship_set_roles
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
            "entity: string; the id of the entity that holds the key\nkey: string = \"\"; the id of the primary key or unique constraint it points at\nreferences: string = \"\"; without key, the entity whose primary key it points at\ncolumns: array = []; the ids of its own columns, in the key's column order, or none to add new ones\nname: string = \"\"; a constraint name, or nothing\non_delete: string = \"no-action\", no-action|restrict|cascade|set-null|set-default; when the referenced row goes\non_update: string = \"no-action\", no-action|restrict|cascade|set-null|set-default; when the referenced key changes\nmatch: string = \"simple\", simple|full|partial; how null columns match\ndeferrable: string = \"not-deferrable\", not-deferrable|initially-immediate|initially-deferred; when it is checked\nrelationship: string = \"\"; the id of the relationship it realises, or nothing",
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
        cmd!(
            "er.foreign-key.set-options",
            "Set foreign key options",
            &["Entity"],
            None,
            "foreign_key: string; the foreign key's id\nmatch: string, simple|full|partial; how null columns match\ndeferrable: string, not-deferrable|initially-immediate|initially-deferred; when it is checked",
            has_entities,
            foreign_key_set_options
        ),
        cmd!(
            "er.foreign-key.set-key",
            "Point foreign key at",
            &["Entity"],
            None,
            "foreign_key: string; the foreign key's id\nkey: string; the id of the primary key or unique constraint it points at now\ncolumns: array = []; the ids of the columns that hold it, or none to add new ones",
            has_entities,
            foreign_key_set_key
        ),
        cmd!("er.foreign-key.rename", "Rename foreign key", &["Entity"], None, "foreign_key: string; the foreign key's id\nname: string = \"\"; the new name, or nothing", has_entities, foreign_key_rename),
        cmd!("er.foreign-key.remove", "Remove foreign key", &["Entity"], None, "foreign_key: string; the foreign key's id", has_entities, foreign_key_remove),
        cmd!(
            "er.primary-key.set",
            "Set primary key",
            &["Entity"],
            None,
            "entity: string; the entity's id\ncolumns: array; the ids of its columns, in key order\nname: string; a constraint name, or empty for none",
            has_entities,
            primary_key_set
        ),
        cmd!("er.primary-key.remove", "Remove primary key", &["Entity"], None, "entity: string; the entity's id", has_entities, primary_key_remove),
        cmd!(
            "er.unique.add",
            "Add unique constraint",
            &["Entity"],
            None,
            "entity: string; the entity's id\ncolumns: array; the ids of its columns, in order\nname: string = \"\"; a constraint name, or nothing\nnulls_distinct: bool = true; rows with nulls never clash",
            has_entities,
            unique_add
        ),
        cmd!(
            "er.unique.set",
            "Edit unique constraint",
            &["Entity"],
            None,
            "unique: string; the unique constraint's id\ncolumns: array; the ids of its columns, in order\nname: string; a constraint name, or empty for none\nnulls_distinct: bool; rows with nulls never clash",
            has_entities,
            unique_set
        ),
        cmd!("er.unique.remove", "Remove unique constraint", &["Entity"], None, "unique: string; the unique constraint's id", has_entities, unique_remove),
        cmd!(
            "er.check.add",
            "Add check constraint",
            &["Entity"],
            None,
            "entity: string; the entity's id\nexpression: string; the SQL condition every row holds\nname: string = \"\"; a constraint name, or nothing",
            has_entities,
            check_add
        ),
        cmd!(
            "er.check.set",
            "Set check constraint",
            &["Entity"],
            None,
            "check: string; the check constraint's id\nexpression: string; the SQL condition every row holds\nname: string = \"\"; a constraint name, or nothing",
            has_entities,
            check_set
        ),
        cmd!("er.check.remove", "Remove check constraint", &["Entity"], None, "check: string; the check constraint's id", has_entities, check_remove),
        cmd!(
            "er.index.add",
            "Add index",
            &["Entity"],
            None,
            "entity: string; the entity's id\ncolumns: array; the columns in index order, each an attribute id or {attribute | expression, direction: ascending|descending (ascending unless given), nulls: default|first|last (default unless given)}\nname: string = \"\"; an index name, or nothing\nunique: bool = false; a unique index\nwhere: string = \"\"; the condition of a partial index, or nothing\nmethod: string = \"\"; the access method, or nothing",
            has_entities,
            index_add
        ),
        cmd!(
            "er.index.set",
            "Set index",
            &["Entity"],
            None,
            "index: string; the index's id\ncolumns: array; the columns in index order, each an attribute id or {attribute | expression, direction: ascending|descending (ascending unless given), nulls: default|first|last (default unless given)}\nname: string = \"\"; an index name, or nothing\nunique: bool = false; a unique index\nwhere: string = \"\"; the condition of a partial index, or nothing\nmethod: string = \"\"; the access method, or nothing",
            has_entities,
            index_set
        ),
        cmd!("er.index.remove", "Remove index", &["Entity"], None, "index: string; the index's id", has_entities, index_remove),
        cmd!("er.entity.set-kind", "Set entity kind", &["Entity"], None, "entity: string; the entity's id\nkind: string, regular|associative; associative for a mapping table", has_entities, entity_set_kind),
        cmd!(
            "er.relationship.collapse",
            "Collapse mapping table",
            &["Relationship"],
            None,
            "entity: string; the id of a mapping table that holds nothing but its two keys",
            has_entities,
            relationship_collapse
        ),
        cmd!(
            "er.generalization.add",
            "Add subtype",
            &["Entity"],
            None,
            "supertype: string; the id of the general table\nsubtype: string; the id of the table that is a kind of it",
            has_entities,
            generalization_add
        ),
        cmd!(
            "er.generalization.remove",
            "Remove subtype",
            &["Entity"],
            None,
            "generalization: string; the generalization's id; the key it made goes with it",
            has_entities,
            generalization_remove
        ),
        cmd!(
            "er.entity.set-subtyping",
            "Set subtyping",
            &["Entity"],
            None,
            "entity: string; the supertype's id\ncomplete: bool = false; every row is one of the subtypes\ndisjoint: bool = true; no row is two of them\ndiscriminator: string = \"\"; the id of the column that names the subtype, or nothing\nmapping: string = \"table-per-type\", table-per-type|single-table|table-per-concrete-type; how the subtypes become tables",
            has_entities,
            entity_set_subtyping
        ),
        cmd!(query "er.schema.list", "List schemas", &["Schema"], None, "", always, schema_list),
        cmd!(query "er.schema.get", "Get schema", &["Schema"], None, "schema: string; the schema's id", has_schemas, schema_get),
        cmd!(query "er.entity.list", "List entities", &["Entity"], None, "schema: string; the schema's id", has_schemas, entity_list),
        cmd!(query "er.entity.get", "Get entity", &["Entity"], None, "entity: string; the entity's id", has_entities, entity_get),
    ]);
    s
}
