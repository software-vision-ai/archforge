//! A property's modifiers (AR-126): its default value; whether it is derived, read only, ordered and
//! unique, and leaf (AR-138); and the properties it subsets and redefines. They are set with the attribute commands
//! and with `uml.association.set-end`, never on their own, and kept in the property's own UML
//! fields: `defaultValue`, `isDerived`, `isReadOnly`, `isOrdered`, `isUnique`, `isLeaf`,
//! `subsettedProperty` and `redefinedProperty`. Static stays where it was, beside them.
//!
//! A default value is typed as text and read by the property's type: an Integer's as a whole
//! number, a Real's as a number, a Boolean's as true or false, an UnlimitedNatural's as a whole
//! number of 0 or more or `*`, a String's as any text, an enumeration's as one of its literals,
//! kept as an InstanceValue that refers to the literal; any other type, or none, takes the text as
//! an opaque expression's body. A default that does not read is refused, naming the value and the
//! form its type accepts.
//!
//! Subsets offers the other properties of the property's context and of the classifiers that
//! context specializes, whose type the property's type conforms to, whose upper bound is not below
//! the property's, and whose name differs from the property's (UML 2.5.1, 9.9.17). Redefines
//! offers the properties of the classifiers the context specializes alone, those the property is
//! consistent with: a conforming type, a multiplicity whose bounds both lie within the target's,
//! and composite wherever the target is, never a leaf property, which nothing may redefine; a
//! property is made leaf only while nothing redefines it. An attribute's context is its classifier; an association
//! end's is the classifier at the association's other end, so the properties of a classifier are
//! its attributes and the ends that face it. A property without a name is never offered: it could
//! not be named in the braces.

use archforge_mof::{Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, name_of, refuse, short, uuid_of};
use crate::{
    ENUMERATION, ENUMERATION_LITERAL, INSTANCE_VALUE, LITERAL_BOOLEAN, LITERAL_INTEGER, LITERAL_REAL, LITERAL_STRING, LITERAL_UNLIMITED_NATURAL,
    OPAQUE_EXPRESSION, PROPERTY, association, instance_value, literal_boolean, literal_integer, literal_real, literal_string,
    literal_unlimited_natural, multiplicity_element, opaque_expression, property, structural_feature, typed_element,
};
use archforge_engine::{CommandError, Params};

/// The longest default value, in characters.
pub const MAX_DEFAULT: usize = 1000;
/// The most properties one property subsets, and the most it redefines.
pub const MAX_TARGETS: usize = 32;
/// The most literals a refusal lists as the accepted ones.
const LISTED: usize = 12;

/// The live element `o` names under the single reference `prop`, when it names one.
fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// The live elements `o` lists under `prop`, in their order.
fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

/// The boolean `prop` of `o`, `otherwise` when it is unset.
fn flag(store: &Store, o: Obj, prop: archforge_mof::PropId, otherwise: bool) -> bool {
    store.get(o, prop).ok().and_then(|v| v.as_bool()).unwrap_or(otherwise)
}

/// How a default value is read, by the property's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultForm {
    /// The built-in Integer: a whole number.
    Integer,
    /// The built-in Real: a number.
    Real,
    /// The built-in Boolean: true or false.
    Boolean,
    /// The built-in UnlimitedNatural: a whole number of 0 or more, or `*`.
    UnlimitedNatural,
    /// The built-in String: any text.
    String,
    /// An enumeration: one of its literals.
    Enumeration(Obj),
    /// Any other type, or none: the text of an opaque expression.
    Opaque,
}

impl DefaultForm {
    /// The form of the built-in type named `name`, exactly; none for any other name.
    pub fn built_in(name: &str) -> Option<DefaultForm> {
        Some(match name {
            "Integer" => DefaultForm::Integer,
            "Real" => DefaultForm::Real,
            "Boolean" => DefaultForm::Boolean,
            "UnlimitedNatural" => DefaultForm::UnlimitedNatural,
            "String" => DefaultForm::String,
            _ => return None,
        })
    }

    /// The form a property typed by `ty` reads its default in.
    pub fn of(store: &Store, ty: Option<Obj>) -> DefaultForm {
        match ty {
            Some(t) if crate::members::is_built_in(store, t) => DefaultForm::built_in(&name_of(store, t)).unwrap_or(DefaultForm::Opaque),
            Some(t) if store.class_of(t).ok() == Some(ENUMERATION) => DefaultForm::Enumeration(t),
            _ => DefaultForm::Opaque,
        }
    }

    /// What the form accepts, as a refusal says it: the type and an example, or an enumeration's
    /// literals.
    pub fn accepted(self, store: &Store) -> String {
        match self {
            DefaultForm::Integer => "an Integer's default is a whole number, such as 42 or -7".into(),
            DefaultForm::Real => "a Real's default is a number, such as 2.5 or -1e3".into(),
            DefaultForm::Boolean => "a Boolean's default is true or false".into(),
            DefaultForm::UnlimitedNatural => "an UnlimitedNatural's default is a whole number of 0 or more, or *".into(),
            DefaultForm::String => "a String's default is any text".into(),
            DefaultForm::Opaque => "this type's default is any text".into(),
            DefaultForm::Enumeration(e) => {
                let names: Vec<String> = crate::commands::literals(store, e).into_iter().map(|l| name_of(store, l)).collect();
                if names.is_empty() {
                    return format!("the enumeration {} has no literals yet, so it takes no default", name_of(store, e));
                }
                let more = names.len().saturating_sub(LISTED);
                let shown = names.into_iter().take(LISTED).collect::<Vec<_>>().join(", ");
                let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
                format!("a default of the enumeration {} is one of its literals: {shown}{tail}", name_of(store, e))
            }
        }
    }
}

