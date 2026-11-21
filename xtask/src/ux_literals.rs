//! The ux part of the layer check: the interface draws through `ux` (AGENTS.md), so a colour or a
//! font is made there alone. Clippy refuses egui's colour and font constructors everywhere else
//! (`disallowed-methods` and `disallowed-types` in `clippy.toml`); this scan refuses what clippy
//! cannot see: a colour constant (`Color32::RED`, `<Color32>::RED`), a font or a colour built as
//! a struct literal (`FontId { size, family }`, `HsvaGamma { .. }`), and a watched type renamed
//! (`use egui::FontId as Face`) or aliased (`type Ink = egui::Color32`), which would hide its
//! constants and literals from the scan. (A struct's constants cannot be imported, so `use
//! egui::Color32::RED` is the compiler's to refuse.)
//!
//! Every Rust source under `crates/` and `apps/`, tests included, is parsed as Rust (`syn`), so a
//! string, a comment, a raw identifier or a pattern is what the compiler takes it for: a struct
//! pattern (`let FontId { size, family } = …`) takes a font apart and is no literal, and a return
//! type (`-> &egui::FontId {`) is a type. A macro's tokens are read as expressions or statements
//! when they parse as such, and otherwise token by token; a `macro_rules!` matcher is never read,
//! its transcribers are, and `matches!` takes a pattern only when it is the standard library's. A
//! finding's line is its line in the file. `ux` itself ([`UX`]) is exempt.

use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::parse::{ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, Ident, Token, Type};

/// The `ux` module, from the repository's root: where colours and fonts are made.
pub const UX: [&str; 4] = ["crates", "ui-egui", "src", "ux"];

/// The folders, from the root, whose sources are scanned.
const SCANNED: [&str; 2] = ["crates", "apps"];

/// Types whose struct literal is a font or a colour made by hand.
const LITERALS: [&str; 4] = ["FontId", "Hsva", "HsvaGamma", "Rgba"];

/// The types the scan follows: renamed (`use egui::FontId as Face;`) or aliased (`type Ink =
/// egui::Color32;`), their constants and literals would be out of its sight.
const WATCHED: [&str; 5] = ["Color32", "FontId", "Hsva", "HsvaGamma", "Rgba"];

/// What to use instead of a colour constant.
const USE_COLOUR: &str = "the interface draws its colours through ux: take a role of ux::Palette or a named ux colour (ux::CLEAR, ux::UNTINTED, ux::PLACEHOLDER_INK, ux::MARK)";

/// What to use instead of a font built by hand.
const USE_FONT: &str = "the interface draws its fonts through ux: take a style of ux::types, its .font() or its .code()";

/// What to use instead of a colour built by hand.
const USE_DATA: &str =
    "the interface draws its colours through ux: take a role of ux::Palette, ux::data_colour for a colour the document holds, or ux::from_hsv";

/// Whether `name` is written as a constant: capitals, digits and underscores, a capital first.
fn constant_case(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// What to use instead of the watched type `name`.
fn instead(name: &str) -> &'static str {
    match name {
        "Color32" => USE_COLOUR,
        "FontId" => USE_FONT,
        _ => USE_DATA,
    }
}

/// `ident` as written, a raw identifier (`r#Color32`) as its name.
fn name(ident: &Ident) -> String {
    ident.unraw().to_string()
}

/// The line `span` starts on, from 1.
fn line(span: proc_macro2::Span) -> usize {
    span.start().line
}

/// The watched type `ty` is, as a plain path (`egui::Color32`, `(FontId)`); none for a type built
/// on one (`Option<Color32>`, `&Color32`), which has no constants or literal of its own.
fn watched_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Paren(t) => watched_type(&t.elem),
        Type::Group(t) => watched_type(&t.elem),
        Type::Path(t) => {
            let n = name(&t.path.segments.last()?.ident);
            WATCHED.contains(&n.as_str()).then_some(n)
        }
        _ => None,
    }
}

/// The walk over one file's syntax tree, gathering findings with their lines.
#[derive(Default)]
struct Scan {
    found: Vec<(usize, String)>,
    /// The names the file defines as macros or imports itself, so that an unqualified call of one
    /// of [`PATTERN_MACROS`] among them is not the standard library's.
    shadowed: Vec<String>,
}

/// The names a file gives a meaning of its own: its `macro_rules!` and its imports (`use
/// helpers::matches;`, `use helpers::pick as matches;`).
#[derive(Default)]
struct Shadows(Vec<String>);

impl<'ast> Visit<'ast> for Shadows {
    /// `macro_rules! matches { … }`.
    fn visit_item_macro(&mut self, m: &'ast syn::ItemMacro) {
        if m.mac.path.is_ident("macro_rules")
            && let Some(ident) = &m.ident
        {
            self.note(ident);
        }
        visit::visit_item_macro(self, m);
    }

