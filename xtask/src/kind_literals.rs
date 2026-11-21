//! The diagram-kind check (AR-770): a diagram kind is written out once, as the public constant of
//! the crate that defines it, and everywhere else through that constant, so renaming a kind can
//! never leave a copy of the old word behind.
//!
//! The sources are read by a Rust parser (`syn`), not by hand: a literal is what the language says
//! it is (a raw or C string, an escape, a raw identifier), and its value is compared as the program
//! sees it, cooked. [`KINDS`] names each kind's constant; its word is read from that constant among
//! its crate root's own items, so the table holds no copy of it. Every Rust source under `crates/`
//! and `apps/` is parsed, and a string literal (a plain, byte or C string, in the code or inside a
//! macro's tokens such as `json!`) whose whole value is a kind's word is a finding that names the
//! constant to use; the constants' own values are the exceptions. The comparison is exact and on
//! whole literals: no other word matches, whatever it is a kind of (an element's, a refusal's, a
//! label's), and a kind's word inside a longer text (an expected message, an automation id, a line
//! of SQL) is part of that text. Comments, doc comments included, are not code and never match. A
//! literal whose value is a kind's word but means something else (the ER part's name, a search for
//! `er`) carries [`MARK`] in a plain `//` comment, followed by what it is: on its own line, or on a
//! line of its own (nothing but the comment) right above it. A doc comment or a block comment never
//! marks.
//! A source that does not parse is a finding, so nothing is passed unread.
//!
//! Generated code (any `generated` folder) is left out: `cargo xtask generate` writes it from the
//! XMI and the `generated` step checks it; the word there is a metamodel's name.
//!
//! The table cannot fall behind the notations: outside tests (test files, and items under
//! `#[cfg(test)]` or `#[test]`), every `fn kinds` returning a slice of strings (how a notation
//! registers its diagram kinds) must have a body that is one list (`&[…]`) whose every element is a
//! constant of [`KINDS`] that resolves to its crate's root from the module the function is in; a
//! literal, a constant missing from the table, one that resolves elsewhere, and anything else (a
//! cast, a macro, a block, a body that is not one list) is a finding. A `fn kinds` returning strings
//! in any other type (a `Vec`, an array) is a finding too, unless [`GATHERS`] lists it as a view
//! that gathers the notations' kinds: the `Notation` trait fixes the slice, so a kind can only be
//! registered there.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use proc_macro2::{LineColumn, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Block, Expr, ImplItemFn, Item, ItemFn, ItemImpl, ItemMod, ItemTrait, Lit, Meta, ReturnType, Signature, Stmt, TraitItemFn, Type,
};

use crate::util;

/// One diagram kind's constant: the crate root that defines it, its name there, and the path the
/// other crates name it by.
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

/// The `fn kinds` that may return strings in another shape than a notation's slice: views that
/// gather the kinds the notations register, never registering one of their own. Each is the file
/// and the type it is a method of, or `""` for a free function.
pub const GATHERS: &[(&str, &str)] = &[("crates/diagram/src/project.rs", "ProjectView"), ("crates/notations/src/lib.rs", "")];

/// The comment that marks a literal spelled like a kind as no diagram kind; it must be followed by
/// what the literal is.
pub const MARK: &str = "not a diagram kind:";

/// The folders, from the workspace root, whose Rust sources are scanned.
const SCOPE: [&str; 2] = ["crates", "apps"];

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

/// The literals of a source: its string values, and where every literal of any kind starts and
/// ends, so a `//` inside one is no comment.
#[derive(Default)]
struct Literals {
    texts: Vec<Text>,
    spans: Vec<(Pos, Pos)>,
}

impl Literals {
    /// Takes one literal: its place, and its value when it is a string of any kind.
    fn take(&mut self, lit: &Lit) {
        let span = lit.span();
        let start = pos(span.start());
        self.spans.push((start, pos(span.end())));
        let bytes = match lit {
            Lit::Str(s) => Some(s.value().into_bytes()),
            Lit::ByteStr(b) => Some(b.value()),
            Lit::CStr(c) => Some(c.value().into_bytes()),
            _ => None,
        };
        if let Some(bytes) = bytes {
            self.texts.push(Text { bytes, start });
        }
    }