/// A default value as it is kept, read from its text.
#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    Integer(i64),
    Real(f64),
    Boolean(bool),
    /// None is `*`.
    Unlimited(Option<u64>),
    String(String),
    /// The literal of an enumeration, kept as an InstanceValue that refers to it.
    Literal(Obj),
    /// An opaque expression's body.
    Opaque(String),
}

/// Why a typed default does not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultError {
    /// Longer than [`MAX_DEFAULT`] characters.
    TooLong,
    /// Not of the form its type accepts: the value as typed, cut short, and the form.
    Malformed(String, DefaultForm),
}

impl DefaultError {
    /// Why, as a refusal says it: the value and the form its type accepts.
    pub fn said(&self, store: &Store) -> String {
        match self {
            DefaultError::TooLong => format!("the default value is longer than {MAX_DEFAULT} characters; a default is at most {MAX_DEFAULT}"),
            DefaultError::Malformed(value, form) => format!("the default value {value:?} does not read: {}", form.accepted(store)),
        }
    }
}

/// Whether `s` is a whole number as a default writes one: an optional sign, then digits.
fn whole(s: &str) -> bool {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The default typed as `text`, read in `form`: none for an empty text (or one of spaces alone).
/// Whitespace around a number, a Boolean or a literal is dropped; a String's or an expression's
/// text is kept as typed.
pub fn parse_default(store: &Store, form: DefaultForm, text: &str) -> Result<Option<DefaultValue>, DefaultError> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > MAX_DEFAULT {
        return Err(DefaultError::TooLong);
    }
    let bad = || DefaultError::Malformed(short(t), form);
    Ok(Some(match form {
        DefaultForm::Integer => DefaultValue::Integer(if whole(t) { t.parse::<i64>().map_err(|_| bad())? } else { return Err(bad()) }),
        DefaultForm::Real => {
            // Digits, a point, an exponent and signs only: never inf or NaN, and always finite.
            let plain = t.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'));
            let n = if plain && t.bytes().any(|b| b.is_ascii_digit()) { t.parse::<f64>().map_err(|_| bad())? } else { return Err(bad()) };
            if !n.is_finite() {
                return Err(bad());
            }
            DefaultValue::Real(n)
        }
        DefaultForm::Boolean => match t.to_ascii_lowercase().as_str() {
            "true" => DefaultValue::Boolean(true),
            "false" => DefaultValue::Boolean(false),
            _ => return Err(bad()),
        },
        DefaultForm::UnlimitedNatural => {
            if t == "*" {
                DefaultValue::Unlimited(None)
            } else if t.bytes().all(|b| b.is_ascii_digit()) {
                // Kept in a signed 64-bit column: no larger.
                let n = t.parse::<u64>().ok().filter(|n| i64::try_from(*n).is_ok()).ok_or_else(bad)?;
                DefaultValue::Unlimited(Some(n))
            } else {
                return Err(bad());
            }
        }
        DefaultForm::String => DefaultValue::String(text.to_string()),
        DefaultForm::Opaque => DefaultValue::Opaque(text.to_string()),
        DefaultForm::Enumeration(e) => {
            let wanted = t.to_lowercase();
            let l = crate::commands::literals(store, e).into_iter().find(|l| name_of(store, *l).to_lowercase() == wanted).ok_or_else(bad)?;
            DefaultValue::Literal(l)
        }
    }))
}

/// The default of `p` as it is typed: a number, true or false, `*`, a String's text, a literal's
/// name or an expression's text; empty when it has none.
pub fn default_text(store: &Store, p: Obj) -> String {
    one(store, p, property::DEFAULT_VALUE).map(|v| spec_text(store, v)).unwrap_or_default()
}

