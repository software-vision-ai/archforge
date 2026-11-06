//! What ER draws: a table as a card with its columns, a relationship as a line
//! with crow's foot ends, its cardinalities and roles, dashed when it does not identify its child,
//! and a generalization as a slate line to a hollow triangle at the supertype, with the set's facts
//! beside the first subtype's triangle. The canvas draws these descriptions.

use archforge_diagram::figure::{Card, End, EndMark, Line, Row, Text};
use archforge_mof::{Obj, Store, Value};

use crate::rules::{self, name_of};
use crate::{Cardinality, ENTITY, GENERALIZATION, RELATIONSHIP, entity, foreign_key, relationship};

/// A cardinality as the design writes it beside an end.
pub fn cardinality_text(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ZeroOrOne => "0..1",
        Cardinality::ExactlyOne => "1",
        Cardinality::ZeroOrMore => "0..*",
        Cardinality::OneOrMore => "1..*",
    }
}

/// The crow's foot mark of a cardinality.
pub fn end_mark(c: Option<Cardinality>) -> EndMark {
    match c {
        Some(Cardinality::ExactlyOne) => EndMark::ExactlyOne,
        Some(Cardinality::ZeroOrOne) => EndMark::ZeroOrOne,
        Some(Cardinality::ZeroOrMore) => EndMark::ZeroOrMore,
        Some(Cardinality::OneOrMore) => EndMark::OneOrMore,
        None => EndMark::None,
    }
}

/// Whether a foreign key carries the relationship.
pub fn has_foreign_key(store: &Store, rel: Obj) -> bool {
    store.referrers(rel).into_iter().any(|(_, p)| p == foreign_key::RELATIONSHIP)
}

fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

/// The mark of a column: PK when it is in the primary key, FK when a foreign key holds it.
fn mark(store: &Store, table: Obj, column: Obj) -> Option<&'static str> {
    if rules::in_primary_key(store, column) {
        return Some("PK");
    }
    let foreign = refs(store, table, entity::FOREIGN_KEY).iter().any(|fk| refs(store, *fk, foreign_key::COLUMN).contains(&column));
    foreign.then_some("FK")
}

/// A table's card on `diagram`: its columns with their types and marks; a table of another schema
/// than the diagram's is drawn dimmed, its schema named in the eyebrow (slice S7).
pub fn card(store: &Store, table: Obj, diagram: Obj) -> Option<Card> {
    if store.class_of(table).ok()? != ENTITY {
        return None;
    }
    let here = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o));
    let elsewhere = rules::schema_of(store, table).filter(|s| here.is_some_and(|h| h != *s));
    let eyebrow = match elsewhere {
        Some(s) => Text::Said("Entity · {schema}", vec![("schema", name_of(store, s))]),
        None => Text::said("Entity"),
    };
    let rows = refs(store, table, entity::ATTRIBUTE)
        .into_iter()
        .map(|column| {
            let ty = rules::type_display(store, column);
            let name = name_of(store, column);
            Row {
                mark: mark(store, table, column),
                text: if ty.is_empty() { name } else { format!("{name} : {ty}") },
                linked: rules::linked_to(store, column).is_some(),
                slanted: false,
                underlined: false,
            }
        })
        .collect();
    Some(Card {
        eyebrow,
        name: name_of(store, table),
        compartments: vec![rows],
        more: ("+ {n} more column", "+ {n} more columns"),
        hidden: 0,
        elsewhere: elsewhere.is_some(),
        tone: archforge_diagram::figure::Tone::Tinted,
        slanted: false,
        braces: None,
    })
}

/// A relationship's or a generalization's line.
pub fn line(store: &Store, element: Obj) -> Option<Line> {
    match store.class_of(element).ok()? {
        c if c == RELATIONSHIP => {
            let end = |cardinality, slot, role| {
                let c = rules::cardinality_of(store, element, cardinality);
                let role = rules::role_of(store, element, role);
                End { mark: end_mark(c), text: c.map(|c| cardinality_text(c).to_string()), slot, role: (!role.is_empty()).then_some(role) }
            };
            let many_to_many = rules::is_many_to_many(store, element);
            Some(Line {
                ends: [end(relationship::FROM_END, "from_end", relationship::FROM_ROLE), end(relationship::TO_END, "to_end", relationship::TO_ROLE)],
                styled: true,
                dashed: !matches!(store.get(element, relationship::IDENTIFYING), Ok(Value::Bool(true))),
                status: Some(has_foreign_key(store, element) && !many_to_many),
                note: None,
                badge: true,
                resolvable: many_to_many,
            })
        }
        c if c == GENERALIZATION => {
            // The set's facts, once per supertype, beside its first subtype's triangle.
            let supertype = rules::generalization_parts(store, element).0;
            let note = supertype.filter(|s| rules::subtype_links(store, *s).first() == Some(&element)).and_then(|s| set_note(store, s));
            Some(Line {
                ends: [End::marked(EndMark::None), End::marked(EndMark::Triangle)],
                styled: false,
                dashed: false,
                status: None,
                note,
                badge: false,
                resolvable: false,
            })
        }
        _ => None,
    }
}

/// What a supertype's set says on the canvas: complete, disjoint or overlapping, and the
/// discriminator, when any of them says more than the defaults.
pub fn set_note(store: &Store, supertype: Obj) -> Option<Vec<Text>> {
    let (complete, disjoint) = rules::subtyping_flags(store, supertype);
    let discriminator = rules::discriminator_of(store, supertype).map(|a| name_of(store, a));
    if !complete && disjoint && discriminator.is_none() {
        return None;
    }
    let mut parts = Vec::new();
    if complete {
        parts.push(Text::said("complete"));
    }
    parts.push(Text::said(if disjoint { "disjoint" } else { "overlapping" }));
    parts.extend(discriminator.map(Text::Plain));
    Some(parts)
}

/// The text of a relationship's slot a label shows.
pub fn slot_text(store: &Store, element: Obj, slot: &str) -> String {
    if store.class_of(element).ok() != Some(RELATIONSHIP) {
        return String::new();
    }
    let cardinality = |prop| rules::cardinality_of(store, element, prop).map(|c| cardinality_text(c).to_string()).unwrap_or_default();
    match slot {
        "name" => name_of(store, element),
        "from_end" => cardinality(relationship::FROM_END),
        "to_end" => cardinality(relationship::TO_END),
        "from_role" => rules::role_of(store, element, relationship::FROM_ROLE),
        "to_role" => rules::role_of(store, element, relationship::TO_ROLE),
        _ => String::new(),
    }
}

/// The words the descriptions say, for the shell's catalogue: the texts, and the plural.
pub const WORDS: &[&str] = &["Entity", "Entity · {schema}", "complete", "disjoint", "overlapping"];
pub const PLURALS: &[&str] = &["+ {n} more column"];
