//! The problems the shell shows (slice E4 of archforge-gm2.10, from the design's cards "ER ·
//! Canvas · Problems and a note" and "ER · Properties · Problems"): a table without a primary key,
//! a many-to-many relationship still waiting for its mapping table, a foreign key the rules
//! refuse, a key column that may be null, a key whose columns the rules refuse. Each is named by
//! the element it sits on, so the canvas can badge the node or line and Properties can list and
//! select it. The engine's `rules::problems` stays the rule of record for the whole store; this
//! list is the user's view of it, in the user's words.

use std::collections::BTreeMap;

use archforge_mof::{Obj, Store, Value};

use crate::i18n::{fmt, tr};

/// One problem, on the element it sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub element: Obj,
    pub text: String,
}

fn name(store: &Store, o: Obj) -> String {
    archforge_er::rules::name_of(store, o)
}

fn members(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|m| store.is_alive(*m)).collect()
}

/// Every problem of the store, on the element the user would fix it on: a column's or a key's on
/// its table, a relationship's on the relationship.
pub fn all(store: &Store) -> Vec<Problem> {
    use archforge_er::rules as r;
    let mut out = Vec::new();
    for o in store.alive() {
        let Ok(class) = store.class_of(o) else { continue };
        if class == archforge_er::ENTITY {
            if r::primary_key_of(store, o).is_none() {
                out.push(Problem { element: o, text: fmt(&tr("{name} has no primary key"), &[("name", &name(store, o))]) });
            }
            for a in members(store, o, archforge_er::entity::ATTRIBUTE) {
                if r::in_primary_key(store, a) && r::is_nullable(store, a) {
                    out.push(Problem { element: o, text: fmt(&tr("{column} is a key column and may be null"), &[("column", &name(store, a))]) });
                }
            }
            for k in r::primary_key_of(store, o).into_iter().chain(r::unique_constraints(store, o)) {
                let primary = store.class_of(k).ok() == Some(archforge_er::PRIMARY_KEY);
                if let Some(p) = r::key_columns_problem(store, o, &r::key_columns(store, k), primary) {
                    out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::label(store, k)), ("problem", &p)]) });
                }
            }
            for c in r::checks_of(store, o) {
                if let Some(p) = r::check_problem(&r::text_of(store, c, archforge_er::check_constraint::EXPRESSION)) {
                    out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::display_name(store, c)), ("problem", &p)]) });
                }
            }
            for ix in r::indexes_of(store, o) {
                if let Some(p) = r::index_columns_problem(store, o, &r::index_specs(store, ix)) {
                    out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::display_name(store, ix)), ("problem", &p)]) });
                }
            }
            for fk in members(store, o, archforge_er::entity::FOREIGN_KEY) {
                let parts = r::key_parts(store, fk);
                let text = match (parts.entity, parts.key) {
                    (Some(e), Some(k)) => {
                        r::key_problem(store, e, k, &parts.columns, [parts.on_delete, parts.on_update], parts.relationship).map(|(_, p)| p)
                    }
                    _ => Some(tr("lacks its table or its key")),
                };
                if let Some(p) = text {
                    out.push(Problem { element: o, text: fmt(&tr("{key}: {problem}"), &[("key", &r::label(store, fk)), ("problem", &p)]) });
                }
            }
        }
        if class == archforge_er::RELATIONSHIP && r::is_many_to_many(store, o) {
            let end = |p| store.get(o, p).ok().and_then(|v| v.as_ref()).map(|e| name(store, e)).unwrap_or_default();
            out.push(Problem {
                element: o,
                text: fmt(
                    &tr("{from} to {to} is many to many"),
                    &[("from", &end(archforge_er::relationship::FROM)), ("to", &end(archforge_er::relationship::TO))],
                ),
            });
        }
    }
    out
}

/// The problems by the element they sit on.
pub fn by_element(store: &Store) -> BTreeMap<Obj, Vec<String>> {
    let mut out: BTreeMap<Obj, Vec<String>> = BTreeMap::new();
    for p in all(store) {
        out.entry(p.element).or_default().push(p.text);
    }
    out
}

/// The problems a selection shows, from the frame's map: its own, or for a schema or a diagram
/// those of every table and relationship of the schema, each with the element a click selects.
pub fn of(store: &Store, map: &BTreeMap<Obj, Vec<String>>, subject: Obj) -> Vec<Problem> {
    let class = store.class_of(subject).ok();
    let schema = if class == Some(archforge_diagram::DIAGRAM) {
        store.get(subject, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref())
    } else if class == Some(archforge_er::SCHEMA) {
        Some(subject)
    } else {
        None
    };
    let root = if class == Some(archforge_er::ATTRIBUTE) { archforge_er::rules::entity_of(store, subject).unwrap_or(subject) } else { subject };
    map.iter()
        .filter(|(element, _)| match schema {
            Some(s) => archforge_er::rules::schema_of(store, **element) == Some(s),
            None => **element == root,
        })
        .flat_map(|(element, texts)| texts.iter().map(move |text| Problem { element: *element, text: text.clone() }))
        .collect()
}
