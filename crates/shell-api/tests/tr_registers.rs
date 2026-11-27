//! `tr` registers the interfaces' own catalogue when it is the process's first lookup. A test
//! binary of its own, so that nothing else in its process has looked a text up or registered one
//! first.

use archforge_shell_api::i18n::{set_language, tr};

/// The process's first lookup, by `tr`, finds an interfaces' row in German.
#[test]
fn tr_registers_the_interfaces_catalogues() {
    assert!(set_language("de"));
    assert_eq!(tr("There is nothing to undo."), "Es gibt nichts rückgängig zu machen.");
}
