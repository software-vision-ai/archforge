//! The ER model: entities with their attributes, relationships between entities, and diagrams that
//! place entities. One element is one element wherever it is shown. `check` holds the invariants
//! every command keeps and every load verifies.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// An element's id: `e<n>`, `a<n>`, `r<n>` or `d<n>`.
pub type Id = String;

/// The caps a model keeps to.
pub const MAX_ENTITIES: usize = 10_000;
pub const MAX_ATTRIBUTES: usize = 100_000;
pub const MAX_ATTRIBUTES_PER_ENTITY: usize = 200;
pub const MAX_RELATIONSHIPS: usize = 50_000;
pub const MAX_DIAGRAMS: usize = 500;
pub const MAX_PLACES: usize = 100_000;
/// The longest name and kind, in characters.
pub const MAX_NAME: usize = 128;
pub const MAX_KIND: usize = 64;
/// Where a box may be and how big.
pub const MAX_COORD: f64 = 1_000_000.0;
pub const MIN_SIZE: f64 = 20.0;
pub const MAX_SIZE: f64 = 10_000.0;

/// The kinds of element and their id prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Entity,
    Attribute,
    Relationship,
    Diagram,
}

impl Kind {
    pub fn prefix(self) -> char {
        match self {
            Kind::Entity => 'e',
            Kind::Attribute => 'a',
            Kind::Relationship => 'r',
            Kind::Diagram => 'd',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Entity => "an entity",
            Kind::Attribute => "an attribute",
            Kind::Relationship => "a relationship",
            Kind::Diagram => "a diagram",
        }
    }
}

/// The id of a new element of `kind` numbered `n`.
pub fn make_id(kind: Kind, n: u64) -> Id {
    format!("{}{n}", kind.prefix())
}

/// The kind and number of a canonical id (no leading zero, no sign), or None.
pub fn parse_id(id: &str) -> Option<(Kind, u64)> {
    let mut chars = id.chars();
    let kind = match chars.next()? {
        'e' => Kind::Entity,
        'a' => Kind::Attribute,
        'r' => Kind::Relationship,
        'd' => Kind::Diagram,
        _ => return None,
    };
    let digits = chars.as_str();
    if digits.is_empty() || digits.len() > 20 || !digits.bytes().all(|b| b.is_ascii_digit()) || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    Some((kind, digits.parse().ok()?))
}

/// How many of one end take part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMore,
    OneOrMore,
}

impl Cardinality {
    pub const CHOICES: &'static str = "zero-or-one|exactly-one|zero-or-more|one-or-more";

    pub fn as_str(self) -> &'static str {
        match self {
            Cardinality::ZeroOrOne => "zero-or-one",
            Cardinality::ExactlyOne => "exactly-one",
            Cardinality::ZeroOrMore => "zero-or-more",
            Cardinality::OneOrMore => "one-or-more",
        }
    }

    pub fn parse(s: &str) -> Option<Cardinality> {
        Some(match s {
            "zero-or-one" => Cardinality::ZeroOrOne,
            "exactly-one" => Cardinality::ExactlyOne,
            "zero-or-more" => Cardinality::ZeroOrMore,
            "one-or-more" => Cardinality::OneOrMore,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub id: Id,
    pub name: String,
    /// The column type as written, such as `VARCHAR(64)`.
    pub kind: String,
    pub primary_key: bool,
    pub nullable: bool,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: Id,
    pub name: String,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    pub id: Id,
    pub name: Option<String>,
    pub from: Id,
    pub to: Id,
    pub from_end: Cardinality,
    pub to_end: Cardinality,
}

/// Where an entity's box is on a diagram, and how big.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagram {
    pub id: Id,
    pub name: String,
    pub places: BTreeMap<Id, Place>,
}

/// The model: what undo covers and the project stores.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ErModel {
    pub entities: BTreeMap<Id, Entity>,
    pub relationships: BTreeMap<Id, Relationship>,
    pub diagrams: BTreeMap<Id, Diagram>,
}

