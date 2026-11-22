//! The ux part of the layer check: the interface draws through `ux` (AGENTS.md), so a colour or a
//! font is made there alone. The compiler holds that rule: `clippy.toml` bans egui's colour and
//! font types (`Color32`, `FontId`, `Hsva`, `HsvaGamma`, `Rgba`, `HexColor`) and their
//! constructors outside `ux`, so naming one there, its constants, a renamed import of it and an
//! `impl` for it included, fails the lint step. A type ban does not see one form: a struct
//! expression of a type with public fields (`FontId { size, family }`), which this check refuses
//! wherever it is written, by the last segment of its path alone ([`LITERALS`]). `Color32` and
//! `Rgba` keep their fields private, so only egui can write theirs.
//!
//! Every Rust source under `crates/` and `apps/`, tests included, is parsed as Rust (`syn`): a
//! struct expression is found as one, and a pattern, a type or a string never is. The input of a
//! macro and the arguments of an attribute are no Rust until the macro makes them so, so there the
//! check fails closed: a watched name followed by a `{ .. }` group is a finding, whatever the
//! macro would make of it. The check resolves no name; what a rename or an alias could hide is the
//! lint step's to refuse. A finding's line is its line in the file. `ux` itself ([`UX`]) is
//! exempt.
//!
//! The walk is strict ([`util::walk_strict_all`]): `crates` and `apps` are judged by their own
//! kind, never through a link; a folder that cannot be opened stops the check; a link or an entry
//! that is no plain file is a finding, never followed or read; and a source over a byte cap, its
//! own or the total's, is a finding, known from its size before it is read.

use std::path::Path;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};

use crate::util;

/// The `ux` module, from the repository's root: where colours and fonts are made.
pub const UX: [&str; 4] = ["crates", "ui-egui", "src", "ux"];

/// The folders, from the root, whose sources are scanned.
const SCANNED: [&str; 2] = ["crates", "apps"];

/// egui's font and colour types whose fields are public, so a struct expression makes one; each
/// with what to use instead.
const LITERALS: [(&str, &str); 3] = [
    ("FontId", "a font made by hand: take a style of ux::types, its .font() or its .code()"),
    ("Hsva", "a colour made by hand: take a role of ux::Palette, ux::data_colour, ux::from_hsv or ux::swatch_hue"),
    ("HsvaGamma", "a colour made by hand: take a role of ux::Palette, ux::data_colour, ux::from_hsv or ux::swatch_hue"),
];

/// What a struct expression of `name` makes, when `name` is a watched type.
fn watched(name: &str) -> Option<&'static str> {
    LITERALS.iter().find(|(n, _)| *n == name).map(|(_, what)| *what)
}

/// The findings of one source, with their lines.
#[derive(Default)]
struct Scan {
    found: Vec<(usize, String)>,
}

impl Scan {
    /// Records a struct expression of the watched type `name` on `line`, if `name` is one.
    fn literal(&mut self, name: &str, line: usize) {
        if let Some(what) = watched(name) {
            self.found.push((line, format!("{name} {{ .. }}, {what}")));
        }
    }

    /// Reads `tokens`, no Rust yet, failing closed: every watched name followed by a `{ .. }`
    /// group, at any depth, is a struct expression as far as this check knows.
    fn tokens(&mut self, tokens: TokenStream) {
        let mut before: Option<proc_macro2::Ident> = None;
        for tree in tokens {
            if let TokenTree::Group(g) = &tree {
                if g.delimiter() == Delimiter::Brace
                    && let Some(ident) = &before
                {
                    self.literal(&ident.unraw().to_string(), ident.span().start().line);
                }
                self.tokens(g.stream());
            }
            before = match tree {
                TokenTree::Ident(ident) => Some(ident),
                _ => None,
            };
        }
    }
}

impl<'ast> Visit<'ast> for Scan {
    /// A struct expression, found by the last segment of its path (`egui::FontId { .. }`).
    fn visit_expr_struct(&mut self, e: &'ast syn::ExprStruct) {
        if let Some(last) = e.path.segments.last() {
            self.literal(&last.ident.unraw().to_string(), last.ident.span().start().line);
        }
        visit::visit_expr_struct(self, e);
    }

    /// A macro's input, read as tokens.
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        self.tokens(m.tokens.clone());
    }

    /// An attribute's arguments: a list (`#[case(..)]`) read as tokens, since an attribute macro
    /// may make code of them; a value (`#[doc = ..]`) as the expression it is.
    fn visit_attribute(&mut self, a: &'ast syn::Attribute) {
        match &a.meta {
            syn::Meta::List(list) => self.tokens(list.tokens.clone()),
            syn::Meta::NameValue(pair) => self.visit_expr(&pair.value),
            syn::Meta::Path(_) => {}
        }
    }
}

