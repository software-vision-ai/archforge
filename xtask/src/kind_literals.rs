//! The diagram-kind check (AR-770): a diagram kind is written out once, as the public constant of
//! the crate that defines it, and everywhere else through that constant, so renaming a kind can
//! never leave a copy of the old word behind.
//!
//! The kinds are the rows of [`TABLE`], a plain data list: each kind's word, the constant that holds
//! it and the file that defines that constant. The notations' test
//! `the_gates_table_of_kinds_lists_every_registered_kind` compares the table with what the notations
//! register at run time, so a kind registered without its row fails the test step; this check reads
//! no registration itself and resolves no names.
//!
//! The sources are read by a Rust parser (`syn`), not by hand: a literal is what the language says it
//! is (a raw or C string, an escape), and its value is compared as the program sees it, cooked. Every
//! Rust source under `crates/` and `apps/` is parsed, and a string literal (a plain, byte or C string,
//! in the code or inside a macro's tokens such as `json!`) whose whole value is a kind's word is a
//! finding that names the constant to use. The one exception per kind is its constant's own value: in
//! the file the table names, the value of the constant of that name among the file's own top-level
//! items, which must be the table's word. The comparison is exact and on whole literals: no other
//! word matches, whatever it is a kind of (an element's, a refusal's, a label's), and a kind's word
//! inside a longer text (an expected message, an automation id, a line of SQL) is part of that text.
//! Comments and doc comments, in macro tokens too, are not code and never match. A source that does
//! not parse is a finding, so nothing is passed unread.
//!
//! A literal whose value is a kind's word but means something else (the ER part's name, a search for
//! `er`) carries [`MARK`] in a plain `//` comment, followed by what it is: on its own line, or on a
//! line of its own (nothing but the comment) right above it. A doc comment or a block comment never
//! marks.
//!
//! Generated code (any `generated` folder) is left out: `cargo xtask generate` writes it from the XMI
//! and the `generated` step checks it; the word there is a metamodel's name.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use proc_macro2::{Delimiter, LineColumn, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Item, Lit, Meta};

use crate::util;

/// The table of diagram kinds, from the workspace root.
pub const TABLE: &str = "xtask/diagram-kinds.tsv";

/// The comment that marks a literal spelled like a kind as no diagram kind; it must be followed by
/// what the literal is.
pub const MARK: &str = "not a diagram kind:";

/// The folders, from the workspace root, whose Rust sources are scanned.
const SCOPE: [&str; 2] = ["crates", "apps"];

/// One diagram kind of the table: its word, the constant that holds it (the path to use), and the
/// file that defines that constant.
#[derive(Debug, Clone, PartialEq)]
struct Kind {
    word: String,
    constant: String,
    file: String,
}

impl Kind {
    /// The constant's own name, the last part of its path.
    fn name(&self) -> &str {
        self.constant.rsplit("::").next().unwrap_or(&self.constant)
    }
}

/// The kinds of the table's text: one per line, its word, constant and file separated by tabs; a
/// blank line or one starting with `#` is none. A line of another shape, a word listed twice, and a
/// table with no kind are refused, each named.
fn table(text: &str) -> Result<Vec<Kind>, String> {
    let mut kinds: Vec<Kind> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [word, constant, file] = fields.as_slice() else {
            return Err(format!("{TABLE} line {}: not a word, a constant and a file separated by tabs", n + 1));
        };
        if [word, constant, file].iter().any(|f| f.trim().is_empty() || f.trim() != **f) {
            return Err(format!("{TABLE} line {}: an empty field, or one with spaces around it", n + 1));
        }
        if kinds.iter().any(|k| k.word == *word) {
            return Err(format!("{TABLE} line {}: the word \"{word}\" is listed twice", n + 1));
        }
        kinds.push(Kind { word: word.to_string(), constant: constant.to_string(), file: file.to_string() });
    }
    if kinds.is_empty() {
        return Err(format!("{TABLE} lists no diagram kind"));
    }
    Ok(kinds)
}

