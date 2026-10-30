//! The ER rules the MOF core cannot know, checked by the commands before they change anything and
//! available over a whole store.

use std::collections::BTreeSet;

use archforge_mof::{Obj, Store, Value};

use crate::{
    ATTRIBUTE, CHECK_CONSTRAINT, Cardinality, DOMAIN, Deferrability, ENTITY, ENUM_TYPE, EntityKind, FOREIGN_KEY, GENERALIZATION, Generation,
    MatchType, NullsOrder, PRIMARY_KEY, RELATIONSHIP, ReferentialAction, SCHEMA, SortDirection, SubtypeMapping, TABLE_INDEX, UNIQUE_CONSTRAINT,
    attribute, check_constraint, domain, domain_check, entity, enum_type, foreign_key, generalization, index_column, key, named_element,
    relationship, schema, table_index, unique_constraint,
};

/// The most literals of an enum type.
pub const MAX_LITERALS: usize = 256;

/// The most characters in a name, and in an attribute's type.
pub const MAX_NAME: usize = 128;
pub const MAX_TYPE: usize = 64;
/// The most characters a default, an expression or a description may have.
pub const MAX_TEXT: usize = 4096;
/// The most columns in one key or foreign key.
pub const MAX_KEY_COLUMNS: usize = 32;

/// Why a text is not acceptable, or none: an empty text is nothing and always fine.
pub fn text_problem(text: &str, max: usize) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    if text.chars().count() > max {
        return Some(format!("is longer than {max} characters"));
    }
    if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Some("holds a control character".into());
    }
    if text.trim() != text {
        return Some("starts or ends with a space".into());
    }
    None
}

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
        Some(c) if c == CHECK_CONSTRAINT => check_constraint::NAME,
        Some(c) if c == TABLE_INDEX => table_index::NAME,
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