/// The fonts and colours made by hand in one source file's text, each `line N: …`, in the order of
/// their lines. A text that is no Rust file is a finding, so the check fails closed.
pub fn findings(source: &str) -> Vec<String> {
    let mut scan = Scan::default();
    match syn::parse_file(source) {
        Ok(file) => scan.visit_file(&file),
        Err(e) => scan.found.push((e.span().start().line, format!("not read as Rust ({e}), so this check cannot see into it"))),
    }
    scan.found.sort_by_key(|(line, _)| *line);
    scan.found.into_iter().map(|(line, what)| format!("line {line}: {what}")).collect()
}

/// The most bytes the scan reads of one source.
const MAX_FILE: u64 = 16 << 20;

/// The most bytes the scan reads of all the sources together.
const MAX_TOTAL: u64 = 256 << 20;

/// The text of `file`, read up to one byte past `limit`, so a file that holds more than `limit`
/// shows it by its length.
fn read_capped(file: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .and_then(|f| f.take(limit.saturating_add(1)).read_to_end(&mut bytes))
        .map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(bytes)
}

/// The colours and fonts made outside ux in every source under `root`'s crates and applications,
/// each named with its file from `root`. What this check cannot read safely is a finding too: a
/// root (`crates`, `apps`) that is a link or no folder, a link or an entry that is no plain file
/// under them, a source over [`MAX_FILE`] bytes, and one past [`MAX_TOTAL`] in all. A root that is
/// missing or cannot be read, and a folder under one that cannot be opened ([`util::walk_strict`]),
/// stop the check with their paths, so no part of the scope passes unread.
pub fn scan(root: &Path) -> Result<Vec<String>, String> {
    scan_within(root, MAX_FILE, MAX_TOTAL, |p| std::fs::symlink_metadata(p).map(|m| m.len())).map(|(found, _)| found)
}

