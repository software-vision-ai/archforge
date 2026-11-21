//! The diagram-kind check (AR-770): a diagram kind is written out once, as the public constant of
//! the crate that defines it, and everywhere else through that constant, so renaming a kind can
//! never leave a copy of the old word behind.
//!
//! [`KINDS`] names each kind's constant; its word is read from the constant's own source, so the
//! table holds no copy of it. Every Rust source under `crates/` and `apps/` is read with its
//! comments skipped, and a string literal whose whole text is a kind's word is a finding that names
//! the constant to use; the constant's own value is the one exception. The comparison is exact and
//! on whole literals: no other word matches, whatever it is a kind of (an element's, a refusal's, a
//! label's), and a kind's word inside a longer text (an expected message, an automation id, a line
//! of SQL) is part of that text. A literal whose text is a kind's word but means something else (the
//! ER part's name, a search for `er`) carries [`MARK`] in a comment on its line or the line above,
//! followed by what it is.
//!
//! Generated code (any `generated` folder) is left out: `cargo xtask generate` writes it from the
//! XMI and the `generated` step checks it; the word there is a metamodel's name.
//!
//! The table cannot fall behind the notations: outside tests, every `fn kinds` that lists its
//! diagram kinds (`&[…]`, how a notation registers them) must list constants of [`KINDS`], never a
//! literal or a constant missing from the table.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use crate::shell_io::{char_end, raw_start};
use crate::util;

/// One diagram kind's constant: the file that defines it, its name there, and the path the other
/// crates name it by.
pub struct Kind {
    pub file: &'static str,
    pub name: &'static str,
    pub path: &'static str,
}

/// Every diagram kind a notation registers, by its constant.
pub const KINDS: &[Kind] = &[
    Kind { file: "crates/er/src/lib.rs", name: "DIAGRAM_KIND", path: "archforge_er::DIAGRAM_KIND" },
    Kind { file: "crates/uml/src/lib.rs", name: "STRUCTURE_DIAGRAM", path: "archforge_uml::STRUCTURE_DIAGRAM" },
];

/// The comment that marks a literal spelled like a kind as no diagram kind.
pub const MARK: &str = "not a diagram kind:";

/// The folders, from the workspace root, whose Rust sources are scanned.
const SCOPE: [&str; 2] = ["crates", "apps"];

/// A token of a source: a name, the text of a string literal as written between its quotes, or any
/// other character.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Name(String),
    Text(String),
    Punct(char),
}

/// A source read into tokens, each with its line (from 1), and the lines whose comments hold [`MARK`].
struct Lexed {
    tokens: Vec<(Tok, usize)>,
    marked: BTreeSet<usize>,
}

