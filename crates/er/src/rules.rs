//! The ER rules the MOF core cannot know, checked by the commands before they change anything and
//! available over a whole store.

use std::collections::BTreeSet;

use archforge_core::skills::Problem;
use archforge_engine::{DefaultKind, GenerationRule, Reason, SizeRule};
use archforge_mof::{Obj, Store, Value};

use crate::{
    ATTRIBUTE, CHECK_CONSTRAINT, Cardinality, DOMAIN, Deferrability, ENTITY, ENUM_TYPE, EntityKind, FOREIGN_KEY, GENERALIZATION, Generation,
    MatchType, NullsOrder, PRIMARY_KEY, RELATIONSHIP, ReferentialAction, SCHEMA, SEQUENCE, SortDirection, SubtypeMapping, TABLE_INDEX,
    UNIQUE_CONSTRAINT, VIEW, attribute, check_constraint, domain, domain_check, entity, enum_type, foreign_key, generalization, index_column, key,
    relationship, schema, sequence, table_index, unique_constraint, view,
};

/// The most literals of an enum type.
pub const MAX_LITERALS: usize = 256;
/// An enum type's literal, as a typed refusal names the kind.
pub(crate) const LITERAL: &str = "enum literal";

/// The most characters in a name, and in an attribute's type.
pub const MAX_NAME: usize = 128;
pub const MAX_TYPE: usize = 64;
/// The most characters a default, an expression or a description may have.
pub const MAX_TEXT: usize = 4096;
/// The most columns in one key or foreign key.
pub const MAX_KEY_COLUMNS: usize = 32;

/// Why a text is not acceptable, or none: an empty text is nothing and always fine.
pub fn text_problem(text: &str, max: usize) -> Option<Problem> {
    if text.is_empty() {
        return None;
    }
    if text.chars().count() > max {
        return Some(Problem::too_long(max));
    }
    if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Some(control_character());
    }
    if text.trim() != text {
        return Some(Problem::edge_space());
    }
    None
}

/// Why a name is not acceptable, as the end of the sentence "the name ...".
pub fn name_problem(name: &str, max: usize) -> Option<Problem> {
    if name.is_empty() {
        return Some(Problem::new(Reason::NameEmpty, "is empty"));
    }
    if name.chars().count() > max {
        return Some(Problem::too_long(max));
    }
    if name.chars().any(char::is_control) {
        return Some(control_character());
    }
    if name.trim() != name {
        return Some(Problem::edge_space());
    }
    None
}

/// Holds a control character.
fn control_character() -> Problem {
    Problem::new(Reason::ControlCharacter, "holds a control character")
}

/// Whether two names are the same name, ignoring case.
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// The name of an element, or empty: the core's lookup, which every notation shares.
pub fn name_of(store: &Store, obj: Obj) -> String {
    archforge_core::name_of(store, obj)
}

/// Whether `name` is free in `schema`, except for `except`: tables, domains, enum types, sequences
/// and views share one namespace, as relations and types do in SQL.
pub fn entity_name_free(store: &Store, schema: Obj, name: &str, except: Option<Obj>) -> bool {
    crate::names::clash(store, schema, ENTITY, name, except).is_none()
}

/// Whether `name` is free among the relationships of `schema`, except for `except`; an unnamed
/// relationship takes no name.
pub fn relationship_name_free(store: &Store, schema: Obj, name: &str, except: Option<Obj>) -> bool {
    crate::names::clash(store, schema, RELATIONSHIP, name, except).is_none()
}

/// Whether `name` is free among the attributes of `entity`, except for `except`.
pub fn attribute_name_free(store: &Store, entity: Obj, name: &str, except: Option<Obj>) -> bool {
    crate::names::clash(store, entity, ATTRIBUTE, name, except).is_none()
}

/// The first name used twice among `names`, ignoring case, as the second one is written; empty
/// names name nothing.
pub fn first_twice<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    names.into_iter().filter(|n| !n.is_empty()).find(|n| !seen.insert(n.to_lowercase()))
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

// Sequences and views (slice S7).

/// The most characters of a view's query: the store's limit for a text.
pub const MAX_QUERY: usize = archforge_mof::MAX_STRING;

/// A schema's sequences, in order.
pub fn sequences_of(store: &Store, s: Obj) -> Vec<Obj> {
    refs(store, s, schema::SEQUENCE)
}

/// A schema's views, in order.
pub fn views_of(store: &Store, s: Obj) -> Vec<Obj> {
    refs(store, s, schema::VIEW)
}

