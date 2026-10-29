//! The ER rules the MOF core cannot know, checked by the commands before they change anything and
//! available over a whole store.

use std::collections::BTreeSet;

use archforge_mof::{Obj, Store, Value};

use crate::{
    ATTRIBUTE, Cardinality, ENTITY, EntityKind, FOREIGN_KEY, RELATIONSHIP, ReferentialAction, SCHEMA, attribute, entity, foreign_key,
    foreign_key_column, named_element, relationship, schema,
};

/// The most characters in a name, and in an attribute's type.
pub const MAX_NAME: usize = 128;
pub const MAX_TYPE: usize = 64;
/// The most columns in one foreign key.
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

/// The schema an entity, attribute or relationship belongs to.
pub fn schema_of(store: &Store, obj: Obj) -> Option<Obj> {
    let mut cur = obj;
    for _ in 0..4 {
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

/// The entity an attribute belongs to.
pub fn entity_of(store: &Store, a: Obj) -> Option<Obj> {
    store.container(a).ok().flatten().map(|(e, _)| e).filter(|e| store.class_of(*e).ok() == Some(ENTITY))
}

/// An entity's primary key: its key attributes, in order.
pub fn primary_key(store: &Store, e: Obj) -> Vec<Obj> {
    refs(store, e, entity::ATTRIBUTE).into_iter().filter(|a| flag(store, *a, attribute::PRIMARY_KEY)).collect()
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

/// What a foreign key consists of.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyParts {
    pub entity: Option<Obj>,
    pub references: Option<Obj>,
    /// (attribute of the entity, target attribute of the referenced entity)
    pub pairs: Vec<(Obj, Obj)>,
    pub on_delete: ReferentialAction,
    pub on_update: ReferentialAction,
    pub relationship: Option<Obj>,
    /// Columns missing an end, left out of `pairs`.
    pub broken: usize,
}

/// A foreign key's parts as the store holds them; a column missing an end is counted in `broken`.
pub fn key_parts(store: &Store, fk: Obj) -> KeyParts {
    let columns = refs(store, fk, foreign_key::COLUMN);
    let pairs: Vec<(Obj, Obj)> = columns
        .iter()
        .filter_map(|c| Some((reference(store, *c, foreign_key_column::ATTRIBUTE)?, reference(store, *c, foreign_key_column::TARGET)?)))
        .collect();
    let broken = columns.len() - pairs.len();
    KeyParts {
        entity: store.container(fk).ok().flatten().map(|(e, _)| e),
        references: reference(store, fk, foreign_key::REFERENCES),
        pairs,
        on_delete: action_of(store, fk, foreign_key::ON_DELETE),
        on_update: action_of(store, fk, foreign_key::ON_UPDATE),
        relationship: reference(store, fk, foreign_key::RELATIONSHIP),
        broken,
    }
}

/// An entity's foreign keys, in order.
pub fn foreign_keys(store: &Store, e: Obj) -> Vec<Obj> {
    refs(store, e, entity::FOREIGN_KEY)
}

/// Why a foreign key on `entity` referencing `references` through `pairs` would break the rules:
/// the parameter it concerns (`columns`, `references`, `on_delete` or `relationship`) and a
/// sentence; None when it keeps them. Types are compared ignoring case, as SQL does.
pub fn key_problem(
    store: &Store,
    entity: Obj,
    references: Obj,
    pairs: &[(Obj, Obj)],
    actions: [ReferentialAction; 2],
    relationship: Option<Obj>,
) -> Option<(&'static str, String)> {
    let columns = |why: String| Some(("columns", why));
    if pairs.is_empty() {
        return columns("a foreign key needs at least one column".into());
    }
    if pairs.len() > MAX_KEY_COLUMNS {
        return columns(format!("a foreign key has at most {MAX_KEY_COLUMNS} columns"));
    }
    if schema_of(store, entity).is_none() || schema_of(store, entity) != schema_of(store, references) {
        return Some(("references", "the referenced entity is in another schema".into()));
    }
    let mut seen = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for (a, t) in pairs {
        if store.class_of(*a).ok() != Some(ATTRIBUTE) || entity_of(store, *a) != Some(entity) {
            return columns(format!("{} is not an attribute of {}", label(store, *a), label(store, entity)));
        }
        if store.class_of(*t).ok() != Some(ATTRIBUTE) || entity_of(store, *t) != Some(references) {
            return columns(format!("{} is not an attribute of {}", label(store, *t), label(store, references)));
        }
        if !seen.insert(*a) {
            return columns(format!("{} appears twice in the foreign key", label(store, *a)));
        }
        if !targets.insert(*t) {
            return columns(format!("{} is targeted twice", label(store, *t)));
        }
        if !same_name(&type_of(store, *a), &type_of(store, *t)) {
            return columns(format!("{} is {:?} but {} is {:?}", label(store, *a), type_of(store, *a), label(store, *t), type_of(store, *t)));
        }
    }
    let key: BTreeSet<Obj> = primary_key(store, references).into_iter().collect();
    let single_unique = pairs.len() == 1 && pairs.first().is_some_and(|(_, t)| flag(store, *t, attribute::UNIQUE));
    if !(single_unique || (!key.is_empty() && key == targets)) {
        return columns(format!("the targets are neither the primary key of {} nor one unique attribute", label(store, references)));
    }
    if actions.contains(&ReferentialAction::SetNull) && pairs.iter().any(|(a, _)| !flag(store, *a, attribute::NULLABLE)) {
        return Some(("on_delete", "set null needs nullable columns".into()));
    }
    if let Some(r) = relationship {
        let ends: BTreeSet<Option<Obj>> = [reference(store, r, relationship::FROM), reference(store, r, relationship::TO)].into_iter().collect();
        let want: BTreeSet<Option<Obj>> = [Some(entity), Some(references)].into_iter().collect();
        if store.class_of(r).ok() != Some(RELATIONSHIP) || ends != want {
            return Some(("relationship", "the relationship does not join these two entities".into()));
        }
        if flag(store, r, relationship::IDENTIFYING) && pairs.iter().any(|(a, _)| !flag(store, *a, attribute::PRIMARY_KEY)) {
            return Some(("relationship", "an identifying relationship's key columns must be part of the primary key".into()));
        }
    }
    None
}

/// Why an entity cannot be associative: it needs at least two foreign keys whose columns are all
/// part of its primary key.
pub fn associative_problem(store: &Store, e: Obj) -> Option<String> {
    let keys = foreign_keys(store, e)
        .into_iter()
        .filter(|fk| {
            let parts = key_parts(store, *fk);
            !parts.pairs.is_empty() && parts.pairs.iter().all(|(a, _)| flag(store, *a, attribute::PRIMARY_KEY))
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
        }
        if class == ENTITY {
            let attributes: Vec<Obj> = store.list(o, entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
            for name in duplicate_names(store, &attributes) {
                out.push(format!("the attribute name {name:?} is used twice in {o}"));
            }
        }
        if class == ATTRIBUTE {
            let get = |p| store.get(o, p).ok().and_then(|v| v.as_bool()).unwrap_or(false);
            if get(attribute::PRIMARY_KEY) && get(attribute::NULLABLE) {
                out.push(format!("{o} is a key and nullable"));
            }
            if let Some(t) = store.get(o, attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string))
                && let Some(p) = name_problem(&t, MAX_TYPE)
            {
                out.push(format!("the type of {o} {p}"));
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
            if parts.broken > 0 {
                out.push(format!("the foreign key {} has a column missing an end", label(store, o)));
            }
            match (parts.entity, parts.references) {
                (Some(e), Some(r)) => {
                    if let Some((_, p)) = key_problem(store, e, r, &parts.pairs, [parts.on_delete, parts.on_update], parts.relationship) {
                        out.push(format!("the foreign key {}: {p}", label(store, o)));
                    }
                }
                _ => out.push(format!("the foreign key {} lacks its entity or its reference", label(store, o))),
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