/// Which invariant a model breaks, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(pub String);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a name cannot be used, if it cannot: 1 to `max` characters without control characters.
pub fn name_problem(name: &str, max: usize) -> Option<String> {
    let n = name.chars().count();
    if n == 0 {
        Some("is empty".into())
    } else if n > max {
        Some(format!("is longer than {max} characters"))
    } else if name.chars().any(char::is_control) {
        Some("holds a control character".into())
    } else {
        None
    }
}

/// Names compare without regard to case, by Unicode lower case.
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

pub fn coord_ok(v: f64) -> bool {
    v.is_finite() && (-MAX_COORD..=MAX_COORD).contains(&v)
}

pub fn size_ok(v: f64) -> bool {
    v.is_finite() && (MIN_SIZE..=MAX_SIZE).contains(&v)
}

impl ErModel {
    /// Every invariant, given the counter new ids come from.
    pub fn check(&self, counter: u64) -> Result<(), Invalid> {
        let bad = |s: String| Err(Invalid(s));
        if self.entities.len() > MAX_ENTITIES || self.relationships.len() > MAX_RELATIONSHIPS || self.diagrams.len() > MAX_DIAGRAMS {
            return bad("the model is over its caps".into());
        }
        let mut ids = BTreeSet::new();
        let mut take = |id: &str, kind: Kind| -> Result<(), Invalid> {
            match parse_id(id) {
                Some((k, n)) if k == kind && n < counter => {}
                _ => return Err(Invalid(format!("{id:?} is not {} id below {counter}", kind.name()))),
            }
            if !ids.insert(id.to_string()) {
                return Err(Invalid(format!("the id {id} is used twice")));
            }
            Ok(())
        };
        let mut attributes = 0usize;
        let mut names: BTreeSet<String> = BTreeSet::new();
        for (key, e) in &self.entities {
            if *key != e.id {
                return bad(format!("the entity {} is filed as {key}", e.id));
            }
            take(&e.id, Kind::Entity)?;
            if let Some(p) = name_problem(&e.name, MAX_NAME) {
                return bad(format!("the name of {} {p}", e.id));
            }
            if !names.insert(e.name.to_lowercase()) {
                return bad(format!("the entity name {:?} is used twice", e.name));
            }
            if e.attributes.len() > MAX_ATTRIBUTES_PER_ENTITY {
                return bad(format!("{} has more than {MAX_ATTRIBUTES_PER_ENTITY} attributes", e.id));
            }
            attributes = attributes.saturating_add(e.attributes.len());
            let mut attr_names: BTreeSet<String> = BTreeSet::new();
            for a in &e.attributes {
                take(&a.id, Kind::Attribute)?;
                if let Some(p) = name_problem(&a.name, MAX_NAME) {
                    return bad(format!("the name of {} {p}", a.id));
                }
                if let Some(p) = name_problem(&a.kind, MAX_KIND) {
                    return bad(format!("the kind of {} {p}", a.id));
                }
                if !attr_names.insert(a.name.to_lowercase()) {
                    return bad(format!("the attribute name {:?} is used twice in {}", a.name, e.id));
                }
                if a.primary_key && a.nullable {
                    return bad(format!("{} is a key and nullable", a.id));
                }
            }
        }
        if attributes > MAX_ATTRIBUTES {
            return bad(format!("the model has more than {MAX_ATTRIBUTES} attributes"));
        }
        for (key, r) in &self.relationships {
            if *key != r.id {
                return bad(format!("the relationship {} is filed as {key}", r.id));
            }
            take(&r.id, Kind::Relationship)?;
            if let Some(name) = &r.name
                && let Some(p) = name_problem(name, MAX_NAME)
            {
                return bad(format!("the name of {} {p}", r.id));
            }
            for end in [&r.from, &r.to] {
                if !self.entities.contains_key(end) {
                    return bad(format!("{} names the missing entity {end}", r.id));
                }
            }
        }
        let mut places = 0usize;
        let mut diagram_names: BTreeSet<String> = BTreeSet::new();
        for (key, d) in &self.diagrams {
            if *key != d.id {
                return bad(format!("the diagram {} is filed as {key}", d.id));
            }
            take(&d.id, Kind::Diagram)?;
            if let Some(p) = name_problem(&d.name, MAX_NAME) {
                return bad(format!("the name of {} {p}", d.id));
            }
            if !diagram_names.insert(d.name.to_lowercase()) {
                return bad(format!("the diagram name {:?} is used twice", d.name));
            }
            places = places.saturating_add(d.places.len());
            for (entity, p) in &d.places {
                if !self.entities.contains_key(entity) {
                    return bad(format!("{} places the missing entity {entity}", d.id));
                }
                if !(coord_ok(p.x) && coord_ok(p.y) && size_ok(p.width) && size_ok(p.height)) {
                    return bad(format!("{} places {entity} out of range", d.id));
                }
            }
        }
        if places > MAX_PLACES {
            return bad(format!("the model has more than {MAX_PLACES} places"));
        }
        Ok(())
    }