/// A sequence's numbers and flag as stored: start, increment, minimum and maximum (text, empty for
/// none) and cycle.
pub struct SequenceNumbers {
    pub start: i64,
    pub increment: i64,
    pub minimum: String,
    pub maximum: String,
    pub cycle: bool,
}

/// What a sequence holds.
pub fn sequence_numbers(store: &Store, q: Obj) -> SequenceNumbers {
    let int = |prop| store.get(q, prop).ok().and_then(|v| v.as_int()).unwrap_or(1);
    SequenceNumbers {
        start: int(sequence::START),
        increment: int(sequence::INCREMENT),
        minimum: text_of(store, q, sequence::MINIMUM),
        maximum: text_of(store, q, sequence::MAXIMUM),
        cycle: flag(store, q, sequence::CYCLE),
    }
}

/// The column that owns a sequence: the sequence goes when its column does in SQL; here it forgets
/// the column.
pub fn owner_of_sequence(store: &Store, q: Obj) -> Option<Obj> {
    reference(store, q, sequence::OWNED_BY).filter(|a| store.is_alive(*a))
}

/// A bound kept as text: none when empty, else a 64-bit whole number; the error is the sentence.
pub fn bound_of(text: &str) -> Result<Option<i64>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<i64>().map(Some).map_err(|_| {
        let shown: String = text.chars().take(40).collect();
        format!("{shown:?} is not a whole number between {} and {}", i64::MIN, i64::MAX)
    })
}

/// Why a sequence's numbers break the rules: the parameter it concerns and the problem, which
/// displays as a sentence after the parameter and is typed by the rule it breaks
/// ([`Reason::Sequence`]); none when they keep them.
pub fn sequence_problem(n: &SequenceNumbers) -> Option<(&'static str, Problem)> {
    use archforge_engine::SequenceRule;
    let broken = |key, rule, said: String| Some((key, Problem::new(Reason::Sequence(rule), said)));
    if n.increment == 0 {
        return broken("increment", SequenceRule::IncrementZero, "is 0; a sequence moves by at least 1".into());
    }
    let minimum = match bound_of(&n.minimum) {
        Ok(m) => m,
        Err(why) => return broken("minimum", SequenceRule::MinimumUnread { value: n.minimum.chars().take(40).collect() }, why),
    };
    let maximum = match bound_of(&n.maximum) {
        Ok(m) => m,
        Err(why) => return broken("maximum", SequenceRule::MaximumUnread { value: n.maximum.chars().take(40).collect() }, why),
    };
    if let (Some(lo), Some(hi)) = (minimum, maximum)
        && lo >= hi
    {
        return broken("maximum", SequenceRule::MaximumNotAbove { maximum: hi, minimum: lo }, format!("{hi} is not greater than the minimum {lo}"));
    }
    if let Some(lo) = minimum
        && n.start < lo
    {
        return broken("start", SequenceRule::StartBelow { start: n.start, minimum: lo }, format!("{} is below the minimum {lo}", n.start));
    }
    if let Some(hi) = maximum
        && n.start > hi
    {
        return broken("start", SequenceRule::StartAbove { start: n.start, maximum: hi }, format!("{} is above the maximum {hi}", n.start));
    }
    None
}

/// Why `column` cannot own a sequence of schema `s`: it is no table's column, or a table's of
/// another schema. Setting an owner also wants a whole-number column (`owner_type_problem`); a
/// column retyped later keeps its sequence. The problem displays as a sentence and is typed with
/// the column's [`label`]: as no column ([`Reason::NotA`]), [`Reason::ColumnOfOtherSchema`] or
/// [`Reason::InNoTable`].
pub fn owned_by_problem(store: &Store, s: Obj, column: Obj) -> Option<Problem> {
    let name = label(store, column);
    if store.class_of(column).ok() != Some(ATTRIBUTE) {
        return Some(Problem::new(Reason::NotA { kind: "column", name: name.clone() }, format!("{name} is not a column")));
    }
    match entity_of(store, column) {
        Some(e) if schema_of(store, e) == Some(s) => None,
        Some(e) => Some(Problem::new(
            Reason::ColumnOfOtherSchema { column: format!("{}.{name}", label(store, e)).into() },
            format!("{}.{} is a column of another schema", name_of(store, e), name_of(store, column)),
        )),
        None => Some(Problem::new(Reason::InNoTable { column: name.as_str().into() }, format!("{name} belongs to no table"))),
    }
}