/// A value specification `v` as it is typed: a number, true or false, `*`, a String's text, a
/// literal's name or an expression's text; empty for any other. A default and a stereotype's
/// value (AR-135) are read alike.
pub fn spec_text(store: &Store, v: Obj) -> String {
    let get = |prop| store.get(v, prop).ok();
    match store.class_of(v).ok() {
        Some(c) if c == LITERAL_INTEGER => match get(literal_integer::VALUE) {
            Some(Value::Int(n)) => n.to_string(),
            _ => String::new(),
        },
        Some(c) if c == LITERAL_REAL => match get(literal_real::VALUE) {
            Some(Value::Real(n)) => n.to_string(),
            _ => String::new(),
        },
        Some(c) if c == LITERAL_BOOLEAN => get(literal_boolean::VALUE).and_then(|b| b.as_bool()).map(|b| b.to_string()).unwrap_or_default(),
        Some(c) if c == LITERAL_UNLIMITED_NATURAL => match get(literal_unlimited_natural::VALUE) {
            Some(Value::Unlimited(None)) => "*".into(),
            Some(Value::Unlimited(Some(n))) => n.to_string(),
            _ => String::new(),
        },
        Some(c) if c == LITERAL_STRING => get(literal_string::VALUE).and_then(|s| s.as_str().map(str::to_string)).unwrap_or_default(),
        Some(c) if c == INSTANCE_VALUE => one(store, v, instance_value::INSTANCE).map(|l| name_of(store, l)).unwrap_or_default(),
        Some(c) if c == OPAQUE_EXPRESSION => {
            store.list(v, opaque_expression::BODY).ok().and_then(|b| b.first().and_then(|s| s.as_str().map(str::to_string))).unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// A value specification `v` as a card writes it: a String's text in double quotes, any other as
/// it is typed.
pub fn spec_shown(store: &Store, v: Obj) -> String {
    let text = spec_text(store, v);
    if store.class_of(v).ok() == Some(LITERAL_STRING) { format!("\"{text}\"") } else { text }
}

/// The default of `p` as a card writes it after `=`: a String's text in double quotes, any other
/// as it is typed; none when it has none.
pub fn default_shown(store: &Store, p: Obj) -> Option<String> {
    let v = one(store, p, property::DEFAULT_VALUE)?;
    (!spec_text(store, v).is_empty()).then(|| spec_shown(store, v))
}

/// Gives `p` the default `value`, replacing the one it had; none takes it away.
pub fn set_default(store: &mut Store, p: Obj, value: Option<&DefaultValue>) -> Result<(), CommandError> {
    if let Some(old) = one(store, p, property::DEFAULT_VALUE) {
        store.destroy(old).map_err(fail)?;
    }
    let Some(value) = value else { return Ok(()) };
    let o = make_spec(store, value)?;
    store.set(p, property::DEFAULT_VALUE, Value::Ref(o)).map_err(fail)
}

/// A new value specification holding `value`, owned by nothing yet: a literal of its kind, an
/// InstanceValue for an enumeration's literal, an opaque expression for any other text. A default
/// and a stereotype's value (AR-135) are kept alike.
pub fn make_spec(store: &mut Store, value: &DefaultValue) -> Result<Obj, CommandError> {
    let (class, prop, v) = match value {
        DefaultValue::Integer(n) => (LITERAL_INTEGER, literal_integer::VALUE, Value::Int(*n)),
        DefaultValue::Real(n) => (LITERAL_REAL, literal_real::VALUE, Value::Real(*n)),
        DefaultValue::Boolean(b) => (LITERAL_BOOLEAN, literal_boolean::VALUE, Value::Bool(*b)),
        DefaultValue::Unlimited(n) => (LITERAL_UNLIMITED_NATURAL, literal_unlimited_natural::VALUE, Value::Unlimited(*n)),
        DefaultValue::String(s) => (LITERAL_STRING, literal_string::VALUE, Value::str(s)),
        DefaultValue::Literal(l) => (INSTANCE_VALUE, instance_value::INSTANCE, Value::Ref(*l)),
        DefaultValue::Opaque(s) => {
            let e = store.create(OPAQUE_EXPRESSION).map_err(fail)?;
            store.push(e, opaque_expression::BODY, Value::str(s)).map_err(fail)?;
            return Ok(e);
        }
    };
    let o = store.create(class).map_err(fail)?;
    store.set(o, prop, v).map_err(fail)?;
    Ok(o)
}

/// The defaults that refer to a literal, each with that literal: what removing the literal, or the
/// enumeration or package that holds it, takes along, since an InstanceValue needs its instance.
pub fn literal_defaults(store: &Store) -> Vec<(Obj, Obj)> {
    store
        .alive()
        .filter(|o| store.class_of(*o).ok() == Some(INSTANCE_VALUE))
        .filter_map(|v| {
            let l = one(store, v, instance_value::INSTANCE)?;
            (store.class_of(l).ok() == Some(ENUMERATION_LITERAL)).then_some((v, l))
        })
        .collect()
}

/// A property's bounds: its lower one, and its upper one (none: many); one each when it names
/// none, as UML has it.
pub fn bounds(store: &Store, p: Obj) -> (u64, Option<u64>) {
    let lower = match one(store, p, multiplicity_element::LOWER_VALUE).map(|v| store.get(v, literal_integer::VALUE)) {
        Some(Ok(Value::Int(n))) => u64::try_from(n).unwrap_or(0),
        _ => 1,
    };
    let upper = match one(store, p, multiplicity_element::UPPER_VALUE).map(|v| store.get(v, literal_unlimited_natural::VALUE)) {
        Some(Ok(Value::Unlimited(n))) => n,
        _ => Some(1),
    };
    (lower, upper)
}

/// Whether `p` holds more than one value: its upper bound is above 1 or many. Ordered and unique
/// say something only then.
pub fn multi_valued(store: &Store, p: Obj) -> bool {
    bounds(store, p).1.is_none_or(|u| u > 1)
}

/// Whether `p` is composite: an end whose aggregation is composite.
pub fn is_composite(store: &Store, p: Obj) -> bool {
    matches!(store.get(p, property::AGGREGATION), Ok(Value::Enum(2)))
}

/// The modifier flags of `p`: derived, read only, ordered and unique.
pub fn flags(store: &Store, p: Obj) -> (bool, bool, bool, bool) {
    (
        flag(store, p, property::IS_DERIVED, false),
        flag(store, p, structural_feature::IS_READ_ONLY, false),
        flag(store, p, multiplicity_element::IS_ORDERED, false),
        flag(store, p, multiplicity_element::IS_UNIQUE, true),
    )
}

/// The properties `p` subsets, in their order.
pub fn subsets(store: &Store, p: Obj) -> Vec<Obj> {
    list(store, p, property::SUBSETTED_PROPERTY)
}

/// The properties `p` redefines, in their order.
pub fn redefines(store: &Store, p: Obj) -> Vec<Obj> {
    list(store, p, property::REDEFINED_PROPERTY)
}

/// The words in braces a card writes after `p`: what differs from the usual. Leaf (AR-138) and read
/// only always; ordered and nonunique only for a multi-valued property, where they say something;
/// then each property it subsets and each it redefines, by name.
pub fn braces(store: &Store, p: Obj) -> Vec<String> {
    let (_, read_only, ordered, unique) = flags(store, p);
    let multi = multi_valued(store, p);
    let mut out = Vec::new();
    if crate::relations::is_leaf(store, p) {
        out.push("leaf".to_string());
    }
    if read_only {
        out.push("readOnly".to_string());
    }
    if multi && ordered {
        out.push("ordered".into());
    }
    if multi && !unique {
        out.push("nonunique".into());
    }
    out.extend(subsets(store, p).into_iter().map(|t| format!("subsets {}", name_of(store, t))));
    out.extend(redefines(store, p).into_iter().map(|t| format!("redefines {}", name_of(store, t))));
    out
}

/// What follows a property's multiplicity on a card or beside its end: ` = default` when it has
/// one, then ` {…}` when [`braces`] has anything to say; empty otherwise.
pub fn suffix(store: &Store, p: Obj) -> String {
    let mut text = String::new();
    if let Some(d) = default_shown(store, p) {
        text.push_str(&format!(" = {d}"));
    }
    let b = braces(store, p);
    if !b.is_empty() {
        text.push_str(&format!(" {{{}}}", b.join(", ")));
    }
    text
}

/// Whether `p` is derived: a card writes `/` before its name.
pub fn is_derived(store: &Store, p: Obj) -> bool {
    flag(store, p, property::IS_DERIVED, false)
}

/// The association `p` is an end of, when it is one.
fn association_of(store: &Store, p: Obj) -> Option<Obj> {
    one(store, p, property::ASSOCIATION)
}

/// A property's context: an attribute's classifier, or the classifiers at an association end's
/// other ends.
pub fn context_of(store: &Store, p: Obj) -> Vec<Obj> {
    match association_of(store, p) {
        Some(a) => {
            list(store, a, association::MEMBER_END).into_iter().filter(|e| *e != p).filter_map(|e| one(store, e, typed_element::TYPE)).collect()
        }
        None => {
            store.container(p).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).is_ok_and(crate::is_classifier)).into_iter().collect()
        }
    }
}

/// The properties of classifier `c`: its attributes, and the association ends whose context it is,
/// those across from an end at `c`.
pub fn properties_of(store: &Store, c: Obj) -> Vec<Obj> {
    let mut out = crate::members::attributes(store, c);
    for (src, prop) in store.referrers(c) {
        if prop != typed_element::TYPE || store.class_of(src).ok() != Some(PROPERTY) {
            continue;
        }
        let Some(a) = association_of(store, src) else { continue };
        out.extend(list(store, a, association::MEMBER_END).into_iter().filter(|e| *e != src));
    }
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|p| seen.insert(*p));
    out
}

