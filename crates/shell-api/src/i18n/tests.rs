//! The machinery's tests: each language's plural rule, placeholders, the registering of a crate's
//! catalogues and what the lookups find in them, the interfaces' own catalogue table, and the
//! build-time row check. That each lookup registers the interfaces' own catalogue
//! when it is the process's first is tested in binaries of their own (`tests/`). The rows of every
//! catalogue, the interfaces' own among them, are tested in the shell (`archforge_ui_egui::i18n`).

use super::*;

/// An English plural without a row takes its singular for exactly 1 and its plural for every other
/// count, the largest included.
#[test]
fn plural_forms_hold_at_edge_numbers() {
    set_language("en");
    assert_eq!(trn("{n} shape", "{n} shapes", 0), "0 shapes");
    assert_eq!(trn("{n} shape", "{n} shapes", 1), "1 shape");
    assert_eq!(trn("{n} shape", "{n} shapes", 2), "2 shapes");
    assert_eq!(trn("{n} shape", "{n} shapes", u64::MAX), format!("{} shapes", u64::MAX));
}

/// The counts each language's plural rule is tested at: zero, one, two, a few, the teens,
/// a compound one, a hundred and a thousand.
const EDGES: [u64; 8] = [0, 1, 2, 5, 11, 21, 100, 1000];

/// Each language's plural rule picks the form CLDR's cardinal rule gives at the edge counts, and its
/// catalogue holds as many plural forms as the rule picks from.
#[test]
fn every_languages_plural_rule_picks_the_cldr_form_at_the_edge_numbers() {
    // The form index CLDR's cardinal rule gives at each edge count, per language.
    let expected: &[(&str, [usize; 8])] = &[
        ("en", [1, 0, 1, 1, 1, 1, 1, 1]),
        ("de", [1, 0, 1, 1, 1, 1, 1, 1]),
        ("es", [1, 0, 1, 1, 1, 1, 1, 1]),
        ("pt-PT", [1, 0, 1, 1, 1, 1, 1, 1]),
        // French: one is 0 and 1.
        ("fr", [0, 0, 1, 1, 1, 1, 1, 1]),
        ("ja", [0; 8]),
        ("zh-Hans", [0; 8]),
    ];
    assert_eq!(expected.len(), LANGUAGES.len(), "every registered language has its expected forms");
    for (code, forms) in expected {
        let lang = LANGUAGES.iter().find(|l| l.code == *code).unwrap();
        let got: Vec<usize> = EDGES.iter().map(|n| (lang.plural)(*n)).collect();
        assert_eq!(got, forms, "{code}");
        assert_eq!(plural_forms(lang), if forms.contains(&1) { 2 } else { 1 }, "{code}");
    }
}

/// A translation may put the placeholders in another order, and each is filled once.
#[test]
fn placeholders_can_be_reordered() {
    assert_eq!(fmt("{b} before {a}", &[("a", "1"), ("b", "2")]), "2 before 1");
    // A value is not read for placeholders, and what names no argument stays.
    assert_eq!(fmt("On {branch} · {status} {x} {", &[("branch", "fix {status}"), ("status", "Saved")]), "On fix {status} · Saved {x} {");
}

/// A crate's catalogue with a `str` row, one under a context and a `plural` row. Each test
/// registers tables of its own with keys of its own, as the registry is the process's.
static GERMAN: &[Rows] = &[Rows {
    code: "de",
    catalogue: "de.tsv",
    text: "# A header\nstr\t\tRegistered save\tSpeichern\nstr\tmenu\tRegistered view\tAnsicht\nplural\t\t{n} registered day\tvor {n} Tag|vor {n} Tagen\n",
}];

/// The registered rows are what the lookups find in their language, a context keeps its row
/// apart, and a key without a row, in another language or before its table, is its English text.
#[test]
fn registered_rows_are_found_in_their_language() {
    set_language("de");
    assert_eq!(tr("Registered save"), "Registered save", "before its table is registered");
    assert_eq!(register(GERMAN), Ok(()));
    assert_eq!(tr("Registered save"), "Speichern");
    assert_eq!(tr_ctx("menu", "Registered view"), "Ansicht");
    assert_eq!(tr("Registered view"), "Registered view", "the row is under its context alone");
    assert_eq!(trn("{n} registered day", "{n} registered days", 1), "vor 1 Tag");
    assert_eq!(trn("{n} registered day", "{n} registered days", 2), "vor 2 Tagen");
    set_language("fr");
    assert_eq!(tr("Registered save"), "Registered save", "no row in French");
    assert_eq!(trn("{n} registered day", "{n} registered days", 2), "2 registered days");
}