    /// The total number of attributes and of places, for the caps.
    pub fn counts(&self) -> (usize, usize) {
        (self.entities.values().map(|e| e.attributes.len()).sum(), self.diagrams.values().map(|d| d.places.len()).sum())
    }

    /// The entity holding the attribute `id`, and its position.
    pub fn attribute(&self, id: &str) -> Option<(&Entity, usize)> {
        self.entities.values().find_map(|e| e.attributes.iter().position(|a| a.id == id).map(|i| (e, i)))
    }

    /// Every attribute id, for checking many ids in one pass instead of a scan per id.
    pub fn attribute_ids(&self) -> BTreeSet<&str> {
        self.entities.values().flat_map(|e| e.attributes.iter().map(|a| a.id.as_str())).collect()
    }

    /// Whether `id` names an element of the model.
    pub fn contains(&self, id: &str) -> bool {
        self.contains_with(id, None)
    }

    /// [`ErModel::contains`] with the attribute ids gathered once, for a list of ids.
    pub fn contains_with(&self, id: &str, attributes: Option<&BTreeSet<&str>>) -> bool {
        match parse_id(id).map(|(k, _)| k) {
            Some(Kind::Entity) => self.entities.contains_key(id),
            Some(Kind::Attribute) => attributes.map_or_else(|| self.attribute(id).is_some(), |a| a.contains(id)),
            Some(Kind::Relationship) => self.relationships.contains_key(id),
            Some(Kind::Diagram) => self.diagrams.contains_key(id),
            None => false,
        }
    }
}

impl archforge_engine::Snapshot for ErModel {
    fn weight(&self) -> usize {
        let s = |x: &str| x.len().saturating_add(48);
        let entities: usize =
            self.entities.values().map(|e| s(&e.name).saturating_add(e.attributes.iter().map(|a| s(&a.name).saturating_add(s(&a.kind))).sum())).sum();
        let relationships: usize = self.relationships.values().map(|r| s(r.name.as_deref().unwrap_or("")).saturating_add(96)).sum();
        let diagrams: usize = self.diagrams.values().map(|d| s(&d.name).saturating_add(d.places.len().saturating_mul(80))).sum();
        entities.saturating_add(relationships).saturating_add(diagrams).saturating_add(64)
    }
}

/// What the ER notation's view holds: the open diagram and the selection. Never undone.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ErView {
    pub diagram: Option<Id>,
    pub selection: Vec<Id>,
}

impl archforge_engine::View<ErModel> for ErView {
    fn retain_valid(&mut self, doc: &ErModel) {
        if self.diagram.as_deref().is_some_and(|d| !doc.diagrams.contains_key(d)) {
            self.diagram = None;
        }
        let attributes = self.selection.iter().any(|id| id.starts_with('a')).then(|| doc.attribute_ids());
        self.selection.retain(|id| doc.contains_with(id, attributes.as_ref()));
    }
}
