//! The tests across every catalogue the app registers, here because the app is the one crate that
//! knows every catalogue (ADR 0003): every wrapped literal in the shell's, its interfaces', ER's
//! and UML's screens' and the desktop app's source, and every text the shell and the notations'
//! screens hand the lookups through a table, has a row in every complete language, in the shell's
//! catalogue, the interfaces' or a notation's; every row is read, and is in the catalogue of the
//! code that reads it; no key is in a language's catalogues twice; a partial catalogue reports its
//! gaps without failing; and the words for what the metamodel names are each catalogue's rows.
//! They read the sources and the catalogues by path and find the word lists through
//! `archforge_ui_egui::testing`, `archforge_er_ui::snapshot` and `archforge_uml_ui::snapshot`. What
//! each crate's own catalogue needs alone (well-formed, registered, the screen, the plural edges)
//! is tested in that crate: the shell's in `crates/ui-egui/src/i18n/tests.rs`, ER's and UML's
//! registering in `crates/er-ui` and `crates/uml-ui`, the machinery and the interfaces' in
//! `crates/shell-api`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use archforge_shell_api::notation_ui::NotationUi;
use archforge_ui_egui::i18n::{CATALOGUES, Catalogue, LANGUAGES, Language, Rows, parse, set_language};
use archforge_ui_egui::{Services, Shell};

/// Every catalogue table the app's shell looks its rows up in, each with whose it is: the shell's
/// interfaces', which their own lookups register, then those the shell registers: its own, then
/// each notation's of the app's screens parts ([`crate::SCREENS`]).
fn every_table() -> Vec<(Reader, &'static [Rows])> {
    let shell = std::iter::once(CATALOGUES)
        .chain(crate::SCREENS.catalogues())
        .enumerate()
        .map(|(index, table)| (index.checked_sub(1).map_or(Reader::Shell, Reader::Notation), table));
    std::iter::once((Reader::Interfaces, archforge_shell_api::i18n::CATALOGUES)).chain(shell).collect()
}

/// A catalogue's name in a message: its file name in its folder (`er/de.tsv`), the interfaces'
/// after their crate's folder, as their file names are the shell's.
fn named(holder: Reader, rows: &Rows) -> String {
    if holder == Reader::Interfaces { format!("shell-api/{}", rows.catalogue) } else { rows.catalogue.to_string() }
}

/// Every catalogue of a language the tests' shell looks its rows up in ([`every_table`]), each by
/// its name ([`named`]) and its text.
fn catalogues_of(lang: &Language) -> Vec<(String, &'static str)> {
    let of =
        |(holder, table): (Reader, &'static [Rows])| table.iter().find(|rows| rows.code == lang.code).map(|rows| (named(holder, rows), rows.text));
    every_table().into_iter().map(|table| of(table).unwrap()).collect()
}

/// A language's rows in all its catalogues, read into one as the lookups read them.
fn parsed(lang: &Language) -> Catalogue {
    let mut all = Catalogue::default();
    for (_, text) in catalogues_of(lang) {
        let one = parse(text);
        all.strings.extend(one.strings);
        all.plurals.extend(one.plurals);
    }
    all
}

/// Adds the `.rs` files under `dir` to `out`; a listing that fails part way fails the test.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.map(Result::unwrap) {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The string literals passed as argument `index` of each call `name(`.
fn literal_args(text: &str, name: &str, index: usize) -> Vec<String> {
    let mut found = Vec::new();
    let needle = format!("{name}(\"");
    let mut rest = text;
    while let Some(at) = rest.find(&needle) {
        let before = rest[..at].chars().last();
        rest = &rest[at + name.len() + 1..];
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        // Collect consecutive string-literal arguments; stop at the first argument that is not one.
        let mut args = Vec::new();
        let mut cursor = rest;
        while args.len() <= index {
            let Some(body) = cursor.strip_prefix('"') else { break };
            let mut end = None;
            let mut escaped = false;
            for (i, c) in body.char_indices() {
                match c {
                    '\\' if !escaped => escaped = true,
                    '"' if !escaped => {
                        end = Some(i);
                        break;
                    }
                    _ => escaped = false,
                }
            }
            let Some(end) = end else { break };
            args.push(body[..end].replace("\\\"", "\""));
            let after = body[end + 1..].trim_start();
            let Some(next) = after.strip_prefix(',') else { break };
            cursor = next.trim_start();
        }
        if let Some(arg) = args.get(index) {
            found.push(arg.clone());
        }
    }
    found
}

/// Who reads a row: the shell's code (the desktop app's with it), the shell's interfaces' code
/// (`archforge_shell_api`), one notation's code, by the index of its screens part in
/// `SCREENS.parts()`, or a test, which reads a row to check it and so says nothing about whose
/// catalogue holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reader {
    Shell,
    Interfaces,
    Notation(usize),
    Test,
}

