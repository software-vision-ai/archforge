//! The ER rules the MOF core cannot know, checked by the commands before they change anything and
//! available over a whole store.

use std::collections::BTreeSet;

use archforge_mof::{Obj, Store, Value};

use crate::{
    ATTRIBUTE, Cardinality, Deferrability, ENTITY, EntityKind, FOREIGN_KEY, MatchType, PRIMARY_KEY, RELATIONSHIP, ReferentialAction, SCHEMA,
    UNIQUE_CONSTRAINT, attribute, entity, foreign_key, key, named_element, relationship, schema, unique_constraint,
};

/// The most characters in a name, and in an attribute's type.
pub const MAX_NAME: usize = 128;
pub const MAX_TYPE: usize = 64;
/// The most columns in one key or foreign key.
pub const MAX_KEY_COLUMNS: usize = 32;

/// Why a name is not acceptable, as the end of the sentence "the name ...".
pub fn name_problem(name: &str, max: usize) -> Option<String> {
    if name.is_empty() {
        return Some("is empty".into());
    }
    if name.chars().count() > max {
        return Some(format!("is longer than {max} characters"));
    }
    if name.chars().any(char::is_control) {
        return Some("holds a control character".into());
    }
    if name.trim() != name {
        return Some("starts or ends with a space".into());
    }
    None
}