/// Why `column` cannot be made the owner of a sequence: it is not of a whole-number type; the
/// problem displays as a sentence and is typed with the column's [`label`] and its type, cut short
/// ([`Reason::NotIntegerType`]).
pub fn owner_type_problem(store: &Store, column: Obj) -> Option<Problem> {
    let kind = type_of(store, column);
    if is_integer_type(&kind) {
        None
    } else {
        let (name, shown): (String, String) = (label(store, column), kind.chars().take(40).collect());
        let said = format!("{name} is {shown:?}, not a whole-number type");
        Some(Problem::new(Reason::NotIntegerType { column: name.into(), ty: shown.into() }, said))
    }
}

/// Why a view's query is not acceptable: it is SQL text, not parsed, never empty, with no hidden
/// direction or zero-width characters a DDL export would carry. The problem displays as a sentence
/// and is typed as an empty text, a hidden character or the text rule's reason.
pub fn query_problem(query: &str) -> Option<Problem> {
    if query.trim().is_empty() {
        return Some(Problem::new(Reason::TextEmpty, "a view needs its query"));
    }
    if query.chars().any(|c| matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}')) {
        return Some(Problem::new(Reason::ControlCharacter, "the query holds a hidden direction or zero-width character"));
    }
    text_problem(query, MAX_QUERY).map(|p| Problem::new(p.reason.clone(), format!("the query {p}")))
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