/// Two tables for one language, as the shell's and a notation's.
static EARLIER: &[Rows] = &[Rows { code: "es", catalogue: "earlier.tsv", text: "str\t\tReplaced word\tPrimera\nstr\t\tEarlier word\tAntes\n" }];
static LATER: &[Rows] = &[Rows { code: "es", catalogue: "later.tsv", text: "str\t\tReplaced word\tSegunda\n" }];

/// A later table adds its rows to a language's and replaces an earlier table's row under the same
/// key; registering a table again changes nothing.
#[test]
fn a_later_table_adds_to_a_language_and_a_table_counts_once() {
    set_language("es");
    assert_eq!(register(EARLIER), Ok(()));
    assert_eq!(register(LATER), Ok(()));
    assert_eq!((tr("Replaced word"), tr("Earlier word")), ("Segunda".to_string(), "Antes".to_string()));
    assert_eq!(register(EARLIER), Ok(()));
    assert_eq!(tr("Replaced word"), "Segunda", "the earlier table, registered again, replaces nothing");
}

/// A table with one catalogue for a code no language has.
static UNKNOWN: &[Rows] = &[
    Rows { code: "pt-PT", catalogue: "pt-PT.tsv", text: "str\t\tNever registered\tNunca\n" },
    Rows { code: "xx", catalogue: "xx.tsv", text: "str\t\tNever registered\tNie\n" },
];

/// A table with a catalogue for an unknown code is refused whole, naming the catalogue, its code
/// and the codes accepted, and none of its rows is found.
#[test]
fn a_table_for_an_unknown_language_is_refused_whole() {
    let refused = register(UNKNOWN).unwrap_err();
    assert_eq!(refused, Unregistered { catalogue: "xx.tsv", code: "xx" });
    assert_eq!(
        refused.to_string(),
        "the catalogue xx.tsv is for \"xx\", which no language is registered under (accepted: en, de, es, fr, pt-PT, ja, zh-Hans)"
    );
    set_language("pt-PT");
    assert_eq!(tr("Never registered"), "Never registered", "the known language's rows were not taken either");
}

/// Every catalogue file in the interfaces' folder, which the build script checks, is one of their
/// table, and every one of the table is a file there; the table has one catalogue for each
/// language, in the languages' order, and the machinery takes it.
#[test]
fn every_catalogue_in_the_folder_is_registered() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("i18n");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tsv"))
        .collect();
    files.sort();
    let mut registered: Vec<&str> = CATALOGUES.iter().map(|rows| rows.catalogue).collect();
    registered.sort_unstable();
    assert_eq!(files, registered, "a catalogue the build checks is a registered one, and the other way round");
    let codes: Vec<&str> = CATALOGUES.iter().map(|rows| rows.code).collect();
    assert_eq!(codes, LANGUAGES.iter().map(|l| l.code).collect::<Vec<_>>(), "one catalogue for each language, in the languages' order");
    assert_eq!(register(CATALOGUES), Ok(()), "the machinery takes the interfaces' catalogues");
}

/// The build-time row check names the first row without four tab-separated columns, by its line
/// and the columns found, and passes blank lines and comments, whatever they hold.
#[test]
fn the_row_check_names_the_first_row_without_four_columns() {
    use check::malformed_row;
    assert_eq!(malformed_row("# a\tcomment\n\nstr\t\tSave\tSpeichern\nplural\t\t{n} day\ta|b\n"), None);
    assert_eq!(malformed_row(""), None);
    assert_eq!(malformed_row("# header\nstr\t\tSave\tSpeichern\nstr\tSave\tSpeichern\nstr\t\tA\tB\tC\n"), Some((3, 3)));
    assert_eq!(malformed_row("str\t\tSave\tSpeichern\tmore\n"), Some((1, 5)));
    assert_eq!(malformed_row("str\t\tSave\tSpeichern\n\nSpeichern\n"), Some((3, 1)));
    assert_eq!(malformed_row(" # not a comment\t\t\n"), Some((1, 3)), "a comment starts the line");
}
