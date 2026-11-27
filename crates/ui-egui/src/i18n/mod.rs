//! The shell's translations, under the shell's old path: the machinery (the lookups, the
//! languages and their plural rules, `parse`) comes from the shell's interfaces
//! (`archforge_shell_api::i18n`), so a notation's screens translate with it without the shell.
//! What stays here is the catalogues, one per language: the shell's own, `i18n/<code>.tsv`, and
//! each notation's, `i18n/<notation>/<code>.tsv`, which the notation hands over
//! ([`NotationUi::catalogues`](crate::notation_ui::NotationUi::catalogues)). They are embedded and
//! registered with the machinery before the shell's first lookup ([`tables`]); the build script
//! refuses a malformed row in any of them with the machinery's row check. Each row has four
//! columns: kind (`str` or `plural`), context, key, text, as the machinery describes them. A row
//! only one notation's screens name is in that notation's catalogue; the rest are the shell's, read
//! by the shell, by both notations or by the desktop app (its menu).

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

/// Every catalogue table the shell registers: its own, then each notation's of `UIS`, in the order
/// of the notations. It reads `UIS` rather than a shell's `notation_uis`: the lookups take no
/// shell and the machinery's registry is the whole process's, and the shared screens look some of
/// a notation's keys up too (a figure's words), so every notation's rows are in before any lookup.
pub(crate) fn tables() -> impl Iterator<Item = &'static [Rows]> {
    std::iter::once(CATALOGUES).chain(crate::notation_ui::UIS.iter().map(|ui| ui.catalogues()))
}

/// Registers every catalogue table ([`tables`]) once: as the shell's notations are set up
/// ([`Shell::new`](crate::Shell::new)), so nothing the shell's interfaces say, a refusal among them,
/// is looked up before they are in, and before the first lookup through this module, for what is
/// said without a shell; a refusal, which the tests rule out, is said in the log.
pub(crate) fn registered() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for table in tables() {
            if let Err(e) = register(table) {
                log::warn!("{e}");
            }
        }
    });
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