    /// Takes every literal of a macro's or an attribute's tokens, in its groups too.
    fn tokens(&mut self, stream: TokenStream) {
        for tree in stream {
            match tree {
                TokenTree::Group(g) => self.tokens(g.stream()),
                TokenTree::Literal(l) => self.take(&Lit::new(l)),
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for Literals {
    /// A literal of the code.
    fn visit_lit(&mut self, lit: &'ast Lit) {
        self.take(lit);
    }

    /// A macro's path, and the literals of its tokens, which the parser leaves unread.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        visit::visit_macro(self, mac);
        self.tokens(mac.tokens.clone());
    }

    /// An attribute's literals; a doc comment's text is a comment, never code.
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        match &attr.meta {
            // Its text is a comment's: its place is kept, so nothing in it reads as a comment
            // mark or a block comment's start, but its words never match.
            Meta::NameValue(nv) if attr.path().is_ident("doc") => {
                if let Expr::Lit(syn::ExprLit { lit, .. }) = &nv.value {
                    let span = lit.span();
                    self.spans.push((pos(span.start()), pos(span.end())));
                }
            }
            Meta::NameValue(nv) => self.visit_expr(&nv.value),
            Meta::List(list) => self.tokens(list.tokens.clone()),
            Meta::Path(_) => {}
        }
    }
}

/// The marks of `text`: the lines whose plain `//` comment holds [`MARK`] with some words after it,
/// and of those the lines that hold nothing else (no code before the comment). The text is read
/// once, with the literals the parser placed (`spans`) skipped as code: a `//` or `/*` inside one is
/// text. Block comments, nested ones too, never mark, and neither do doc comments: the parser
/// places them as doc text, so they are skipped with the literals.
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
            code.insert(line_of(i));
            i = (*end).max(i + 1);
        } else if (c, after) == ('/', Some('/')) {
            let end = chars.iter().skip(i).position(|c| *c == '\n').map_or(chars.len(), |n| i + n);
            let comment = chars.get(i..end).unwrap_or_default();
            let text: String = comment.iter().collect();
            let worded = text.find(MARK).is_some_and(|at| text.get(at + MARK.len()..).is_some_and(|rest| !rest.trim().is_empty()));
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
fn literals(file: &syn::File, text: &str, words: &[(String, &str)], exempt: &BTreeSet<Pos>) -> Vec<String> {
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

/// Whether an item's attributes make it test code alone: `#[test]`, or `#[cfg(test)]` exactly (a
/// condition that merely mentions `test`, such as `not(test)`, is no test).
fn is_test_item(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().is_ident("test") || (a.path().is_ident("cfg") && matches!(&a.meta, Meta::List(l) if l.tokens.to_string() == "test")))
}

/// The value of the string constant `name` among the crate root's own items, and where its literal
/// starts; or why it cannot be read. A constant of the same name in a module of the root is another
/// constant and does not count.
fn defined(items: &[Item], name: &str) -> Result<(String, Pos), String> {
    let consts: Vec<&syn::ItemConst> = items
        .iter()
        .filter_map(|i| match i {
            Item::Const(c) if c.ident.unraw() == name && !is_test_item(&c.attrs) => Some(c),
            _ => None,
        })
        .collect();
    let [only] = consts.as_slice() else {
        return Err(format!(
            "{} constants named {name} at the crate root, which KINDS (xtask/src/kind_literals.rs) names; one is needed",
            consts.len()
        ));
    };
    match strip_expr(&only.expr) {
        Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) => Ok((s.value(), pos(s.span().start()))),
        _ => Err(format!("{name}, which KINDS (xtask/src/kind_literals.rs) names, is no string literal")),
    }
}

/// An expression with its parentheses and invisible groups taken off.
fn strip_expr(e: &Expr) -> &Expr {
    match e {
        Expr::Paren(p) => strip_expr(&p.expr),
        Expr::Group(g) => strip_expr(&g.expr),
        other => other,
    }
}

/// A type with its parentheses and invisible groups taken off.
fn strip_type(t: &Type) -> &Type {
    match t {
        Type::Paren(p) => strip_type(&p.elem),
        Type::Group(g) => strip_type(&g.elem),
        other => other,
    }
}

/// Whether a type names `str` anywhere in it (`Vec<&str>`, `[&str; 2]`, `&[&str]`): a type that can
/// carry kinds.
fn holds_str(ty: &Type) -> bool {
    /// Finds a path segment `str`.
    struct Finds(bool);
    impl<'ast> Visit<'ast> for Finds {
        /// A segment of a path in the type.
        fn visit_path_segment(&mut self, s: &'ast syn::PathSegment) {
            self.0 |= s.ident == "str";
            visit::visit_path_segment(self, s);
        }
    }
    let mut finds = Finds(false);
    finds.visit_type(ty);
    finds.0
}

/// Whether a function returns a slice of strings (`&[&str]`, whatever its lifetimes and
/// parentheses): the shape a notation registers its kinds in.
fn returns_strs(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = output else { return false };
    let Type::Reference(outer) = strip_type(ty) else { return false };
    let Type::Slice(slice) = strip_type(&outer.elem) else { return false };
    let Type::Reference(inner) = strip_type(&slice.elem) else { return false };
    matches!(strip_type(&inner.elem), Type::Path(p) if p.qself.is_none() && p.path.is_ident("str"))
}

/// What a notation registers as one of its diagram kinds: a constant by its path, a literal, or an
/// element this check cannot read (a cast, a macro, a block, a body that is not one list), written
/// out as it reads.
#[derive(Debug, Clone, PartialEq)]
enum Registered {
    Constant(Vec<String>),
    Literal(String),
    Unreadable(String),
}

/// `node`'s source text on one line, or `fallback` when the parser has none.
fn shown(node: &impl Spanned, fallback: &str) -> String {
    node.span().source_text().map_or_else(|| fallback.to_string(), |t| t.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// What one element of a kinds list registers.
fn element(e: &Expr) -> Registered {
    match strip_expr(e) {
        Expr::Path(p) if p.qself.is_none() && p.path.segments.iter().all(|s| s.arguments.is_none()) => {
            let mut path: Vec<String> = p.path.segments.iter().map(|s| s.ident.unraw().to_string()).collect();
            // A leading `::` names an outside crate, which is never this one's root.
            if p.path.leading_colon.is_some() {
                path.insert(0, String::new());
            }
            Registered::Constant(path)
        }
        Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) => Registered::Literal(s.value()),
        other => Registered::Unreadable(shown(other, "an expression")),
    }
}

/// What a `fn kinds` body registers: each element of the one list it is (`&[A, crate::B, …]`), or
/// the body unreadable when it is anything else.
fn listed(body: &Block) -> Vec<Registered> {
    let unreadable = || vec![Registered::Unreadable(format!("a body that is not one list: {}", shown(body, "a block")))];
    let [Stmt::Expr(tail, None)] = body.stmts.as_slice() else { return unreadable() };
    let Expr::Reference(reference) = strip_expr(tail) else { return unreadable() };
    let Expr::Array(array) = strip_expr(&reference.expr) else { return unreadable() };
    if reference.mutability.is_some() {
        return unreadable();
    }
    array.elems.iter().map(element).collect()
}

/// One registration: the line of its `fn kinds`, the module it is in (from the crate root), and
/// what it registers.
struct Registration {
    line: usize,
    module: Vec<String>,
    shape: Shape,
}

/// How a `fn kinds` returns its strings: as a notation's slice, with what its list registers; or in
/// another type (shown), with the type it is a method of (`""` for a free function).
enum Shape {
    Slice(Vec<Registered>),
    Other { returns: String, owner: String },
}

/// The registrations of a parsed source outside test items, read as the parser gives them, with
/// the module each is in.
struct Registrations {
    module: Vec<String>,
    owner: String,
    found: Vec<Registration>,
}

impl Registrations {
    /// Records a function named `kinds` (a raw `r#kinds` too) returning strings: a slice of them,
    /// with what its list registers, or any other type that names `str`.
    fn consider(&mut self, sig: &Signature, body: &Block) {
        let ReturnType::Type(_, ty) = &sig.output else { return };
        if sig.ident.unraw() != "kinds" || !holds_str(ty) {
            return;
        }
        let shape = if returns_strs(&sig.output) {
            Shape::Slice(listed(body))
        } else {
            Shape::Other { returns: shown(ty.as_ref(), "a type"), owner: self.owner.clone() }
        };
        self.found.push(Registration { line: sig.ident.span().start().line, module: self.module.clone(), shape });
    }

    /// Visits an impl's or a trait's items with `owner` as the type their methods belong to.
    fn owned(&mut self, owner: String, visit: impl FnOnce(&mut Self)) {
        let before = std::mem::replace(&mut self.owner, owner);
        visit(self);
        self.owner = before;
    }
}

impl<'ast> Visit<'ast> for Registrations {
    /// An inline module: its items are in it; a test module is skipped.
    fn visit_item_mod(&mut self, m: &'ast ItemMod) {
        if is_test_item(&m.attrs) {
            return;
        }
        self.module.push(m.ident.unraw().to_string());
        visit::visit_item_mod(self, m);
        self.module.pop();
    }

    /// A free function, unless it is a test.
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        if !is_test_item(&f.attrs) {
            self.consider(&f.sig, &f.block);
            visit::visit_item_fn(self, f);
        }
    }

    /// An impl block, unless it is test code.
    fn visit_item_impl(&mut self, i: &'ast ItemImpl) {
        if !is_test_item(&i.attrs) {
            let owner = match strip_type(&i.self_ty) {
                Type::Path(p) => p.path.segments.last().map(|s| s.ident.unraw().to_string()).unwrap_or_default(),
                other => shown(other, "a type"),
            };
            self.owned(owner, |me| visit::visit_item_impl(me, i));
        }
    }

    /// A method, unless it is test code.
    fn visit_impl_item_fn(&mut self, f: &'ast ImplItemFn) {
        if !is_test_item(&f.attrs) {
            self.consider(&f.sig, &f.block);
            visit::visit_impl_item_fn(self, f);
        }
    }

    /// A trait, unless it is test code.
    fn visit_item_trait(&mut self, t: &'ast ItemTrait) {
        if !is_test_item(&t.attrs) {
            self.owned(t.ident.unraw().to_string(), |me| visit::visit_item_trait(me, t));
        }
    }

    /// A trait's method with a default body, unless it is test code.
    fn visit_trait_item_fn(&mut self, f: &'ast TraitItemFn) {
        if !is_test_item(&f.attrs) {
            if let Some(body) = &f.default {
                self.consider(&f.sig, body);
            }
            visit::visit_trait_item_fn(self, f);
        }
    }
}

/// Whether `through` (a constant path's segments before its name), written in `module`, leads to
/// the root of the crate whose own name is `own`: through `crate` or `own` from anywhere, through
/// `self` or nothing only from the root, through `super` from as deep as it climbs.
fn reaches_root(through: &[String], module: &[String], own: &str) -> bool {
    let mut at = module.to_vec();
    let mut rest = through;
    match rest.split_first() {
        Some((first, more)) if first == "crate" || first == own => {
            at.clear();
            rest = more;
        }
        Some((first, more)) if first == "self" => rest = more,
        _ => {}
    }
    while let Some((first, more)) = rest.split_first()
        && first == "super"
    {
        if at.pop().is_none() {
            return false;
        }
        rest = more;
    }
    rest.is_empty() && at.is_empty()
}

/// The findings for what a source of the crate `krate`, in `module` (outside its tests),
/// registers. A constant passes when [`KINDS`] lists its name for that crate and its path resolves
/// to the crate's root from where it is written; a literal, a constant missing from the table or
/// resolving elsewhere, an element or body this check cannot read, and a `kinds` returning strings
/// in another shape than the slice, unless [`GATHERS`] lists it, are findings.
fn unlisted(rel: &str, krate: &str, module: &[String], file: &syn::File) -> Vec<String> {
    let home = format!("crates/{krate}/");
    let own = format!("archforge_{}", krate.replace('-', "_"));
    let mut found = Registrations { module: module.to_vec(), owner: String::new(), found: Vec::new() };
    found.visit_file(file);
    let mut out = Vec::new();
    for Registration { line, module, shape } in found.found {
        let registered = match shape {
            Shape::Slice(registered) => registered,
            Shape::Other { owner, .. } if GATHERS.contains(&(rel, owner.as_str())) => continue,
            Shape::Other { returns, .. } => {
                out.push(format!(
                    "{rel} line {line}: kinds returns {returns}, which this check cannot read; a notation registers its kinds as &'static [&'static str] (Notation::kinds), and a view that gathers them is listed in GATHERS (xtask/src/kind_literals.rs)"
                ));
                continue;
            }
        };
        for r in registered {
            match r {
                Registered::Literal(v) => out.push(format!(
                    "{rel} line {line}: the diagram kind \"{v}\" is registered as a literal; give it a public constant at this crate's root and list it in KINDS (xtask/src/kind_literals.rs)"
                )),
                Registered::Unreadable(what) => out.push(format!(
                    "{rel} line {line}: a diagram kind registered as {what}, which this check cannot read; list the kind's constant by name"
                )),
                Registered::Constant(path) => {
                    let Some((name, through)) = path.split_last() else { continue };
                    if !reaches_root(through, &module, &own) {
                        out.push(format!(
                            "{rel} line {line}: the diagram kind {} is not the crate root's constant from here; name it crate::{name}, as KINDS (xtask/src/kind_literals.rs) lists it",
                            path.join("::")
                        ));
                    } else if !KINDS.iter().any(|k| k.file.starts_with(&home) && k.name == name) {
                        out.push(format!("{rel} line {line}: the diagram kind {name} is missing from KINDS (xtask/src/kind_literals.rs), so its word is not checked"));
                    }
                }
            }
        }
    }
    out
}

/// Whether `rel` is a test source: under a `tests` folder or named `tests.rs`.
fn is_test(rel: &Path) -> bool {
    rel.components().any(|c| c == Component::Normal("tests".as_ref())) || rel.file_name().is_some_and(|n| n == "tests.rs")
}

/// The crate a source under `crates/<name>/src/` belongs to, and the module the file is, from its
/// path (`lib.rs` the root, `a/mod.rs` and `a.rs` the module `a`).
fn crate_of(rel: &Path) -> Option<(String, Vec<String>)> {
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let [first, name, src, rest @ ..] = parts.as_slice() else { return None };
    if first != "crates" || src != "src" {
        return None;
    }
    let mut module: Vec<String> = rest.iter().map(|p| p.strip_suffix(".rs").unwrap_or(p).to_string()).collect();
    if module.last().is_some_and(|m| m == "mod") || module.as_slice() == ["lib"] || module.as_slice() == ["main"] {
        module.pop();
    }
    Some((name.clone(), module))
}

/// The findings of one source, each named with `rel`: its kind literals, and, for a source of a
/// crate outside its tests (`registers`: the crate and the file's module), its registrations.
fn check(rel: &str, text: &str, words: &[(String, &str)], exempt: &BTreeSet<Pos>, registers: Option<(&str, &[String])>) -> Vec<String> {
    let file = match parse(text) {
        Ok(file) => file,
        Err(e) => return vec![format!("{rel} {e}")],
    };
    let mut out: Vec<String> = literals(&file, text, words, exempt).into_iter().map(|f| format!("{rel} {f}")).collect();
    if let Some((krate, module)) = registers {
        out.extend(unlisted(rel, krate, module, &file));
    }
    out
}

/// Every finding of the check over the workspace at `root`, each named with its file, and how many
/// sources were read.
fn scan(root: &Path) -> Result<(Vec<String>, usize), String> {
    let mut out = Vec::new();
    // Each kind's word and the constant to use, and where the constants' own values are.
    let mut words = Vec::new();
    let mut exempt: BTreeSet<(String, Pos)> = BTreeSet::new();
    for k in KINDS {
        let text = std::fs::read_to_string(root.join(k.file)).map_err(|e| format!("{}: {e}", k.file))?;
        match parse(&text).and_then(|file| defined(&file.items, k.name)) {
            Ok((word, at)) => {
                words.push((word, k.path));
                exempt.insert((k.file.to_string(), at));
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
            let shown = rel.display().to_string();
            let own: BTreeSet<Pos> = exempt.iter().filter(|(file, _)| Path::new(file) == rel).map(|(_, at)| *at).collect();
            let registers = crate_of(&rel).filter(|_| !is_test(&rel));
            out.extend(check(&shown, &text, &words, &own, registers.as_ref().map(|(k, m)| (k.as_str(), m.as_slice()))));
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

    /// The literal findings for `source`, with no exempt literal and no registration check.
    fn found(source: &str) -> Vec<String> {
        check("f.rs", source, &words(), &BTreeSet::new(), None)
    }

    /// The finding for `er` written out on `line` of `f.rs`.
    fn er_at(line: usize) -> String {
        format!("f.rs line {line}: the diagram kind \"er\" is written out; use archforge_er::DIAGRAM_KIND")
    }

    /// The registration findings of `source` as ER's crate root, or as ER's module `module`.
    fn registers(source: &str, module: &[&str]) -> Vec<String> {
        let module: Vec<String> = module.iter().map(|m| m.to_string()).collect();
        let file = parse(source).unwrap();
        unlisted("crates/er/src/lib.rs", "er", &module, &file)
    }

    /// `fn kinds` registering `list` in an impl, as a notation does.
    fn kinds(list: &str) -> String {
        format!("impl Notation for N {{ fn kinds(&self) -> &'static [&'static str] {{ {list} }} }}")
    }

    /// The finding for a constant missing from KINDS.
    const MISSING: &str =
        "crates/er/src/lib.rs line 1: the diagram kind NEW_KIND is missing from KINDS (xtask/src/kind_literals.rs), so its word is not checked";

    /// The finding for a `kinds` on line 1 of ER's root returning `returns`.
    fn other(returns: &str) -> String {
        format!(
            "crates/er/src/lib.rs line 1: kinds returns {returns}, which this check cannot read; a notation registers its kinds as &'static [&'static str] (Notation::kinds), and a view that gathers them is listed in GATHERS (xtask/src/kind_literals.rs)"
        )
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
    /// is that line's (pr-agent on c01213a6). Doc comments and block comments never mark, and a
    /// doc attribute's text opens no comment.
    #[test]
    fn only_a_plain_line_comment_marks_and_only_one_alone_marks_the_line_under_it() {
        assert_eq!(found("fn f() {\n    let label = \"x\"; // not a diagram kind: a label\n    json!({ \"kind\": \"er\" });\n}"), [er_at(3)]);
        assert_eq!(found("fn f() {\n    // not a diagram kind: a label\n    json!({ \"kind\": \"er\" });\n}"), Vec::<String>::new());
        // A literal before the comment is code, even with nothing else on its line.
        assert_eq!(found("const F: [&str; 2] = [\n    \"x\" // not a diagram kind: a label\n    , \"er\"];"), [er_at(3)]);
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

    /// The constants' own values are read from their crate root's items, cooked, and every one in
    /// the file is exempt; a constant of the same name in a module is another one (CodeRabbit on
    /// #572: the root's value, and every definition in a file).
    #[test]
    fn the_root_constants_are_read_and_all_of_them_exempt() {
        let source = "mod sample {\n    pub const DIAGRAM_KIND: &str = \"sample\";\n}\n/// The kind.\npub const DIAGRAM_KIND: &str = \"\\x65r\";\npub const OTHER_KIND: &str = (\"uml.structure\");\nconst COPY: &str = \"er\";";
        let file = parse(source).unwrap();
        let (er, at_er) = defined(&file.items, "DIAGRAM_KIND").unwrap();
        let (other, at_other) = defined(&file.items, "OTHER_KIND").unwrap();
        assert_eq!((er.as_str(), at_er, other.as_str(), at_other), ("er", (5, 31), "uml.structure", (6, 30)));
        let exempt = BTreeSet::from([at_er, at_other]);
        assert_eq!(check("f.rs", source, &words(), &exempt, None), [er_at(7)]);
        assert!(defined(&file.items, "MISSING").unwrap_err().starts_with("0 constants named MISSING"));
        let concat = parse("pub const DIAGRAM_KIND: &str = concat!(\"e\", \"r\");").unwrap();
        assert!(defined(&concat.items, "DIAGRAM_KIND").unwrap_err().contains("is no string literal"));
        let raw = parse("pub const r#DIAGRAM_KIND: &str = \"er\";").unwrap();
        assert_eq!(defined(&raw.items, "DIAGRAM_KIND").map(|d| d.0), Ok("er".to_string()));
        let tested = parse("#[cfg(test)]\npub const DIAGRAM_KIND: &str = \"fixture\";\npub const DIAGRAM_KIND: &str = \"er\";").unwrap();
        assert_eq!(defined(&tested.items, "DIAGRAM_KIND").map(|d| d.0), Ok("er".to_string()));
    }

    /// A source that does not parse is a finding, never passed unread.
    #[test]
    fn a_source_that_does_not_parse_is_a_finding() {
        let found = found("fn f( {\n\"er\"");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found.iter().all(|f| f.starts_with("f.rs line ") && f.contains("not read as Rust")), "{found:?}");
    }

    /// A notation's kinds list names constants; a literal there, or a constant KINDS lacks, is a
    /// finding, and a kinds that is declared only, or returns no slice of strings, registers nothing.
    #[test]
    fn a_registered_kind_must_be_a_listed_constant() {
        assert_eq!(registers(&kinds("&[DIAGRAM_KIND]"), &[]), Vec::<String>::new());
        assert_eq!(
            registers(&kinds("&[crate::DIAGRAM_KIND, SKETCH, \"bpmn\"]"), &[]),
            [
                "crates/er/src/lib.rs line 1: the diagram kind SKETCH is missing from KINDS (xtask/src/kind_literals.rs), so its word is not checked",
                "crates/er/src/lib.rs line 1: the diagram kind \"bpmn\" is registered as a literal; give it a public constant at this crate's root and list it in KINDS (xtask/src/kind_literals.rs)",
            ]
        );
        // A trait's default body registers as an impl's does.
        assert_eq!(registers("trait Notation { fn kinds(&self) -> &'static [&'static str] { &[NEW_KIND] } }", &[]), [MISSING]);
        // The constant must be listed for the crate that registers it.
        let file = parse(&kinds("&[DIAGRAM_KIND]")).unwrap();
        assert_eq!(unlisted("crates/uml/src/lib.rs", "uml", &[], &file).len(), 1);
        for none in [
            "trait Notation { fn kinds(&self) -> &'static [&'static str]; }",
            "trait Notation { fn kinds(&self) -> &'static [&'static str] { &[] } }",
            "impl V { fn kind_names(&self) -> &'static [&'static str] { &[\"er\"] } }",
            "fn kinds<F: Fn() -> &'static [&'static str]>(f: F) { let _ = f; }",
        ] {
            assert_eq!(registers(none, &[]), Vec::<String>::new(), "{none}");
        }
    }

    /// A `kinds` returning strings in any shape but the slice is a finding, so no notation registers
    /// a kind past the check (pr-agent on c01213a6), unless GATHERS lists it as a view gathering the
    /// notations' kinds; a `kinds` returning no strings is no registration.
    #[test]
    fn a_kinds_returning_strings_in_another_shape_is_a_finding_unless_it_gathers() {
        assert_eq!(registers("impl V { fn kinds(&self) -> Vec<&'static str> { vec![NEW_KIND] } }", &[]), [other("Vec<&'static str>")]);
        assert_eq!(registers("impl V { fn kinds(&self) -> [&'static str; 1] { [\"bpmn\"] } }", &[]), [other("[&'static str; 1]")]);
        assert_eq!(registers("fn kinds() -> Box<[&'static str]> { Box::new([]) }", &[]), [other("Box<[&'static str]>")]);
        assert_eq!(registers("trait T { fn kinds(&self) -> Vec<&'static str> { Vec::new() } }", &[]), [other("Vec<&'static str>")]);
        let gather = "impl DiagramView for ProjectView {\n    fn kinds(&self) -> Vec<&'static str> {\n        self.notations.iter().flat_map(|n| n.kinds().iter().copied()).collect()\n    }\n}";
        let free = "pub fn kinds() -> Vec<&'static str> {\n    view().kinds()\n}";
        let at = |rel: &str, krate: &str, source: &str| unlisted(rel, krate, &[], &parse(source).unwrap());
        assert_eq!(at("crates/diagram/src/project.rs", "diagram", gather), Vec::<String>::new());
        assert_eq!(at("crates/notations/src/lib.rs", "notations", free), Vec::<String>::new());
        // Listed for one file and one owner only.
        assert_eq!(at("crates/er/src/lib.rs", "er", gather).len(), 1);
        assert_eq!(at("crates/notations/src/lib.rs", "notations", gather).len(), 1);
        assert_eq!(at("crates/diagram/src/project.rs", "diagram", free).len(), 1);
        for none in [
            "pub fn kinds(metaclass: &str, n: u64) -> String { String::new() }",
            "trait DiagramView { fn kinds(&self) -> Vec<&'static str>; }",
            "#[cfg(test)]\nimpl V { fn kinds(&self) -> Vec<&'static str> { vec![\"x\"] } }",
        ] {
            assert_eq!(registers(none, &[]), Vec::<String>::new(), "{none}");
        }
    }

    /// Every element of a kinds list is read, in parentheses too, and what the check cannot read is
    /// a finding, never skipped.
    #[test]
    fn every_element_of_a_kinds_list_is_read_and_what_cannot_be_is_a_finding() {
        for passes in
            ["&[(DIAGRAM_KIND)]", "&[((crate::DIAGRAM_KIND))]", "&[self::DIAGRAM_KIND, archforge_er::DIAGRAM_KIND,]", "(&[\n    DIAGRAM_KIND,\n])"]
        {
            assert_eq!(registers(&kinds(passes), &[]), Vec::<String>::new(), "{passes}");
        }
        assert_eq!(registers(&kinds("&[(NEW_KIND)]"), &[]), [MISSING]);
        assert_eq!(registers(&kinds("&[DIAGRAM_KIND, (crate::NEW_KIND)]"), &[]), [MISSING]);
        let unread = |what: &str| {
            format!(
                "crates/er/src/lib.rs line 1: a diagram kind registered as {what}, which this check cannot read; list the kind's constant by name"
            )
        };
        assert_eq!(registers(&kinds("&[DIAGRAM_KIND as &str]"), &[]), [unread("DIAGRAM_KIND as &str")]);
        assert_eq!(registers(&kinds("&[concat!(\"e\", \"r\"), DIAGRAM_KIND]"), &[]), [unread("concat!(\"e\", \"r\")")]);
        assert_eq!(registers(&kinds("&[{ NEW_KIND }]"), &[]), [unread("{ NEW_KIND }")]);
        assert_eq!(registers(&kinds("&[[NEW_KIND][0]]"), &[]), [unread("[NEW_KIND][0]")]);
        assert_eq!(registers(&kinds("KINDS"), &[]), [unread("a body that is not one list: { KINDS }")]);
        assert_eq!(registers(&kinds("&KINDS"), &[]), [unread("a body that is not one list: { &KINDS }")]);
        assert_eq!(registers(&kinds("&mut [DIAGRAM_KIND]"), &[]), [unread("a body that is not one list: { &mut [DIAGRAM_KIND] }")]);
    }

    /// The list must be the whole body: a list before another expression is not what is returned
    /// (CodeRabbit on #572).
    #[test]
    fn the_list_must_be_the_whole_body() {
        let what = "a body that is not one list: { &[DIAGRAM_KIND]; &[NEW_KIND] }";
        assert_eq!(
            registers(&kinds("&[DIAGRAM_KIND]; &[NEW_KIND]"), &[]),
            [format!(
                "crates/er/src/lib.rs line 1: a diagram kind registered as {what}, which this check cannot read; list the kind's constant by name"
            )]
        );
        // A statement before the list is read as a body that is not one list too.
        assert_eq!(registers(&kinds("let _ = 1; &[DIAGRAM_KIND]"), &[]).len(), 1);
    }

    /// A raw `r#kinds` is `kinds`, and a raw constant name is its name (CodeRabbit on #572).
    #[test]
    fn a_raw_identifier_is_the_name_it_spells() {
        let raw = "impl Notation for N { fn r#kinds(&self) -> &'static [&'static str] { &[NEW_KIND] } }";
        assert_eq!(registers(raw, &[]), [MISSING]);
        assert_eq!(registers(&kinds("&[r#DIAGRAM_KIND]"), &[]), Vec::<String>::new());
    }

    /// A slice of strings in parentheses is still one (CodeRabbit on #572), and a long signature is
    /// read to its end.
    #[test]
    fn a_parenthesized_or_long_signature_is_read_as_the_type_it_is() {
        let paren = "impl Notation for N { fn kinds(&self) -> (&'static [(&'static str)]) { &[NEW_KIND] } }";
        assert_eq!(registers(paren, &[]), [MISSING]);
        let bounds = (0..12).map(|n| format!("T{n}: Clone + Send")).collect::<Vec<_>>().join(", ");
        let long = format!("fn kinds<{bounds}>(p: T0) -> &'static [&'static str] where T1: Fn() -> u8 {{ &[NEW_KIND] }}");
        assert_eq!(registers(&long, &[]), [MISSING]);
        let long_vec = format!("fn kinds<{bounds}>(p: T0) -> Vec<&'static str> where T1: Fn() -> &'static [&'static str] {{ vec![] }}");
        assert_eq!(registers(&long_vec, &[]), [other("Vec<&'static str>")]);
    }

    /// A constant resolves from the module its kinds is in: `self::` and a bare name only at the
    /// root, `super::` as far as it climbs, `crate::` and the crate's own name from anywhere
    /// (CodeRabbit on #572: `self::` in a submodule is that module's).
    #[test]
    fn a_constant_resolves_from_the_module_it_is_named_in() {
        let inline = format!(
            "pub mod sub {{\n    pub const DIAGRAM_KIND: &str = \"bpmn\";\n    {}\n}}",
            kinds("&[self::DIAGRAM_KIND, DIAGRAM_KIND, crate::DIAGRAM_KIND, super::DIAGRAM_KIND]")
        );
        let elsewhere = |path: &str| {
            let name = path.rsplit("::").next().unwrap_or(path);
            format!(
                "crates/er/src/lib.rs line 3: the diagram kind {path} is not the crate root's constant from here; name it crate::{name}, as KINDS (xtask/src/kind_literals.rs) lists it"
            )
        };
        assert_eq!(registers(&inline, &[]), [elsewhere("self::DIAGRAM_KIND"), elsewhere("DIAGRAM_KIND")]);
        // A file module (`src/sub.rs`) is the same, and two levels down `super::super::` climbs to the root.
        let file = kinds("&[self::DIAGRAM_KIND, super::DIAGRAM_KIND, archforge_er::DIAGRAM_KIND]");
        let line1 = |path: &str| elsewhere(path).replace("line 3", "line 1");
        assert_eq!(registers(&file, &["sub"]), [line1("self::DIAGRAM_KIND")]);
        let deep = kinds(
            "&[super::DIAGRAM_KIND, super::super::DIAGRAM_KIND, super::super::super::DIAGRAM_KIND, generated::DIAGRAM_KIND, ::archforge_er::DIAGRAM_KIND]",
        );
        assert_eq!(
            registers(&deep, &["a", "b"]),
            [
                line1("super::DIAGRAM_KIND"),
                line1("super::super::super::DIAGRAM_KIND"),
                line1("generated::DIAGRAM_KIND"),
                line1("::archforge_er::DIAGRAM_KIND")
            ]
        );
    }

    /// Test items, by their attributes, register nothing, yet their literals are still read
    /// (CodeRabbit on #572: inline test modules).
    #[test]
    fn test_items_register_nothing_but_their_literals_are_read() {
        let source = "#[cfg(test)]\nmod tests {\n    impl Notation for T { fn kinds(&self) -> &'static [&'static str] { &[\"fixture\", \"er\"] } }\n}\n#[test]\nfn t() { fn kinds() -> &'static [&'static str] { &[\"fixture\"] } }\n#[cfg(not(test))]\nfn kinds() -> &'static [&'static str] { &[\"live\"] }\n#[cfg(test)]\nimpl T { fn kinds(&self) -> &'static [&'static str] { &[\"fixture\"] } }\n#[cfg(test)]\ntrait U { fn kinds(&self) -> &'static [&'static str] { &[\"fixture\"] } }\nimpl V {\n    #[test]\n    fn kinds(&self) -> &'static [&'static str] { &[\"fixture\"] }\n}\ntrait W {\n    #[cfg(test)]\n    fn kinds(&self) -> &'static [&'static str] { &[\"fixture\"] }\n}";
        let file = parse(source).unwrap();
        assert_eq!(
            unlisted("crates/er/src/lib.rs", "er", &[], &file),
            [
                "crates/er/src/lib.rs line 8: the diagram kind \"live\" is registered as a literal; give it a public constant at this crate's root and list it in KINDS (xtask/src/kind_literals.rs)"
            ]
        );
        assert_eq!(found(source), [er_at(3)]);
    }

    /// Tests and generated code are told apart by their paths, and a source's crate and module by
    /// its folder.
    #[test]
    fn tests_crates_and_modules_are_told_by_their_paths() {
        assert!(is_test(Path::new("crates/er/tests/diagram.rs")) && is_test(Path::new("crates/ui-egui/src/tests.rs")));
        assert!(is_test(Path::new("crates/ui-egui/src/tests/branches.rs")) && !is_test(Path::new("crates/er/src/lib.rs")));
        let module = |p: &str| crate_of(Path::new(p)).map(|(c, m)| (c, m.join("::")));
        assert_eq!(module("crates/er/src/lib.rs"), Some(("er".into(), String::new())));
        assert_eq!(module("crates/er/src/rules.rs"), Some(("er".into(), "rules".into())));
        assert_eq!(module("crates/ui-egui/src/a/mod.rs"), Some(("ui-egui".into(), "a".into())));
        assert_eq!(module("crates/ui-egui/src/a/b.rs"), Some(("ui-egui".into(), "a::b".into())));
        assert_eq!(module("crates/er/tests/er.rs"), None);
        assert_eq!(module("apps/archforge/src/main.rs"), None);
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