/// [`scan`] with its caps given, and a source's size in bytes from `size_of`, so a test can give
/// one that grows after it was measured; also how many bytes were read in all.
fn scan_within(root: &Path, max_file: u64, max_total: u64, size_of: impl Fn(&Path) -> std::io::Result<u64>) -> Result<(Vec<String>, u64), String> {
    let ux = UX.iter().fold(root.to_path_buf(), |p, part| p.join(part));
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let (mut out, mut files) = (Vec::new(), Vec::new());
    for dir in SCANNED.iter().map(|d| root.join(d)) {
        // Judged by its own kind: `is_dir` would follow a link, and take a dangling or unreadable
        // root for an absent one.
        let kind = std::fs::symlink_metadata(&dir)
            .map_err(|e| format!("{}: cannot be read, so nothing under it is checked: {e}", dir.display()))?
            .file_type();
        if kind.is_symlink() {
            out.push(format!("{}: a link, which this check does not follow", rel(&dir)));
        } else if !kind.is_dir() {
            out.push(format!("{}: no folder, so this check reads nothing there", rel(&dir)));
        } else {
            // `ux` is left out by its path, not by a folder name another crate might use too.
            let walked = util::walk_strict_all(&dir, &[])?;
            out.extend(walked.passed_over.iter().map(|p| format!("{}: a link or no plain file, which this check neither follows nor reads", rel(p))));
            files.extend(walked.files.into_iter().filter(|p| !p.starts_with(&ux) && p.extension().is_some_and(|x| x == "rs")));
        }
    }
    let (mut budget, mut read) = (max_total, 0u64);
    let over_file = |p: &Path| format!("{}: more than {max_file} bytes, so this check does not read it", rel(p));
    let over_total = |p: &Path| format!("{}: past the {max_total} bytes this check reads in all, so it is not read", rel(p));
    for file in files {
        let size = size_of(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        if size > max_file {
            out.push(over_file(&file));
            continue;
        }
        if size > budget {
            out.push(over_total(&file));
            continue;
        }
        // The file may have grown since it was measured: what is read counts, kept or not.
        let limit = max_file.min(budget);
        let bytes = read_capped(&file, limit)?;
        let taken = bytes.len() as u64;
        read += taken;
        budget = budget.saturating_sub(taken);
        if taken > limit {
            out.push(if limit < max_file { over_total(&file) } else { over_file(&file) });
            continue;
        }
        let text = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        out.extend(findings(&text).into_iter().map(|f| format!("{} {f}", rel(&file))));
    }
    Ok((out, read))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A struct expression of a watched type is found with its line, by the last segment of its
    /// path, a raw identifier by its name; each finding says what to use instead.
    #[test]
    fn a_struct_expression_of_a_watched_type_is_found_with_its_line() {
        let found = findings(
            "fn f() {\n    let a = FontId { size: 12.0, family };\n    let b = egui::ecolor::HsvaGamma { h, s, v, a: 1.0 };\n    let c = r#Hsva { h, s, v, a };\n}\n",
        );
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[0].starts_with("line 2: FontId { .. }, a font made by hand: ") && found[0].contains("ux::types"), "{found:?}");
        assert!(found[1].starts_with("line 3: HsvaGamma { .. }, a colour made by hand: ") && found[1].contains("ux::from_hsv"), "{found:?}");
        assert!(found[2].starts_with("line 4: Hsva { .. }, a colour made by hand: "), "{found:?}");
    }

    /// Only a struct expression of a watched type is found: another type's (one whose name holds a
    /// watched one too), a pattern taking a font apart, a type, a string and a comment are not.
    #[test]
    fn other_struct_expressions_patterns_types_and_strings_are_not() {
        let source = "fn f(font: FontId) -> FontId {\n    let FontId { size, .. } = font;\n    let s = Stroke { width: 1.0, color };\n    let t = MyFontId { size };\n    let u = \"FontId { size }\";\n    // FontId { size }\n    font\n}\n";
        assert_eq!(findings(source), Vec::<String>::new());
    }

    /// A macro's input and an attribute's arguments are read as tokens and fail closed: a watched
    /// name followed by a `{ .. }` group is a finding, at any depth; one followed by anything else,
    /// a string, and another name are not.
    #[test]
    fn macro_input_and_attribute_arguments_are_read_as_tokens() {
        let source = "#[case(egui::FontId { size: 1.0, family })]\n#[cfg_attr(test, case((Hsva { h, s, v, a })))]\nfn f() {\n    let v = vec![FontId { size, family }];\n    m!((x, [HsvaGamma { h, s, v, a }]));\n    m!(FontId [size] FontId (size) FontId::new(1.0, f) \"FontId { }\" Font { });\n}\n";
        let found = findings(source);
        let lines: Vec<&str> = found.iter().map(|f| f.split(", ").next().unwrap_or_default()).collect();
        assert_eq!(lines, ["line 1: FontId { .. }", "line 2: Hsva { .. }", "line 4: FontId { .. }", "line 5: HsvaGamma { .. }"], "{found:?}");
    }

    /// A source that is not Rust is a finding, so nothing passes unread.
    #[test]
    fn a_source_not_read_as_rust_is_a_finding() {
        let found = findings("fn f( {\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("not read as Rust"), "{found:?}");
    }

    /// `crates` and `apps` are judged by their own kind, never through a link: a linked root, a
    /// dangling one and a file are findings, and a root that is missing or cannot be read stops
    /// the scan with its path (CodeRabbit on #573).
    #[cfg(unix)]
    #[test]
    fn scan_roots_are_judged_by_their_own_kind() {
        let s = crate::util::Scratch::new("ux-roots").unwrap();
        let outside = crate::util::Scratch::new("ux-roots-outside").unwrap();
        outside.file("a/src/lib.rs", "fn f() { egui::FontId { size: 1.0, family }; }\n").unwrap();
        std::os::unix::fs::symlink(&outside.path, s.path.join("crates")).unwrap();
        std::os::unix::fs::symlink(s.path.join("gone"), s.path.join("apps")).unwrap();
        assert_eq!(
            scan(&s.path),
            Ok(vec!["crates: a link, which this check does not follow".to_string(), "apps: a link, which this check does not follow".to_string()])
        );
        let s = crate::util::Scratch::new("ux-roots-file").unwrap();
        s.file("crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        s.file("apps", "").unwrap();
        assert_eq!(scan(&s.path), Ok(vec!["apps: no folder, so this check reads nothing there".to_string()]));
        let s = crate::util::Scratch::new("ux-roots-missing").unwrap();
        s.file("crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        let missing = scan(&s.path);
        assert!(missing.as_ref().is_err_and(|e| e.starts_with(&format!("{}: ", s.path.join("apps").display()))), "{missing:?}");
        let s = crate::util::Scratch::new("ux-roots-locked").unwrap();
        s.file("r/crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        s.file("r/apps/b/src/main.rs", "fn main() {}\n").unwrap();
        if s.lock("r").unwrap() {
            let locked = scan(&s.path.join("r"));
            let crates = s.path.join("r").join("crates");
            assert!(locked.as_ref().is_err_and(|e| e.starts_with(&format!("{}: ", crates.display()))), "{locked:?}");
        }
    }

    /// Under the roots, a link, to a file or a folder, and an entry that is no plain file (a pipe,
    /// whose reading could block) are findings, never followed or read.
    #[cfg(unix)]
    #[test]
    fn links_and_pipes_under_the_roots_are_findings_and_never_followed() {
        let s = crate::util::Scratch::new("ux-links").unwrap();
        s.file("crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        s.file("apps/b/src/main.rs", "fn main() {}\n").unwrap();
        let outside = crate::util::Scratch::new("ux-links-outside").unwrap();
        outside.file("far.rs", "fn f() { FontId { size: 1.0, family }; }\n").unwrap();
        let src = s.path.join("crates").join("a").join("src");
        std::os::unix::fs::symlink(outside.path.join("far.rs"), src.join("linked.rs")).unwrap();
        std::os::unix::fs::symlink(&outside.path, src.join("folder")).unwrap();
        let made = std::process::Command::new("mkfifo").arg(src.join("pipe.rs")).status().unwrap();
        assert!(made.success(), "mkfifo");
        let passed = "a link or no plain file, which this check neither follows nor reads";
        assert_eq!(scan(&s.path), Ok(["folder", "linked.rs", "pipe.rs"].iter().map(|n| format!("crates/a/src/{n}: {passed}")).collect::<Vec<_>>()));
    }

    /// A source known from its size to be over a cap is never read: over the cap of one file, or
    /// past what is left of the total. Every byte read counts against the total, those of a file
    /// that grew past its cap and was put aside too (CodeRabbit on #573).
    #[test]
    fn the_byte_caps_reject_by_size_and_count_every_byte_read() {
        let more = "more than 100 bytes, so this check does not read it";
        let past = "past the 150 bytes this check reads in all, so it is not read";
        let s = crate::util::Scratch::new("ux-caps").unwrap();
        std::fs::create_dir(s.path.join("apps")).unwrap();
        let big = "x".repeat(200);
        s.file("crates/a/one.rs", &big).unwrap();
        s.file("crates/a/two.rs", &big).unwrap();
        let size = |p: &Path| std::fs::symlink_metadata(p).map(|m| m.len());
        let (found, read) = scan_within(&s.path, 100, 150, size).unwrap();
        assert_eq!(found, [format!("crates/a/one.rs: {more}"), format!("crates/a/two.rs: {more}")]);
        assert_eq!(read, 0, "a source known to be too big is never read");
        // a.rs and d.rs say they are small (as a file still being written may) and are not.
        let s = crate::util::Scratch::new("ux-caps-read").unwrap();
        std::fs::create_dir(s.path.join("apps")).unwrap();
        s.file("crates/a/a.rs", &big).unwrap();
        s.file("crates/a/b.rs", &format!("//{}\n", "b".repeat(57))).unwrap();
        s.file("crates/a/c.rs", "fn c(){FontId{size:1.0,family};}\n").unwrap();
        s.file("crates/a/d.rs", &"x".repeat(40)).unwrap();
        let lying = |p: &Path| if p.ends_with("a.rs") || p.ends_with("d.rs") { Ok(5) } else { size(p) };
        let (found, read) = scan_within(&s.path, 100, 150, lying).unwrap();
        assert_eq!(found.len(), 4, "{found:?}");
        assert_eq!(found[0], format!("crates/a/a.rs: {more}"));
        // a.rs took 101 bytes of the 150, so b.rs (60) is past the total; c.rs (33) still fits.
        assert_eq!(found[1], format!("crates/a/b.rs: {past}"));
        assert!(found[2].starts_with("crates/a/c.rs line 1: FontId { .. }, a font made by hand"), "{found:?}");
        // 16 bytes are left for d.rs, which holds 40: 17 are read and put aside.
        assert_eq!(found[3], format!("crates/a/d.rs: {past}"));
        assert_eq!(read, 101 + 33 + 17);
    }

    /// `ux` is left out by its path, where its fonts are made, and the same literal anywhere else
    /// is found.
    #[test]
    fn ux_is_left_out_and_nothing_else_is() {
        let s = crate::util::Scratch::new("ux-exempt").unwrap();
        std::fs::create_dir(s.path.join("apps")).unwrap();
        let literal = "fn f() { FontId { size: 1.0, family }; }\n";
        s.file("crates/ui-egui/src/ux/mod.rs", literal).unwrap();
        s.file("crates/ui-egui/src/screen.rs", literal).unwrap();
        s.file("crates/other/src/ux/mod.rs", literal).unwrap();
        let found = scan(&s.path).unwrap();
        let files: Vec<&str> = found.iter().map(|f| f.split(' ').next().unwrap_or_default()).collect();
        assert_eq!(files, ["crates/other/src/ux/mod.rs", "crates/ui-egui/src/screen.rs"], "{found:?}");
    }

    /// The repository as it stands makes no font or colour by hand outside ux.
    #[test]
    fn the_repository_makes_fonts_and_colours_in_ux_alone() {
        assert_eq!(scan(&crate::util::root()).unwrap(), Vec::<String>::new());
    }
}
