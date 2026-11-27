//! `trn` registers the interfaces' own catalogue when it is the process's first lookup. A test
//! binary of its own, so that nothing else in its process has looked a text up or registered one
//! first.

use archforge_shell_api::i18n::{Rows, register, set_language, tr, trn};

/// A table registered after the process's first lookup, with a row under a key of the interfaces'.
static LATER: &[Rows] = &[Rows { code: "de", catalogue: "later.tsv", text: "str\t\tThere is nothing to undo.\tLater row\n" }];

/// The process's first lookup, by `trn`, registers the interfaces' catalogue. The interfaces say no
/// plural of their own, so a table registered after it tells: a later table's row replaces an
/// earlier one's, so the later row is found only if `trn` registered the interfaces' table first,
/// and not if the next lookup (`tr`) registered it after.
#[test]
fn trn_registers_the_interfaces_catalogues() {
    assert!(set_language("de"));
    assert_eq!(trn("{n} unregistered day", "{n} unregistered days", 2), "2 unregistered days");
    assert_eq!(register(LATER), Ok(()));
    assert_eq!(tr("There is nothing to undo."), "Later row", "the interfaces' table was registered before the later one");
}