/// Whether two names are the same name, ignoring case.
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// The name of an ER element, or empty.
pub fn name_of(store: &Store, obj: Obj) -> String {
    let prop = match store.class_of(obj).ok() {
        Some(c) if c == RELATIONSHIP => relationship::NAME,
        Some(c) if c == FOREIGN_KEY => foreign_key::NAME,
        Some(c) if c == PRIMARY_KEY || c == UNIQUE_CONSTRAINT => key::NAME,
        _ => named_element::NAME,
    };
    store.get(obj, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Whether `name` is free among the children of `parent` named through the shared name property,
/// except for `except`: a lookup in the store's name index, filtered by the container.
fn name_free_under(store: &Store, parent: Obj, name: &str, except: Option<Obj>) -> bool {
    !store
        .by_name(named_element::NAME, name)
        .into_iter()
        .any(|o| Some(o) != except && store.container(o).ok().flatten().map(|(c, _)| c) == Some(parent))
}

/// Whether `name` is free among the entities of `schema`, except for `except`.
pub fn entity_name_free(store: &Store, schema: Obj, name: &str, except: Option<Obj>) -> bool {
    name_free_under(store, schema, name, except)
}

/// Whether `name` is free among the attributes of `entity`, except for `except`.
pub fn attribute_name_free(store: &Store, entity: Obj, name: &str, except: Option<Obj>) -> bool {
    name_free_under(store, entity, name, except)
}

/// The names used twice among `children`, ignoring case, in one pass.
fn duplicate_names(store: &Store, children: &[Obj]) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for c in children {
        let name = name_of(store, *c).to_lowercase();
        if !seen.insert(name.clone()) {
            out.push(name);
        }
    }
    out
}

/// The schema an ER element belongs to.
pub fn schema_of(store: &Store, obj: Obj) -> Option<Obj> {
    let mut cur = obj;
    for _ in 0..5 {
        if store.class_of(cur).ok() == Some(SCHEMA) {
            return Some(cur);
        }
        cur = store.container(cur).ok().flatten()?.0;
    }
    None
}

/// How a sentence names an element: its name, or its id when it has none.
pub fn label(store: &Store, obj: Obj) -> String {
    let name = name_of(store, obj);
    if name.is_empty() { store.uuid(obj).map(|u| u.to_string()).unwrap_or_else(|_| obj.to_string()) } else { name }
}

/// Whether a cardinality allows more than one.
pub fn is_many(c: Cardinality) -> bool {
    matches!(c, Cardinality::ZeroOrMore | Cardinality::OneOrMore)
}

/// A cardinality property's value.
pub fn cardinality_of(store: &Store, r: Obj, prop: archforge_mof::PropId) -> Option<Cardinality> {
    match store.get(r, prop) {
        Ok(Value::Enum(i)) => Cardinality::from_index(i),
        _ => None,
    }
}

/// Whether a relationship has many at both ends.
pub fn is_many_to_many(store: &Store, r: Obj) -> bool {
    cardinality_of(store, r, relationship::FROM_END).is_some_and(is_many) && cardinality_of(store, r, relationship::TO_END).is_some_and(is_many)
}

fn flag(store: &Store, obj: Obj, prop: archforge_mof::PropId) -> bool {
    store.get(obj, prop).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

fn reference(store: &Store, obj: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(obj, prop).ok().and_then(|v| v.as_ref())
}

fn refs(store: &Store, obj: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(obj, prop).unwrap_or_default().iter().filter_map(Value::as_ref).collect()
}

/// An attribute's type, or empty.
pub fn type_of(store: &Store, a: Obj) -> String {
    store.get(a, attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The entity an attribute, key or foreign key belongs to.
pub fn entity_of(store: &Store, a: Obj) -> Option<Obj> {
    store.container(a).ok().flatten().map(|(e, _)| e).filter(|e| store.class_of(*e).ok() == Some(ENTITY))
}

/// Whether `obj` is a key: a primary key or a unique constraint.
pub fn is_key(store: &Store, obj: Obj) -> bool {
    matches!(store.class_of(obj).ok(), Some(c) if c == PRIMARY_KEY || c == UNIQUE_CONSTRAINT)
}

/// An entity's primary key element, when it has one.
pub fn primary_key_of(store: &Store, e: Obj) -> Option<Obj> {
    reference(store, e, entity::PRIMARY_KEY)
}

/// A key's columns, in the key's order.
pub fn key_columns(store: &Store, k: Obj) -> Vec<Obj> {
    refs(store, k, key::COLUMN)
}

/// An entity's primary key columns, in the key's order; empty without a primary key.
pub fn primary_key(store: &Store, e: Obj) -> Vec<Obj> {
    primary_key_of(store, e).map(|k| key_columns(store, k)).unwrap_or_default()
}

/// An entity's unique constraints, in order.
pub fn unique_constraints(store: &Store, e: Obj) -> Vec<Obj> {
    refs(store, e, entity::UNIQUE_CONSTRAINT)
}

/// An entity's keys: its primary key first, then its unique constraints.
pub fn keys_of(store: &Store, e: Obj) -> Vec<Obj> {
    primary_key_of(store, e).into_iter().chain(unique_constraints(store, e)).collect()
}

/// Whether an attribute is part of its entity's primary key.
pub fn in_primary_key(store: &Store, a: Obj) -> bool {
    entity_of(store, a).is_some_and(|e| primary_key(store, e).contains(&a))
}

/// The unique constraints over `a` alone.
pub fn unique_on(store: &Store, a: Obj) -> Vec<Obj> {
    entity_of(store, a).map(|e| unique_constraints(store, e).into_iter().filter(|u| key_columns(store, *u) == [a]).collect()).unwrap_or_default()
}

/// Whether a unique constraint covers `a` alone: the column editor's Unique checkbox.
pub fn is_unique(store: &Store, a: Obj) -> bool {
    !unique_on(store, a).is_empty()
}

/// Whether an attribute may hold no value.
pub fn is_nullable(store: &Store, a: Obj) -> bool {
    flag(store, a, attribute::NULLABLE)
}

/// Whether a unique constraint counts nulls as distinct.
pub fn nulls_distinct(store: &Store, u: Obj) -> bool {
    store.get(u, unique_constraint::NULLS_DISTINCT).ok().and_then(|v| v.as_bool()).unwrap_or(true)
}

/// An entity's kind.
pub fn kind_of(store: &Store, e: Obj) -> EntityKind {
    match store.get(e, entity::KIND) {
        Ok(Value::Enum(i)) => EntityKind::from_index(i).unwrap_or(EntityKind::Regular),
        _ => EntityKind::Regular,
    }
}

/// A referential action property's value.
pub fn action_of(store: &Store, fk: Obj, prop: archforge_mof::PropId) -> ReferentialAction {
    match store.get(fk, prop) {
        Ok(Value::Enum(i)) => ReferentialAction::from_index(i).unwrap_or(ReferentialAction::NoAction),
        _ => ReferentialAction::NoAction,
    }
}

/// A foreign key's match type.
pub fn match_of(store: &Store, fk: Obj) -> MatchType {
    match store.get(fk, foreign_key::MATCH) {
        Ok(Value::Enum(i)) => MatchType::from_index(i).unwrap_or(MatchType::Simple),
        _ => MatchType::Simple,
    }
}

/// A foreign key's deferrability.
pub fn deferrability_of(store: &Store, fk: Obj) -> Deferrability {
    match store.get(fk, foreign_key::DEFERRABLE) {
        Ok(Value::Enum(i)) => Deferrability::from_index(i).unwrap_or(Deferrability::NotDeferrable),
        _ => Deferrability::NotDeferrable,
    }
}

/// What a foreign key consists of.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyParts {
    pub entity: Option<Obj>,
    /// The key it points at.
    pub key: Option<Obj>,
    /// The entity that key belongs to.
    pub references: Option<Obj>,
    /// Its own columns, matched to `targets` by position.
    pub columns: Vec<Obj>,
    /// The key's columns.
    pub targets: Vec<Obj>,
    pub on_delete: ReferentialAction,
    pub on_update: ReferentialAction,
    pub relationship: Option<Obj>,
}

/// A foreign key's parts as the store holds them.
pub fn key_parts(store: &Store, fk: Obj) -> KeyParts {
    let k = reference(store, fk, foreign_key::REFERENCES);
    KeyParts {
        entity: entity_of(store, fk),
        key: k,
        references: k.and_then(|k| entity_of(store, k)),
        columns: refs(store, fk, foreign_key::COLUMN),
        targets: k.map(|k| key_columns(store, k)).unwrap_or_default(),
        on_delete: action_of(store, fk, foreign_key::ON_DELETE),
        on_update: action_of(store, fk, foreign_key::ON_UPDATE),
        relationship: reference(store, fk, foreign_key::RELATIONSHIP),
    }
}

/// An entity's foreign keys, in order.
pub fn foreign_keys(store: &Store, e: Obj) -> Vec<Obj> {
    refs(store, e, entity::FOREIGN_KEY)
}

/// The foreign keys that point at key `k`.
pub fn keys_onto(store: &Store, k: Obj) -> Vec<Obj> {
    store.referrers(k).into_iter().filter(|(_, prop)| *prop == foreign_key::REFERENCES).map(|(fk, _)| fk).collect()
}

/// Why `columns` cannot be a key of `entity`, as a sentence; None when they can. A primary key's
/// columns are never nullable.
pub fn key_columns_problem(store: &Store, entity: Obj, columns: &[Obj], primary: bool) -> Option<String> {
    if columns.is_empty() {
        return Some("a key needs at least one column".into());
    }
    if columns.len() > MAX_KEY_COLUMNS {
        return Some(format!("a key has at most {MAX_KEY_COLUMNS} columns"));
    }
    let mut seen = BTreeSet::new();
    for a in columns {
        if store.class_of(*a).ok() != Some(ATTRIBUTE) || entity_of(store, *a) != Some(entity) {
            return Some(format!("{} is not an attribute of {}", label(store, *a), label(store, entity)));
        }
        if !seen.insert(*a) {
            return Some(format!("{} appears twice in the key", label(store, *a)));
        }
        if primary && is_nullable(store, *a) {
            return Some(format!("{} is nullable, and a primary key column cannot be", label(store, *a)));
        }
    }
    None
}

/// Why a foreign key on `entity` with `columns` pointing at key `k` would break the rules: the
/// parameter it concerns (`columns`, `key`, `on_delete` or `relationship`) and a sentence; None when
/// it keeps them. Columns match the key's by position; types are compared ignoring case, as SQL does.
pub fn key_problem(
    store: &Store,
    entity: Obj,
    k: Obj,
    columns: &[Obj],
    actions: [ReferentialAction; 2],
    relationship: Option<Obj>,
) -> Option<(&'static str, String)> {
    let col = |why: String| Some(("columns", why));
    if !is_key(store, k) {
        return Some(("key", format!("{} is neither a primary key nor a unique constraint", label(store, k))));
    }
    let Some(references) = entity_of(store, k) else { return Some(("key", "the key belongs to no entity".into())) };
    if schema_of(store, entity).is_none() || schema_of(store, entity) != schema_of(store, references) {
        return Some(("key", "the key is in another schema".into()));
    }
    let targets = key_columns(store, k);
    if targets.is_empty() {
        return Some(("key", format!("the key of {} has no columns", label(store, references))));
    }
    if columns.is_empty() {
        return col("a foreign key needs at least one column".into());
    }
    if columns.len() > MAX_KEY_COLUMNS {
        return col(format!("a foreign key has at most {MAX_KEY_COLUMNS} columns"));
    }
    if columns.len() != targets.len() {
        return col(format!("the foreign key has {} columns but the key of {} has {}", columns.len(), label(store, references), targets.len()));
    }
    let mut seen = BTreeSet::new();
    for (a, t) in columns.iter().zip(&targets) {
        if store.class_of(*a).ok() != Some(ATTRIBUTE) || entity_of(store, *a) != Some(entity) {
            return col(format!("{} is not an attribute of {}", label(store, *a), label(store, entity)));
        }
        if !seen.insert(*a) {
            return col(format!("{} appears twice in the foreign key", label(store, *a)));
        }
        if !same_name(&type_of(store, *a), &type_of(store, *t)) {
            return col(format!("{} is {:?} but {} is {:?}", label(store, *a), type_of(store, *a), label(store, *t), type_of(store, *t)));
        }
    }
    if actions.contains(&ReferentialAction::SetNull) && columns.iter().any(|a| !is_nullable(store, *a)) {
        return Some(("on_delete", "set null needs nullable columns".into()));
    }
    if let Some(r) = relationship {
        let ends: BTreeSet<Option<Obj>> = [reference(store, r, relationship::FROM), reference(store, r, relationship::TO)].into_iter().collect();
        let want: BTreeSet<Option<Obj>> = [Some(entity), Some(references)].into_iter().collect();
        if store.class_of(r).ok() != Some(RELATIONSHIP) || ends != want {
            return Some(("relationship", "the relationship does not join these two entities".into()));
        }
        let pk = primary_key(store, entity);
        if flag(store, r, relationship::IDENTIFYING) && columns.iter().any(|a| !pk.contains(a)) {
            return Some(("relationship", "an identifying relationship's key columns must be part of the primary key".into()));
        }
    }
    None
}

/// The named keys and foreign keys of a schema with their names.
fn constraint_names(store: &Store, s: Obj) -> Vec<(Obj, String)> {
    refs(store, s, schema::ENTITY)
        .into_iter()
        .flat_map(|e| keys_of(store, e).into_iter().chain(foreign_keys(store, e)))
        .map(|c| (c, name_of(store, c)))
        .filter(|(_, n)| !n.is_empty())
        .collect()
}

/// Whether `name` is free among the constraint names of `schema` (keys and foreign keys), ignoring
/// case, except for `except`.
pub fn constraint_name_free(store: &Store, s: Obj, name: &str, except: Option<Obj>) -> bool {
    !constraint_names(store, s).into_iter().any(|(c, n)| Some(c) != except && same_name(&n, name))
}

/// Why an entity cannot be associative: it needs at least two foreign keys whose columns are all
/// part of its primary key.
pub fn associative_problem(store: &Store, e: Obj) -> Option<String> {
    let pk = primary_key(store, e);
    let keys = foreign_keys(store, e)
        .into_iter()
        .filter(|fk| {
            let columns = refs(store, *fk, foreign_key::COLUMN);
            !columns.is_empty() && columns.iter().all(|a| pk.contains(a))
        })
        .count();
    if keys < 2 { Some(format!("a mapping table needs two foreign keys within its primary key; {} has {keys}", label(store, e))) } else { None }
}

/// The notice for a many-to-many relationship.
pub const MANY_TO_MANY: &str = "many-to-many: it becomes a table only through a mapping table";

/// What the rules point out without calling it wrong: the many-to-many relationships, each with
/// its notice.
pub fn notices(store: &Store) -> Vec<(Obj, &'static str)> {
    store.alive().filter(|o| store.class_of(*o).ok() == Some(RELATIONSHIP) && is_many_to_many(store, *o)).map(|r| (r, MANY_TO_MANY)).collect()
}

/// What the rules find wrong in a store, each as a sentence.
pub fn problems(store: &Store) -> Vec<String> {
    let mut out = Vec::new();
    for o in store.alive() {
        let Ok(class) = store.class_of(o) else { continue };
        if class == SCHEMA || class == ENTITY || class == ATTRIBUTE {
            let name = name_of(store, o);
            if let Some(p) = name_problem(&name, MAX_NAME) {
                out.push(format!("the name of {o} {p}"));
            }
        }
        if class == SCHEMA {
            let entities: Vec<Obj> = store.list(o, schema::ENTITY).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
            for name in duplicate_names(store, &entities) {
                out.push(format!("the entity name {name:?} is used twice in {o}"));
            }
            let mut seen: BTreeSet<String> = BTreeSet::new();
            for (_, name) in constraint_names(store, o) {
                if !seen.insert(name.to_lowercase()) {
                    out.push(format!("the constraint name {name:?} is used twice in {o}"));
                }
            }
        }
        if class == ENTITY {
            let attributes: Vec<Obj> = store.list(o, entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
            for name in duplicate_names(store, &attributes) {
                out.push(format!("the attribute name {name:?} is used twice in {o}"));
            }
        }
        if class == ATTRIBUTE {
            if in_primary_key(store, o) && is_nullable(store, o) {
                out.push(format!("{o} is a key and nullable"));
            }
            if let Some(t) = store.get(o, attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string))
                && let Some(p) = name_problem(&t, MAX_TYPE)
            {
                out.push(format!("the type of {o} {p}"));
            }
        }
        if class == PRIMARY_KEY || class == UNIQUE_CONSTRAINT {
            match entity_of(store, o) {
                Some(e) => {
                    if let Some(p) = key_columns_problem(store, e, &key_columns(store, o), class == PRIMARY_KEY) {
                        out.push(format!("the key {}: {p}", label(store, o)));
                    }
                }
                None => out.push(format!("the key {} belongs to no entity", label(store, o))),
            }
        }
        if class == ENTITY
            && kind_of(store, o) == EntityKind::Associative
            && let Some(p) = associative_problem(store, o)
        {
            out.push(p);
        }
        if class == FOREIGN_KEY {
            let parts = key_parts(store, o);
            match (parts.entity, parts.key) {
                (Some(e), Some(k)) => {
                    if let Some((_, p)) = key_problem(store, e, k, &parts.columns, [parts.on_delete, parts.on_update], parts.relationship) {
                        out.push(format!("the foreign key {}: {p}", label(store, o)));
                    }
                }
                _ => out.push(format!("the foreign key {} lacks its entity or its key", label(store, o))),
            }
        }
        if class == RELATIONSHIP {
            let ends = [store.get(o, relationship::FROM), store.get(o, relationship::TO)];
            let here = schema_of(store, o);
            for end in ends {
                match end.ok().and_then(|v| v.as_ref()) {
                    Some(e) if schema_of(store, e) == here && here.is_some() => {}
                    Some(e) => out.push(format!("{o} reaches {e} outside its schema")),
                    None => out.push(format!("{o} lacks an end")),
                }
            }
        }
    }
    out
}
