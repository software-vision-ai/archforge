//! Localisation: the machinery the shell and the notations' screens translate with.
//!
//! A crate that brings text keeps one tab-separated catalogue per language, `<code>.tsv`, keyed by
//! the English source string; it embeds them, registers them here (`register`), and its build
//! script refuses a malformed row (`check`). The interfaces keep their own the same way, the words
//! only their code says (`CATALOGUES`), and the lookups register them before the first one. Each row has four columns: kind, context, key, text.
//! Kinds: `str` (a wrapped literal, with an optional context for ambiguous words) and `plural`
//! (key is the English singular; text holds the plural forms separated by `|`). Each key is in a
//! catalogue once: a `str` row by its context and key, a `plural` row by its key alone, as `parse`
//! reads them. Rows go beside related entries, never at the end of the file.
//!
//! Lookup happens at draw time, so a language change applies on the next frame; the
//! language is a per-thread setting, and every test harness pins it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{LazyLock, PoisonError, RwLock};

pub mod check;

/// One registered language.
pub struct Language {
    pub code: &'static str,
    pub native_name: &'static str,
    /// Index of the plural form for `n`.
    pub plural: fn(u64) -> usize,
    /// A complete language fails the coverage tests on a missing row; a partial one reports a gap.
    pub complete: bool,
}

/// `one|other`, one for exactly 1: English, German and Spanish (CLDR `n = 1`), and European
/// Portuguese (CLDR `i = 1 and v = 0`, which is the same for a count). Spanish and European
/// Portuguese also have CLDR `many` (`e = 0 and i != 0 and i % 1000000 = 0 and v = 0`, or an
/// exponent past 5): a whole number of millions, 1000000, 2000000 and so on, where the language
/// writes "de" before the noun. The catalogues hold two forms, so those counts take `other`
/// ("1000000 clases", not "1000000 de clases").
fn plural_one_other(n: u64) -> usize {
    usize::from(n != 1)
}

/// `one|other`, one for 0 and 1: French (CLDR `i = 0,1`). French's CLDR `many`, a whole number of
/// millions ("1000000 de classes"), takes `other`, as for Spanish.
fn plural_french(n: u64) -> usize {
    usize::from(n > 1)
}

/// A single form whatever the count: Japanese and Chinese (CLDR `other` only).
fn plural_none(_: u64) -> usize {
    0
}

/// The language registry; English first, the source every catalogue is keyed by. The other
/// catalogues were written from the meaning of the English source; their provenance is in
/// each catalogue's header.
pub static LANGUAGES: &[Language] = &[
    Language { code: "en", native_name: "English", plural: plural_one_other, complete: true },
    Language { code: "de", native_name: "Deutsch", plural: plural_one_other, complete: true },
    Language { code: "es", native_name: "Español", plural: plural_one_other, complete: true },
    Language { code: "fr", native_name: "Français", plural: plural_french, complete: true },
    Language { code: "pt-PT", native_name: "Português (Portugal)", plural: plural_one_other, complete: true },
    Language { code: "ja", native_name: "日本語", plural: plural_none, complete: true },
    Language { code: "zh-Hans", native_name: "简体中文", plural: plural_none, complete: true },
];

/// How many plural forms a language's catalogue holds: one more than the largest index its plural
/// function gives over the counts that tell the CLDR categories apart.
pub fn plural_forms(lang: &Language) -> usize {
    let counts = (0..=200u64).chain([1000, 1_000_000, u64::MAX]);
    counts.map(lang.plural).max().unwrap_or(0).saturating_add(1)
}

/// A parsed catalogue.
#[derive(Default)]
pub struct Catalogue {
    pub strings: HashMap<(String, String), String>,
    pub plurals: HashMap<String, Vec<String>>,
}

/// Parses catalogue text; malformed rows are skipped here and refused by the build script.
pub fn parse(text: &str) -> Catalogue {
    let mut c = Catalogue::default();
    for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut cols = line.split('\t');
        let (Some(kind), Some(context), Some(key), Some(value)) = (cols.next(), cols.next(), cols.next(), cols.next()) else { continue };
        match kind {
            "str" => {
                c.strings.insert((context.to_string(), key.to_string()), value.to_string());
            }
            "plural" => {
                c.plurals.insert(key.to_string(), value.split('|').map(str::to_string).collect());
            }
            _ => {}
        }
    }
    c
}

/// One language's rows in one crate's catalogue, as the crate registers them (`register`).
pub struct Rows {
    /// The language's code, one of `LANGUAGES`.
    pub code: &'static str,
    /// The catalogue's file name in the folder of the crate that holds it.
    pub catalogue: &'static str,
    /// The catalogue's text, embedded.
    pub text: &'static str,
}

/// A catalogue registered for a code no language has, which `register` refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unregistered {
    /// The catalogue's file name.
    pub catalogue: &'static str,
    /// The code it was registered for.
    pub code: &'static str,
}

impl std::fmt::Display for Unregistered {
    /// Names the catalogue, its code and the codes accepted.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the catalogue {} is for {:?}, which no language is registered under (accepted: {})", self.catalogue, self.code, codes())
    }
}

impl std::error::Error for Unregistered {}

