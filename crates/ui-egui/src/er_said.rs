//! The ER commands' typed refusals in the user's words (AR-438): `panels::said` asks here last, so
//! a reason UML says too keeps UML's row. Only the ER kinds are matched, by the words the interface
//! uses for them; a kind not listed is left to the engine's English. A word none of a choice's
//! words is said under the choice's label. A key's columns (AR-492): none, a column of another
//! table, one that may be null in a primary key, a key a foreign key points at, and no primary key.
//! The key a foreign key points at (AR-501): one of another table, one without columns, one it
//! points at already, and identifying relationships that would run in a cycle. Found inside a step
//! (AR-493): a key that would break, with why in the same words, and keys built on each other too
//! deep. A column's generation (AR-494): the rule it breaks, also for a column on
//! a domain that would break, and a key or identity column asked to be nullable. A column's own
//! (AR-495): a default or collation its domain gives, an unlink of one that follows no column, and
//! a place past its table's last; a default none of an enum type's literals is UML's row. A
//! column's or a domain's size (AR-496): the rule it breaks, with the type and the size. Checks
//! (AR-499): a domain's checks too many, one named twice, an item of another shape and an item's
//! key the list does not take (said without a kind, for any list of items); an empty expression is
//! a constraint's row.
//! A removal (AR-508): an element removed where it is edited, and a part of a subtype's key
//! without its subtype.
//! What a column is typed by (AR-497): a type name that holds its own size, a domain and an enum
//! type together, and a type name or a size beside either.

use archforge_engine::{GenerationRule, NotOneOf, Reason, SizeRule, WordPlace};

use crate::i18n::{fmt, tr, trn};

/// The kinds of ER element a refusal names, as `archforge_er`'s commands type them.
const KINDS: [&str; 16] = [
    "schema",
    "table",
    "column",
    "relationship",
    "domain",
    "enum type",
    "sequence",
    "view",
    "primary key",
    "unique constraint",
    "foreign key",
    "check",
    "index",
    "subtype",
    "key",
    "key column",
];

/// A key's columns, as a list refused names them.
const KEY_COLUMN: &str = "key column";