/// The classifiers `c` specializes, directly or through others, each once, `c` itself not among
/// them.
pub fn generals_of(store: &Store, c: Obj) -> Vec<Obj> {
    let mut seen = std::collections::BTreeSet::from([c]);
    let mut stack = crate::relations::generals(store, c);
    let mut out = Vec::new();
    while let Some(g) = stack.pop() {
        if seen.insert(g) {
            out.push(g);
            stack.extend(crate::relations::generals(store, g));
        }
    }
    out
}

/// A property as its subsets and redefines are checked against: the property itself (none while
/// it is being added), its context, its type (none for no type, or for a built-in type not made
/// yet, which no property has yet either), its bounds, whether it is composite, and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    pub itself: Option<Obj>,
    pub context: Vec<Obj>,
    pub ty: Option<Obj>,
    pub lower: u64,
    pub upper: Option<u64>,
    pub composite: bool,
    pub name: String,
}

impl Shape {
    /// The shape `p` has in the model.
    pub fn of(store: &Store, p: Obj) -> Shape {
        let (lower, upper) = bounds(store, p);
        Shape {
            itself: Some(p),
            context: context_of(store, p),
            ty: one(store, p, typed_element::TYPE),
            lower,
            upper,
            composite: is_composite(store, p),
            name: name_of(store, p),
        }
    }

    /// The classifiers whose properties Subsets offers: the context, then what it specializes.
    fn subset_owners(&self, store: &Store) -> Vec<Obj> {
        let mut out = self.context.clone();
        for c in &self.context {
            out.extend(generals_of(store, *c));
        }
        let mut seen = std::collections::BTreeSet::new();
        out.retain(|c| seen.insert(*c));
        out
    }

    /// The classifiers whose properties Redefines offers: what the context specializes.
    fn redefine_owners(&self, store: &Store) -> Vec<Obj> {
        let mut out = Vec::new();
        for c in &self.context {
            out.extend(generals_of(store, *c));
        }
        let mut seen = std::collections::BTreeSet::new();
        out.retain(|c| seen.insert(*c));
        out
    }