/// A place in a source: its line (from 1) and its column in characters (from 0).
type Pos = (usize, usize);

/// The place a parser's line and column stand for.
fn pos(at: LineColumn) -> Pos {
    (at.line, at.column)
}

/// `text` parsed as a Rust file, or what stops it, with its line.
fn parse(text: &str) -> Result<syn::File, String> {
    syn::parse_file(text).map_err(|e| format!("line {}: not read as Rust ({e}), so its kinds cannot be checked", e.span().start().line))
}

/// A string literal's value (of a plain, byte or C string, as the program sees it) and where it
/// starts.
struct Text {
    bytes: Vec<u8>,
    start: Pos,
}

/// The literals of a source: its string values, and where every literal of any kind (doc text
/// too) starts and ends, so a `//` inside one is no comment. `placing` is set while documentation
/// is walked: its literals are placed, never read.
#[derive(Default)]
struct Literals {
    texts: Vec<Text>,
    spans: Vec<(Pos, Pos)>,
    placing: bool,
}

impl Literals {
    /// Places one literal; its value is read when `read` and it is a string of any kind.
    fn take(&mut self, lit: &Lit, read: bool) {
        let span = lit.span();
        let start = pos(span.start());
        self.spans.push((start, pos(span.end())));
        let bytes = match lit {
            Lit::Str(s) if read => Some(s.value().into_bytes()),
            Lit::ByteStr(b) if read => Some(b.value()),
            Lit::CStr(c) if read => Some(c.value().into_bytes()),
            _ => None,
        };
        if let Some(bytes) = bytes {
            self.texts.push(Text { bytes, start });
        }
    }

    /// Takes every literal of a macro's tokens, in its groups too. An attribute among them (`#` or
    /// `#!`, then a bracketed group) is read as an attribute is ([`Literals::attribute`]), so a doc
    /// comment's text is placed but never read.
    fn tokens(&mut self, stream: TokenStream, read: bool) {
        let trees: Vec<TokenTree> = stream.into_iter().collect();
        let mut i = 0;
        while let Some(tree) = trees.get(i) {
            let hash = matches!(tree, TokenTree::Punct(p) if p.as_char() == '#');
            let bang = matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!');
            let at = if bang { i + 2 } else { i + 1 };
            if hash
                && let Some(TokenTree::Group(g)) = trees.get(at)
                && g.delimiter() == Delimiter::Bracket
            {
                if read {
                    self.attribute(g.stream());
                } else {
                    self.tokens(g.stream(), false);
                }
                i = at + 1;
                continue;
            }
            match tree {
                TokenTree::Group(g) => self.tokens(g.stream(), read),
                TokenTree::Literal(l) => self.take(&Lit::new(l.clone()), read),
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
            i += 1;
        }
    }
}

impl Literals {
    /// Takes the literals of an attribute's tokens (`doc = …` or `cfg_attr(…)` in a macro, the list
    /// of a `cfg_attr(…)` or `serde(…)` in the code):
    /// each part between commas that starts with `doc` (`doc = …`, `doc(…)`) is documentation, at
    /// any depth, so `cfg_attr(…, doc = …)` is documentation as `doc` is: its literals, in macros
    /// such as `concat!` too, are placed but never read. Every other part is read.
    fn attribute(&mut self, stream: TokenStream) {
        let trees: Vec<TokenTree> = stream.into_iter().collect();
        for part in trees.split(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ',')) {
            let doc = matches!(part.first(), Some(TokenTree::Ident(i)) if i == "doc");
            for tree in part {
                match tree {
                    TokenTree::Group(g) if doc => self.tokens(g.stream(), false),
                    TokenTree::Group(g) => self.attribute(g.stream()),
                    TokenTree::Literal(l) => self.take(&Lit::new(l.clone()), !doc),
                    TokenTree::Ident(_) | TokenTree::Punct(_) => {}
                }
            }
        }
    }