/// An ER command's refusal in the user's words; none for a reason or a kind this doesn't say.
pub(crate) fn said(reason: &Reason) -> Option<String> {
    let what = |kind: &str| KINDS.contains(&kind).then(|| crate::panels::a_kind(kind)).flatten();
    Some(match reason {
        Reason::IdEmpty { kind } => fmt(&tr("No id is given. Give the id of {what}."), &[("what", &what(kind)?)]),
        Reason::NotA { kind, name } => fmt(&tr("{name} is not {what}."), &[("name", name), ("what", &what(kind)?)]),
        Reason::IdsExpected { kind: KEY_COLUMN } => tr("Give the key's columns as a list of ids."),
        Reason::TooMany { kind: KEY_COLUMN, max } => fmt(&tr("A key holds at most {max} columns."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: KEY_COLUMN, .. } => tr("An element is named twice. Name each element once."),
        Reason::TooFew { kind: KEY_COLUMN, min: 1 } => tr("A key needs at least one column."),
        Reason::NotColumnOf { column, table } => fmt(&tr("{column} is not a column of {table}."), &[("column", column), ("table", table)]),
        Reason::NullableKeyColumn { name } => fmt(&tr("{name} may be null, which a primary key column never is."), &[("name", name)]),
        Reason::KeyInUse { foreign_key, table } => fmt(
            &tr("The foreign key {foreign_key} points at this key of {table}. Change or remove it first."),
            &[("foreign_key", foreign_key), ("table", table)],
        ),
        Reason::NoPrimaryKey { table } => fmt(&tr("{table} has no primary key."), &[("table", table)]),
        Reason::KeyNotOf { table } => fmt(&tr("The key is not one of {table}'s keys."), &[("table", table)]),
        Reason::NoKeyColumns { table } => fmt(&tr("The key of {table} has no columns to refer to."), &[("table", table)]),
        Reason::IdentifyingCycle { child, parent } => {
            fmt(&tr("Identifying relationships would run in a cycle between {child} and {parent}."), &[("child", child), ("parent", parent)])
        }
        Reason::Again { relationship: "foreign key" } => tr("The foreign key points at this key already. Name the columns to change them."),
        Reason::WouldBreak { kind: "primary key" | "unique constraint", name, why } => {
            fmt(&tr("The key {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(why)?)])
        }
        Reason::TooDeep { kind: "key", max } => {
            fmt(&tr("Keys built on each other run deeper than {max} levels here, or in a loop."), &[("max", &max.to_string())])
        }
        Reason::RemovedWhereEdited => tr("This element is removed where it is edited."),
        Reason::SubtypeKeyPart => tr("This is part of a subtype's key, which goes only with its subtype."),
        Reason::WouldBreak { kind: "column", name, why } => {
            fmt(&tr("The column {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(why)?)])
        }
        Reason::Sequence(rule) => sequence_said(rule),
        Reason::ColumnOfOtherSchema { column } => {
            fmt(&tr("{column} is a column of another schema. A sequence's owner is a column of its schema."), &[("column", column)])
        }
        Reason::InNoTable { column } => fmt(&tr("The column {column} belongs to no table."), &[("column", column)]),
        Reason::NotIntegerType { column, ty } if ty.is_empty() => {
            fmt(&tr("{column} has no type. A sequence's owner has a whole-number type."), &[("column", column)])
        }
        Reason::NotIntegerType { column, ty } => {
            fmt(&tr("{column} is of type {type}. A sequence's owner has a whole-number type."), &[("column", column), ("type", ty)])
        }
        Reason::Generation(rule) => generation_said(*rule),
        Reason::NotNullable => tr("A key or identity column may never be null."),
        Reason::SizeMisfit { rule, ty, value } => size_said(*rule, ty, *value),
        Reason::SizeInTypeName => tr("A type name holds no size; give the length, precision or scale instead."),
        Reason::DomainAndEnum => tr("A column takes a domain or an enum type, not both."),
        Reason::DomainGives { what: "type" } => tr("A column on a domain takes the domain's type."),
        Reason::DomainGives { what: "size" } => tr("A column on a domain takes the domain's size."),
        Reason::DomainGives { what: "default" } => tr("A column on a domain takes the domain's default."),
        Reason::DomainGives { what: "collation" } => tr("A column on a domain takes the domain's collation."),
        Reason::EnumTypeTakesNo { what: "type" } => tr("A column of an enum type takes no type name of its own."),
        Reason::EnumTypeTakesNo { what: "size" } => tr("A column of an enum type takes no size."),
        Reason::FollowsNoColumn { name } => fmt(&tr("{name} follows no column."), &[("name", name)]),
        Reason::PastLastColumn { columns } => trn("The table has {n} column.", "The table has {n} columns.", u64::try_from(*columns).ok()?),
        Reason::TooMany { kind: "check", max } => fmt(&tr("Give at most {max} checks."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: "check", name } => fmt(&tr("Two checks are named {name}. Give each check its own name."), &[("name", name)]),
        Reason::ItemsExpected { kind: "check" } => tr("Give each check as text, or as an object with an expression and a name."),
        Reason::UnknownKey { key, takes } => {
            let template = if key.is_empty() {
                tr("An item holds a key without a name. It takes {takes}.")
            } else {
                tr("An item takes no {key}. It takes {takes}.")
            };
            fmt(&template, &[("key", key), ("takes", &takes.join(", "))])
        }
        Reason::NotOneOf(no) => return not_one_of_said(no),
        _ => return None,
    })
}

/// The rule a sequence's numbers break, in the user's words (AR-507).
fn sequence_said(rule: &archforge_engine::SequenceRule) -> String {
    use archforge_engine::SequenceRule;
    let (min, max) = (i64::MIN.to_string(), i64::MAX.to_string());
    let unread = |template: String, value: &str| fmt(&template, &[("value", value), ("min", &min), ("max", &max)]);
    match rule {
        SequenceRule::IncrementZero => tr("The increment can't be 0. A sequence moves by at least 1."),
        SequenceRule::MinimumUnread { value } => unread(tr("The minimum \"{value}\" is not a whole number between {min} and {max}."), value),
        SequenceRule::MaximumUnread { value } => unread(tr("The maximum \"{value}\" is not a whole number between {min} and {max}."), value),
        SequenceRule::MaximumNotAbove { maximum, minimum } => fmt(
            &tr("The maximum {maximum} is not greater than the minimum {minimum}."),
            &[("maximum", &maximum.to_string()), ("minimum", &minimum.to_string())],
        ),
        SequenceRule::StartBelow { start, minimum } => {
            fmt(&tr("The start {start} is below the minimum {minimum}."), &[("start", &start.to_string()), ("minimum", &minimum.to_string())])
        }
        SequenceRule::StartAbove { start, maximum } => {
            fmt(&tr("The start {start} is above the maximum {maximum}."), &[("start", &start.to_string()), ("maximum", &maximum.to_string())])
        }
    }
}

/// The rule a column's generation breaks, in the user's words (AR-494).
fn generation_said(rule: GenerationRule) -> String {
    match rule {
        GenerationRule::ExpressionNeedsComputed => tr("Only a computed column takes an expression."),
        GenerationRule::ComputedNeedsExpression => tr("A computed column needs an expression."),
        GenerationRule::ComputedTakesNoDefault => tr("A computed column takes no default."),
        GenerationRule::IdentityNeedsInteger => tr("An identity column needs an integer type."),
        GenerationRule::IdentityNotNullable => tr("An identity column may never be null."),
        GenerationRule::IdentityTakesNoDefault => tr("An identity column takes no default."),
        GenerationRule::IdentityTakesNoExpression => tr("An identity column takes no expression."),
    }
}

/// A size that does not fit its type, by the rule it breaks, in the user's words (AR-496); a column
/// without a type name of its own takes no size at all.
fn size_said(rule: SizeRule, ty: &str, value: i64) -> String {
    let value = value.to_string();
    let args = [("type", ty), ("value", value.as_str())];
    match rule {
        SizeRule::OutOfRange => fmt(&tr("The size {value} is not between 1 and 65535."), &args),
        SizeRule::TakesNoLength | SizeRule::TakesNoPrecision | SizeRule::TakesNoScale if ty.is_empty() => {
            tr("A column without a type name of its own takes no size.")
        }
        SizeRule::TakesNoLength => fmt(&tr("{type} takes no length."), &args),
        SizeRule::TakesNoPrecision => fmt(&tr("{type} takes no precision."), &args),
        SizeRule::TakesNoScale => fmt(&tr("{type} takes no scale."), &args),
        SizeRule::BinaryTooLong => fmt(&tr("A BINARY holds at most 255 bytes, not {value}."), &args),
        SizeRule::ScaleOverPrecision => fmt(&tr("The scale {value} is more than the precision."), &args),
        SizeRule::ScaleWithoutPrecision => tr("A scale needs a precision."),
    }
}

/// A word that is none of an ER choice's words, under the label the interface shows the choice by,
/// with the words it takes: an index column's direction or nulls (AR-491), an empty word (the
/// commands trim it, so one of only spaces too) said as no choice given; none for a place that is
/// not ER's.
fn not_one_of_said(no: &NotOneOf) -> Option<String> {
    let field = match no.what {
        WordPlace::IndexDirection => tr("Direction"),
        WordPlace::IndexNulls => tr("Nulls"),
        _ => return None,
    };
    let words = no.known.join(", ");
    let args = [("field", field.as_str()), ("word", no.word.as_str()), ("words", words.as_str())];
    if no.word.is_empty() {
        return Some(fmt(&tr("{field}: no choice is given. The choices are {words}."), &args));
    }
    Some(fmt(&tr("{field}: {word} is not one of the choices. The choices are {words}."), &args))
}