/// Why `literals` cannot be an enum type's: at least one, at most 256, each a name, none twice; in
/// English as it displays, and typed, a literal by its first 40 characters, of the kind
/// `"enum literal"`.
pub fn literals_problem(literals: &[String]) -> Option<Problem> {
    if literals.is_empty() {
        return Some(Problem::new(Reason::TooFew { kind: LITERAL, min: 1 }, "an enum type needs at least one literal"));
    }
    if literals.len() > MAX_LITERALS {
        let reason = Reason::TooMany { kind: LITERAL, max: MAX_LITERALS };
        return Some(Problem::new(reason, format!("an enum type has at most {MAX_LITERALS} literals")));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for l in literals {
        let shown: String = l.chars().take(40).collect();
        if let Some(p) = name_problem(l, MAX_NAME) {
            let said = format!("the literal {shown:?} {p}");
            return Some(Problem::new(Reason::LiteralName { name: shown.into(), why: Box::new(p.reason) }, said));
        }
        if !seen.insert(l.to_lowercase()) {
            let said = format!("the literal {shown:?} appears twice");
            return Some(Problem::new(Reason::NamedTwice { kind: LITERAL, name: shown }, said));
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
    if let Some(problem) = size_problem(&base, l, p, s) {
        return Some(problem.said());
    }
    for c in domain_checks(store, d) {
        if let Some(p) = check_problem(&text_of(store, c, domain_check::EXPRESSION)) {
            return Some(p.said());
        }
    }
    None
}

/// Why a column's default does not fit its enum type: it is none of the literals (quoted or not);
/// None when it fits or the column has no enum type or no default.
pub fn enum_default_problem(store: &Store, a: Obj) -> Option<EnumDefaultProblem> {
    let e = enum_of(store, a)?;
    enum_default_check(store, e, &text_of(store, a, attribute::DEFAULT))
}

/// A default that is none of an enum type's literals ([`enum_default_check`]): said as an English
/// sentence ([`EnumDefaultProblem::said`]) or typed ([`EnumDefaultProblem::reason`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDefaultProblem {
    /// The default, cut to its first 40 characters.
    pub value: String,
    /// The enum type's name.
    pub enumeration: String,
}

impl EnumDefaultProblem {
    /// The problem as an English sentence, as `the default "x" is not a literal of Status`.
    pub fn said(&self) -> String {
        format!("the default {:?} is not a literal of {}", self.value, self.enumeration)
    }

    /// The problem typed, as a default that does not read as one of the enum type's literals.
    pub fn reason(&self) -> Reason {
        Reason::DefaultUnread { value: self.value.as_str().into(), kind: DefaultKind::Literal(self.enumeration.as_str().into()) }
    }
}

/// Why `default` does not fit enum type `e`: it is none of the literals (quoted or not); None when
/// it fits or is empty.
pub fn enum_default_check(store: &Store, e: Obj, default: &str) -> Option<EnumDefaultProblem> {
    if default.is_empty() {
        return None;
    }
    let bare = default.trim().trim_matches('\'');
    if literals_of(store, e).iter().any(|l| l == bare) {
        return None;
    }
    Some(EnumDefaultProblem { value: default.chars().take(40).collect(), enumeration: name_of(store, e) })
}

/// What a type takes after its name, in the PostgreSQL list (owner decision 2026-10-30: this
/// list first, other dialects behind a dialect setting later), and MySQL's BINARY and VARBINARY,
/// which a project's own ids are (BINARY(16)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeShape {
    /// Nothing: `INTEGER`, `TEXT`, `BOOLEAN`, `DATE`, `UUID`, …
    Plain,
    /// A length: `VARCHAR(n)`, `CHAR(n)`, `BIT(n)`, `VARBIT(n)`, `BINARY(n)`, `VARBINARY(n)`.
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
        "varchar" | "character varying" | "char" | "character" | "bpchar" | "bit" | "bit varying" | "varbit" | "binary" | "varbinary" => {
            TypeShape::Length
        }
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

/// What a column's or a domain's size breaks ([`size_problem`]): said as an English sentence
/// ([`SizeProblem::said`]) or typed ([`SizeProblem::reason`]), under the parameter it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeProblem {
    /// The part of the size that breaks it: `"length"`, `"precision"` or `"scale"`.
    pub key: &'static str,
    /// The rule it breaks.
    pub rule: SizeRule,
    /// The type name, as given; empty for none.
    pub ty: String,
    /// The length, precision or scale the rule is about.
    pub value: i64,
}

impl SizeProblem {
    /// The problem as an English sentence, as "INT takes no length".
    pub fn said(&self) -> String {
        let kind = if self.ty.is_empty() { "a type without a name" } else { self.ty.as_str() };
        match self.rule {
            SizeRule::OutOfRange => format!("{} is not between 1 and 65535", self.value),
            SizeRule::TakesNoLength => format!("{kind} takes no length"),
            SizeRule::BinaryTooLong => format!("{} is more than the 255 bytes a BINARY holds", self.value),
            SizeRule::TakesNoPrecision => format!("{kind} takes no precision"),
            SizeRule::TakesNoScale => format!("{kind} takes no scale"),
            SizeRule::ScaleOverPrecision => "the scale is more than the precision".into(),
            SizeRule::ScaleWithoutPrecision => "a scale needs a precision".into(),
        }
    }

    /// The problem typed, with the type and the size, for an interface to say in its own words.
    pub fn reason(&self) -> Reason {
        Reason::SizeMisfit { rule: self.rule, ty: self.ty.as_str().into(), value: self.value }
    }
}

/// Why a column's size does not fit its type, or none: a length only where the type takes one,
/// precision and scale likewise, each 1 to 65535 (a BINARY's length to 255) and the scale no
/// more than the precision.
pub fn size_problem(kind: &str, length: Option<i64>, precision: Option<i64>, scale: Option<i64>) -> Option<SizeProblem> {
    let shape = type_shape(kind);
    let problem = |key, rule, value| Some(SizeProblem { key, rule, ty: kind.to_string(), value });
    for (key, value) in [("length", length), ("precision", precision), ("scale", scale)] {
        if let Some(v) = value
            && !(1..=65_535).contains(&v)
        {
            return problem(key, SizeRule::OutOfRange, v);
        }
    }
    if let Some(n) = length
        && shape != TypeShape::Length
    {
        return problem("length", SizeRule::TakesNoLength, n);
    }
    if let Some(n) = length
        && n > 255
        && kind.trim().eq_ignore_ascii_case("binary")
    {
        return problem("length", SizeRule::BinaryTooLong, n);
    }
    if let Some(p) = precision
        && !matches!(shape, TypeShape::Precision | TypeShape::PrecisionScale)
    {
        return problem("precision", SizeRule::TakesNoPrecision, p);
    }
    if let Some(s) = scale
        && shape != TypeShape::PrecisionScale
    {
        return problem("scale", SizeRule::TakesNoScale, s);
    }
    if let (Some(p), Some(s)) = (precision, scale)
        && s > p
    {
        return problem("scale", SizeRule::ScaleOverPrecision, s);
    }
    if let (None, Some(s)) = (precision, scale) {
        return problem("scale", SizeRule::ScaleWithoutPrecision, s);
    }
    None
}

/// What a column's generation breaks ([`generation_problem`]): said as an English sentence
/// ([`GenerationProblem::said`]) or typed ([`GenerationProblem::reason`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationProblem(pub GenerationRule);

impl GenerationProblem {
    /// The problem as an English sentence, as "a computed column needs its expression".
    pub fn said(self) -> &'static str {
        match self.0 {
            GenerationRule::ExpressionNeedsComputed => "an expression needs a computed generation",
            GenerationRule::ComputedNeedsExpression => "a computed column needs its expression",
            GenerationRule::ComputedTakesNoDefault => "a computed column takes no default",
            GenerationRule::IdentityNeedsInteger => "an identity column needs an integer type",
            GenerationRule::IdentityNotNullable => "an identity column cannot be nullable",
            GenerationRule::IdentityTakesNoDefault => "an identity column takes no default",
            GenerationRule::IdentityTakesNoExpression => "an identity column takes no expression",
        }
    }

    /// The problem typed, for an interface to say in its own words.
    pub fn reason(self) -> Reason {
        Reason::Generation(self.0)
    }
}