    /// Runs `walk` with the literals it meets placed but never read: documentation.
    fn placed(&mut self, walk: impl FnOnce(&mut Self)) {
        let before = std::mem::replace(&mut self.placing, true);
        walk(self);
        self.placing = before;
    }
}

impl<'ast> Visit<'ast> for Literals {
    /// A literal of the code.
    fn visit_lit(&mut self, lit: &'ast Lit) {
        self.take(lit, !self.placing);
    }

    /// A macro's path, and the literals of its tokens, which the parser leaves unread.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        visit::visit_macro(self, mac);
        self.tokens(mac.tokens.clone(), !self.placing);
    }

    /// An attribute's literals. Documentation (`doc = …` whatever expression it holds, `doc(…)`, and
    /// a `doc` part of a list such as `cfg_attr(…, doc = …)`) is placed, so nothing in it reads as a
    /// comment mark or a block comment's start, but never read; every other value is read.
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        let doc = attr.path().is_ident("doc");
        match &attr.meta {
            Meta::NameValue(nv) if doc => self.placed(|me| me.visit_expr(&nv.value)),
            Meta::NameValue(nv) => self.visit_expr(&nv.value),
            Meta::List(list) if doc => self.tokens(list.tokens.clone(), false),
            Meta::List(list) => self.attribute(list.tokens.clone()),
            Meta::Path(_) => {}
        }
    }
}

/// The marks of `text`: the lines whose plain `//` comment holds [`MARK`] with some words after it,
/// and of those the lines that hold nothing else (no code before the comment). The text is read
/// once, with the literals the parser placed (`spans`) skipped as code on the lines they start and
/// end on: a `//` or `/*` inside one is text. Block comments, nested ones too, never mark, and
/// neither do doc comments: the parser places them as doc text, so they are skipped with the
/// literals.
fn marks(text: &str, spans: &[(Pos, Pos)]) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let chars: Vec<char> = text.chars().collect();
    // Where each line starts, as a character index, to place the parser's line and column.
    let mut starts = vec![0usize];
    starts.extend(chars.iter().enumerate().filter(|(_, c)| **c == '\n').map(|(i, _)| i + 1));
    let index = |(line, column): Pos| starts.get(line.checked_sub(1)?).map(|s| s + column);
    let line_of = |i: usize| starts.partition_point(|s| *s <= i);
    let mut literals: Vec<(usize, usize)> = spans.iter().filter_map(|(s, e)| Some((index(*s)?, index(*e)?))).collect();
    literals.sort_unstable();
    let (mut any, mut alone, mut code) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let (mut i, mut next, mut depth) = (0usize, 0usize, 0usize);
    while let Some(&c) = chars.get(i) {
        let after = chars.get(i + 1).copied();
        while literals.get(next).is_some_and(|(_, end)| *end <= i) {
            next += 1;
        }
        if depth > 0 {
            match (c, after) {
                ('/', Some('*')) => (depth, i) = (depth + 1, i + 2),
                ('*', Some('/')) => (depth, i) = (depth - 1, i + 2),
                _ => i += 1,
            }
        } else if let Some((_, end)) = literals.get(next).filter(|(start, _)| *start == i) {
            // A literal is code on the line it opens and on the line it closes.
            code.insert(line_of(i));
            code.insert(line_of(end.saturating_sub(1).max(i)));
            i = (*end).max(i + 1);
        } else if (c, after) == ('/', Some('/')) {
            let end = chars.iter().skip(i).position(|c| *c == '\n').map_or(chars.len(), |n| i + n);
            let comment: String = chars.get(i..end).unwrap_or_default().iter().collect();
            let worded = comment.find(MARK).is_some_and(|at| comment.get(at + MARK.len()..).is_some_and(|rest| !rest.trim().is_empty()));
            if worded {
                let line = line_of(i);
                any.insert(line);
                if !code.contains(&line) {
                    alone.insert(line);
                }
            }
            i = end;
        } else if (c, after) == ('/', Some('*')) {
            (depth, i) = (1, i + 2);
        } else {
            if !c.is_whitespace() {
                code.insert(line_of(i));
            }
            i += 1;
        }
    }
    (any, alone)
}