    /// `use helpers::matches;`.
    fn visit_use_name(&mut self, u: &'ast syn::UseName) {
        self.note(&u.ident);
    }

    /// `use helpers::pick as matches;`.
    fn visit_use_rename(&mut self, u: &'ast syn::UseRename) {
        self.note(&u.rename);
    }
}

impl Shadows {
    /// Notes the name `ident` gives a meaning to.
    fn note(&mut self, ident: &Ident) {
        self.0.push(name(ident));
    }
}

/// Whether the macro called through `path` is the standard library's pattern macro: one of
/// [`PATTERN_MACROS`] named through `std` or `core` (`std::matches!`, `::core::matches!`), or
/// unqualified when the file gives that name no meaning of its own (`shadowed`).
fn std_pattern_macro(path: &syn::Path, shadowed: &[String]) -> bool {
    let Some(last) = path.segments.last().map(|s| name(&s.ident)) else { return false };
    if !PATTERN_MACROS.contains(&last.as_str()) {
        return false;
    }
    match path.segments.first().map(|s| name(&s.ident)) {
        _ if path.segments.len() == 1 && path.leading_colon.is_none() => !shadowed.contains(&last),
        Some(root) => path.segments.len() > 1 && (root == "std" || root == "core"),
        None => false,
    }
}

/// `tokens` of a `macro_rules!` transcriber made readable as Rust: a metavariable (`$x`) becomes
/// a plain name, and a repetition (`$( … ) sep *`) its body once.
fn transcribed(tokens: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let is_punct = |k: usize, cs: &str| matches!(trees.get(k), Some(TokenTree::Punct(p)) if cs.contains(p.as_char()));
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(tree) = trees.get(i) {
        match (tree, trees.get(i + 1)) {
            (TokenTree::Punct(p), Some(TokenTree::Ident(id))) if p.as_char() == '$' => {
                out.push(TokenTree::Ident(Ident::new("metavariable", id.span())));
                i += 2;
            }
            (TokenTree::Punct(p), Some(TokenTree::Group(g))) if p.as_char() == '$' && g.delimiter() == Delimiter::Parenthesis => {
                out.extend(transcribed(g.stream()));
                // The repetition's operator, after an optional separator.
                i += if is_punct(i + 2, "*+?") {
                    3
                } else if is_punct(i + 3, "*+?") {
                    4
                } else {
                    2
                };
            }
            (TokenTree::Group(g), _) => {
                let mut group = proc_macro2::Group::new(g.delimiter(), transcribed(g.stream()));
                group.set_span(g.span());
                out.push(TokenTree::Group(group));
                i += 1;
            }
            _ => {
                out.push(tree.clone());
                i += 1;
            }
        }
    }
    out.into_iter().collect()
}

impl Scan {
    /// Records a finding on `line`.
    fn find(&mut self, line: usize, what: String) {
        self.found.push((line, what));
    }

    /// The finding for a colour constant `constant` of `Color32`, written on `line`.
    fn constant(&mut self, line: usize, constant: &str) {
        self.find(line, format!("Color32::{constant}, a colour constant: {USE_COLOUR}"));
    }

    /// The finding for a watched type `n` renamed on `line`.
    fn renamed(&mut self, line: usize, n: &str) {
        self.find(line, format!("{n} renamed, which hides its uses from this check: {}", instead(n)));
    }

    /// Walks an assignment's target (`FontId { size, family } = font;`, `(a, Pair(b, c)) = …`): its
    /// structs, tuples, arrays and tuple structs take the value apart as a pattern does, so a
    /// struct there is no literal. Any other place (`*slot(FontId { .. })`) is an expression.
    fn visit_target(&mut self, e: &Expr) {
        match e {
            Expr::Struct(s) => s.fields.iter().for_each(|f| self.visit_target(&f.expr)),
            Expr::Tuple(t) => t.elems.iter().for_each(|x| self.visit_target(x)),
            Expr::Array(a) => a.elems.iter().for_each(|x| self.visit_target(x)),
            Expr::Paren(p) => self.visit_target(&p.expr),
            Expr::Call(c) => {
                self.visit_expr(&c.func);
                c.args.iter().for_each(|x| self.visit_target(x));
            }
            _ => self.visit_expr(e),
        }
    }

    /// Reads a macro's tokens: as expressions separated by commas (`vec![…]`, `format!(…)`), as
    /// `expression; count` (`vec![x; n]`), or as statements and items; tokens that are none of
    /// these (`json!`, a `macro_rules!` body) are read one by one ([`Scan::tokens`]).
    fn macro_body(&mut self, tokens: &TokenStream) {
        if let Ok(exprs) = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(tokens.clone()) {
            exprs.iter().for_each(|e| self.visit_expr(e));
        } else if let Ok((value, count)) = (|input: ParseStream| {
            let value: Expr = input.parse()?;
            input.parse::<Token![;]>()?;
            Ok((value, input.parse::<Expr>()?))
        })
        .parse2(tokens.clone())
        {
            self.visit_expr(&value);
            self.visit_expr(&count);
        } else if let Ok(stmts) = syn::Block::parse_within.parse2(tokens.clone()) {
            stmts.iter().for_each(|s| self.visit_stmt(s));
        } else {
            self.tokens(tokens.clone());
        }
    }