/// A row's key as the loader reads it: kind (`str` or `plural`), context (none for a `plural`
/// row, which is one key whatever its context) and key.
type Key = (String, String, String);

/// The key of a catalogue row's first three columns, read as the loader reads it ([`Key`]).
fn key_of(kind: &str, context: &str, key: &str) -> Key {
    let context = if kind == "plural" { "" } else { context };
    (kind.to_string(), context.to_string(), key.to_string())
}

/// The name of the crate of the notation of screens part `part`: `archforge_` and the folder of
/// its catalogues (`er/en.tsv` is `archforge_er`'s).
fn notation_crate(part: usize) -> String {
    let catalogue = crate::SCREENS.parts()[part].ui.catalogues()[0].catalogue;
    let (folder, _) = catalogue.split_once('/').expect("a notation's catalogues are in a folder of their own");
    format!("archforge_{folder}")
}

/// `text` with its comments (nested block comments among them) and its string, raw string and
/// character literals blanked, each blanked character a space per byte and a line break kept, so
/// offsets and lines stay those of `text`: what is left is the code, which names crates and whose
/// brackets nest.
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let ident = |i: Option<usize>| i.and_then(at).is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(c) = at(i) {
        // How far a comment or a literal that starts here reaches, in characters.
        let end = match (c, at(i + 1)) {
            ('/', Some('/')) => Some((i..).find(|j| at(*j).is_none_or(|c| c == '\n')).unwrap_or(chars.len())),
            ('/', Some('*')) => {
                let (mut j, mut depth) = (i + 2, 1);
                while depth > 0 && j < chars.len() {
                    match (at(j), at(j + 1)) {
                        (Some('/'), Some('*')) => (depth, j) = (depth + 1, j + 2),
                        (Some('*'), Some('/')) => (depth, j) = (depth - 1, j + 2),
                        _ => j += 1,
                    }
                }
                Some(j)
            }
            ('"', _) => {
                let mut j = i + 1;
                while let Some(c) = at(j) {
                    j += if c == '\\' { 2 } else { 1 };
                    if c == '"' {
                        break;
                    }
                }
                Some(j.min(chars.len()))
            }
            // A raw string, `r"…"` or `r#"…"#` (`br` too), which no escape ends early.
            ('r', _) if !ident(i.checked_sub(1)) || (i >= 2 && at(i - 1) == Some('b') && !ident(i.checked_sub(2))) => {
                let hashes = (i + 1..).take_while(|j| at(*j) == Some('#')).count();
                (at(i + 1 + hashes) == Some('"')).then(|| {
                    let closing = |j: &usize| at(*j) == Some('"') && (1..=hashes).all(|h| at(j + h) == Some('#'));
                    (i + 2 + hashes..chars.len()).find(closing).map_or(chars.len(), |j| j + 1 + hashes)
                })
            }
            // A character literal, `'x'` or `'\n'`; a lifetime, `'a`, is code.
            ('\'', Some('\\')) => Some((i + 3..).find(|j| at(*j).is_none_or(|c| c == '\'')).map_or(chars.len(), |j| (j + 1).min(chars.len()))),
            ('\'', Some(_)) if at(i + 2) == Some('\'') => Some(i + 3),
            _ => None,
        };
        match end {
            Some(end) => {
                for c in &chars[i..end] {
                    if *c == '\n' {
                        out.push('\n');
                    } else {
                        out.extend(std::iter::repeat_n(' ', c.len_utf8()));
                    }
                }
                i = end;
            }
            None => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// The offset just past the bracket that closes the one at `open` in `code` (comments and
/// literals blanked, [`code_only`]), or the end of `code` when none does.
fn closing(code: &str, open: usize) -> usize {
    let (first, last) = match code.as_bytes()[open] {
        b'[' => (b'[', b']'),
        b'(' => (b'(', b')'),
        _ => (b'{', b'}'),
    };
    let mut depth = 0;
    for (at, byte) in code.bytes().enumerate().skip(open) {
        depth += i32::from(byte == first) - i32::from(byte == last);
        if depth == 0 {
            return at + 1;
        }
    }
    code.len()
}

/// Whether `code` names the crate `name` as a whole word, as a path or a `use` does.
fn names(code: &str, name: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    code.match_indices(name).any(|(at, _)| !word(code[..at].chars().last()) && !word(code[at + name.len()..].chars().next()))
}

/// Whose code `code` is: the notation's whose crate its code alone names, comments and literals
/// aside ([`code_only`]), or the shell's, which names none, or both (as the offscreen screens and
/// the desktop app's projects do).
fn reader_of(code: &str) -> Reader {
    let code = code_only(code);
    let named: Vec<usize> = (0..crate::SCREENS.parts().len()).filter(|part| names(&code, &notation_crate(*part))).collect();
    match named.as_slice() {
        [part] => Reader::Notation(*part),
        _ => Reader::Shell,
    }
}

/// The attributes that build a module only for the tests, or for them and the snapshots, whose
/// feature the app never turns on.
const TEST_BUILDS: [&str; 2] = ["#[cfg(test)]", "#[cfg(any(test, feature = \"snapshot\"))]"];

/// A module declared for the tests' builds alone ([`TEST_BUILDS`]).
struct TestModule {
    /// The module's name.
    name: String,
    /// Where its declaration starts, at the attribute, and ends: past `;`, or past the brace that
    /// closes an inline module's body.
    span: std::ops::Range<usize>,
    /// Whether its body is inline (`mod tests { … }`) rather than a file of its own (`mod tests;`).
    inline: bool,
}

/// The modules `text` declares for the tests' builds alone, in code only ([`code_only`]): the
/// attribute, then any doc comments and other attributes, then the declaration with its
/// visibility, if any.
fn test_modules(text: &str) -> Vec<TestModule> {
    let code = code_only(text);
    let mut found = Vec::new();
    for (at, _) in code.match_indices("#[cfg(") {
        let after_attribute = closing(&code, at + 1);
        if !TEST_BUILDS.contains(&&text[at..after_attribute]) {
            continue;
        }
        // Past the whitespace (the blanked doc comments among it) and the attributes that follow.
        let mut cursor = after_attribute;
        loop {
            cursor += code[cursor..].len() - code[cursor..].trim_start().len();
            if !code[cursor..].starts_with("#[") {
                break;
            }
            cursor = closing(&code, cursor + 1);
        }
        if let Some(rest) = code[cursor..].strip_prefix("pub") {
            cursor = if rest.starts_with('(') { closing(&code, cursor + 3) } else { cursor + 3 };
            cursor += code[cursor..].len() - code[cursor..].trim_start().len();
        }
        let Some(rest) = code[cursor..].strip_prefix("mod ") else { continue };
        let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let after_name = cursor + 4 + (rest.len() - rest.trim_start().len()) + name.len();
        let body = after_name + code[after_name..].len() - code[after_name..].trim_start().len();
        let (end, inline) = match code.as_bytes().get(body) {
            Some(b'{') => (closing(&code, body), true),
            Some(b';') => (body + 1, false),
            _ => continue,
        };
        found.push(TestModule { name, span: at..end, inline });
    }
    found
}

/// Whether only the tests' builds take `file`: it, or a module around it, is declared for them
/// alone ([`test_modules`]), as the shell's tests are and the screens the app's screenshot tool
/// stages (`offscreen`).
fn test_only(file: &Path) -> bool {
    let (Some(dir), Some(stem)) = (file.parent(), file.file_stem().and_then(|s| s.to_str())) else { return false };
    let (dir, name) = match stem {
        "lib" | "main" => return false,
        "mod" => match (dir.parent(), dir.file_name().and_then(|s| s.to_str())) {
            (Some(up), Some(name)) => (up, name),
            _ => return false,
        },
        _ => (dir, stem),
    };
    // The file that declares it: its folder's crate root or `mod.rs`, or the file named after the folder.
    let declaring = [dir.join("lib.rs"), dir.join("main.rs"), dir.join("mod.rs"), dir.with_extension("rs")];
    let Some(parent) = declaring.into_iter().find(|parent| parent.is_file()) else { return false };
    let text = std::fs::read_to_string(&parent).unwrap_or_default();
    test_modules(&text).iter().any(|module| !module.inline && module.name == name) || test_only(&parent)
}

/// A source file's text split into its code and its tests, each with its reader: a file only tests
/// build ([`test_only`]) is all tests; another file's tests are its inline test modules, each to
/// the brace that closes it, and the rest, before and after them, is the code of whose crate it
/// names ([`reader_of`]).
fn split(file: &Path, text: &str) -> Vec<(Reader, String)> {
    if test_only(file) {
        return vec![(Reader::Test, text.to_string())];
    }
    let (mut code, mut tests, mut from) = (String::new(), String::new(), 0);
    for module in test_modules(text).iter().filter(|module| module.inline) {
        // A module nested in one already taken is taken with it.
        if module.span.start < from {
            continue;
        }
        code.push_str(&text[from..module.span.start]);
        tests.push_str(&text[module.span.clone()]);
        from = module.span.end;
    }
    code.push_str(&text[from..]);
    vec![(reader_of(&code), code), (Reader::Test, tests)]
}

/// Every key the catalogues must hold, each with a reader of it ([`Reader`]): the literals the
/// sources wrap, and the texts the tables and the notations hand to the lookups.
fn reads() -> Vec<(Key, Reader)> {
    // The shell's source, its interfaces', ER's and UML's screens', and the desktop app's, which
    // looks its menu up in these catalogues.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let shell = root.join("../../crates/ui-egui/src");
    assert!(shell.is_dir(), "the shell's source is not at {}", shell.display());
    let mut files = Vec::new();
    sources(&shell, &mut files);
    let interfaces = root.join("../../crates/shell-api/src");
    assert!(interfaces.is_dir(), "the shell's interfaces' source is not at {}", interfaces.display());
    sources(&interfaces, &mut files);
    let er = root.join("../../crates/er-ui/src");
    assert!(er.is_dir(), "ER's screens' source is not at {}", er.display());
    sources(&er, &mut files);
    let uml = root.join("../../crates/uml-ui/src");
    assert!(uml.is_dir(), "UML's screens' source is not at {}", uml.display());
    sources(&uml, &mut files);
    sources(&root.join("src"), &mut files);
    let mut reads = Vec::new();
    let str_key = |key: &str| key_of("str", "", key);
    for file in files.iter().filter(|f| !f.ends_with("i18n/tests.rs")) {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        for (reader, text) in split(file, &text) {
            // The interfaces name no notation: their code is their own.
            let reader = if reader == Reader::Shell && file.starts_with(&interfaces) { Reader::Interfaces } else { reader };
            reads.extend(literal_args(&text, "tr", 0).iter().map(|s| (str_key(s), reader)));
            let contexts = literal_args(&text, "tr_ctx", 0);
            let keys = literal_args(&text, "tr_ctx", 1);
            reads.extend(contexts.iter().zip(&keys).map(|(context, key)| (key_of("str", context, key), reader)));
            reads.extend(literal_args(&text, "trn", 0).iter().map(|s| (key_of("plural", "", s), reader)));
        }
    }
    // The texts each module hands `tr` through a table, read by whose code that module is: a
    // module of the shell's, or of the crate `src` holds.
    let module_in = |src: &Path, name: &str| {
        let file = src.join(format!("{name}.rs"));
        let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        split(&file, &text)[0].0
    };
    // The start cards' texts, the panels', and each notation's figures' words, which `tr` takes
    // through the figures' descriptions.
    let module = |name: &str| module_in(&shell, name);
    let lists = archforge_ui_egui::testing::word_lists().into_iter().map(|(name, texts)| (module(name), texts));
    let er_lists = archforge_er_ui::snapshot::word_lists().into_iter().map(|(name, texts)| (module_in(&er, name), texts));
    let uml_lists = archforge_uml_ui::snapshot::word_lists().into_iter().map(|(name, texts)| (module_in(&uml, name), texts));
    for (reader, texts) in lists.chain(er_lists).chain(uml_lists) {
        reads.extend(texts.into_iter().map(|s| (str_key(s), reader)));
    }
    // The hover texts of each diagram kind's tools, which the toolbar takes through the kind: its
    // notation's.
    for (part, screens) in crate::SCREENS.parts().iter().enumerate() {
        let tools = screens.kinds.iter().flat_map(|k| k.tools());
        reads
            .extend(tools.flat_map(|t| std::iter::once(t.hint).chain(t.chevron.map(|(_, hint)| hint))).map(|s| (str_key(s), Reader::Notation(part))));
    }
    // The words for what the metamodel names: a notation's metamodel's its notation's, the core's
    // the shell's.
    let uis = crate::SCREENS.notation_uis();
    let of_metamodel = |index: u16| uis.iter().position(|ui| ui.metamodel() == index).map_or(Reader::Shell, Reader::Notation);
    reads.extend(archforge_ui_egui::testing::model_words(uis).into_iter().map(|(index, s)| (str_key(&s), of_metamodel(index))));
    // The in-window menu bar's access letters, keyed by each title's and row's id.
    reads.extend(
        archforge_ui_egui::testing::access_keys(crate::SCREENS.kinds()).into_iter().map(|key| (key_of("str", "access", &key), module("menu_bar"))),
    );
    // The plurals the notations' cards say, which `trn` takes through their descriptions: read by
    // the notation whose crate holds them.
    let plurals = [("archforge_er", archforge_er::figures::PLURALS), ("archforge_uml", archforge_uml::PLURALS)];
    for (owner, keys) in plurals {
        reads.extend(keys.iter().map(|s| (key_of("plural", "", s), reader_of(owner))));
    }
    reads
}

/// The `str` keys (context and key) and the `plural` keys the catalogues must hold, whoever reads
/// them ([`reads`]).
fn required() -> (Vec<(String, String)>, Vec<String>) {
    let (mut strings, mut plurals) = (Vec::new(), Vec::new());
    for ((kind, context, key), _) in reads() {
        if kind == "plural" {
            plurals.push(key);
        } else {
            strings.push((context, key));
        }
    }
    strings.sort();
    strings.dedup();
    plurals.sort();
    plurals.dedup();
    (strings, plurals)
}

/// The keys a catalogue holds twice, each named with the file and both line numbers. Two rows are
/// one key when `parse` reads them into the same entry, so the later row silently replaces the
/// earlier: a `str` row by its context and key (a `tr_ctx` context makes a row of its own), a
/// `plural` row by its key alone, whatever its context.
fn duplicates(catalogue: &str, text: &str) -> Vec<String> {
    let mut first_line = HashMap::new();
    let mut found = Vec::new();
    for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.is_empty() && !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        let [kind, context, key, _] = cols.as_slice() else { continue };
        let context = if *kind == "plural" { "" } else { context };
        let first = *first_line.entry((kind.to_string(), context.to_string(), key.to_string())).or_insert(n + 1);
        if first != n + 1 {
            let within = if context.is_empty() { String::new() } else { format!(" in context {context:?}") };
            found.push(format!("{catalogue} lines {first} and {}: {kind} {key:?}{within}", n + 1));
        }
    }
    found
}

/// The keys two of a language's catalogues both hold, each named with the two catalogues, read as
/// [`duplicates`] reads a key: a lookup would find only the row of the table registered later.
fn in_two(catalogues: &[(&str, &str)]) -> Vec<String> {
    let mut first = HashMap::new();
    let mut found = Vec::new();
    for (catalogue, text) in catalogues {
        for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let cols: Vec<&str> = line.split('\t').collect();
            let [kind, context, key, _] = cols.as_slice() else { continue };
            let context = if *kind == "plural" { "" } else { context };
            let holder = *first.entry((kind.to_string(), context.to_string(), key.to_string())).or_insert(*catalogue);
            if holder != *catalogue {
                let within = if context.is_empty() { String::new() } else { format!(" in context {context:?}") };
                found.push(format!("{holder} and {catalogue}: {kind} {key:?}{within}"));
            }
        }
    }
    found
}