/// The kind literals of a parsed source, each `line N: …`: every string whose value is one of
/// `words` (a kind's word and the constant to use), but those starting at `exempt` (the constants'
/// own values), those on a line marked [`MARK`], and those under a line that holds the mark alone.
fn literals(file: &syn::File, text: &str, words: &[(String, String)], exempt: &BTreeSet<Pos>) -> Vec<String> {
    let mut found = Literals::default();
    found.visit_file(file);
    let (any, alone) = marks(text, &found.spans);
    // A mark stands for its own line, or, on a line of its own, for the line under it.
    let is_marked = |line: usize| any.contains(&line) || line.checked_sub(1).is_some_and(|above| alone.contains(&above));
    let mut out = Vec::new();
    for t in &found.texts {
        let Some((word, path)) = words.iter().find(|(w, _)| w.as_bytes() == t.bytes.as_slice()) else { continue };
        let (line, _) = t.start;
        if !exempt.contains(&t.start) && !is_marked(line) {
            out.push(format!("line {line}: the diagram kind \"{word}\" is written out; use {path}"));
        }
    }
    out
}

/// An expression with its parentheses and invisible groups taken off.
fn strip_expr(e: &Expr) -> &Expr {
    match e {
        Expr::Paren(p) => strip_expr(&p.expr),
        Expr::Group(g) => strip_expr(&g.expr),
        other => other,
    }
}

/// Where the literal of `kind`'s constant starts in `text`, its defining file: the value of the one
/// constant of that name among the file's own top-level items, which must be a string literal
/// holding the table's word. Anything else is refused, named.
fn definition(kind: &Kind, text: &str) -> Result<Pos, String> {
    let file = parse(text)?;
    let name = kind.name();
    let consts: Vec<&syn::ItemConst> = file
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Const(c) if c.ident.unraw() == name => Some(c),
            _ => None,
        })
        .collect();
    let [only] = consts.as_slice() else {
        return Err(format!("{} top-level constants named {name}, which {TABLE} names; one is needed", consts.len()));
    };
    match strip_expr(&only.expr) {
        Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) if s.value() == kind.word => Ok(pos(s.span().start())),
        Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) => {
            Err(format!("{name} holds \"{}\", but {TABLE} lists \"{}\"; the two must agree", s.value(), kind.word))
        }
        _ => Err(format!("{name}, which {TABLE} names, is no string literal")),
    }
}

/// The findings of one source, each named with `rel`: its kind literals, or that it cannot be
/// parsed.
fn check(rel: &str, text: &str, words: &[(String, String)], exempt: &BTreeSet<Pos>) -> Vec<String> {
    match parse(text) {
        Ok(file) => literals(&file, text, words, exempt).into_iter().map(|f| format!("{rel} {f}")).collect(),
        Err(e) => vec![format!("{rel} {e}")],
    }
}