    /// Reads a `macro_rules!` definition: its rules (`(matcher) => { transcriber };`). A matcher only
    /// recognises tokens and is never read; each transcriber is read as Rust with its metavariables
    /// in place ([`transcribed`], [`Scan::macro_body`]). A body not in that shape is read token by
    /// token, failing closed.
    fn macro_rules(&mut self, tokens: &TokenStream) {
        let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
        let punct = |k: usize, c: char| matches!(trees.get(k), Some(TokenTree::Punct(p)) if p.as_char() == c);
        let mut transcribers = Vec::new();
        let mut i = 0;
        while i < trees.len() {
            match (trees.get(i), trees.get(i + 3)) {
                (Some(TokenTree::Group(_)), Some(TokenTree::Group(body))) if punct(i + 1, '=') && punct(i + 2, '>') => {
                    transcribers.push(body.stream());
                    // The rule and the `;` after it, which the last rule may leave out.
                    i += 5;
                }
                _ => return self.tokens(tokens.clone()),
            }
        }
        transcribers.into_iter().for_each(|body| self.macro_body(&transcribed(body)));
    }

    /// Reads tokens that are no Rust one by one, failing closed: `Color32::CONSTANT`, a watched
    /// type followed by `as`, and a literal type followed by braces (unless `->` comes before it)
    /// are findings, inside every group too.
    fn tokens(&mut self, tokens: TokenStream) {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        let punct = |i: usize, c: char| matches!(trees.get(i), Some(TokenTree::Punct(p)) if p.as_char() == c);
        for (i, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(g) => self.macro_body(&g.stream()),
                TokenTree::Ident(id) => {
                    let n = name(id);
                    if !WATCHED.contains(&n.as_str()) {
                        continue;
                    }
                    let at = line(id.span());
                    if n == "Color32"
                        && let Some(constant) = qualified_constant(&trees, i)
                    {
                        self.constant(at, &constant);
                        continue;
                    }
                    let path = matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == ':' && p.spacing() == Spacing::Joint)
                        && punct(i + 2, ':');
                    match (trees.get(i + 1), trees.get(i + 3)) {
                        (_, Some(TokenTree::Ident(c))) if n == "Color32" && path && constant_case(&name(c)) => self.constant(at, &name(c)),
                        (Some(TokenTree::Ident(a)), _) if a == "as" => self.renamed(at, &n),
                        (Some(TokenTree::Group(g)), _) if g.delimiter() == Delimiter::Brace && LITERALS.contains(&n.as_str()) => {
                            // `-> FontId {` opens a body; `&`, `mut` and the path before the type
                            // (`-> &mut egui::FontId {`) are passed over.
                            let ident = |k: usize| matches!(trees.get(k), Some(TokenTree::Ident(_)));
                            let mut j = i;
                            loop {
                                if j >= 1 && (punct(j - 1, '&') || matches!(trees.get(j - 1), Some(TokenTree::Ident(m)) if m == "mut")) {
                                    j -= 1;
                                } else if j >= 2 && punct(j - 1, ':') && punct(j - 2, ':') {
                                    j -= 2;
                                    if j >= 1 && ident(j - 1) {
                                        j -= 1;
                                    }
                                } else {
                                    break;
                                }
                            }
                            let body = j >= 2 && punct(j - 1, '>') && punct(j - 2, '-');
                            if !body {
                                self.find(at, format!("{n} {{ .. }}, built by hand: {}", instead(&n)));
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for Scan {
    /// `use egui::FontId as Face;`: a rename of a watched type.
    fn visit_use_rename(&mut self, r: &'ast syn::UseRename) {
        let n = name(&r.ident);
        if WATCHED.contains(&n.as_str()) {
            self.renamed(line(r.ident.span()), &n);
        }
        visit::visit_use_rename(self, r);
    }

    /// `type Ink = egui::Color32;`: an alias of a watched type.
    fn visit_item_type(&mut self, t: &'ast syn::ItemType) {
        if let Some(n) = watched_type(&t.ty) {
            self.find(
                line(t.type_token.span),
                format!("type {} = {n}, an alias that hides its uses from this check: {}", name(&t.ident), instead(&n)),
            );
        }
        visit::visit_item_type(self, t);
    }

    /// `type Output = Color32;` in an impl: an alias too.
    fn visit_impl_item_type(&mut self, t: &'ast syn::ImplItemType) {
        if let Some(n) = watched_type(&t.ty) {
            self.find(
                line(t.type_token.span),
                format!("type {} = {n}, an alias that hides its uses from this check: {}", name(&t.ident), instead(&n)),
            );
        }
        visit::visit_impl_item_type(self, t);
    }

    /// `Color32::RED`, in an expression, a pattern or a type.
    fn visit_path(&mut self, p: &'ast syn::Path) {
        let segments: Vec<&syn::PathSegment> = p.segments.iter().collect();
        for pair in segments.windows(2) {
            if let [owner, member] = pair
                && name(&owner.ident) == "Color32"
                && constant_case(&name(&member.ident))
            {
                self.constant(line(owner.ident.span()), &name(&member.ident));
            }
        }
        visit::visit_path(self, p);
    }

    /// `<Color32>::RED` and `<egui::Color32 as Trait>::RED`: a constant through a qualified path.
    fn visit_expr_path(&mut self, e: &'ast syn::ExprPath) {
        if let Some(q) = &e.qself
            && watched_type(&q.ty).as_deref() == Some("Color32")
            && let Some(member) = e.path.segments.last()
            && constant_case(&name(&member.ident))
        {
            self.constant(line(q.lt_token.spans[0]), &name(&member.ident));
        }
        visit::visit_expr_path(self, e);
    }

    /// `FontId { size, family }`: a font made by hand; not with the rest `..` and no base, which
    /// takes a value apart (`matches!(f, FontId { size, .. })`).
    fn visit_expr_struct(&mut self, e: &'ast syn::ExprStruct) {
        let pattern = e.dot2_token.is_some() && e.rest.is_none();
        if let Some(last) = e.path.segments.last()
            && e.qself.is_none()
            && LITERALS.contains(&name(&last.ident).as_str())
            && !pattern
        {
            let n = name(&last.ident);
            self.find(line(last.ident.span()), format!("{n} {{ .. }}, built by hand: {}", instead(&n)));
        }
        visit::visit_expr_struct(self, e);
    }

    /// `FontId { size, family } = font;`: the target is a pattern ([`Scan::visit_target`]), the
    /// value an expression.
    fn visit_expr_assign(&mut self, e: &'ast syn::ExprAssign) {
        e.attrs.iter().for_each(|a| self.visit_attribute(a));
        self.visit_target(&e.left);
        self.visit_expr(&e.right);
    }

    /// A macro's tokens: `matches!` and its asserting forms take an expression and a pattern
    /// ([`PATTERN_MACROS`]); the others are read by [`Scan::macro_body`].
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        visit::visit_macro(self, m);
        if m.path.is_ident("macro_rules") {
            return self.macro_rules(&m.tokens);
        }
        let takes_pattern = std_pattern_macro(&m.path, &self.shadowed);
        match takes_pattern.then(|| m.parse_body_with(pattern_args).ok()).flatten() {
            Some((value, pattern, guard, rest)) => {
                self.visit_expr(&value);
                self.visit_pat(&pattern);
                guard.iter().chain(rest.iter()).for_each(|e| self.visit_expr(e));
            }
            None => self.macro_body(&m.tokens),
        }
    }
}

/// The constant of a qualified path through `Color32` in raw tokens, `Color32` at `i`:
/// `<egui::Color32>::RED` or `<Color32 as Default>::WHITE`; none for anything else.
fn qualified_constant(trees: &[TokenTree], i: usize) -> Option<String> {
    let punct = |k: usize, c: char| matches!(trees.get(k), Some(TokenTree::Punct(p)) if p.as_char() == c);
    // Back over the path to the `<` that opens the qualified type.
    let mut j = i;
    while j >= 2 && punct(j - 1, ':') && punct(j - 2, ':') {
        j -= 2;
        if j >= 1 && matches!(trees.get(j - 1), Some(TokenTree::Ident(_))) {
            j -= 1;
        }
    }
    // A turbofish (`Wrapper::<Color32>::RED`) is a generic argument, not a qualified type.
    let turbofish = j >= 3 && punct(j - 2, ':') && punct(j - 3, ':');
    if j == 0 || !punct(j - 1, '<') || turbofish {
        return None;
    }
    // On to the `>` that closes it, past `as Trait<…>`.
    let mut k = i + 1;
    if matches!(trees.get(k), Some(TokenTree::Ident(a)) if a == "as") {
        let mut depth = 0usize;
        loop {
            k += 1;
            match trees.get(k) {
                None => return None,
                Some(TokenTree::Punct(p)) if p.as_char() == '<' => depth += 1,
                Some(TokenTree::Punct(p)) if p.as_char() == '>' && depth == 0 => break,
                Some(TokenTree::Punct(p)) if p.as_char() == '>' => depth -= 1,
                _ => {}
            }
        }
    }
    let Some(TokenTree::Ident(member)) = trees.get(k + 3) else { return None };
    (punct(k, '>') && punct(k + 1, ':') && punct(k + 2, ':') && constant_case(&name(member))).then(|| name(member))
}

/// Macros whose second argument is a pattern, with an optional guard: `matches!(font, FontId {
/// size, family } if size > 1.0)`, and the asserting forms, which may add a message.
const PATTERN_MACROS: [&str; 3] = ["matches", "assert_matches", "debug_assert_matches"];

/// The arguments of a [`PATTERN_MACROS`] macro: the value, the pattern, its guard and what follows
/// (an assertion's message and its arguments).
fn pattern_args(input: ParseStream) -> syn::Result<(Expr, syn::Pat, Option<Expr>, Vec<Expr>)> {
    let value: Expr = input.parse()?;
    input.parse::<Token![,]>()?;
    let pattern = syn::Pat::parse_multi_with_leading_vert(input)?;
    let guard = if input.parse::<Option<Token![if]>>()?.is_some() { Some(input.parse::<Expr>()?) } else { None };
    let mut rest = Vec::new();
    while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
        rest.push(input.parse::<Expr>()?);
    }
    Ok((value, pattern, guard, rest))
}

/// The colours and fonts made outside ux in one source file's text, each `line N: …`, in the
/// order of their lines. A text that is no Rust file is read as statements (a fragment, as the
/// tests give); one that is neither is a finding, so the check fails closed.
pub fn findings(source: &str) -> Vec<String> {
    let (mut scan, mut shadows) = (Scan::default(), Shadows::default());
    match syn::parse_file(source) {
        Ok(file) => {
            shadows.visit_file(&file);
            scan.shadowed = shadows.0;
            scan.visit_file(&file);
        }
        Err(e) => match syn::Block::parse_within.parse_str(source) {
            Ok(stmts) => {
                stmts.iter().for_each(|s| shadows.visit_stmt(s));
                scan.shadowed = shadows.0;
                stmts.iter().for_each(|s| scan.visit_stmt(s));
            }
            Err(_) => scan.find(line(e.span()), format!("not read as Rust ({e}), so this check cannot see into it")),
        },
    }
    scan.found.sort_by_key(|(line, _)| *line);
    scan.found.into_iter().map(|(line, what)| format!("line {line}: {what}")).collect()
}

/// Every `.rs` file under `dir`, sorted, leaving out `skip`: the walk of the shell's process scan
/// (`shell_io.rs`), with `ux` left out.
fn sources(dir: &Path, skip: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let path = entry.map_err(|e| format!("{}: {e}", d.display()))?.path();
            if path.is_dir() {
                if path != skip {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|x| x == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// The colours and fonts made outside ux in every source under `root`'s crates and applications,
/// each named with its file from `root`.
pub fn scan(root: &Path) -> Result<Vec<String>, String> {
    let ux = UX.iter().fold(root.to_path_buf(), |p, part| p.join(part));
    let mut out = Vec::new();
    for dir in SCANNED.iter().map(|d| root.join(d)).filter(|d| d.is_dir()) {
        for file in sources(&dir, &ux)? {
            let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let rel = file.strip_prefix(root).unwrap_or(&file).display().to_string();
            out.extend(findings(&text).into_iter().map(|f| format!("{rel} {f}")));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Colour constants are found, each with its line, written out or through egui's path.
    #[test]
    fn colour_constants_are_found_with_their_lines() {
        let source =
            "fn a() {\n    let _ = Color32::WHITE;\n}\nconst B: egui::Color32 = egui::Color32 :: TRANSPARENT;\nlet c = Color32::from_rgb(1, 2, 3);\n";
        let found = findings(source);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("line 2: Color32::WHITE, a colour constant: "), "{found:?}");
        assert!(found[1].starts_with("line 4: Color32::TRANSPARENT, a colour constant: "), "{found:?}");
        assert!(found[0].contains("ux::Palette"), "the finding says what to use: {found:?}");
        // A method is clippy's to refuse (clippy.toml), and a type alone is no colour.
        assert_eq!(findings("let c = Color32::from_gray(1); fn f(c: Color32) -> Color32 { c }"), Vec::<String>::new());
    }

    /// A font or a colour built as a struct literal is found; a function returning one is not.
    #[test]
    fn struct_literals_are_found_and_return_types_are_not() {
        let found = findings("let f = FontId { size: 12.0, family };\nlet c: Color32 = egui::ecolor::HsvaGamma { h, s, v, a: 1.0 }.into();");
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("line 1: FontId { .. }, built by hand: ") && found[0].contains("ux::types"), "{found:?}");
        assert!(found[1].starts_with("line 2: HsvaGamma { .. }, built by hand: ") && found[1].contains("ux::data_colour"), "{found:?}");
        assert_eq!(findings("fn font() -> FontId {\n    types::UI.font()\n}\nfn g() -> egui::FontId { f }"), Vec::<String>::new());
    }

    /// Renaming or aliasing a watched type would hide its constants and literals, so every rename
    /// and every `type` alias of one is a finding, in a group too; an alias of something else, or
    /// of a type built on a watched one, is not.
    #[test]
    fn renamed_and_aliased_types_are_found() {
        let found = findings("use egui::Color32 as C;\nlet x = C::RED;");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("line 1: Color32 renamed, which hides its uses from this check: ") && found[0].contains("ux::Palette"),
            "{found:?}"
        );
        let found = findings("type Ink = egui::Color32;\nlet c = Ink::RED;");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("line 1: type Ink = Color32, an alias that hides its uses from this check: "), "{found:?}");
        let found = findings("use egui::FontId as Face;\nlet f = Face { size: 12.0, family };");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("line 1: FontId renamed, which hides its uses from this check: ") && found[0].contains("ux::types"),
            "{found:?}"
        );
        for (source, name) in [
            ("use egui::{Color32 as C, Pos2};", "Color32"),
            ("use egui::ecolor::{Hsva as H};", "Hsva"),
            ("use egui::ecolor::HsvaGamma as G;", "HsvaGamma"),
            ("use egui::Rgba as R;", "Rgba"),
        ] {
            let found = findings(source);
            assert_eq!(found.len(), 1, "{source}: {found:?}");
            assert!(found[0].starts_with(&format!("line 1: {name} renamed, ")), "{source}: {found:?}");
        }
        for (source, name) in [
            ("pub(crate) type Face = ::egui::FontId;", "FontId"),
            ("type H<T> = egui::ecolor::Hsva;", "Hsva"),
            ("type G = HsvaGamma ;", "HsvaGamma"),
            ("type R = egui::Rgba;", "Rgba"),
        ] {
            let found = findings(source);
            assert_eq!(found.len(), 1, "{source}: {found:?}");
            assert!(found[0].starts_with("line 1: type ") && found[0].contains(&format!(" = {name}, an alias")), "{source}: {found:?}");
        }
        let none = "type Fonts = Vec<FontId>;\ntype Pick = Option<Color32>;\ntype Size = f32;\nlet a = b as Color32;\nfn ty() -> u8 { 1 }";
        // A trait's associated type is declared with no `=`: what follows it is no alias.
        let declared = "trait Paint {\n    type Ink;\n    const BASE: egui::Color32;\n}";
        assert_eq!(findings(none), Vec::<String>::new());
        assert_eq!(findings(declared), Vec::<String>::new());
    }

    /// A struct pattern takes a font or a colour apart and makes none: in a let, an if let, a
    /// match arm, a parameter, a closure, a for loop and a tuple. The same braces as a value are
    /// still a literal: in a let, an argument, a tuple, a field, a block's end or behind a `&`.
    #[test]
    fn struct_patterns_are_not_literals_and_literals_still_are() {
        let patterns = [
            "let egui::FontId { size, family } = ux::types::UI.font();",
            "if let Some(FontId { size, family }) = font { }",
            "while let Some(FontId { size, .. }) = fonts.pop() { }",
            "match font { FontId { size, family } => size, }",
            "match font { egui::FontId { size, family } if size > 1.0 => size, _ => 0.0 }",
            "match font { FontId { size: 12.0, .. } | FontId { size: 13.0, .. } => 1, _ => 0 }",
            "fn f(FontId { size, family }: FontId) {}",
            "let g = |FontId { size, family }| size;",
            "for FontId { size, family } in fonts {}",
            "for (FontId { size, family }, n) in pairs {}",
            "let (Hsva { h, s, v, a }, b) = pair;",
            "let [FontId { size, family }, rest] = fonts;",
            "FontId { size, family } = font;",
            "S { font: FontId { size, family } } = s;",
            "[FontId { size, family }, rest] = fonts;",
            "(FontId { size, family }) = font;",
            "let S { font: FontId { size, .. } } = s;",
        ];
        for source in patterns {
            assert_eq!(findings(source), Vec::<String>::new(), "{source}");
        }
        let literals = [
            "let f = egui::FontId { size, family };",
            "let f = ::egui::FontId { size, family };",
            "set(FontId { size, family }, 2);",
            "let t = (FontId { size, family }, x == y);",
            "let same = FontId { size, family } == font;",
            "let s = S { font: FontId { size, family }, b: 1 };",
            "fn f() -> X { FontId { size, family } }\nconst A: u8 = 1;",
            "let r = &FontId { size, family };",
            "let m = &mut FontId { size, family };",
            "if a == (FontId { size, family }) {}",
            "let v = vec![Rgba { r, g, b, a }];",
            "match k { A => FontId { size, family }, B => x }",
            "let f = FontId { size, ..base };",
            "return FontId { size, family };",
        ];
        for source in literals {
            let found = findings(source);
            assert_eq!(found.len(), 1, "{source}: {found:?}");
            assert!(found[0].contains("{ .. }, built by hand"), "{source}: {found:?}");
        }
        // Unparenthesized, the compiler refuses that condition; the scan, which cannot read it,
        // still refuses it.
        let unread = findings("if a == FontId { size, family } {}");
        assert_eq!(unread.len(), 1, "{unread:?}");
        assert!(unread[0].starts_with("line 1: not read as Rust ("), "{unread:?}");
    }

    /// A `{` after a type that opens a body is no literal: a function's return type behind a
    /// reference, a lifetime, `mut` or a raw pointer, a closure type's return, and an impl's type.
    #[test]
    fn return_and_impl_types_open_bodies() {
        let bodies = [
            "fn font(f: &egui::FontId) -> &egui::FontId { f }",
            "fn font<'a>(f: &'a mut FontId) -> &'a mut FontId { f }",
            "fn font(f: *const FontId) -> *const FontId { f }",
            "fn font() -> impl Fn() -> FontId { || ux::types::UI.font() }",
            "impl Describe for FontId { fn describe(&self) {} }",
            "impl Describe for &egui::FontId { fn describe(&self) {} }",
            "fn hsv() -> &'static Hsva { &H }",
            "fn font() -> &::egui::FontId { f }",
        ];
        for source in bodies {
            assert_eq!(findings(source), Vec::<String>::new(), "{source}");
        }
        // A parameter or a return type in angle brackets is followed by no `{` at all.
        assert_eq!(findings("fn f(a: Option<&FontId>, b: &mut Rgba) -> Option<&FontId> { a }"), Vec::<String>::new());
        assert_eq!(findings("fn f() -> Box<dyn Fn() -> FontId> { g() }"), Vec::<String>::new());
    }

    /// What a hand-written token scan misreads is read as the compiler reads it: a raw C string
    /// holding a quote, a raw identifier, a parenthesized type, a qualified path, and a struct
    /// pattern among several closure parameters.
    #[test]
    fn token_corners_are_read_as_the_compiler_reads_them() {
        let alias = "type Ink = Color32, an alias that hides its uses from this check: ";
        let cases: [(&str, &[&str]); 8] = [
            ("let s = cr#\"a\"b Color32::RED\"#;\nlet t = c\"x\";", &[]),
            ("type Ink = egui::r#Color32;", &[alias]),
            ("type Ink = (egui::Color32);", &[alias]),
            ("let c = <egui::Color32>::RED;", &["Color32::RED, a colour constant: "]),
            ("let d = egui::r#Color32::WHITE;", &["Color32::WHITE, a colour constant: "]),
            ("let g = |FontId { size, family }, scale: f32| size * scale;", &[]),
            ("*slot(FontId { size, family }) = font;", &["FontId { .. }, built by hand: "]),
            ("(Pair(FontId { size, family }, n), m) = pairs;", &[]),
        ];
        // Every case is checked before the test fails, so a failure names all the misread ones.
        let misread: Vec<String> = cases
            .iter()
            .filter_map(|(source, expected)| {
                let found = findings(source);
                let right = found.len() == expected.len() && found.iter().zip(expected.iter()).all(|(f, e)| f.starts_with(&format!("line 1: {e}")));
                (!right).then(|| format!("{source} gave {found:?}"))
            })
            .collect();
        assert_eq!(misread, Vec::<String>::new());
    }

    /// A macro's tokens are read as expressions or statements where they parse as such, and token
    /// by token where they do not, so a constant or a literal in one is found and a pattern is not.
    #[test]
    fn macros_are_read_too() {
        for source in [
            "let v = vec![FontId { size, family }; 3];",
            "let j = json!({ \"font\": FontId { size: 1.0, family } });",
            "println!(\"{:?}\", Color32::RED);",
            "macro_rules! m { () => { Color32::RED } }",
            "macro_rules! m { ($f:expr) => { use egui::Color32 as C; } }",
            "assert!(matches!(c, egui::Color32::BLACK));",
            "thread_local! { static C: Color32 = Color32::RED; }",
            "macro_rules! red { () => { <egui::Color32>::RED }; }",
            "assert_matches!(font, FontId { .. }, \"{:?}\", Color32::RED);",
            "assert!(matches!(FontId { size, family }, _));",
            "m!(::FontId { size, family } and more);",
        ] {
            let found = findings(source);
            assert_eq!(found.len(), 1, "{source}: {found:?}");
        }
        for source in [
            "assert!(matches!(f, FontId { size, .. }));",
            "assert!(matches!(font, egui::FontId { size, family }));",
            "let big = matches!(font, FontId { size, family } if size > 1.0);",
            "assert_matches!(font, FontId { size, family } | FontId { size, family }, \"font {}\", 1);",
            "with_font! { let FontId { size, family } = font; }",
            "with_font! { FontId { size, family } = font; }",
            "macro_rules! m { () => { fn f() -> &egui::FontId { x } } }",
            "macro_rules! m { () => { fn f() -> &mut egui::FontId { x } } }",
            "m!(fn f() -> &egui::FontId { x } @);",
            "m!(fn f() -> &mut egui::FontId { x } @);",
            "let s = format!(\"Color32::RED {}\", 1);",
        ] {
            assert_eq!(findings(source), Vec::<String>::new(), "{source}");
        }
    }

    /// A `macro_rules!` matcher only recognises tokens, so it is never read; a transcriber is read
    /// as Rust with its metavariables and repetitions in place. `matches!` takes a pattern only
    /// when it is the standard library's: unqualified and not shadowed by a macro or an import of
    /// that name in the file, or named through `std` or `core`. A qualified constant
    /// (`<egui::Color32>::RED`) is found in tokens that are no Rust too.
    #[test]
    fn macro_rules_and_pattern_macros_are_read_by_what_they_are() {
        let constant = "Color32::";
        let built = "{ .. }, built by hand";
        let found = [
            ("macro_rules! red { ($note:expr) => {{ let _ = $note; <egui::Color32>::RED }}; }", constant),
            ("macro_rules! reds { ($($x:ident),*) => { $(let $x = Color32::RED;)* }; }", constant),
            ("macro_rules! paint { ($c:expr) => { $crate::fill(<Color32 as Default>::WHITE, $c) }; }", constant),
            ("macro_rules! m { Color32::RED }", constant),
            ("let j = json!({ \"c\": <egui::Color32>::RED });", constant),
            ("let j = json!({ \"c\": <egui::Color32 as Default>::WHITE });", "Color32::WHITE, a colour constant"),
            ("macro_rules! matches { ($x:expr, $font:expr) => { $font }; }\nlet f = matches!((), FontId { size, family });", built),
            ("use helpers::matches;\nlet f = matches!((), FontId { size, family });", built),
            ("use helpers::pick as matches;\nlet f = matches!((), FontId { size, family });", built),
            ("let f = other::matches!((), FontId { size, family });", built),
            ("let f = ::matches!((), FontId { size, family });", built),
            ("let j = json!({ \"c\": <egui::Color32 as Tint<u8>>::RED });", "Color32::RED, a colour constant"),
        ];
        let none = [
            "macro_rules! accepts_face { (FontId { $($fields:tt)* }) => { () }; }",
            "macro_rules! sizes { ($($f:ident),*) => { $(let FontId { size, .. } = $f;)* }; }",
            "macro_rules! pairs { ($($f:expr),*) => { let ($(FontId { size, family }),*) = ($($f),*); }; }",
            "macro_rules! inner { ($f:expr) => {{ let FontId { size, .. } = $f; size }}; }",
            "let b = std::matches!(f, FontId { size, family });",
            "let b = ::core::matches!(f, FontId { size, family });",
            "let b = matches!(f, FontId { size, family });",
            "use helpers::other;\nlet b = matches!(f, FontId { size, family });",
            "let j = json!({ \"c\": <egui::Color32>::default() });",
            "let j = json!({ \"c\": Wrapper::<Color32>::RED });",
            "m!(shift: Color32 >::RED @);",
        ];
        // Every case is checked before the test fails, so a failure names all the misread ones.
        let misread: Vec<String> = found
            .iter()
            .map(|(s, says)| (s, Some(*says)))
            .chain(none.iter().map(|s| (s, None)))
            .filter_map(|(source, says)| {
                let got = findings(source);
                let right = match says {
                    Some(says) => got.len() == 1 && got[0].contains(says),
                    None => got.is_empty(),
                };
                (!right).then(|| format!("{source} gave {got:?}"))
            })
            .collect();
        assert_eq!(misread, Vec::<String>::new());
    }

    /// Strings, characters and comments never count.
    #[test]
    fn strings_and_comments_are_blanked() {
        let source =
            "// Color32::RED in a comment\n/// FontId { size } in a doc comment\nlet s = \"Color32::BLACK\";\nlet r = r#\"FontId { x }\"#;\n";
        assert_eq!(findings(source), Vec::<String>::new());
    }

    /// The repository as it stands makes no colour or font outside ux, and ux itself is left out.
    #[test]
    fn the_repository_makes_colours_and_fonts_in_ux_alone() {
        let root = crate::util::root();
        assert_eq!(scan(&root).unwrap(), Vec::<String>::new());
        let ux = UX.iter().fold(root.clone(), |p, part| p.join(part));
        let made = std::fs::read_to_string(ux.join("mod.rs")).unwrap();
        assert!(!findings(&made).is_empty(), "ux makes its colours from egui's constants, and the scan would see them");
    }
}
