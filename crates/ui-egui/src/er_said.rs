//! The ER commands' typed refusals in the user's words (AR-438): `panels::said` asks here last, so
//! a reason UML says too keeps UML's row. Only the ER kinds are matched, by the words the interface
//! uses for them; a kind not listed is left to the engine's English.

use archforge_engine::Reason;

use crate::i18n::{fmt, tr};

/// The kinds of ER element a refusal names, as `archforge_er`'s commands type them.
const KINDS: [&str; 15] = [
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
        _ => return None,
    })
}