/// A key in two catalogues of a language is named with both; a context makes a `str` row of its
/// own, and a `plural` row is one key whatever its context.
#[test]
fn a_key_in_two_catalogues_is_named_with_both() {
    let shell = "# A header\nstr\t\tFile\tDatei\nplural\t\t{n} day ago\ta|b\nstr\tmacOS\tFile\tAblage\n";
    let er = "str\t\tTable\tTabelle\nstr\tmenu\tFile\tDatei\nplural\tx\t{n} day ago\ta|b\n";
    let uml = "str\t\tFile\tDatei\nstr\tmacOS\tFile\tAblage\nstr\t\tTable\tTabelle\n";
    let found = in_two(&[("de.tsv", shell), ("er/de.tsv", er), ("uml/de.tsv", uml)]);
    assert_eq!(
        found,
        [
            "de.tsv and er/de.tsv: plural \"{n} day ago\"",
            "de.tsv and uml/de.tsv: str \"File\"",
            "de.tsv and uml/de.tsv: str \"File\" in context \"macOS\"",
            "er/de.tsv and uml/de.tsv: str \"Table\""
        ]
    );
    assert!(in_two(&[("de.tsv", shell), ("er/de.tsv", "str\t\tTable\tTabelle\n")]).is_empty(), "keys of their own are no duplicates");
}