    /// Whether this shape's type conforms to `target`'s: an untyped target takes any type; a typed
    /// one the same type or one that specializes it.
    fn conforms(&self, store: &Store, target: Obj) -> bool {
        match (one(store, target, typed_element::TYPE), self.ty) {
            (None, _) => true,
            (Some(t), Some(mine)) => mine == t || crate::relations::specializes(store, mine, t),
            (Some(_), None) => false,
        }
    }
}

/// The properties of some classifiers, each once, in their order: where a shape's targets must
/// be, built once per check or listing.
struct Reach {
    order: Vec<Obj>,
    set: std::collections::BTreeSet<Obj>,
}

impl Reach {
    /// The properties of `owners`, each classifier's walked once.
    fn of(store: &Store, owners: &[Obj]) -> Reach {
        let mut set = std::collections::BTreeSet::new();
        let order = owners.iter().flat_map(|c| properties_of(store, *c)).filter(|p| set.insert(*p)).collect();
        Reach { order, set }
    }
}

/// Why a property may not be subset or redefined by a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetProblem {
    /// Not a property at all.
    NotAProperty,
    /// The property itself.
    Itself,
    /// It has no name to be named by in the braces.
    Unnamed,
    /// Not a property of the context or of what the context specializes (Subsets), or not one of
    /// what the context specializes (Redefines).
    OutOfReach,
    /// The same name, whatever the case: a subset takes another.
    SameName,
    /// It subsets the property already, directly or through others: subsets never run round a
    /// cycle.
    Cycle,
    /// Its type is not one the shape's type conforms to.
    Type,
    /// Its upper bound is below the shape's.
    Upper,
    /// The shape's bounds do not both lie within its bounds.
    Multiplicity,
    /// It is composite and the shape is not.
    Composite,
    /// It is leaf (`isLeaf`): nothing may redefine it (AR-138).
    Leaf,
}

impl TargetProblem {
    /// Why, as a refusal says it after the target's id and name, with the rule.
    pub fn why(self, redefining: bool) -> &'static str {
        match (self, redefining) {
            (TargetProblem::NotAProperty, _) => "is not a property; an attribute or an association's end is",
            (TargetProblem::Itself, false) => "is the property itself; a property subsets another",
            (TargetProblem::Itself, true) => "is the property itself; a property redefines another",
            (TargetProblem::Unnamed, _) => "has no name, so the braces could not name it; name it first",
            (TargetProblem::OutOfReach, false) => {
                "is not a property of this property's classifier or of a classifier it specializes, which is where a subset's properties are"
            }
            (TargetProblem::OutOfReach, true) => {
                "is not a property of a classifier this property's classifier specializes, which is what it may redefine"
            }
            (TargetProblem::SameName, _) => "has this property's name, whatever the case; a subset takes another name",
            (TargetProblem::Cycle, _) => "subsets this property already, directly or through others, and subsets never run round a cycle",
            (TargetProblem::Type, _) => "has a type this property's type does not conform to; its type must be that type or one that specializes it",
            (TargetProblem::Upper, _) => "allows fewer values than this property; a subset's upper bound is not above the one it subsets",
            (TargetProblem::Multiplicity, _) => "has a multiplicity this property's lower and upper bounds do not both lie within",
            (TargetProblem::Composite, _) => "is composite and this property is not; only a composite property redefines a composite one",
            (TargetProblem::Leaf, _) => {
                "is leaf (isLeaf), so nothing may redefine it; clear leaf on it with uml.attribute.set or uml.association.set-end first"
            }
        }
    }
}

/// Whether `t` is a property, with a name, other than the shape's own.
fn basic_problem(store: &Store, shape: &Shape, t: Obj) -> Option<TargetProblem> {
    if store.class_of(t).ok() != Some(PROPERTY) || !store.is_alive(t) {
        return Some(TargetProblem::NotAProperty);
    }
    if shape.itself == Some(t) {
        return Some(TargetProblem::Itself);
    }
    name_of(store, t).trim().is_empty().then_some(TargetProblem::Unnamed)
}

/// Why `shape` may not subset `t`, when it may not, its targets in `reach`. With `named` false the
/// name rule is left out, for a list a shell filters by the name being typed.
fn subsets_problem_in(store: &Store, shape: &Shape, t: Obj, reach: &Reach, named: bool) -> Option<TargetProblem> {
    if let Some(p) = basic_problem(store, shape, t) {
        return Some(p);
    }
    if !reach.set.contains(&t) {
        return Some(TargetProblem::OutOfReach);
    }
    if named && name_of(store, t).to_lowercase() == shape.name.trim().to_lowercase() {
        return Some(TargetProblem::SameName);
    }
    if shape.itself.is_some_and(|me| subsets_reaches(store, t, me)) {
        return Some(TargetProblem::Cycle);
    }
    if !shape.conforms(store, t) {
        return Some(TargetProblem::Type);
    }
    let (_, upper) = bounds(store, t);
    let above = match (shape.upper, upper) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(mine), Some(theirs)) => mine > theirs,
    };
    above.then_some(TargetProblem::Upper)
}

/// Whether `from` subsets `to`, directly or through the properties it subsets, and so on: a
/// seen-set bounds the walk, so a cycle the model holds already never hangs it.
fn subsets_reaches(store: &Store, from: Obj, to: Obj) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = subsets(store, from);
    while let Some(p) = stack.pop() {
        if p == to {
            return true;
        }
        if seen.insert(p) {
            stack.extend(subsets(store, p));
        }
    }
    false
}