/// `source` read into tokens, its comments skipped but their marks kept. Every step moves forward,
/// so a damaged file (an unclosed string or comment) ends the reading at its end.
fn lex(source: &str) -> Lexed {
    let chars: Vec<char> = source.chars().collect();
    let (mut tokens, mut marked) = (Vec::new(), BTreeSet::new());
    let (mut i, mut line) = (0usize, 1usize);
    // The characters from `from` to `to`, and the line breaks among them.
    let span = |from: usize, to: usize| chars.get(from..to.min(chars.len())).unwrap_or_default();
    let breaks = |from: usize, to: usize| span(from, to).iter().filter(|c| **c == '\n').count();
    while let Some(&c) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && next == Some('/') {
            let end = chars.iter().skip(i).position(|c| *c == '\n').map_or(chars.len(), |n| i + n);
            if span(i, end).iter().collect::<String>().contains(MARK) {
                marked.insert(line);
            }
            i = end;
        } else if c == '/' && next == Some('*') {
            // Block comments nest.
            let (mut depth, mut j) = (0usize, i);
            while j < chars.len() {
                match (chars.get(j), chars.get(j + 1)) {
                    (Some('/'), Some('*')) => {
                        depth += 1;
                        j += 2;
                    }
                    (Some('*'), Some('/')) => {
                        depth = depth.saturating_sub(1);
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => j += 1,
                }
            }
            line += breaks(i, j);
            i = j;
        } else if (c == 'r' || (c == 'b' && next == Some('r'))) && raw_start(&chars, i).is_some() {
            let (open, hashes) = raw_start(&chars, i).unwrap_or((i, 0));
            // The closing quote followed by as many hashes.
            let mut j = open + 1;
            while j < chars.len() && !(chars.get(j) == Some(&'"') && (1..=hashes).all(|h| chars.get(j + h) == Some(&'#'))) {
                j += 1;
            }
            tokens.push((Tok::Text(span(open + 1, j).iter().collect()), line));
            line += breaks(i, j);
            i = j + hashes + 1;
        } else if c == '"' || (c == 'b' && next == Some('"')) {
            let open = if c == 'b' { i + 1 } else { i };
            let mut j = open + 1;
            while j < chars.len() && chars.get(j) != Some(&'"') {
                j += if chars.get(j) == Some(&'\\') { 2 } else { 1 };
            }
            tokens.push((Tok::Text(span(open + 1, j).iter().collect()), line));
            line += breaks(i, j);
            i = j + 1;
        } else if c == '\'' || (c == 'b' && next == Some('\'')) {
            let start = if c == 'b' { i + 1 } else { i };
            // A character literal is skipped; a lifetime or a label leaves its quote, and its name
            // follows.
            match char_end(&chars, start) {
                Some(end) => i = end,
                None => {
                    tokens.push((Tok::Punct('\''), line));
                    i = start + 1;
                }
            }
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while chars.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                i += 1;
            }
            tokens.push((Tok::Name(span(start, i).iter().collect()), line));
        } else {
            tokens.push((Tok::Punct(c), line));
            i += 1;
        }
    }
    Lexed { tokens, marked }
}

/// The value of the string constant `name` in `lexed` and the index of its literal: the first
/// literal after `const <name>` and its `=`, within a few tokens.
fn defined(lexed: &Lexed, name: &str) -> Option<(String, usize)> {
    let t = &lexed.tokens;
    let at = t.windows(2).position(|w| matches!(w, [(Tok::Name(a), _), (Tok::Name(b), _)] if a == "const" && b == name))?;
    let eq = (at + 2..at + 10).find(|j| matches!(t.get(*j), Some((Tok::Punct('='), _))))?;
    match t.get(eq + 1) {
        Some((Tok::Text(v), _)) => Some((v.clone(), eq + 1)),
        _ => None,
    }
}

/// The kind literals of `lexed`, each `line N: …`: every literal whose text is one of `words` (a
/// kind's word and the constant to use), but the one at `exempt` (the constant's own value) and
/// those on or under a line marked [`MARK`].
fn literals(lexed: &Lexed, words: &[(String, &str)], exempt: Option<usize>) -> Vec<String> {
    let marked = |line: usize| lexed.marked.contains(&line) || lexed.marked.contains(&line.saturating_sub(1));
    let mut out = Vec::new();
    for (at, (token, line)) in lexed.tokens.iter().enumerate() {
        let Tok::Text(text) = token else { continue };
        let Some((word, path)) = words.iter().find(|(w, _)| w == text) else { continue };
        if Some(at) != exempt && !marked(*line) {
            out.push(format!("line {line}: the diagram kind \"{word}\" is written out; use {path}"));
        }
    }
    out
}

/// What a notation registers as one of its diagram kinds: a constant by name, or a literal.
#[derive(Debug, Clone, PartialEq)]
enum Registered {
    Constant(String),
    Literal(String),
}

