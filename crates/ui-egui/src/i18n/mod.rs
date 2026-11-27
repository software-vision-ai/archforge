//! The shell's translations, under the shell's old path: the machinery (the lookups, the
//! languages and their plural rules, `parse`) comes from the shell's interfaces
//! (`archforge_shell_api::i18n`), so a notation's screens translate with it without the shell.
//! What stays here is the catalogues, one per language: the shell's own, `i18n/<code>.tsv`, and
//! each notation's, `i18n/<notation>/<code>.tsv`, which the notation hands over
//! ([`NotationUi::catalogues`](crate::notation_ui::NotationUi::catalogues)). They are embedded; the
//! shell's own is registered with the machinery before the first lookup through this module
//! ([`registered`]), and a shell registers those of the notations it is given as it sets them up
//! ([`register_notations`]); the build script refuses a malformed row in any of them with the
//! machinery's row check. Each row has four columns: kind (`str` or `plural`), context, key, text,
//! as the machinery describes them. A row only one notation's screens name is in that notation's
//! catalogue, and a row only the shell's interfaces' code says is in theirs
//! (`archforge_shell_api::i18n::CATALOGUES`, which their lookups register); the rest are the
//! shell's, read by the shell, by more than one of them or by the desktop app (its menu). The
//! interfaces' `CATALOGUES` is not re-exported here: this module's is the shell's own.

pub use archforge_shell_api::i18n::*;

/// The shell's catalogue of each language, in the order of `LANGUAGES`, embedded.
pub static CATALOGUES: &[Rows] = &[
    Rows { code: "en", catalogue: "en.tsv", text: include_str!("en.tsv") },
    Rows { code: "de", catalogue: "de.tsv", text: include_str!("de.tsv") },
    Rows { code: "es", catalogue: "es.tsv", text: include_str!("es.tsv") },
    Rows { code: "fr", catalogue: "fr.tsv", text: include_str!("fr.tsv") },
    Rows { code: "pt-PT", catalogue: "pt-PT.tsv", text: include_str!("pt-PT.tsv") },
    Rows { code: "ja", catalogue: "ja.tsv", text: include_str!("ja.tsv") },
    Rows { code: "zh-Hans", catalogue: "zh-Hans.tsv", text: include_str!("zh-Hans.tsv") },
];

/// Every catalogue table a shell with the notations' tables `notations` registers: its own, then
/// each of `notations`, in their order, so a notation's row under a key of the shell's would
/// replace the shell's.
pub(crate) fn tables(notations: impl IntoIterator<Item = &'static [Rows]>) -> impl Iterator<Item = &'static [Rows]> {
    std::iter::once(CATALOGUES).chain(notations)
}

/// Registers the shell's own catalogue table once, before the first lookup through this module:
/// what is said before any shell exists, or with none at hand (the host's words on a thread of its
/// own), is the shell's own and is found in the user's language; a notation's rows are said by
/// the notation's code, which runs under a shell that registered them ([`register_notations`]). A
/// refusal, which the tests rule out, is said in the log.
pub(crate) fn registered() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Err(e) = register(CATALOGUES) {
            log::warn!("{e}");
        }
    });
}

/// Registers the catalogue tables of a shell with the notations' tables `notations` ([`tables`]),
/// the shell's own first: as a shell sets up the notations of the screens parts it is given
/// ([`Shell::new`](crate::Shell::new), `ScreensRegistry::catalogues`), so nothing the shell's
/// interfaces or the notations say, a refusal among them, is looked up before they are in. The
/// registry is the whole process's and a table registered already changes nothing, so each shell
/// adds only the tables of notations no shell gave it before. A refusal, which the tests rule out,
/// is said in the log.
pub(crate) fn register_notations(notations: impl IntoIterator<Item = &'static [Rows]>) {
    registered();
    for table in tables(notations) {
        if let Err(e) = register(table) {
            log::warn!("{e}");
        }
    }
}

/// Translates a literal, in the shell's and the notations' catalogues.
pub fn tr(s: &str) -> String {
    registered();
    archforge_shell_api::i18n::tr(s)
}

/// Translates a literal that needs a context key to be unambiguous, in the shell's and the
/// notations' catalogues.
pub fn tr_ctx(context: &str, s: &str) -> String {
    registered();
    archforge_shell_api::i18n::tr_ctx(context, s)
}

/// A plural, in the shell's and the notations' catalogues: `one` is the English singular and the
/// key; `{n}` is replaced by the count.
pub fn trn(one: &str, other: &str, n: u64) -> String {
    registered();
    archforge_shell_api::i18n::trn(one, other, n)
}

#[cfg(test)]
mod tests;