/// Every finding of the check over the workspace at `root`, each named with its file, and how many
/// sources were read.
fn scan(root: &Path) -> Result<(Vec<String>, usize), String> {
    let kinds = table(&std::fs::read_to_string(root.join(TABLE)).map_err(|e| format!("{TABLE}: {e}"))?)?;
    let words: Vec<(String, String)> = kinds.iter().map(|k| (k.word.clone(), k.constant.clone())).collect();
    let mut out = Vec::new();
    // Where the constants' own values are, in their files.
    let mut exempt: BTreeSet<(String, Pos)> = BTreeSet::new();
    for k in &kinds {
        let text = std::fs::read_to_string(root.join(&k.file)).map_err(|e| format!("{}: {e}", k.file))?;
        match definition(k, &text) {
            Ok(at) => {
                exempt.insert((k.file.clone(), at));
            }
            Err(e) => out.push(format!("{}: {e}", k.file)),
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
            read += 1;
            let own: BTreeSet<Pos> = exempt.iter().filter(|(file, _)| Path::new(file) == rel).map(|(_, at)| *at).collect();
            out.extend(check(&rel.display().to_string(), &text, &words, &own));
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
        "{} diagram kind finding(s); a literal spelled like a kind that is none carries `// {MARK} <what it is>` on its line or on a line of its own above it",
        found.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words of the tests: `er` and `uml.structure`, by their constants.
    fn words() -> Vec<(String, String)> {
        vec![("er".into(), "archforge_er::DIAGRAM_KIND".into()), ("uml.structure".into(), "archforge_uml::STRUCTURE_DIAGRAM".into())]
    }

    /// The findings for `source`, with no exempt literal.
    fn found(source: &str) -> Vec<String> {
        check("f.rs", source, &words(), &BTreeSet::new())
    }

    /// The finding for `er` written out on `line` of `f.rs`.
    fn er_at(line: usize) -> String {
        format!("f.rs line {line}: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND")
    }

    /// The table's row for ER's kind.
    fn er() -> Kind {
        Kind { word: "er".into(), constant: "archforge_er::DIAGRAM_KIND".into(), file: "crates/er/src/lib.rs".into() }
    }

    /// A kind's word as a whole literal, in any shape and inside macros, is found with its line and
    /// its constant.
    #[test]
    fn a_kind_written_out_is_found_with_its_line_and_its_constant() {
        let source = "fn f() {\n    json!({ \"kind\": \"er\" });\n    let k = \"uml.structure\";\n    let r = r#\"er\"#;\n    match kind {\n        \"er\" => 1,\n        _ => 2,\n    };\n    let v = vec![format!(\"{}\", \"er\")];\n    let b = (b\"er\", c\"er\", br\"er\");\n}";
        assert_eq!(
            found(source),
            [
                er_at(2),
                "f.rs line 3: the diagram kind \"uml.structure\" is written out; use archforge_uml::STRUCTURE_DIAGRAM".into(),
                er_at(4),
                er_at(6),
                er_at(9),
                er_at(10),
                er_at(10),
                er_at(10),
            ]
        );
    }

    /// Other words, a kind's word inside a longer text, in a comment, a doc comment or a character,
    /// are no finding.
    #[test]
    fn other_words_longer_texts_and_comments_are_no_finding() {
        let source = "// \"er\" in a comment\n/* \"er\" /* nested */ \"uml.structure\" */\n/// er\n#[doc = \"er\"]\nfn f<'a>(x: &'a str) -> char {\n    let a = (\"ER\", \"er \", \"cardinality\", \"name\", \"uml.class\");\n    let b = \"is not one of er, uml.structure\";\n    let c = \"workspace.new-diagram.type.uml.structure\";\n    'e'\n}";
        assert_eq!(found(source), Vec::<String>::new());
    }

    /// A doc comment inside a macro's tokens is a comment too, outer or inner, while a string beside
    /// it is read (CodeRabbit on #572: `///er` in `stringify!`).
    #[test]
    fn a_doc_comment_in_a_macro_is_no_finding_and_its_neighbours_are() {
        assert_eq!(found("const S: &str = stringify! {\n    ///er\n    struct S;\n};"), Vec::<String>::new());
        assert_eq!(found("m! {\n    //!er\n    fn f() {}\n}"), Vec::<String>::new());
        assert_eq!(found("m! {\n    ///er\n    const K: &str = \"er\";\n}"), [er_at(3)]);
        // An attribute other than doc is read, in a macro and outside one.
        assert_eq!(found("m! {\n    #[name = \"er\"]\n    fn f() {}\n}"), [er_at(2)]);
        assert_eq!(found("#[cfg(feature = \"er\")]\nfn f() {}"), [er_at(1)]);
    }

    /// Documentation is placed but never read whatever expression or list holds it: a mark or a
    /// kind's word inside it is text (CodeRabbit on #572: `doc = concat!(…)`), while every other
    /// attribute value is read.
    #[test]
    fn documentation_in_any_attribute_shape_is_placed_and_never_read() {
        assert_eq!(found("#[doc = concat!(\"// not a diagram kind: documentation\")] pub const COPY: &str = \"er\";"), [er_at(1)]);
        assert_eq!(found("#[doc = concat!(\"e\", \"r\", \"er\")]\nfn f() {}"), Vec::<String>::new());
        assert_eq!(found("#[cfg_attr(test, doc = \"er\")]\nfn f() {}"), Vec::<String>::new());
        assert_eq!(found("#[cfg_attr(test, cfg_attr(unix, doc = concat!(\"er\")))]\nfn f() {}"), Vec::<String>::new());
        assert_eq!(found("#[cfg_attr(test, doc = \"// not a diagram kind: x\")] pub const COPY: &str = \"er\";"), [er_at(1)]);
        assert_eq!(found("#[doc(alias = \"er\")]\nfn f() {}"), Vec::<String>::new());
        assert_eq!(found("m! {\n    #[cfg_attr(test, doc = \"er\")]\n    #[doc = concat!(\"er\")]\n    fn f() {}\n}"), Vec::<String>::new());
        // A doc expression before a real mark leaves the mark working.
        assert_eq!(found("#[doc = concat!(\"a\", \"b\")]\nconst F: &str = \"er\"; // not a diagram kind: a search"), Vec::<String>::new());
        // Every other attribute value is read: a name-value, a list's parts beside a doc part, in a
        // macro too.
        assert_eq!(found("#[name = concat!(\"er\")]\nfn f() {}"), [er_at(1)]);
        assert_eq!(found("#[cfg_attr(feature = \"er\", doc = \"x\")]\nfn f() {}"), [er_at(1)]);
        assert_eq!(found("#[serde(rename = \"er\")]\nstruct S;"), [er_at(1)]);
        assert_eq!(found("m! {\n    #[cfg_attr(feature = \"er\", doc = \"x\")]\n    fn f() {}\n}"), [er_at(2)]);
    }

    /// A mark with words after it, on the literal's line or the line above, lets it stand; two
    /// lines above, empty, or inside a string, it does not (CodeRabbit on #572: a bare mark).
    #[test]
    fn a_marked_literal_stands_and_the_mark_needs_its_words_and_its_place() {
        assert_eq!(found("fn f() { let filter = \"er\"; } // not a diagram kind: a search"), Vec::<String>::new());
        assert_eq!(found("// not a diagram kind: a search\nconst F: &str = \"er\";"), Vec::<String>::new());
        assert_eq!(found("// not a diagram kind: a search\n\nconst F: &str = \"er\";"), [er_at(3)]);
        assert_eq!(found("const F: &str = \"er\"; // not a diagram kind"), [er_at(1)]);
        assert_eq!(found("const F: &str = \"er\"; // not a diagram kind:"), [er_at(1)]);
        assert_eq!(found("const F: &str = \"er\"; // not a diagram kind:   "), [er_at(1)]);
        assert_eq!(found("const F: (&str, &str) = (\"er\", \"// not a diagram kind: x\");"), [er_at(1)]);
        // A `//` inside a literal opens no comment, and the real one after it still counts.
        assert_eq!(found("const F: (&str, &str) = (\"http://x\", \"er\"); // not a diagram kind: a search"), Vec::<String>::new());
        // Columns count characters, as the parser's do: thirty bytes more than characters before a
        // literal leave its `//` inside it, so a mark written in a string marks nothing.
        let wide = "€".repeat(15);
        assert_eq!(found(&format!("const F: (&str, &str) = (\"{wide}\", \"// not a diagram kind: x\");\nconst G: &str = \"er\";")), [er_at(2)]);
        assert_eq!(found(&format!("const F: (&str, &str) = (\"{wide}\", \"er\"); // not a diagram kind: a search")), Vec::<String>::new());
    }

    /// A mark on the line above counts only when that line holds nothing else: a mark trailing code
    /// is that line's (pr-agent on c01213a6), and the closing line of a literal spanning lines is
    /// code (CodeRabbit on #572). Doc comments and block comments never mark, and a doc attribute's
    /// text opens no comment.
    #[test]
    fn only_a_plain_line_comment_marks_and_only_one_alone_marks_the_line_under_it() {
        assert_eq!(found("fn f() {\n    let label = \"x\"; // not a diagram kind: a label\n    json!({ \"kind\": \"er\" });\n}"), [er_at(3)]);
        assert_eq!(found("fn f() {\n    // not a diagram kind: a label\n    json!({ \"kind\": \"er\" });\n}"), Vec::<String>::new());
        // A literal before the comment is code, even with nothing else on its line.
        assert_eq!(found("const F: [&str; 2] = [\n    \"x\" // not a diagram kind: a label\n    , \"er\"];"), [er_at(3)]);
        // So is the closing line of a raw string that spans lines.
        assert_eq!(found("const F: [&str; 2] = [r#\"a\nb\"# // not a diagram kind: a label\n, \"er\"];"), [er_at(3)]);
        assert_eq!(found("const F: [&str; 2] = [\"a\nb\" // not a diagram kind: a label\n, \"er\"];"), [er_at(3)]);
        // Code before the comment only in a block comment is no code.
        assert_eq!(found("/* c */ // not a diagram kind: a label\nconst F: &str = \"er\";"), Vec::<String>::new());
        assert_eq!(found("/// not a diagram kind: a doc\nconst F: &str = \"er\";"), [er_at(2)]);
        assert_eq!(found("//! not a diagram kind: a doc\nconst F: &str = \"er\";"), [er_at(2)]);
        assert_eq!(found("const F: &str = \"er\"; /// not a diagram kind: a doc\nconst G: u8 = 1;"), [er_at(1)]);
        // Four slashes are a plain comment.
        assert_eq!(found("const F: &str = \"er\"; //// not a diagram kind: a search"), Vec::<String>::new());
        assert_eq!(found("/* // not a diagram kind: x */\nconst F: &str = \"er\";"), [er_at(2)]);
        assert_eq!(found("const F: &str = \"er\"; /* // not a diagram kind: x */"), [er_at(1)]);
        // Block comments nest: the inner one's end leaves the mark inside the outer one.
        assert_eq!(found("/* /* */ // not a diagram kind: x */\nconst F: &str = \"er\";"), [er_at(2)]);
        // After a block comment ends, a line comment marks again.
        assert_eq!(found("/* x */ const F: &str = \"er\"; // not a diagram kind: a search"), Vec::<String>::new());
        assert_eq!(found("#[doc = \"a /* b\"]\nconst F: &str = \"er\"; // not a diagram kind: a search"), Vec::<String>::new());
    }

    /// A raw C string is one literal, so the string after it is read as it is (CodeRabbit on #572).
    #[test]
    fn a_raw_c_string_does_not_swallow_the_next_literal() {
        assert_eq!(found("fn f() { let _ = cr#\"a\"b\"#; let kind = \"er\"; }"), [er_at(1)]);
    }

    /// A string is compared by its value, escapes cooked: `"\x65r"` is `er` (CodeRabbit on #572).
    #[test]
    fn a_string_is_compared_by_its_cooked_value() {
        assert_eq!(found("const K: &str = \"\\x65r\";"), [er_at(1)]);
        assert_eq!(found("const K: &str = \"e\\u{72}\";"), [er_at(1)]);
        assert_eq!(found("const K: &str = r\"\\x65r\";"), Vec::<String>::new());
    }

    /// The constant's own value is found among its file's top-level items, cooked, and must be the
    /// table's word; only that literal is exempt (CodeRabbit on #572: the top-level value, and every
    /// definition in a file).
    #[test]
    fn the_defining_constant_is_read_from_its_file_and_only_its_value_is_exempt() {
        let source = "mod sample {\n    pub const DIAGRAM_KIND: &str = \"sample\";\n}\n/// The kind.\npub const DIAGRAM_KIND: &str = \"\\x65r\";\npub const STRUCTURE_DIAGRAM: &str = (\"uml.structure\");\nconst COPY: &str = \"er\";";
        let uml = Kind { word: "uml.structure".into(), constant: "archforge_uml::STRUCTURE_DIAGRAM".into(), file: "f.rs".into() };
        let (at_er, at_uml) = (definition(&er(), source).unwrap(), definition(&uml, source).unwrap());
        assert_eq!((at_er, at_uml), ((5, 31), (6, 37)));
        assert_eq!(check("f.rs", source, &words(), &BTreeSet::from([at_er, at_uml])), [er_at(7)]);
        assert_eq!(check("f.rs", source, &words(), &BTreeSet::from([at_er])).len(), 2, "only the listed values are exempt");
        assert_eq!(
            definition(&er(), "pub const OTHER: &str = \"er\";"),
            Err(format!("0 top-level constants named DIAGRAM_KIND, which {TABLE} names; one is needed"))
        );
        assert_eq!(
            definition(&er(), "pub const DIAGRAM_KIND: &str = \"erd\";"),
            Err(format!("DIAGRAM_KIND holds \"erd\", but {TABLE} lists \"er\"; the two must agree"))
        );
        assert!(definition(&er(), "pub const DIAGRAM_KIND: &str = concat!(\"e\", \"r\");").unwrap_err().contains("is no string literal"));
        assert!(
            definition(&er(), "pub const DIAGRAM_KIND: &str = \"er\";\npub const DIAGRAM_KIND: &str = \"er\";")
                .unwrap_err()
                .starts_with("2 top-level")
        );
        assert_eq!(definition(&er(), "pub const r#DIAGRAM_KIND: &str = \"er\";"), Ok((1, 33)));
        assert!(definition(&er(), "fn f( {").unwrap_err().contains("not read as Rust"));
    }

    /// The table is read line by line; a line of another shape, a word listed twice and an empty
    /// table are refused, and the repository's own table reads.
    #[test]
    fn the_table_is_read_and_a_damaged_one_refused() {
        let text = "# a comment\n\ner\tarchforge_er::DIAGRAM_KIND\tcrates/er/src/lib.rs\n";
        assert_eq!(table(text), Ok(vec![er()]));
        assert_eq!(er().name(), "DIAGRAM_KIND");
        for (damaged, why) in [
            ("er\tarchforge_er::DIAGRAM_KIND", "line 1: not a word, a constant and a file"),
            ("er\tarchforge_er::DIAGRAM_KIND\tcrates/er/src/lib.rs\tmore", "line 1: not a word, a constant and a file"),
            ("er\t\tcrates/er/src/lib.rs", "line 1: an empty field"),
            ("er \tarchforge_er::DIAGRAM_KIND\tcrates/er/src/lib.rs", "line 1: an empty field, or one with spaces"),
            ("er\tA\tf.rs\ner\tB\tg.rs", "line 2: the word \"er\" is listed twice"),
            ("# only a comment", "lists no diagram kind"),
        ] {
            assert!(table(damaged).is_err_and(|e| e.contains(why)), "{damaged:?}: {:?}", table(damaged));
        }
        let own = table(&std::fs::read_to_string(util::root().join(TABLE)).unwrap()).unwrap();
        assert!(own.iter().any(|k| *k == er()), "{own:?}");
    }

    /// A source that does not parse is a finding, never passed unread.
    #[test]
    fn a_source_that_does_not_parse_is_a_finding() {
        let found = found("fn f( {\n\"er\"");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found.iter().all(|f| f.starts_with("f.rs line ") && f.contains("not read as Rust")), "{found:?}");
    }

    /// The repository as it stands writes no diagram kind out, and its table's constants hold the
    /// table's words.
    #[test]
    fn the_repository_writes_no_diagram_kind_out() {
        let (found, read) = scan(&util::root()).unwrap();
        assert!(read > 0, "sources were read");
        assert_eq!(found, Vec::<String>::new());
    }
}
