//! The shell's translations, under the shell's old path: the machinery (the lookups, the
//! languages and their plural rules, `parse`) comes from the shell's interfaces
//! (`archforge_shell_api::i18n`), so a notation's screens translate with it without the shell.
//! What stays here is the shell's catalogues, one per language, `i18n/<code>.tsv`, embedded and
//! registered with the machinery before the shell's first lookup; the build script refuses a
//! malformed row with the machinery's row check. Each row has four columns: kind (`str` or
//! `plural`), context, key, text, as the machinery describes them. Every row is read by the shell
//! or by the desktop app (its menu).

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

/// Registers the shell's catalogues once, before the first lookup; a refusal, which the tests
/// rule out, is said in the log.
fn registered() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Err(e) = register(CATALOGUES) {
            log::warn!("{e}");
        }
    });
}

/// Translates a literal, in the shell's catalogues.
pub fn tr(s: &str) -> String {
    registered();
    archforge_shell_api::i18n::tr(s)
}

/// Translates a literal that needs a context key to be unambiguous, in the shell's catalogues.
pub fn tr_ctx(context: &str, s: &str) -> String {
    registered();
    archforge_shell_api::i18n::tr_ctx(context, s)
}

/// A plural, in the shell's catalogues: `one` is the English singular and the key; `{n}` is
/// replaced by the count.
pub fn trn(one: &str, other: &str, n: u64) -> String {
    registered();
    archforge_shell_api::i18n::trn(one, other, n)
}

#[cfg(test)]
mod tests;