/// Why a column's generation does not fit, or none: a computed column has an expression and no
/// default; an identity column is an integer type, not nullable, and has no default.
pub fn generation_problem(store: &Store, a: Obj) -> Option<GenerationProblem> {
    let generation = generation_of(store, a);
    let expression = text_of(store, a, attribute::EXPRESSION);
    let default = text_of(store, a, attribute::DEFAULT);
    let rule = match generation {
        Generation::NoneKind => (!expression.is_empty()).then_some(GenerationRule::ExpressionNeedsComputed),
        Generation::ComputedStored | Generation::ComputedVirtual => {
            if expression.is_empty() {
                Some(GenerationRule::ComputedNeedsExpression)
            } else if !default.is_empty() {
                Some(GenerationRule::ComputedTakesNoDefault)
            } else {
                None
            }
        }
        Generation::IdentityAlways | Generation::IdentityByDefault => {
            if !is_integer_type(&type_of(store, a)) {
                Some(GenerationRule::IdentityNeedsInteger)
            } else if is_nullable(store, a) {
                Some(GenerationRule::IdentityNotNullable)
            } else if !default.is_empty() {
                Some(GenerationRule::IdentityTakesNoDefault)
            } else if !expression.is_empty() {
                Some(GenerationRule::IdentityTakesNoExpression)
            } else {
                None
            }
        }
    };
    rule.map(GenerationProblem)
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

/// Why an expression cannot be a check's ([`check_problem`]): said as an English sentence
/// ([`CheckProblem::said`]) or typed ([`CheckProblem::reason`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckProblem {
    /// The expression is empty or holds only spaces.
    NoExpression,
    /// The expression breaks a rule of every text: too long, a control character, a space at an end.
    Text(Problem),
}

impl CheckProblem {
    /// The problem as an English sentence, as "a check needs an expression".
    pub fn said(&self) -> String {
        match self {
            Self::NoExpression => "a check needs an expression".into(),
            Self::Text(p) => format!("the expression {p}"),
        }
    }

    /// The problem typed, for an interface to say in its own words.
    pub fn reason(&self) -> Reason {
        match self {
            Self::NoExpression => Reason::NoExpression,
            Self::Text(p) => p.reason.clone(),
        }
    }
}

/// Why `expression` cannot be a check constraint's or a domain check's; None when it can.
pub fn check_problem(expression: &str) -> Option<CheckProblem> {
    if expression.trim().is_empty() {
        return Some(CheckProblem::NoExpression);
    }
    text_problem(expression, MAX_TEXT).map(CheckProblem::Text)
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

/// Why columns cannot be a key of a table ([`key_columns_problem`]), each column and table by its
/// label: said as an English sentence ([`KeyColumnsProblem::said`]) or typed
/// ([`KeyColumnsProblem::reason`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyColumnsProblem {
    /// The key has no column.
    NoColumn,
    /// The key has more than [`MAX_KEY_COLUMNS`] columns.
    TooMany,
    /// The element `column` is no column of the table `table`.
    NotColumnOf { column: String, table: String },
    /// The column `column` is in the key twice.
    Twice { column: String },
    /// The column `column` may be null, in a primary key.
    Nullable { column: String },
}

impl KeyColumnsProblem {
    /// The problem as an English sentence, as "a key needs at least one column".
    pub fn said(&self) -> String {
        match self {
            Self::NoColumn => "a key needs at least one column".into(),
            Self::TooMany => format!("a key has at most {MAX_KEY_COLUMNS} columns"),
            Self::NotColumnOf { column, table } => format!("{column} is not an attribute of {table}"),
            Self::Twice { column } => format!("{column} appears twice in the key"),
            Self::Nullable { column } => format!("{column} is nullable, and a primary key column cannot be"),
        }
    }

    /// The problem typed, for an interface to say in its own words; a key's columns are of the kind
    /// `"key column"`.
    pub fn reason(&self) -> Reason {
        const KIND: &str = "key column";
        match self {
            Self::NoColumn => Reason::TooFew { kind: KIND, min: 1 },
            Self::TooMany => Reason::TooMany { kind: KIND, max: MAX_KEY_COLUMNS },
            Self::NotColumnOf { column, table } => Reason::NotColumnOf { column: column.as_str().into(), table: table.as_str().into() },
            Self::Twice { column } => Reason::NamedTwice { kind: KIND, name: column.clone() },
            Self::Nullable { column } => Reason::NullableKeyColumn { name: column.clone() },
        }
    }
}