/// What each `fn kinds` of `lexed` lists (`&[A, crate::B, …]`), with its line. A `kinds` that
/// declares no body, or returns anything but such a list, registers nothing here.
fn registered(lexed: &Lexed) -> Vec<(Registered, usize)> {
    let t = &lexed.tokens;
    let mut out = Vec::new();
    for at in 0..t.len() {
        if !matches!(t.get(at..at + 2), Some([(Tok::Name(f), _), (Tok::Name(k), _)]) if f == "fn" && k == "kinds") {
            continue;
        }
        // The body opens after the signature; a declaration ends at its semicolon.
        let Some(open) = (at + 2..at + 40).find(|j| matches!(t.get(*j), Some((Tok::Punct('{' | ';'), _)))) else { continue };
        if !matches!(t.get(open..open + 3), Some([(Tok::Punct('{'), _), (Tok::Punct('&'), _), (Tok::Punct('['), _)])) {
            continue;
        }
        let mut last: Option<(String, usize)> = None;
        for (token, line) in t.iter().skip(open + 3) {
            match token {
                Tok::Text(v) => out.push((Registered::Literal(v.clone()), *line)),
                Tok::Name(n) => last = Some((n.clone(), *line)),
                Tok::Punct(':') => {}
                Tok::Punct(',') => out.extend(last.take().map(|(n, l)| (Registered::Constant(n), l))),
                _ => {
                    out.extend(last.take().map(|(n, l)| (Registered::Constant(n), l)));
                    break;
                }
            }
        }
    }
    out
}

/// The findings for what `rel` (a source of the crate `krate`, outside its tests) registers: a
/// literal, or a constant [`KINDS`] does not name for that crate.
fn unlisted(rel: &str, krate: &str, lexed: &Lexed) -> Vec<String> {
    let home = format!("crates/{krate}/");
    registered(lexed)
        .into_iter()
        .filter_map(|(r, line)| match r {
            Registered::Literal(v) => Some(format!(
                "{rel} line {line}: the diagram kind \"{v}\" is registered as a literal; give it a public constant in this crate and list it in KINDS (xtask/src/kind_literals.rs)"
            )),
            Registered::Constant(n) if KINDS.iter().any(|k| k.file.starts_with(&home) && k.name == n) => None,
            Registered::Constant(n) => Some(format!(
                "{rel} line {line}: the diagram kind {n} is missing from KINDS (xtask/src/kind_literals.rs), so its word is not checked"
            )),
        })
        .collect()
}

/// Whether `rel` is a test source: under a `tests` folder or named `tests.rs`.
fn is_test(rel: &Path) -> bool {
    rel.components().any(|c| c == Component::Normal("tests".as_ref())) || rel.file_name().is_some_and(|n| n == "tests.rs")
}

/// The crate a source under `crates/<name>/src/` belongs to, if it is one.
fn crate_of(rel: &Path) -> Option<String> {
    let mut parts = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned());
    match (parts.next().as_deref(), parts.next(), parts.next().as_deref()) {
        (Some("crates"), Some(name), Some("src")) => Some(name),
        _ => None,
    }
}

/// Every finding of the check over the workspace at `root`, each named with its file, and how many
/// sources were read.
fn scan(root: &Path) -> Result<(Vec<String>, usize), String> {
    let mut out = Vec::new();
    // Each kind's word, the constant to use, and where that constant's own value is.
    let mut words = Vec::new();
    let mut exempt = Vec::new();
    for k in KINDS {
        let text = std::fs::read_to_string(root.join(k.file)).map_err(|e| format!("{}: {e}", k.file))?;
        match defined(&lex(&text), k.name) {
            Some((word, at)) => {
                words.push((word, k.path));
                exempt.push((k.file, at));
            }
            None => out.push(format!("{}: no string constant {}, which KINDS (xtask/src/kind_literals.rs) names", k.file, k.name)),
        }
    }
    let mut read = 0usize;
    for dir in SCOPE {
        for path in util::walk(&root.join(dir))? {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if rel.extension().is_none_or(|x| x != "rs") || rel.components().any(|c| c == Component::Normal("generated".as_ref())) {
                continue;
            }
            let text = std::fs::read_to_string(root.join(&rel)).map_err(|e| format!("{}: {e}", rel.display()))?;
            let lexed = lex(&text);
            read += 1;
            let shown = rel.display().to_string();
            let own = exempt.iter().find(|(file, _)| Path::new(file) == rel).map(|(_, at)| *at);
            out.extend(literals(&lexed, &words, own).into_iter().map(|f| format!("{shown} {f}")));
            if let Some(krate) = crate_of(&rel).filter(|_| !is_test(&rel)) {
                out.extend(unlisted(&shown, &krate, &lexed));
            }
        }
    }
    Ok((out, read))
}