/// A key twice in one catalogue is named with both lines; a context makes a `str` row of its own,
/// a `plural` row is one key whatever its context, and the two kinds never share a key.
#[test]
fn a_key_twice_in_a_catalogue_is_named_with_both_lines() {
    let text = "# A header\nstr\t\tFile\tFile\nstr\tmacOS\tFile\tFile\nplural\t\t{n} day ago\ta|b\nstr\t\tFile\tDatei\nplural\tx\t{n} day ago\ta|b\nplural\t\tFile\ta|b\nstr\tmacOS\tFile\tAblage\n";
    let found = duplicates("xx.tsv", text);
    assert_eq!(
        found,
        [
            "xx.tsv lines 2 and 5: str \"File\"",
            "xx.tsv lines 4 and 6: plural \"{n} day ago\"",
            "xx.tsv lines 3 and 8: str \"File\" in context \"macOS\""
        ]
    );
}

/// Every complete language has a row for every literal the source wraps, in the shell's catalogue
/// or a notation's, and no catalogue holds a key twice, nor two catalogues of a language one key,
/// which the loader would read as the later row alone.
#[test]
fn every_complete_catalogue_has_every_row() {
    let (strings, plurals) = required();
    assert!(!strings.is_empty(), "the scan found nothing; the scanner is broken");
    let twice: Vec<String> = LANGUAGES.iter().flat_map(catalogues_of).flat_map(|(name, text)| duplicates(&name, text)).collect();
    assert!(twice.is_empty(), "{} key(s) are in a catalogue twice; keep one row of each:\n{}", twice.len(), twice.join("\n"));
    let shared: Vec<String> = LANGUAGES
        .iter()
        .flat_map(|lang| in_two(&catalogues_of(lang).iter().map(|(name, text)| (name.as_str(), *text)).collect::<Vec<_>>()))
        .collect();
    assert!(shared.is_empty(), "{} key(s) are in two catalogues of a language; keep the row where it is read:\n{}", shared.len(), shared.join("\n"));
    for lang in LANGUAGES {
        let c = parsed(lang);
        assert!(!c.strings.is_empty(), "{} unpacked empty", lang.code);
        let mut missing = Vec::new();
        missing.extend(strings.iter().filter(|k| !c.strings.contains_key(*k)).map(|(ctx, s)| format!("str\t{ctx}\t{s}")));
        missing.extend(plurals.iter().filter(|k| !c.plurals.contains_key(*k)).map(|k| format!("plural\t\t{k}")));
        if lang.complete {
            assert!(missing.is_empty(), "{} is complete but lacks {} row(s):\n{}", lang.code, missing.len(), missing.join("\n"));
        } else if !missing.is_empty() {
            println!("{}: {} gap(s), reported, not failed", lang.code, missing.len());
        }
    }
}