/// The role at one end of a relationship (`relationship::FROM_ROLE` or `TO_ROLE`), or empty.
pub fn role_of(store: &Store, r: Obj, prop: archforge_mof::PropId) -> String {
    store.get(r, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// What tells a relationship apart from others between the same two entities: each end's entity
/// with its role, in a fixed order.
fn ends_with_roles(store: &Store, r: Obj) -> Option<[(Obj, String); 2]> {
    let from = (reference(store, r, relationship::FROM)?, role_of(store, r, relationship::FROM_ROLE).to_lowercase());
    let to = (reference(store, r, relationship::TO)?, role_of(store, r, relationship::TO_ROLE).to_lowercase());
    Some(if from <= to { [from, to] } else { [to, from] })
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
    if let Some(d) = domain_of(store, a) {
        return text_of(store, d, domain::BASE_TYPE);
    }
    if let Some(e) = enum_of(store, a) {
        return name_of(store, e);
    }
    own_type_of(store, a)
}

/// A column's own type name, empty when it takes a domain or an enum type.
pub fn own_type_of(store: &Store, a: Obj) -> String {
    store.get(a, attribute::TYPE).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

// Subtypes (slice S6).

/// A generalization's supertype, subtype and key relationship, when alive.
pub fn generalization_parts(store: &Store, g: Obj) -> (Option<Obj>, Option<Obj>, Option<Obj>) {
    let live = |p| reference(store, g, p).filter(|o| store.is_alive(*o));
    (live(generalization::SUPERTYPE), live(generalization::SUBTYPE), live(generalization::KEY_RELATIONSHIP))
}

/// The generalizations whose subtype is `e`.
pub fn supertype_links(store: &Store, e: Obj) -> Vec<Obj> {
    store.referrers(e).into_iter().filter(|(g, p)| *p == generalization::SUBTYPE && store.is_alive(*g)).map(|(g, _)| g).collect()
}

/// The generalizations whose supertype is `e`, in the schema's order.
pub fn subtype_links(store: &Store, e: Obj) -> Vec<Obj> {
    let Some(s) = schema_of(store, e) else { return Vec::new() };
    refs(store, s, schema::GENERALIZATION).into_iter().filter(|g| store.is_alive(*g) && generalization_parts(store, *g).0 == Some(e)).collect()
}

/// `e`'s supertype, when it has one.
pub fn supertype_of(store: &Store, e: Obj) -> Option<Obj> {
    supertype_links(store, e).into_iter().find_map(|g| generalization_parts(store, g).0)
}

/// Whether `a` is `b` or one of `b`'s supertypes, up the chain with a seen-set.
pub fn is_supertype_or_self(store: &Store, a: Obj, b: Obj) -> bool {
    let mut seen: BTreeSet<Obj> = BTreeSet::new();
    let mut cur = Some(b);
    while let Some(c) = cur {
        if c == a {
            return true;
        }
        if !seen.insert(c) {
            return false;
        }
        cur = supertype_of(store, c);
    }
    false
}

/// How a supertype's subtypes become tables.
pub fn mapping_of(store: &Store, e: Obj) -> SubtypeMapping {
    match store.get(e, entity::SUBTYPE_MAPPING) {
        Ok(Value::Enum(i)) => SubtypeMapping::from_index(i).unwrap_or(SubtypeMapping::TablePerType),
        _ => SubtypeMapping::TablePerType,
    }
}

/// A supertype's discriminator column, when it has one.
pub fn discriminator_of(store: &Store, e: Obj) -> Option<Obj> {
    reference(store, e, entity::DISCRIMINATOR).filter(|a| store.is_alive(*a))
}

/// Whether a supertype's subtypes are complete (every row is one of them) and disjoint (no row is
/// two of them).
pub fn subtyping_flags(store: &Store, e: Obj) -> (bool, bool) {
    (flag(store, e, entity::SUBTYPES_COMPLETE), store.get(e, entity::SUBTYPES_DISJOINT).ok().and_then(|v| v.as_bool()).unwrap_or(true))
}

/// Why `subtype` cannot become a subtype of `supertype`; None when it can.
pub fn generalization_problem(store: &Store, supertype: Obj, subtype: Obj, except: Option<Obj>) -> Option<(&'static str, String)> {
    if supertype == subtype {
        return Some(("subtype", "a table cannot be a subtype of itself".into()));
    }
    if schema_of(store, supertype).is_none() || schema_of(store, supertype) != schema_of(store, subtype) {
        return Some(("subtype", "the two tables are in different schemas".into()));
    }
    if supertype_links(store, subtype).into_iter().any(|g| Some(g) != except) {
        return Some(("subtype", format!("{} has a supertype already", label(store, subtype))));
    }
    if is_supertype_or_self(store, subtype, supertype) {
        return Some((
            "supertype",
            format!("{} is a subtype of {} already, so this would close a circle", label(store, supertype), label(store, subtype)),
        ));
    }
    None
}

// Domains and enum types (slice S5).

/// The domain a column takes its type from.
pub fn domain_of(store: &Store, a: Obj) -> Option<Obj> {
    reference(store, a, attribute::DOMAIN).filter(|d| store.is_alive(*d))
}

/// The enum type a column takes its values from.
pub fn enum_of(store: &Store, a: Obj) -> Option<Obj> {
    reference(store, a, attribute::ENUM_TYPE).filter(|e| store.is_alive(*e))
}

/// A schema's domains, in order.
pub fn domains_of(store: &Store, s: Obj) -> Vec<Obj> {
    refs(store, s, schema::DOMAIN)
}

/// A schema's enum types, in order.
pub fn enums_of(store: &Store, s: Obj) -> Vec<Obj> {
    refs(store, s, schema::ENUM_TYPE)
}

/// An enum type's literals, in order.
pub fn literals_of(store: &Store, e: Obj) -> Vec<String> {
    store.list(e, enum_type::LITERAL).unwrap_or_default().iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
}

/// A domain's checks, in order.
pub fn domain_checks(store: &Store, d: Obj) -> Vec<Obj> {
    refs(store, d, domain::CHECK)
}

/// The columns that take a domain or an enum type.
pub fn type_columns(store: &Store, t: Obj) -> Vec<Obj> {
    store
        .referrers(t)
        .into_iter()
        .filter(|(_, p)| *p == attribute::DOMAIN || *p == attribute::ENUM_TYPE)
        .map(|(a, _)| a)
        .filter(|a| store.is_alive(*a))
        .collect()
}

/// Why `literals` cannot be an enum type's: at least one, at most 256, each a name, none twice.
pub fn literals_problem(literals: &[String]) -> Option<String> {
    if literals.is_empty() {
        return Some("an enum type needs at least one literal".into());
    }
    if literals.len() > MAX_LITERALS {
        return Some(format!("an enum type has at most {MAX_LITERALS} literals"));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for l in literals {
        let shown: String = l.chars().take(40).collect();
        if let Some(p) = name_problem(l, MAX_NAME) {
            return Some(format!("the literal {shown:?} {p}"));
        }
        if !seen.insert(l.to_lowercase()) {
            return Some(format!("the literal {shown:?} appears twice"));
        }
    }
    None
}

/// Why a domain's base type and size do not hold together; None when they do.
pub fn domain_problem(store: &Store, d: Obj) -> Option<String> {
    let base = text_of(store, d, domain::BASE_TYPE);
    if let Some(p) = name_problem(&base, MAX_TYPE) {
        return Some(format!("the base type {p}"));
    }
    if base.contains('(') {
        return Some("the base type carries its own size; give length, precision or scale instead".into());
    }
    let (l, p, s) = own_size_of(store, d);
    if let Some((_, problem)) = size_problem(&base, l, p, s) {
        return Some(problem);
    }
    for c in domain_checks(store, d) {
        if let Some(p) = check_problem(&text_of(store, c, domain_check::EXPRESSION)) {
            return Some(p);
        }
    }
    None
}

/// Why a column's default does not fit its enum type: it is none of the literals (quoted or not);
/// None when it fits or the column has no enum type or no default.
pub fn enum_default_problem(store: &Store, a: Obj) -> Option<String> {
    let e = enum_of(store, a)?;
    enum_default_check(store, e, &text_of(store, a, attribute::DEFAULT))
}

/// Why `default` does not fit enum type `e`: it is none of the literals (quoted or not); None when
/// it fits or is empty.
pub fn enum_default_check(store: &Store, e: Obj, default: &str) -> Option<String> {
    if default.is_empty() {
        return None;
    }
    let bare = default.trim().trim_matches('\'');
    if literals_of(store, e).iter().any(|l| l == bare) {
        return None;
    }
    let shown: String = default.chars().take(40).collect();
    Some(format!("the default {shown:?} is not a literal of {}", name_of(store, e)))
}

/// What a type takes after its name, in the PostgreSQL list (owner decision 2026-10-30: this
/// list first, other dialects behind a dialect setting later).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeShape {
    /// Nothing: `INTEGER`, `TEXT`, `BOOLEAN`, `DATE`, `UUID`, …
    Plain,
    /// A length: `VARCHAR(n)`, `CHAR(n)`, `BIT(n)`, `VARBIT(n)`.
    Length,
    /// A precision: `TIME(p)`, `TIMESTAMP(p)`, `INTERVAL(p)`.
    Precision,
    /// A precision and a scale: `NUMERIC(p, s)`, `DECIMAL(p, s)`.
    PrecisionScale,
}

/// The shape of a type name in the PostgreSQL list; a name not in the list is free text and takes
/// nothing. Case and the `character varying` spellings are read as PostgreSQL reads them.
pub fn type_shape(kind: &str) -> TypeShape {
    let k = kind.trim().to_ascii_lowercase();
    let k = k.split('(').next().unwrap_or("").trim();
    match k {
        "varchar" | "character varying" | "char" | "character" | "bpchar" | "bit" | "bit varying" | "varbit" => TypeShape::Length,
        "time"
        | "timetz"
        | "timestamp"
        | "timestamptz"
        | "interval"
        | "time with time zone"
        | "time without time zone"
        | "timestamp with time zone"
        | "timestamp without time zone" => TypeShape::Precision,
        "numeric" | "decimal" => TypeShape::PrecisionScale,
        _ => TypeShape::Plain,
    }
}

/// Whether a type name is an integer type, which an identity column needs.
pub fn is_integer_type(kind: &str) -> bool {
    matches!(
        kind.trim().to_ascii_lowercase().as_str(),
        "smallint" | "integer" | "int" | "int2" | "int4" | "int8" | "bigint" | "smallserial" | "serial" | "bigserial"
    )
}

/// A column's size as stored: length, precision, scale.
pub fn size_of(store: &Store, a: Obj) -> (Option<i64>, Option<i64>, Option<i64>) {
    if let Some(d) = domain_of(store, a) {
        return own_size_of(store, d);
    }
    own_size_of(store, a)
}

/// A column's or a domain's own size as stored: length, precision, scale (the properties share
/// their names).
pub fn own_size_of(store: &Store, a: Obj) -> (Option<i64>, Option<i64>, Option<i64>) {
    let is_domain = store.class_of(a).ok() == Some(DOMAIN);
    let props =
        if is_domain { [domain::LENGTH, domain::PRECISION, domain::SCALE] } else { [attribute::LENGTH, attribute::PRECISION, attribute::SCALE] };
    let read = |p| match store.get(a, p) {
        Ok(Value::Int(n)) if n > 0 => Some(n),
        _ => None,
    };
    // 0 is none: the generated integers take no null.
    (read(props[0]), read(props[1]), read(props[2]))
}

/// A column's type as DDL writes it: the name with its length, or precision and scale, when set.
pub fn type_display(store: &Store, a: Obj) -> String {
    if let Some(d) = domain_of(store, a) {
        return name_of(store, d);
    }
    if let Some(e) = enum_of(store, a) {
        return name_of(store, e);
    }
    let kind = type_of(store, a);
    match size_of(store, a) {
        (Some(n), _, _) => format!("{kind}({n})"),
        (None, Some(p), Some(s)) => format!("{kind}({p}, {s})"),
        (None, Some(p), None) => format!("{kind}({p})"),
        _ => kind,
    }
}

/// The generation of a column.
pub fn generation_of(store: &Store, a: Obj) -> Generation {
    match store.get(a, attribute::GENERATION) {
        Ok(Value::Enum(i)) => Generation::from_index(i).unwrap_or(Generation::NoneKind),
        _ => Generation::NoneKind,
    }
}

/// A string property of an element, or empty.
pub fn text_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Why a column's size does not fit its type, or none: a length only where the type takes one,
/// precision and scale likewise, each 1 to 65535 and the scale no more than the precision.
pub fn size_problem(kind: &str, length: Option<i64>, precision: Option<i64>, scale: Option<i64>) -> Option<(&'static str, String)> {
    let shape = type_shape(kind);
    for (key, value) in [("length", length), ("precision", precision), ("scale", scale)] {
        if let Some(v) = value
            && !(1..=65_535).contains(&v)
        {
            return Some((key, format!("{v} is not between 1 and 65535")));
        }
    }
    let kind = if kind.is_empty() { "a type without a name" } else { kind };
    if length.is_some() && shape != TypeShape::Length {
        return Some(("length", format!("{kind} takes no length")));
    }
    if precision.is_some() && !matches!(shape, TypeShape::Precision | TypeShape::PrecisionScale) {
        return Some(("precision", format!("{kind} takes no precision")));
    }
    if scale.is_some() && shape != TypeShape::PrecisionScale {
        return Some(("scale", format!("{kind} takes no scale")));
    }
    if let (Some(p), Some(s)) = (precision, scale)
        && s > p
    {
        return Some(("scale", "the scale is more than the precision".into()));
    }
    if scale.is_some() && precision.is_none() {
        return Some(("scale", "a scale needs a precision".into()));
    }
    None
}

/// Why a column's generation does not fit, or none: a computed column has an expression and no
/// default; an identity column is an integer type, not nullable, and has no default.
pub fn generation_problem(store: &Store, a: Obj) -> Option<String> {
    let generation = generation_of(store, a);
    let expression = text_of(store, a, attribute::EXPRESSION);
    let default = text_of(store, a, attribute::DEFAULT);
    match generation {
        Generation::NoneKind => (!expression.is_empty()).then(|| "an expression needs a computed generation".to_string()),
        Generation::ComputedStored | Generation::ComputedVirtual => {
            if expression.is_empty() {
                Some("a computed column needs its expression".into())
            } else if !default.is_empty() {
                Some("a computed column takes no default".into())
            } else {
                None
            }
        }
        Generation::IdentityAlways | Generation::IdentityByDefault => {
            if !is_integer_type(&type_of(store, a)) {
                Some("an identity column needs an integer type".into())
            } else if is_nullable(store, a) {
                Some("an identity column cannot be nullable".into())
            } else if !default.is_empty() {
                Some("an identity column takes no default".into())
            } else if !expression.is_empty() {
                Some("an identity column takes no expression".into())
            } else {
                None
            }
        }
    }
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

// Linked key columns and mapping tables (slice S4).

/// The column a key column follows: the one it was made to point at, until the user edits it.
pub fn linked_to(store: &Store, a: Obj) -> Option<Obj> {
    reference(store, a, attribute::LINKED_TO).filter(|t| store.is_alive(*t))
}

/// The columns that follow `a`.
pub fn linked_from(store: &Store, a: Obj) -> Vec<Obj> {
    store.referrers(a).into_iter().filter(|(_, p)| *p == attribute::LINKED_TO).map(|(c, _)| c).filter(|c| store.is_alive(*c)).collect()
}

/// Why an entity is no pure mapping table that could collapse back into a relationship: it needs
/// exactly two foreign keys with their relationships, columns that are exactly theirs and all in its
/// primary key, and no unique constraint, check or index. None when it can collapse.
pub fn collapse_problem(store: &Store, e: Obj) -> Option<String> {
    let fks = foreign_keys(store, e);
    if fks.len() != 2 {
        return Some("a mapping table has exactly two foreign keys".into());
    }
    let mut covered: BTreeSet<Obj> = BTreeSet::new();
    let mut own: BTreeSet<Obj> = BTreeSet::new();
    for fk in &fks {
        let parts = key_parts(store, *fk);
        let Some(r) = parts.relationship else { return Some(format!("the foreign key {} realises no relationship", label(store, *fk))) };
        own.insert(r);
        covered.extend(parts.columns);
    }
    let touching = store.referrers(e).into_iter().any(|(r, p)| (p == relationship::FROM || p == relationship::TO) && !own.contains(&r));
    if touching {
        return Some("it has a relationship of its own".into());
    }
    let attributes: BTreeSet<Obj> = refs(store, e, entity::ATTRIBUTE).into_iter().collect();
    if attributes != covered {
        return Some("it has columns of its own".into());
    }
    if primary_key(store, e).into_iter().collect::<BTreeSet<Obj>>() != attributes {
        return Some("its columns are not all in its primary key".into());
    }
    if !unique_constraints(store, e).is_empty() {
        return Some("it has a unique constraint".into());
    }
    if !checks_of(store, e).is_empty() {
        return Some("it has a check constraint".into());
    }
    if !indexes_of(store, e).is_empty() {
        return Some("it has an index".into());
    }
    None
}

/// The entities whose primary key `e`'s primary key is built on through identifying relationships:
/// the parents of its identifying foreign keys, and theirs, with a seen-set.
pub fn identifying_parents(store: &Store, e: Obj) -> BTreeSet<Obj> {
    let mut seen: BTreeSet<Obj> = BTreeSet::new();
    let mut queue = vec![e];
    while let Some(c) = queue.pop() {
        for fk in foreign_keys(store, c) {
            let parts = key_parts(store, fk);
            let Some(r) = parts.relationship else { continue };
            if !flag(store, r, relationship::IDENTIFYING) {
                continue;
            }
            if let Some(p) = parts.key.and_then(|k| entity_of(store, k))
                && seen.insert(p)
            {
                queue.push(p);
            }
        }
    }
    seen
}

/// Whether a foreign key of `child` onto key `k` that identifies would close a cycle: the key's
/// entity is built on `child` already, directly or through others, or `k` is `child`'s own primary
/// key (a self-reference onto a unique key cannot recurse, so it may identify).
pub fn identifying_cycle(store: &Store, child: Obj, k: Obj) -> bool {
    let Some(parent) = entity_of(store, k) else { return false };
    if child == parent {
        return store.class_of(k).ok() == Some(PRIMARY_KEY);
    }
    identifying_parents(store, parent).contains(&child)
}

/// Whether a relationship's child end is one-or-more, which no key enforces.
pub fn child_end_one_or_more(store: &Store, r: Obj) -> bool {
    let from = cardinality_of(store, r, relationship::FROM_END);
    let to = cardinality_of(store, r, relationship::TO_END);
    match (from, to) {
        (Some(f), Some(t)) if is_many(f) && is_many(t) => false,
        (Some(Cardinality::OneOrMore), _) | (_, Some(Cardinality::OneOrMore)) => true,
        _ => false,
    }
}

// Checks and indexes (slice S3).

/// An entity's check constraints, in order.
pub fn checks_of(store: &Store, e: Obj) -> Vec<Obj> {
    refs(store, e, entity::CHECK_CONSTRAINT)
}

/// An entity's indexes, in order.
pub fn indexes_of(store: &Store, e: Obj) -> Vec<Obj> {
    refs(store, e, entity::TABLE_INDEX)
}

/// An index's columns, in index order.
pub fn index_columns(store: &Store, ix: Obj) -> Vec<Obj> {
    refs(store, ix, table_index::COLUMN)
}

/// Whether an index is unique.
pub fn is_unique_index(store: &Store, ix: Obj) -> bool {
    flag(store, ix, table_index::UNIQUE)
}

/// One column of an index: the attribute or the expression it sorts by, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexColumnSpec {
    pub attribute: Option<Obj>,
    pub expression: String,
    pub direction: SortDirection,
    pub nulls: NullsOrder,
}

/// An index column as stored.
pub fn index_column_spec(store: &Store, c: Obj) -> IndexColumnSpec {
    IndexColumnSpec {
        attribute: reference(store, c, index_column::ATTRIBUTE).filter(|a| store.is_alive(*a)),
        expression: text_of(store, c, index_column::EXPRESSION),
        direction: match store.get(c, index_column::DIRECTION) {
            Ok(Value::Enum(i)) => SortDirection::from_index(i).unwrap_or(SortDirection::Ascending),
            _ => SortDirection::Ascending,
        },
        nulls: match store.get(c, index_column::NULLS) {
            Ok(Value::Enum(i)) => NullsOrder::from_index(i).unwrap_or(NullsOrder::Default),
            _ => NullsOrder::Default,
        },
    }
}

/// An index's columns as specs, in index order.
pub fn index_specs(store: &Store, ix: Obj) -> Vec<IndexColumnSpec> {
    index_columns(store, ix).into_iter().map(|c| index_column_spec(store, c)).collect()
}

/// Why `columns` cannot be the columns of an index of `entity`, as a sentence; None when they can.
pub fn index_columns_problem(store: &Store, entity: Obj, columns: &[IndexColumnSpec]) -> Option<String> {
    if columns.is_empty() {
        return Some("an index needs at least one column".into());
    }
    if columns.len() > MAX_KEY_COLUMNS {
        return Some(format!("an index has at most {MAX_KEY_COLUMNS} columns"));
    }
    let mut seen = BTreeSet::new();
    for c in columns {
        match (c.attribute, c.expression.is_empty()) {
            (Some(a), true) => {
                if store.class_of(a).ok() != Some(ATTRIBUTE) || entity_of(store, a) != Some(entity) {
                    return Some(format!("{} is not an attribute of {}", label(store, a), label(store, entity)));
                }
                if !seen.insert(a) {
                    return Some(format!("{} appears twice in the index", label(store, a)));
                }
            }
            (None, false) => {
                if let Some(p) = text_problem(&c.expression, MAX_TEXT) {
                    return Some(format!("the expression {p}"));
                }
            }
            (Some(a), false) => return Some(format!("{} has both an attribute and an expression", label(store, a))),
            (None, true) => return Some("a column needs an attribute or an expression".into()),
        }
    }
    None
}

/// Why `expression` cannot be a check constraint's, as a sentence; None when it can.
pub fn check_problem(expression: &str) -> Option<String> {
    if expression.trim().is_empty() {
        return Some("a check needs an expression".into());
    }
    text_problem(expression, MAX_TEXT).map(|p| format!("the expression {p}"))
}

/// A name in lower snake case for generated constraint names.
fn snake_lower(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// A constraint's or index's name to show: its own, or one made from its table the way DDL export
/// names an unnamed one (`pk_<table>`, `uq_<table>_<columns>`, `ck_<table>_<n>`,
/// `ix_<table>_<columns>`, an expression column counting as `expr`).
pub fn display_name(store: &Store, obj: Obj) -> String {
    let own = name_of(store, obj);
    if !own.is_empty() {
        return own;
    }
    let Some(e) = entity_of(store, obj) else { return label(store, obj) };
    let table = snake_lower(&name_of(store, e));
    let columns = |names: Vec<String>| names.into_iter().map(|n| snake_lower(&n)).collect::<Vec<_>>().join("_");
    match store.class_of(obj).ok() {
        Some(c) if c == PRIMARY_KEY => format!("pk_{table}"),
        Some(c) if c == UNIQUE_CONSTRAINT => {
            format!("uq_{table}_{}", columns(key_columns(store, obj).into_iter().map(|a| name_of(store, a)).collect()))
        }
        Some(c) if c == CHECK_CONSTRAINT => {
            let n = checks_of(store, e).iter().position(|c| *c == obj).map_or(1, |i| i + 1);
            format!("ck_{table}_{n}")
        }
        Some(c) if c == TABLE_INDEX => format!(
            "ix_{table}_{}",
            columns(index_specs(store, obj).into_iter().map(|s| s.attribute.map(|a| name_of(store, a)).unwrap_or_else(|| "expr".into())).collect())
        ),
        _ => label(store, obj),
    }
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
        .flat_map(|e| keys_of(store, e).into_iter().chain(foreign_keys(store, e)).chain(checks_of(store, e)).chain(indexes_of(store, e)))
        .map(|c| (c, name_of(store, c)))
        .filter(|(_, n)| !n.is_empty())
        .collect()
}

/// Whether `name` is free among the constraint names of `schema` (keys, foreign keys, checks and
/// indexes), ignoring case, except for `except`.
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

/// The notice on a relationship whose child end is one-or-more.
pub const ONE_OR_MORE: &str = "one-or-more at the child end: no key enforces it";

/// The notice for relationships between the same two entities that no role tells apart.
pub const SAME_ENDS: &str = "joins the same two entities as another relationship with the same roles; role names tell them apart";

/// What the rules point out without calling it wrong, each relationship with its notice: the
/// many-to-many relationships, and relationships that no role tells apart from another between the
/// same two entities.
pub fn notices(store: &Store) -> Vec<(Obj, &'static str)> {
    let relationships: Vec<Obj> = store.alive().filter(|o| store.class_of(*o).ok() == Some(RELATIONSHIP)).collect();
    let mut count: std::collections::BTreeMap<[(Obj, String); 2], usize> = std::collections::BTreeMap::new();
    for r in &relationships {
        if let Some(k) = ends_with_roles(store, *r) {
            *count.entry(k).or_default() += 1;
        }
    }
    let mut out = Vec::new();
    for r in relationships {
        if is_many_to_many(store, r) {
            out.push((r, MANY_TO_MANY));
        }
        if child_end_one_or_more(store, r) {
            out.push((r, ONE_OR_MORE));
        }
        if ends_with_roles(store, r).is_some_and(|k| count.get(&k).copied().unwrap_or(0) > 1) {
            out.push((r, SAME_ENDS));
        }
    }
    out
}

/// What the rules find wrong in a store, each as a sentence.
pub fn problems(store: &Store) -> Vec<String> {
    let mut out = Vec::new();
    for o in store.alive() {
        let Ok(class) = store.class_of(o) else { continue };
        if class == SCHEMA || class == ENTITY || class == ATTRIBUTE || class == DOMAIN || class == ENUM_TYPE {
            let name = name_of(store, o);
            if let Some(p) = name_problem(&name, MAX_NAME) {
                out.push(format!("the name of {o} {p}"));
            }
        }
        if class == SCHEMA {
            // Entities, domains and enum types share one namespace in a schema, as in SQL.
            let mut members: Vec<Obj> = store.list(o, schema::ENTITY).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
            members.extend(domains_of(store, o));
            members.extend(enums_of(store, o));
            for name in duplicate_names(store, &members) {
                out.push(format!("the name {name:?} is used twice in {o}"));
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
            let (length, precision, scale) = size_of(store, o);
            if let Some((_, p)) = size_problem(&type_of(store, o), length, precision, scale) {
                out.push(format!("the size of {o}: {p}"));
            }
            if let Some(p) = generation_problem(store, o) {
                out.push(format!("the generation of {o}: {p}"));
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
        if class == ATTRIBUTE {
            if let Some(p) = enum_default_problem(store, o) {
                out.push(format!("{o}: {p}"));
            }
            if domain_of(store, o).is_some() && (enum_of(store, o).is_some() || !own_type_of(store, o).is_empty()) {
                out.push(format!("{o} has more than one type"));
            }
            if domain_of(store, o).is_some()
                && (!text_of(store, o, attribute::DEFAULT).is_empty() || !text_of(store, o, attribute::COLLATION).is_empty())
            {
                out.push(format!("{o} is on a domain and has a default or collation of its own"));
            }
        }
        if class == GENERALIZATION {
            match generalization_parts(store, o) {
                (Some(sup), Some(sub), key_rel) => {
                    if sup == sub || schema_of(store, sup) != schema_of(store, sub) {
                        out.push(format!("the generalization {o} joins {} to itself or across schemas", label(store, sub)));
                    }
                    let wants_key = mapping_of(store, sup) == SubtypeMapping::TablePerType && primary_key_of(store, sup).is_some();
                    if wants_key && key_rel.is_none() {
                        out.push(format!("the subtype {} lacks its key onto {}", label(store, sub), label(store, sup)));
                    }
                    if let Some(r) = key_rel
                        && !flag(store, r, relationship::IDENTIFYING)
                    {
                        out.push(format!("the key of the subtype {} onto {} no longer identifies it", label(store, sub), label(store, sup)));
                    }
                    if !wants_key && key_rel.is_some() {
                        out.push(format!("the subtype {} keeps a key onto {} its mapping does not ask for", label(store, sub), label(store, sup)));
                    }
                }
                _ => out.push(format!("the generalization {o} lacks its supertype or its subtype")),
            }
            if store.container(o).ok().flatten().map(|(c, _)| c).and_then(|c| store.class_of(c).ok()) != Some(SCHEMA) {
                out.push(format!("the generalization {o} belongs to no schema"));
            }
        }
        if class == ENTITY {
            if supertype_links(store, o).len() > 1 {
                out.push(format!("{} has more than one supertype", label(store, o)));
            }
            if let Some(s) = supertype_of(store, o)
                && is_supertype_or_self(store, o, s)
            {
                out.push(format!("{} is its own supertype through others", label(store, o)));
            }
            if let Some(d) = discriminator_of(store, o)
                && entity_of(store, d) != Some(o)
            {
                out.push(format!("the discriminator of {} is not one of its columns", label(store, o)));
            }
        }
        if class == DOMAIN
            && let Some(p) = domain_problem(store, o)
        {
            out.push(format!("the domain {}: {p}", label(store, o)));
        }
        if class == ENUM_TYPE
            && let Some(p) = literals_problem(&literals_of(store, o))
        {
            out.push(format!("the enum type {}: {p}", label(store, o)));
        }
        if class == CHECK_CONSTRAINT {
            if entity_of(store, o).is_none() {
                out.push(format!("the check {} belongs to no entity", label(store, o)));
            }
            if let Some(p) = check_problem(&text_of(store, o, check_constraint::EXPRESSION)) {
                out.push(format!("the check {}: {p}", label(store, o)));
            }
        }
        if class == TABLE_INDEX {
            match entity_of(store, o) {
                Some(e) => {
                    if let Some(p) = index_columns_problem(store, e, &index_specs(store, o)) {
                        out.push(format!("the index {}: {p}", label(store, o)));
                    }
                }
                None => out.push(format!("the index {} belongs to no entity", label(store, o))),
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
            for prop in [relationship::FROM_ROLE, relationship::TO_ROLE] {
                let role = role_of(store, o, prop);
                if !role.is_empty()
                    && let Some(p) = name_problem(&role, MAX_NAME)
                {
                    out.push(format!("a role of {o} {p}"));
                }
            }
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