/// Why `columns` cannot be a key of `entity`; None when they can. A primary key's columns are never
/// nullable.
pub fn key_columns_problem(store: &Store, entity: Obj, columns: &[Obj], primary: bool) -> Option<KeyColumnsProblem> {
    if columns.is_empty() {
        return Some(KeyColumnsProblem::NoColumn);
    }
    if columns.len() > MAX_KEY_COLUMNS {
        return Some(KeyColumnsProblem::TooMany);
    }
    let mut seen = BTreeSet::new();
    for a in columns {
        if store.class_of(*a).ok() != Some(ATTRIBUTE) || entity_of(store, *a) != Some(entity) {
            return Some(KeyColumnsProblem::NotColumnOf { column: label(store, *a), table: label(store, entity) });
        }
        if !seen.insert(*a) {
            return Some(KeyColumnsProblem::Twice { column: label(store, *a) });
        }
        if primary && is_nullable(store, *a) {
            return Some(KeyColumnsProblem::Nullable { column: label(store, *a) });
        }
    }
    None
}

/// Why a foreign key breaks the rules ([`key_problem`]), each key, column and table by its label: said
/// as an English sentence ([`KeyProblem::said`]) or typed ([`KeyProblem::reason`]), under the
/// parameter it concerns ([`KeyProblem::key`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyProblem {
    /// The foreign key has no column.
    NoColumn,
    /// The foreign key has more than [`MAX_KEY_COLUMNS`] columns.
    TooMany,
    /// The foreign key has `columns` columns, where the key of `table` has `key`.
    ColumnCount { columns: usize, table: String, key: usize },
    /// The element `column` is no column of the table `table`.
    NotColumnOf { column: String, table: String },
    /// The column `column` is in the foreign key twice.
    Twice { column: String },
    /// The column `column`, of type `ty`, and the key's column `target` it matches, of `target_ty`,
    /// differ in type; a type is empty for a column without one.
    TypesDiffer { column: String, ty: String, target: String, target_ty: String },
    /// The element `name` the foreign key points at is no key.
    NotAKey { name: String },
    /// The key the foreign key points at, of the table `table`, has no columns.
    KeyWithoutColumns { table: String },
    /// An action sets the columns to null, and the column `column` may never be null.
    SetNull { column: String },
    /// The relationship does not join the foreign key's table `table` and the table `references`.
    NotJoining { table: String, references: String },
    /// The relationship identifies, and the column `column` is outside the primary key.
    OutsideKey { column: String },
    /// The key or the foreign key belongs to no table or schema, which containment never leaves:
    /// said `said` in English only.
    Unplaced { said: &'static str },
}