/// Why `shape` may not redefine `t`, when it may not, its targets in `reach`.
fn redefines_problem_in(store: &Store, shape: &Shape, t: Obj, reach: &Reach) -> Option<TargetProblem> {
    if let Some(p) = basic_problem(store, shape, t) {
        return Some(p);
    }
    if !reach.set.contains(&t) {
        return Some(TargetProblem::OutOfReach);
    }
    if !shape.conforms(store, t) {
        return Some(TargetProblem::Type);
    }
    let (lower, upper) = bounds(store, t);
    let upper_within = match (shape.upper, upper) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(mine), Some(theirs)) => mine <= theirs,
    };
    if shape.lower < lower || !upper_within {
        return Some(TargetProblem::Multiplicity);
    }
    if is_composite(store, t) && !shape.composite {
        return Some(TargetProblem::Composite);
    }
    crate::relations::is_leaf(store, t).then_some(TargetProblem::Leaf)
}

/// Why `shape` may not subset `t`, when it may not.
pub fn subsets_problem(store: &Store, shape: &Shape, t: Obj) -> Option<TargetProblem> {
    subsets_problem_in(store, shape, t, &Reach::of(store, &shape.subset_owners(store)), true)
}

/// Why `shape` may not redefine `t`, when it may not.
pub fn redefines_problem(store: &Store, shape: &Shape, t: Obj) -> Option<TargetProblem> {
    redefines_problem_in(store, shape, t, &Reach::of(store, &shape.redefine_owners(store)))
}

/// The properties `shape` may subset, each once, in the order of its classifiers, the context
/// first; each classifier's properties are walked once.
pub fn subsets_candidates(store: &Store, shape: &Shape) -> Vec<Obj> {
    subsets_candidates_named(store, shape, true)
}

/// [`subsets_candidates`], with the name rule left out when `named` is false: the shell keeps
/// that list while a name is typed and drops the property named alike itself.
pub fn subsets_candidates_named(store: &Store, shape: &Shape, named: bool) -> Vec<Obj> {
    let reach = Reach::of(store, &shape.subset_owners(store));
    reach.order.iter().copied().filter(|t| subsets_problem_in(store, shape, *t, &reach, named).is_none()).collect()
}

/// The properties `shape` may redefine, each once, in the order of the classifiers its context
/// specializes; each classifier's properties are walked once.
pub fn redefines_candidates(store: &Store, shape: &Shape) -> Vec<Obj> {
    let reach = Reach::of(store, &shape.redefine_owners(store));
    reach.order.iter().copied().filter(|t| redefines_problem_in(store, shape, *t, &reach).is_none()).collect()
}

/// The properties `shape` could redefine but for their being leaf (AR-138), in the order of
/// [`redefines_candidates`]: a shell shows them with the reason they may not be picked.
pub fn redefines_leaf(store: &Store, shape: &Shape) -> Vec<Obj> {
    let reach = Reach::of(store, &shape.redefine_owners(store));
    reach.order.iter().copied().filter(|t| redefines_problem_in(store, shape, *t, &reach) == Some(TargetProblem::Leaf)).collect()
}

/// `targets` as a refusal lists the accepted ones: at most [`LISTED`] as `name (Class)`, then how
/// many more; "none" when there are none.
fn listed(store: &Store, targets: &[Obj]) -> String {
    if targets.is_empty() {
        return "none".into();
    }
    let shown: Vec<String> = targets
        .iter()
        .take(LISTED)
        .map(|t| match context_of(store, *t).first() {
            Some(c) => format!("{} ({})", name_of(store, *t), name_of(store, *c)),
            None => name_of(store, *t),
        })
        .collect();
    let more = targets.len().saturating_sub(LISTED);
    if more > 0 { format!("{} and {more} more", shown.join(", ")) } else { shown.join(", ") }
}

/// A property's modifiers as a command sets them, read and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Modifiers {
    pub default: Option<DefaultValue>,
    pub derived: bool,
    pub read_only: bool,
    pub ordered: bool,
    pub unique: bool,
    pub leaf: bool,
    pub subsets: Vec<Obj>,
    pub redefines: Vec<Obj>,
}

/// The properties under `key`, at most [`MAX_TARGETS`], each a live element named once: each item
/// an id as text, or an object with its `id`, as the reads give them (`{id, name}`).
fn targets(store: &Store, items: &[Json], key: &str) -> Result<Vec<Obj>, CommandError> {
    if items.len() > MAX_TARGETS {
        return Err(refuse(key, format!("{} ids; a property names at most {MAX_TARGETS} here", items.len())));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if let Some(extra) = item.as_object().and_then(|o| o.keys().find(|k| *k != "id" && *k != "name")) {
            return Err(refuse(key, format!("{} is not a key of a property here; an object holds id and name", short(extra))));
        }
        let Some(id) = item.as_str().or_else(|| item.get("id").and_then(Json::as_str)) else {
            return Err(refuse(
                key,
                format!(
                    "{} is not a property's id; the list holds ids as text, or objects with an id as the reads give them",
                    short(&item.to_string())
                ),
            ));
        };
        let o = Uuid::parse(id)
            .and_then(|u| store.by_uuid(u))
            .filter(|o| store.is_alive(*o))
            .ok_or_else(|| refuse(key, format!("there is no element {}; the list holds ids of attributes or association ends", short(id))))?;
        if out.contains(&o) {
            return Err(refuse(key, format!("{id} is named twice; name each property once")));
        }
        out.push(o);
    }
    Ok(out)
}