/// The check: fails with every finding, each naming the constant to use.
pub fn run() -> Result<(), String> {
    let (found, read) = scan(&util::root())?;
    if read == 0 {
        return Err("kinds: no Rust source found under crates/ or apps/".into());
    }
    if found.is_empty() {
        println!("kinds: {read} sources, no diagram kind written out");
        return Ok(());
    }
    for line in &found {
        eprintln!("kinds: {line}");
    }
    Err(format!(
        "{} diagram kind finding(s); a literal spelled like a kind that is none carries `// {MARK} <what it is>` on its line or the line above",
        found.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words of the tests: `er` and `uml.structure`, by their constants.
    fn words() -> Vec<(String, &'static str)> {
        vec![("er".into(), "archforge_er::DIAGRAM_KIND"), ("uml.structure".into(), "archforge_uml::STRUCTURE_DIAGRAM")]
    }

    /// The findings for `source` with no exempt literal.
    fn found(source: &str) -> Vec<String> {
        literals(&lex(source), &words(), None)
    }

    /// A kind's word as a whole literal, in any shape, is found with its line and its constant.
    #[test]
    fn a_kind_written_out_is_found_with_its_line_and_its_constant() {
        let source = "json!({ \"kind\": \"er\" });\nlet k = \"uml.structure\";\nlet r = r#\"er\"#;\nmatch kind {\n    \"er\" => 1,\n}\njson!({\"kind\":\"er\"});";
        assert_eq!(
            found(source),
            [
                "line 1: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND",
                "line 2: the diagram kind \"uml.structure\" is written out; use archforge_uml::STRUCTURE_DIAGRAM",
                "line 3: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND",
                "line 5: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND",
                "line 7: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND",
            ]
        );
    }

    /// Other words, a kind's word inside a longer text, in a comment or a character, are no finding.
    #[test]
    fn other_words_longer_texts_and_comments_are_no_finding() {
        let source = "// \"er\" in a comment\n/* \"er\" /* nested */ \"uml.structure\" */\n/// \"er\"\nlet a = (\"ER\", \"er \", \"cardinality\", \"name\", \"uml.class\");\nlet b = \"is not one of er, uml.structure\";\nlet c = \"workspace.new-diagram.type.uml.structure\";\nfn f<'a>(x: &'a str) -> char { 'e' }";
        assert_eq!(found(source), Vec::<String>::new());
    }

    /// A mark on the literal's line or the line above lets it stand; two lines above, it does not.
    #[test]
    fn a_marked_literal_stands_and_the_mark_reaches_one_line() {
        assert_eq!(found("let filter = \"er\"; // not a diagram kind: a search"), Vec::<String>::new());
        assert_eq!(found("// not a diagram kind: a search\nlet filter = \"er\";"), Vec::<String>::new());
        assert_eq!(
            found("// not a diagram kind: a search\n\nlet filter = \"er\";"),
            ["line 3: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND"]
        );
        // A mark without its colon, or inside a string, is none.
        assert_eq!(found("let filter = \"er\"; // not a diagram kind").len(), 1);
        assert_eq!(found("let filter = (\"er\", \"// not a diagram kind: x\");").len(), 1);
    }

    /// The constant's own value is read from its source and is the one literal allowed there.
    #[test]
    fn the_constant_value_is_read_and_exempt() {
        let source = "/// The kind.\npub const DIAGRAM_KIND: &str = \"er\";\nconst OTHER: &str = \"er\";";
        let lexed = lex(source);
        let (word, at) = defined(&lexed, "DIAGRAM_KIND").unwrap();
        assert_eq!(word, "er");
        assert_eq!(literals(&lexed, &words(), Some(at)), ["line 3: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND"]);
        assert_eq!(defined(&lex("pub const DIAGRAM_KIND: &'static str = \"erd\";"), "DIAGRAM_KIND").map(|d| d.0), Some("erd".into()));
        assert_eq!(defined(&lex("pub const DIAGRAM_KIND: &str = concat!(\"e\", \"r\");"), "DIAGRAM_KIND"), None);
        assert_eq!(defined(&lex("pub static DIAGRAM_KIND: &str = \"er\";"), "DIAGRAM_KIND"), None);
    }

    /// A notation's kinds list names constants; a literal there, or a constant KINDS lacks, is a
    /// finding, and a kinds that is declared only, or returns no list, registers nothing.
    #[test]
    fn a_registered_kind_must_be_a_listed_constant() {
        let er = "fn kinds(&self) -> &'static [&'static str] {\n    &[DIAGRAM_KIND]\n}";
        assert_eq!(unlisted("crates/er/src/lib.rs", "er", &lex(er)), Vec::<String>::new());
        let both = "fn kinds(&self) -> &'static [&'static str] { &[crate::DIAGRAM_KIND, SKETCH, \"bpmn\"] }";
        assert_eq!(
            unlisted("crates/er/src/lib.rs", "er", &lex(both)),
            [
                "crates/er/src/lib.rs line 1: the diagram kind SKETCH is missing from KINDS (xtask/src/kind_literals.rs), so its word is not checked",
                "crates/er/src/lib.rs line 1: the diagram kind \"bpmn\" is registered as a literal; give it a public constant in this crate and list it in KINDS (xtask/src/kind_literals.rs)",
            ]
        );
        // The constant must be listed for the crate that registers it.
        assert_eq!(unlisted("crates/uml/src/lib.rs", "uml", &lex(er)).len(), 1);
        let declared = "fn kinds(&self) -> &'static [&'static str];\nfn other() -> u8 { &[\"x\"]; 1 }";
        assert_eq!(registered(&lex(declared)), Vec::new());
        assert_eq!(registered(&lex("fn kinds(&self) -> Vec<&'static str> { self.all() }")), Vec::new());
        assert_eq!(registered(&lex("fn kinds(&self) -> &'static [&'static str] { &[] }")), Vec::new());
    }

    /// Tests and generated code are told apart by their paths, and a source's crate by its folder.
    #[test]
    fn tests_and_crates_are_told_by_their_paths() {
        assert!(is_test(Path::new("crates/er/tests/diagram.rs")) && is_test(Path::new("crates/ui-egui/src/tests.rs")));
        assert!(is_test(Path::new("crates/ui-egui/src/tests/branches.rs")) && !is_test(Path::new("crates/er/src/lib.rs")));
        assert_eq!(crate_of(Path::new("crates/er/src/lib.rs")), Some("er".into()));
        assert_eq!(crate_of(Path::new("crates/er/tests/er.rs")), None);
        assert_eq!(crate_of(Path::new("apps/archforge/src/main.rs")), None);
    }

    /// A damaged file never hangs the reading.
    #[test]
    fn a_damaged_file_never_hangs_the_reading() {
        for source in ["\"er", "r#\"er", "/* \"er\"", "'", "b'", "\"\\", "fn kinds() { &[A, "] {
            let lexed = lex(source);
            let _ = (literals(&lexed, &words(), None), registered(&lexed));
        }
    }

    /// The repository as it stands writes no diagram kind out, and every kind its notations
    /// register is listed.
    #[test]
    fn the_repository_writes_no_diagram_kind_out() {
        let (found, read) = scan(&util::root()).unwrap();
        assert!(read > 0, "sources were read");
        assert_eq!(found, Vec::<String>::new());
    }
}