impl KeyProblem {
    /// The parameter the problem concerns: `columns`, `key`, `on_delete` or `relationship`.
    pub fn key(&self) -> &'static str {
        match self {
            Self::NotAKey { .. } | Self::KeyWithoutColumns { .. } | Self::Unplaced { .. } => "key",
            Self::SetNull { .. } => "on_delete",
            Self::NotJoining { .. } | Self::OutsideKey { .. } => "relationship",
            _ => "columns",
        }
    }

    /// The problem as an English sentence, as "a foreign key needs at least one column".
    pub fn said(&self) -> String {
        match self {
            Self::NoColumn => "a foreign key needs at least one column".into(),
            Self::TooMany => format!("a foreign key has at most {MAX_KEY_COLUMNS} columns"),
            Self::ColumnCount { columns, table, key } => format!("the foreign key has {columns} columns but the key of {table} has {key}"),
            Self::NotColumnOf { column, table } => format!("{column} is not an attribute of {table}"),
            Self::Twice { column } => format!("{column} appears twice in the foreign key"),
            Self::TypesDiffer { column, ty, target, target_ty } => format!("{column} is {ty:?} but {target} is {target_ty:?}"),
            Self::NotAKey { name } => format!("{name} is neither a primary key nor a unique constraint"),
            Self::KeyWithoutColumns { table } => format!("the key of {table} has no columns"),
            Self::SetNull { .. } => "set null needs nullable columns".into(),
            Self::NotJoining { .. } => "the relationship does not join these two entities".into(),
            Self::OutsideKey { .. } => "an identifying relationship's key columns must be part of the primary key".into(),
            Self::Unplaced { said } => (*said).into(),
        }
    }

    /// The problem typed, for an interface to say in its own words; a foreign key's columns are of
    /// the kind `"foreign key column"`. None for a key or a foreign key that belongs to no table or
    /// schema ([`KeyProblem::Unplaced`]).
    pub fn reason(&self) -> Option<Reason> {
        const KIND: &str = "foreign key column";
        let ty = |t: &str| (!t.is_empty()).then(|| t.into());
        Some(match self {
            Self::NoColumn => Reason::TooFew { kind: KIND, min: 1 },
            Self::TooMany => Reason::TooMany { kind: KIND, max: MAX_KEY_COLUMNS },
            Self::ColumnCount { columns, table, key } => Reason::ColumnCount { columns: *columns, table: table.as_str().into(), key: *key },
            Self::NotColumnOf { column, table } => Reason::NotColumnOf { column: column.as_str().into(), table: table.as_str().into() },
            Self::Twice { column } => Reason::NamedTwice { kind: KIND, name: column.clone() },
            Self::TypesDiffer { column, ty: own, target, target_ty } => {
                Reason::TypesDiffer { column: column.as_str().into(), target: target.as_str().into(), types: Box::new([ty(own), ty(target_ty)]) }
            }
            Self::NotAKey { name } => Reason::NotA { kind: "key", name: name.clone() },
            Self::KeyWithoutColumns { table } => Reason::NoKeyColumns { table: table.as_str().into() },
            Self::SetNull { column } => Reason::SetNullNotNullable { column: column.as_str().into() },
            Self::NotJoining { table, references } => {
                Reason::RelationshipNotJoining { table: table.as_str().into(), references: references.as_str().into() }
            }
            Self::OutsideKey { column } => Reason::IdentifyingOutsideKey { column: column.as_str().into() },
            Self::Unplaced { .. } => return None,
        })
    }
}

