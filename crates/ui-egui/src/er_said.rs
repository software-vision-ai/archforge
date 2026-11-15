//! The ER commands' typed refusals in the user's words (AR-438): `panels::said` asks here last, so
//! a reason UML says too keeps UML's row. Only the ER kinds are matched, by the words the interface
//! uses for them; a kind not listed is left to the engine's English. A word none of a choice's
//! words is said under the choice's label. A key's columns (AR-492): none, a column of another
//! table, one that may be null in a primary key, a key a foreign key points at, and no primary key.
//! Found inside a step (AR-493): a key that would break, with why in the same words, and keys built
//! on each other too deep.

use archforge_engine::{NotOneOf, Reason, WordPlace};

use crate::i18n::{fmt, tr};

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
        Reason::WouldBreak { kind: "primary key" | "unique constraint", name, why } => {
            fmt(&tr("The key {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(why)?)])
        }
        Reason::TooDeep { kind: "key", max } => {
            fmt(&tr("Keys built on each other run deeper than {max} levels here, or in a loop."), &[("max", &max.to_string())])
        }
        Reason::NotOneOf(no) => return not_one_of_said(no),
        _ => return None,
    })
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