/// The modifiers the parameters ask of a property shaped `shape`, checked before anything
/// changes; `current` is the property as the model has it (none while it is being added), and a
/// modifier the call leaves out keeps its value there (a new property's the usual: no default,
/// unique, nothing else). A default left out is kept as stored; one typed, or kept under a new type,
/// is read by the shape's type, and one that fitted the old type but not a new one refuses the type
/// change, naming the default, never clearing it. Every target is checked against the new shape
/// except one the property had that the rules no longer allowed already, which never blocks an
/// unrelated edit; a refusal names the target, why, and the properties it may name instead. Leaf
/// is taken only while nothing redefines the property (AR-138).
pub fn read(store: &Store, p: &Params, shape: &Shape, current: Option<Obj>) -> Result<Modifiers, CommandError> {
    let stored = current.map(|c| default_text(store, c)).unwrap_or_default();
    let type_changed = current.is_some_and(|c| one(store, c, typed_element::TYPE) != shape.ty);
    let form = DefaultForm::of(store, shape.ty);
    let type_refusal = |typed: &str| {
        let ty = shape.ty.map(|t| name_of(store, t)).unwrap_or_else(|| "no type".into());
        refuse(
            "type",
            format!(
                "the default value {:?} does not fit {ty}: {}; change or clear the default before changing the type",
                short(typed.trim()),
                form.accepted(store)
            ),
        )
    };
    let default = match (p.str("default"), current) {
        // Left out under the same type: the stored value as it is, never read again, so one that
        // would not read back never blocks another edit.
        (None, Some(c)) if !type_changed => stored_default(store, c),
        // Left out under a new type: the kept default must fit it.
        (None, Some(_)) => parse_default(store, form, &stored).map_err(|_| type_refusal(&stored))?,
        (None, None) => None,
        (Some(typed), _) => match parse_default(store, form, typed) {
            Ok(d) => d,
            Err(_) if type_changed && !stored.is_empty() && typed.trim() == stored.trim() => return Err(type_refusal(typed)),
            Err(e) => return Err(refuse("default", e.said(store))),
        },
    };
    let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
    let list_of = |key: &str, kept: Vec<Obj>| -> Result<(Vec<Obj>, Vec<Obj>), CommandError> {
        match p.get(key).and_then(Json::as_array) {
            Some(items) => Ok((targets(store, items, key)?, kept)),
            None => Ok((kept.clone(), kept)),
        }
    };
    // A target the property had and the rules no longer allowed it already is tolerated, so it
    // never blocks an unrelated edit; one this call adds, or one it keeps that its new shape breaks
    // (a multiplicity, an aggregation or a type), is refused.
    let was = current.map(|c| Shape::of(store, c));
    let (subsets_wanted, subsets_had) = list_of("subsets", current.map(|c| subsets(store, c)).unwrap_or_default())?;
    for t in &subsets_wanted {
        let stale = subsets_had.contains(t) && was.as_ref().is_some_and(|w| subsets_problem(store, w, *t).is_some());
        if let Some(problem) = subsets_problem(store, shape, *t).filter(|_| !stale) {
            let may = listed(store, &subsets_candidates(store, shape));
            return Err(refuse("subsets", format!("{} ({}) {}; it may subset: {may}", id(*t), name_of(store, *t), problem.why(false))));
        }
    }
    let (redefines_wanted, redefines_had) = list_of("redefines", current.map(|c| redefines(store, c)).unwrap_or_default())?;
    for t in &redefines_wanted {
        let stale = redefines_had.contains(t) && was.as_ref().is_some_and(|w| redefines_problem(store, w, *t).is_some());
        if let Some(problem) = redefines_problem(store, shape, *t).filter(|_| !stale) {
            let may = listed(store, &redefines_candidates(store, shape));
            return Err(refuse("redefines", format!("{} ({}) {}; it may redefine: {may}", id(*t), name_of(store, *t), problem.why(true))));
        }
    }
    let was_leaf = current.is_some_and(|c| crate::relations::is_leaf(store, c));
    let leaf = p.bool("leaf").unwrap_or(was_leaf);
    if leaf
        && !was_leaf
        && let Some(why) = current.and_then(|c| crate::relations::leaf_problem(store, c))
    {
        return Err(refuse("leaf", why));
    }
    let (derived, read_only, ordered, unique) = current.map(|c| flags(store, c)).unwrap_or((false, false, false, true));
    Ok(Modifiers {
        default,
        derived: p.bool("derived").unwrap_or(derived),
        read_only: p.bool("read_only").unwrap_or(read_only),
        ordered: p.bool("ordered").unwrap_or(ordered),
        unique: p.bool("unique").unwrap_or(unique),
        leaf,
        subsets: subsets_wanted,
        redefines: redefines_wanted,
    })
}

/// The default `p` keeps, as [`parse_default`] would give it back; none when it has none.
fn stored_default(store: &Store, p: Obj) -> Option<DefaultValue> {
    stored_spec(store, one(store, p, property::DEFAULT_VALUE)?)
}

