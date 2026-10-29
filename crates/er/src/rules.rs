//! The ER rules the MOF core cannot know, checked by the commands before they change anything and
//! available over a whole store.

use std::collections::BTreeSet;

use archforge_mof::{Obj, Store, Value};

use crate::{ATTRIBUTE, ENTITY, RELATIONSHIP, SCHEMA, attribute, entity, named_element, relationship, schema};

/// The most characters in a name, and in an attribute's type.
pub const MAX_NAME: usize = 128;
pub const MAX_TYPE: usize = 64;

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
    let prop = if store.class_of(obj).ok() == Some(RELATIONSHIP) { relationship::NAME } else { named_element::NAME };
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