/// Every row of every catalogue, the shell's and the notations', is one a source or a table looks
/// up.
#[test]
fn every_catalogue_row_is_read() {
    // A row nothing looks up is dead text a translator would keep for nothing.
    let (strings, plurals) = required();
    for (catalogue, text) in LANGUAGES.iter().flat_map(catalogues_of) {
        let unread: Vec<&str> = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter(|line| {
                let cols: Vec<&str> = line.split('\t').collect();
                match cols.as_slice() {
                    ["str", ctx, key, _] => !strings.contains(&(ctx.to_string(), key.to_string())),
                    ["plural", _, key, _] => !plurals.iter().any(|p| p == key),
                    _ => true,
                }
            })
            .collect();
        assert!(unread.is_empty(), "{catalogue} has {} row(s) nothing reads:\n{}", unread.len(), unread.join("\n"));
    }
}

/// Every row is in the catalogue of the code that reads it: a row only one notation's code reads
/// (the words its own code says, the words for what only its metamodel names, its tools' hover
/// texts and its cards' plurals) in that notation's catalogue, a row only the shell's interfaces'
/// code reads in theirs, and a row the shell reads, or more than one of them, in the shell's. A
/// test that reads a row places it nowhere, so a row no other code reads stays in the shell's.
#[test]
fn every_row_is_in_the_catalogue_of_the_code_that_reads_it() {
    let mut readers: HashMap<Key, std::collections::BTreeSet<Reader>> = HashMap::new();
    for (key, reader) in reads().into_iter().filter(|(_, reader)| *reader != Reader::Test) {
        readers.entry(key).or_default().insert(reader);
    }
    let parts = crate::SCREENS.parts();
    let found = |reader: Reader| readers.values().any(|by| by.contains(&reader));
    assert!(
        found(Reader::Interfaces) && (0..parts.len()).all(|part| found(Reader::Notation(part))),
        "the scan found no reader in the interfaces' or some notation's code; the scanner is broken"
    );
    // A reader by its name: the shell, its interfaces, or a notation by the folder of its catalogues.
    let name = |reader: &Reader| match reader {
        Reader::Notation(part) => parts[*part].ui.catalogues()[0].catalogue.split('/').next().unwrap_or_default().to_string(),
        Reader::Interfaces => "shell-api".to_string(),
        _ => "the shell".to_string(),
    };
    let mut wrong = Vec::new();
    for (holder, table) in every_table() {
        for rows in table {
            for line in rows.text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
                let cols: Vec<&str> = line.split('\t').collect();
                let [kind, context, key, _] = cols.as_slice() else { continue };
                let by: Vec<&Reader> = readers.get(&key_of(kind, context, key)).into_iter().flatten().collect();
                let home = match by.as_slice() {
                    [Reader::Notation(part)] => Reader::Notation(*part),
                    [Reader::Interfaces] => Reader::Interfaces,
                    _ => Reader::Shell,
                };
                if home != holder {
                    let names: Vec<String> = by.iter().map(|reader| name(reader)).collect();
                    let read = if names.is_empty() { "no code but a test".to_string() } else { names.join(" and ") };
                    wrong.push(format!("{}: {kind} {key:?} is read by {read}; it belongs in {}'s catalogue", named(holder, rows), name(&home)));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} row(s) are not in the catalogue of the code that reads them:\n{}", wrong.len(), wrong.join("\n"));
}

/// A source file's code is the notation's whose crate it alone names, and the shell's when it
/// names none or both; its inline test module, and a whole file only the tests' builds take, is
/// a test's.
#[test]
fn a_files_code_is_whose_crate_it_names_and_its_tests_are_a_tests() {
    let (er, uml) = (reader_of("archforge_er"), reader_of("archforge_uml"));
    assert!(matches!(er, Reader::Notation(_)) && matches!(uml, Reader::Notation(_)) && er != uml, "{er:?} and {uml:?}");
    assert_eq!(reader_of("use archforge_er::ENTITY;\nuse archforge_uml::CLASS;"), Reader::Shell, "both notations' crates");
    assert_eq!(reader_of("use archforge_core::LINK;"), Reader::Shell, "no notation's crate");
    assert_eq!(reader_of("archforge_ers::X; my_archforge_er::X"), Reader::Shell, "a longer name is another crate's");
    // A crate named in a comment or a literal is not named by the code.
    let commented = "use archforge_er::ENTITY;\n// as archforge_uml::CLASS does\n/* outer /* archforge_uml */ archforge_uml */\nfn f() {}\n";
    assert_eq!(reader_of(commented), er, "another notation's crate only in comments");
    let quoted =
        "use archforge_er::ENTITY;\nconst S: &str = \"\\\" archforge_uml\";\nconst R: &str = r#\"\" archforge_uml\"#;\nconst C: char = '\"';\n";
    assert_eq!(reader_of(quoted), er, "another notation's crate only in literals");
    assert_eq!(reader_of("fn f<'a>(x: &'a str) -> &'a str { x }\nuse archforge_er::ENTITY;\n"), er, "a lifetime is no literal");
    // An inline test module is a test's to the brace that closes it, whatever comes after it, and
    // doc comments and other attributes may stand between its attribute and its declaration.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ui-egui/src");
    let code = "use archforge_core::LINK;\nfn f() { tr(\"Open\"); }\n\n";
    let tests = "#[cfg(test)]\n/// The tests.\n#[allow(clippy::unwrap_used)]\nmod tests {\n    use archforge_er::ENTITY;\n    fn g() -> (char, &'static str) { ('}', \"}\") }\n}";
    let after = "\n\nfn h() { tr(\"Close\"); }\n";
    let text = format!("{code}{tests}{after}");
    assert_eq!(split(&root.join("canvas.rs"), &text), [(Reader::Shell, format!("{code}{after}")), (Reader::Test, tests.to_string())]);
    for file in [
        "tests.rs",
        "tests/edit_menu.rs",
        "../../../apps/archforge/src/offscreen.rs",
        "../../../apps/archforge/src/offscreen/versions.rs",
        "../../../apps/archforge/src/acceptance.rs",
    ] {
        assert!(test_only(&root.join(file)), "{file} is built for the tests alone");
    }
    for file in
        ["canvas.rs", "lib.rs", "change_view/summary.rs", "../../../apps/archforge/src/agents/mod.rs", "../../../apps/archforge/src/macos_menu.rs"]
    {
        assert!(!test_only(&root.join(file)), "{file} is built for the app");
    }
}

/// A label is its catalogue row in the thread's language; what has no row is refused, not worded.
#[test]
fn model_words_are_the_catalogues_rows_in_the_language_shown() {
    // The app's shell registers its notations' catalogues, whose rows this says.
    let _shell = Shell::new(Services::default(), &crate::SCREENS);
    use archforge_mof::{DataId, DataKind, TypeRef};
    use archforge_ui_egui::model_words::{kind_label, literal_label, property_label};
    let universe = archforge_notations::universe().unwrap();
    let row = |lang: &str, context: &str, key: &str| {
        let language = LANGUAGES.iter().find(|l| l.code == lang).unwrap();
        parsed(language).strings[&(context.to_string(), key.to_string())].clone()
    };
    let data = |prop| match universe.prop(prop).unwrap().ty {
        TypeRef::Data(ty) => ty,
        other => panic!("{other:?} is no data type"),
    };
    let visibility: DataId = data(archforge_uml::named_element::VISIBILITY);
    let DataKind::Enumeration(literals) = &universe.data_type(visibility).unwrap().kind else { panic!("VisibilityKind is an enumeration") };
    let package = u32::try_from(literals.iter().position(|l| &**l == "package").unwrap()).unwrap();
    set_language("de");
    let german = |key: &str| row("de", "", key);
    let uis = crate::SCREENS.notation_uis();
    assert_eq!(property_label(uis, &universe, archforge_uml::classifier::IS_ABSTRACT), Some(german("Is abstract")));
    assert_ne!(german("Is abstract"), "Is abstract", "the row checked is really a translation");
    assert_eq!(property_label(uis, &universe, archforge_er::foreign_key::ON_DELETE), Some(german("On delete")));
    assert_eq!(literal_label(uis, &universe, visibility, package), Some(german("Package")));
    assert_eq!(kind_label(uis, &universe, archforge_uml::INTERFACE_REALIZATION), german("Interface realization"));
    assert_eq!(kind_label(uis, &universe, archforge_er::ENTITY), german("Table"));
    // Each notation's own word, where the metamodel's name isn't what the rest of the interface says.
    for (class, word) in [
        (archforge_er::ATTRIBUTE, "Column"),
        (archforge_er::GENERALIZATION, "Subtype"),
        (archforge_uml::PROPERTY, "Attribute"),
        (archforge_uml::ENUMERATION_LITERAL, "Literal"),
    ] {
        assert_eq!(kind_label(uis, &universe, class), german(word), "{word}");
    }
    assert_eq!(kind_label(uis, &universe, archforge_uml::GENERALIZATION), german("Generalization"), "UML's keeps its name");
    // A package's default-diagram binding is the diagrams' one class a comparison lists as the model's.
    assert_eq!(kind_label(uis, &universe, archforge_diagram::DEFAULT_DIAGRAM_BINDING), german("Default diagram"));
    // A concrete class no notation creates, and a diagram's own, say the one word for the rest.
    assert_eq!(kind_label(uis, &universe, archforge_uml::ACTOR), german("Element"));
    assert_eq!(kind_label(uis, &universe, archforge_diagram::NODE), german("Element"));
    // What the words have no row for is told apart from a word.
    assert_eq!(literal_label(uis, &universe, visibility, u32::MAX), None);
    assert_eq!(literal_label(uis, &universe, data(archforge_uml::comment::BODY), 0), None, "a string is no enumeration");
    assert_eq!(property_label(uis, &universe, archforge_diagram::style::FILL), None);
    assert_eq!(literal_label(uis, &universe, data(archforge_diagram::style::ROUTING), 0), None);
    // The class Profile says the modeller's word, not the row of a person's profile.
    set_language("ja");
    assert_eq!(kind_label(uis, &universe, archforge_uml::PROFILE), row("ja", "uml", "Profile"));
    assert_ne!(row("ja", "uml", "Profile"), row("ja", "", "Profile"), "the two rows differ here, so the check can tell them apart");
    set_language("en");
}

/// The words are said for the core and the notations a shell is given, from the catalogues it
/// registered, and for no other metamodel: a property or literal of a notation the shell was not
/// given has no label, as that notation's rows are not the shell's to say.
#[test]
fn model_words_are_said_for_the_core_and_the_notations_given() {
    // The app's shell registers its notations' catalogues, whose rows this says.
    let _shell = Shell::new(Services::default(), &crate::SCREENS);
    use archforge_mof::{DataKind, TypeRef};
    use archforge_ui_egui::model_words::{literal_label, property_label, words};
    let universe = archforge_notations::universe().unwrap();
    let german = |key: &str| parsed(LANGUAGES.iter().find(|l| l.code == "de").unwrap()).strings[&(String::new(), key.to_string())].clone();
    // A property's enumeration and the word of its first literal.
    let first_literal = |prop| {
        let TypeRef::Data(ty) = universe.prop(prop).unwrap().ty else { panic!("{prop:?} is no data") };
        let DataKind::Enumeration(literals) = &universe.data_type(ty).unwrap().kind else { panic!("{prop:?} is no enumeration") };
        (ty, words(&literals[0]))
    };
    let er_only: &[&'static dyn NotationUi] = &[archforge_er_ui::ER.ui];
    let (on_delete, no_action) = first_literal(archforge_er::foreign_key::ON_DELETE);
    let (visibility, _) = first_literal(archforge_uml::named_element::VISIBILITY);
    let (link_kind, trace) = first_literal(archforge_core::link::KIND);
    set_language("de");
    // ER's words, from its own catalogue, and the core's, from the shell's.
    assert_eq!(property_label(er_only, &universe, archforge_er::foreign_key::ON_DELETE), Some(german("On delete")));
    assert_eq!(literal_label(er_only, &universe, on_delete, 0), Some(german(&no_action)));
    assert_eq!(property_label(er_only, &universe, archforge_core::link::KIND), Some(german("Kind")));
    assert_eq!(literal_label(er_only, &universe, link_kind, 0), Some(german(&trace)));
    // UML's are none, as that shell was not given UML.
    assert_eq!(property_label(er_only, &universe, archforge_uml::classifier::IS_ABSTRACT), None);
    assert_eq!(literal_label(er_only, &universe, visibility, 0), None);
    // A shell given no notation says the core's alone.
    assert_eq!(property_label(&[], &universe, archforge_er::foreign_key::ON_DELETE), None);
    assert_eq!(property_label(&[], &universe, archforge_core::link::KIND), Some(german("Kind")));
    set_language("en");
}