/// The catalogues the crates registered: each table once, and its rows parsed into the catalogue
/// of their language.
#[derive(Default)]
struct Registry {
    tables: Vec<&'static [Rows]>,
    catalogues: HashMap<&'static str, Catalogue>,
}

/// Every thread looks its text up in the one registry.
static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(RwLock::default);

/// Registers a crate's catalogues, one `Rows` per language, so the lookups find their rows. A table
/// registered again changes nothing; a row of a later table replaces an earlier table's row under
/// the same key. A table with rows for a code no language has is refused whole, before anything
/// is registered. Until a crate registers, its keys are their own English text.
pub fn register(table: &'static [Rows]) -> Result<(), Unregistered> {
    if let Some(rows) = table.iter().find(|rows| !LANGUAGES.iter().any(|l| l.code == rows.code)) {
        return Err(Unregistered { catalogue: rows.catalogue, code: rows.code });
    }
    let mut registry = REGISTRY.write().unwrap_or_else(PoisonError::into_inner);
    if registry.tables.iter().any(|t| std::ptr::eq(*t, table)) {
        return Ok(());
    }
    for rows in table {
        let parsed = parse(rows.text);
        let catalogue = registry.catalogues.entry(rows.code).or_default();
        catalogue.strings.extend(parsed.strings);
        catalogue.plurals.extend(parsed.plurals);
    }
    registry.tables.push(table);
    Ok(())
}

thread_local! {
    static CURRENT: RefCell<&'static str> = const { RefCell::new("en") };
}

/// The registered codes, comma-separated, for a message that names the accepted choices.
pub fn codes() -> String {
    LANGUAGES.iter().map(|l| l.code).collect::<Vec<_>>().join(", ")
}

/// Sets this thread's interface language; returns false for an unregistered code.
pub fn set_language(code: &str) -> bool {
    let Some(lang) = LANGUAGES.iter().find(|l| l.code == code) else { return false };
    CURRENT.with(|c| *c.borrow_mut() = lang.code);
    true
}

/// This thread's interface language, by its code.
pub fn language() -> &'static str {
    CURRENT.with(|c| *c.borrow())
}

/// `f` over this thread's language and the rows registered for it (none before a crate
/// registers); `None` for a code no language has.
fn with_current<T>(f: impl FnOnce(Option<&Catalogue>, &Language) -> T) -> Option<T> {
    let code = language();
    let lang = LANGUAGES.iter().find(|l| l.code == code)?;
    let registry = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
    Some(f(registry.catalogues.get(code), lang))
}

/// The interfaces' own catalogue of each language, in the order of `LANGUAGES`, embedded: the rows
/// only their own code says (the panels' and the engine's refusal words among them).
pub static CATALOGUES: &[Rows] = &[
    Rows { code: "en", catalogue: "en.tsv", text: include_str!("en.tsv") },
    Rows { code: "de", catalogue: "de.tsv", text: include_str!("de.tsv") },
    Rows { code: "es", catalogue: "es.tsv", text: include_str!("es.tsv") },
    Rows { code: "fr", catalogue: "fr.tsv", text: include_str!("fr.tsv") },
    Rows { code: "pt-PT", catalogue: "pt-PT.tsv", text: include_str!("pt-PT.tsv") },
    Rows { code: "ja", catalogue: "ja.tsv", text: include_str!("ja.tsv") },
    Rows { code: "zh-Hans", catalogue: "zh-Hans.tsv", text: include_str!("zh-Hans.tsv") },
];

/// Registers the interfaces' own catalogue table once, before the first lookup: what their code
/// says is found in the user's language whoever looks it up, a shell or a notation's screens
/// without one. A refusal, which the tests rule out, is said in the log.
fn registered() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Err(e) = register(CATALOGUES) {
            log::warn!("{e}");
        }
    });
}

/// Translates a literal.
pub fn tr(s: &str) -> String {
    tr_ctx("", s)
}

/// Translates a literal that needs a context key to be unambiguous.
pub fn tr_ctx(context: &str, s: &str) -> String {
    registered();
    with_current(|c, _| c.and_then(|c| c.strings.get(&(context.to_string(), s.to_string())).cloned())).flatten().unwrap_or_else(|| s.to_string())
}

/// A plural: `one` is the English singular and the key; `{n}` is replaced by the count.
pub fn trn(one: &str, other: &str, n: u64) -> String {
    registered();
    let form = with_current(|c, lang| c.and_then(|c| c.plurals.get(one)).and_then(|forms| forms.get((lang.plural)(n)).cloned())).flatten();
    let english = if n == 1 { one } else { other };
    form.unwrap_or_else(|| english.to_string()).replace("{n}", &n.to_string())
}

/// Fills named placeholders, so a translation may reorder them, in one pass over the template: a
/// value is never read for placeholders itself, so a name holding `{status}` stays as it is.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    let (mut out, mut rest) = (String::with_capacity(template.len()), template);
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        let found = after.split_once('}').and_then(|(key, tail)| Some((args.iter().find(|(k, _)| *k == key)?.1, tail)));
        let (value, tail) = found.unwrap_or(("{", after));
        out.push_str(value);
        rest = tail;
    }
    out + rest
}

#[cfg(test)]
mod tests;