/// The value specification `v` as [`parse_default`] would give it back; none for one of another
/// kind. A default and a stereotype's value (AR-135) are read alike.
pub fn stored_spec(store: &Store, v: Obj) -> Option<DefaultValue> {
    let get = |prop| store.get(v, prop).ok();
    Some(match store.class_of(v).ok()? {
        c if c == LITERAL_INTEGER => DefaultValue::Integer(match get(literal_integer::VALUE)? {
            Value::Int(n) => n,
            _ => return None,
        }),
        c if c == LITERAL_REAL => DefaultValue::Real(match get(literal_real::VALUE)? {
            Value::Real(n) => n,
            _ => return None,
        }),
        c if c == LITERAL_BOOLEAN => DefaultValue::Boolean(get(literal_boolean::VALUE)?.as_bool()?),
        c if c == LITERAL_UNLIMITED_NATURAL => DefaultValue::Unlimited(match get(literal_unlimited_natural::VALUE)? {
            Value::Unlimited(n) => n,
            _ => return None,
        }),
        c if c == LITERAL_STRING => DefaultValue::String(get(literal_string::VALUE)?.as_str()?.to_string()),
        c if c == INSTANCE_VALUE => DefaultValue::Literal(one(store, v, instance_value::INSTANCE)?),
        c if c == OPAQUE_EXPRESSION => DefaultValue::Opaque(spec_text(store, v)),
        _ => return None,
    })
}

/// Replaces the list `prop` of `p` with `want`, in its order.
fn set_list(store: &mut Store, p: Obj, prop: archforge_mof::PropId, want: &[Obj]) -> Result<(), CommandError> {
    if list(store, p, prop) == want {
        return Ok(());
    }
    for old in store.list(p, prop).map_err(fail)? {
        store.remove_value(p, prop, &old).map_err(fail)?;
    }
    for t in want {
        store.push(p, prop, Value::Ref(*t)).map_err(fail)?;
    }
    Ok(())
}

/// Gives `p` the modifiers `m`, replacing the ones it had; a default the same as the one it has
/// is left as it is.
pub fn apply(store: &mut Store, p: Obj, m: &Modifiers) -> Result<(), CommandError> {
    if stored_default(store, p) != m.default {
        set_default(store, p, m.default.as_ref())?;
    }
    store.set(p, property::IS_DERIVED, Value::Bool(m.derived)).map_err(fail)?;
    store.set(p, structural_feature::IS_READ_ONLY, Value::Bool(m.read_only)).map_err(fail)?;
    store.set(p, multiplicity_element::IS_ORDERED, Value::Bool(m.ordered)).map_err(fail)?;
    store.set(p, multiplicity_element::IS_UNIQUE, Value::Bool(m.unique)).map_err(fail)?;
    if m.leaf != crate::relations::is_leaf(store, p) {
        store.set(p, crate::redefinable_element::IS_LEAF, Value::Bool(m.leaf)).map_err(fail)?;
    }
    set_list(store, p, property::SUBSETTED_PROPERTY, &m.subsets)?;
    set_list(store, p, property::REDEFINED_PROPERTY, &m.redefines)
}

/// Takes from `end`'s Subsets and Redefines the properties the rules no longer allow it, as its
/// association's ends moved: what a reconnection does in the same step. Gives each dropped one,
/// with the list it was in (`subsets` or `redefines`).
pub fn drop_stale(store: &mut Store, end: Obj) -> Result<Vec<(&'static str, Obj)>, CommandError> {
    let shape = Shape::of(store, end);
    let subset_reach = Reach::of(store, &shape.subset_owners(store));
    let redefine_reach = Reach::of(store, &shape.redefine_owners(store));
    let had_subsets = subsets(store, end);
    let had_redefines = redefines(store, end);
    let mut dropped = Vec::new();
    let keep_subsets: Vec<Obj> = had_subsets
        .iter()
        .copied()
        .filter(|t| {
            let ok = subsets_problem_in(store, &shape, *t, &subset_reach, true).is_none();
            if !ok {
                dropped.push(("subsets", *t));
            }
            ok
        })
        .collect();
    let keep_redefines: Vec<Obj> = had_redefines
        .iter()
        .copied()
        .filter(|t| {
            let ok = redefines_problem_in(store, &shape, *t, &redefine_reach).is_none();
            if !ok {
                dropped.push(("redefines", *t));
            }
            ok
        })
        .collect();
    set_list(store, end, property::SUBSETTED_PROPERTY, &keep_subsets)?;
    set_list(store, end, property::REDEFINED_PROPERTY, &keep_redefines)?;
    Ok(dropped)
}

/// The modifiers of `p` as the query commands give them; the attribute commands and
/// `uml.association.set-end` take the same `subsets` and `redefines` (`[{id, name}]`) back, so a
/// read can be copied into a set.
pub fn json(store: &Store, p: Obj) -> Json {
    let (derived, read_only, ordered, unique) = flags(store, p);
    let named = |ts: Vec<Obj>| -> Vec<Json> {
        ts.into_iter().map(|t| json!({ "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t) })).collect()
    };
    let default = default_text(store, p);
    json!({
        "default": if default.is_empty() { Json::Null } else { json!(default) },
        "derived": derived,
        "read_only": read_only,
        "ordered": ordered,
        "unique": unique,
        "leaf": crate::relations::is_leaf(store, p),
        "subsets": named(subsets(store, p)),
        "redefines": named(redefines(store, p)),
    })
}

/// Merges the modifiers of `p` ([`json`]) into the object `out`.
pub fn merge_into(store: &Store, p: Obj, out: &mut Json) {
    if let (Some(fields), Json::Object(m)) = (out.as_object_mut(), json(store, p)) {
        fields.extend(m);
    }
}