/// Why a foreign key on `entity` with `columns` pointing at key `k` would break the rules; None when
/// it keeps them. Columns match the key's by position; types are compared ignoring case, as SQL does.
pub fn key_problem(
    store: &Store,
    entity: Obj,
    k: Obj,
    columns: &[Obj],
    actions: [ReferentialAction; 2],
    relationship: Option<Obj>,
) -> Option<KeyProblem> {
    if !is_key(store, k) {
        return Some(KeyProblem::NotAKey { name: label(store, k) });
    }
    let Some(references) = entity_of(store, k) else { return Some(KeyProblem::Unplaced { said: "the key belongs to no entity" }) };
    // A foreign key may point into another schema of the model (slice S7).
    if schema_of(store, entity).is_none() || schema_of(store, references).is_none() {
        return Some(KeyProblem::Unplaced { said: "the key or the foreign key belongs to no schema" });
    }
    let targets = key_columns(store, k);
    if targets.is_empty() {
        return Some(KeyProblem::KeyWithoutColumns { table: label(store, references) });
    }
    if columns.is_empty() {
        return Some(KeyProblem::NoColumn);
    }
    if columns.len() > MAX_KEY_COLUMNS {
        return Some(KeyProblem::TooMany);
    }
    if columns.len() != targets.len() {
        return Some(KeyProblem::ColumnCount { columns: columns.len(), table: label(store, references), key: targets.len() });
    }
    let mut seen = BTreeSet::new();
    for (a, t) in columns.iter().zip(&targets) {
        if store.class_of(*a).ok() != Some(ATTRIBUTE) || entity_of(store, *a) != Some(entity) {
            return Some(KeyProblem::NotColumnOf { column: label(store, *a), table: label(store, entity) });
        }
        if !seen.insert(*a) {
            return Some(KeyProblem::Twice { column: label(store, *a) });
        }
        let (ty, target_ty) = (type_of(store, *a), type_of(store, *t));
        if !same_name(&ty, &target_ty) {
            return Some(KeyProblem::TypesDiffer { column: label(store, *a), ty, target: label(store, *t), target_ty });
        }
    }
    if actions.contains(&ReferentialAction::SetNull)
        && let Some(a) = columns.iter().find(|a| !is_nullable(store, **a))
    {
        return Some(KeyProblem::SetNull { column: label(store, *a) });
    }
    if let Some(r) = relationship {
        let ends: BTreeSet<Option<Obj>> = [reference(store, r, relationship::FROM), reference(store, r, relationship::TO)].into_iter().collect();
        let want: BTreeSet<Option<Obj>> = [Some(entity), Some(references)].into_iter().collect();
        if store.class_of(r).ok() != Some(RELATIONSHIP) || ends != want {
            return Some(KeyProblem::NotJoining { table: label(store, entity), references: label(store, references) });
        }
        let pk = primary_key(store, entity);
        if flag(store, r, relationship::IDENTIFYING)
            && let Some(a) = columns.iter().find(|a| !pk.contains(a))
        {
            return Some(KeyProblem::OutsideKey { column: label(store, *a) });
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
    constraint_clash(store, s, name, except).is_none()
}

/// The name of the constraint of `schema`, other than `except`, that has `name` already, ignoring
/// case; none when `name` is free ([`constraint_name_free`]).
pub fn constraint_clash(store: &Store, s: Obj, name: &str, except: Option<Obj>) -> Option<String> {
    constraint_names(store, s).into_iter().find(|(c, n)| Some(*c) != except && same_name(n, name)).map(|(_, n)| n)
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
        if class == SCHEMA || class == ENTITY || class == ATTRIBUTE || class == DOMAIN || class == ENUM_TYPE || class == SEQUENCE || class == VIEW {
            let name = name_of(store, o);
            if let Some(p) = name_problem(&name, MAX_NAME) {
                out.push(format!("the name of {o} {p}"));
            }
        }
        if class == archforge_core::MODEL {
            // A schema is named once in the model, which is one database (AR-209).
            let schemas: Vec<Obj> = store
                .list(o, archforge_core::model::MEMBER)
                .unwrap_or_default()
                .iter()
                .filter_map(Value::as_ref)
                .filter(|s| store.class_of(*s).ok() == Some(SCHEMA))
                .collect();
            for name in duplicate_names(store, &schemas) {
                out.push(format!("the schema name {name:?} is used twice"));
            }
        }
        if class == DOMAIN {
            let names: Vec<String> = domain_checks(store, o).into_iter().map(|c| name_of(store, c)).collect();
            if let Some(name) = first_twice(names.iter().map(String::as_str)) {
                out.push(format!("the check name {name:?} is used twice in the domain {}", label(store, o)));
            }
        }
        if class == SCHEMA {
            // Entities, domains, enum types, sequences and views share one namespace in a schema, as
            // relations and types do in SQL.
            let mut members: Vec<Obj> = store.list(o, schema::ENTITY).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
            members.extend(domains_of(store, o));
            members.extend(enums_of(store, o));
            members.extend(sequences_of(store, o));
            members.extend(views_of(store, o));
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
            if let Some(p) = size_problem(&type_of(store, o), length, precision, scale) {
                out.push(format!("the size of {o}: {}", p.said()));
            }
            if let Some(p) = generation_problem(store, o) {
                out.push(format!("the generation of {o}: {}", p.said()));
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
                        out.push(format!("the key {}: {}", label(store, o), p.said()));
                    }
                }
                None => out.push(format!("the key {} belongs to no entity", label(store, o))),
            }
        }
        if class == ATTRIBUTE {
            if let Some(p) = enum_default_problem(store, o) {
                out.push(format!("{o}: {}", p.said()));
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
        if class == SEQUENCE {
            if let Some((_, p)) = sequence_problem(&sequence_numbers(store, o)) {
                out.push(format!("the sequence {}: {p}", label(store, o)));
            }
            if let (Some(s), Some(a)) = (schema_of(store, o), owner_of_sequence(store, o))
                && let Some(p) = owned_by_problem(store, s, a)
            {
                out.push(format!("the sequence {}: {p}", label(store, o)));
            }
        }
        if class == VIEW
            && let Some(p) = query_problem(&text_of(store, o, view::QUERY))
        {
            out.push(format!("the view {}: {p}", label(store, o)));
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
                out.push(format!("the check {}: {}", label(store, o), p.said()));
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
                    if let Some(p) = key_problem(store, e, k, &parts.columns, [parts.on_delete, parts.on_update], parts.relationship) {
                        out.push(format!("the foreign key {}: {}", label(store, o), p.said()));
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
            // A relationship may join tables of two schemas (slice S7); it belongs to the schema of
            // one of its ends.
            let ends: Vec<Option<Obj>> =
                [relationship::FROM, relationship::TO].into_iter().map(|p| store.get(o, p).ok().and_then(|v| v.as_ref())).collect();
            let here = schema_of(store, o);
            if ends.iter().any(Option::is_none) {
                out.push(format!("{o} lacks an end"));
            } else if here.is_none() || !ends.iter().flatten().any(|e| schema_of(store, *e) == here) {
                out.push(format!("{o} belongs to neither of its ends' schemas"));
            }
        }
    }
    out
}
