//! The ux part of the layer check: the interface draws through `ux` (AGENTS.md), so a colour or a
//! font is made there alone. Clippy refuses egui's colour and font constructors everywhere else
//! (`disallowed-methods` and `disallowed-types` in `clippy.toml`); this scan refuses what clippy
//! cannot see: a colour constant (`Color32::RED`, or brought in with `use egui::Color32::*`), a
//! font or a colour built as a struct literal (`FontId { size, family }`, `HsvaGamma { .. }`), and
//! a watched type renamed (`use egui::FontId as Face`) or aliased (`type Ink = egui::Color32`),
//! which would hide its constants and literals from the scan. A struct pattern, which takes a font
//! apart (`let FontId { size, family } = …`), and the `{` of a body after a return type (`->
//! &egui::FontId {`) or an impl's type are no literals.
//!
//! Every Rust source under `crates/` and `apps/` is read with its strings, characters and comments
//! blanked ([`crate::shell_io::blank`]), so a finding's line is its line in the file. `ux` itself
//! ([`UX`]) is exempt.

use std::path::{Path, PathBuf};

use crate::shell_io::blank;

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

/// A name in blanked source: the name, where it starts and ends (character indexes), its line.
struct Word {
    name: String,
    start: usize,
    end: usize,
    line: usize,
}

/// The names of `chars` (blanked source), each with its place and line; numbers are skipped.
fn words(chars: &[char]) -> Vec<Word> {
    let (mut out, mut line, mut i) = (Vec::new(), 1usize, 0usize);
    while let Some(&c) = chars.get(i) {
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while chars.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                i += 1;
            }
            if !c.is_ascii_digit() {
                out.push(Word { name: chars.get(start..i).unwrap_or_default().iter().collect(), start, end: i, line });
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The first character at or after `i` that is no white space, and its index.
fn next_char(chars: &[char], i: usize) -> Option<(usize, char)> {
    chars.iter().enumerate().skip(i).find(|(_, c)| !c.is_whitespace()).map(|(j, c)| (j, *c))
}

/// Where the path whose last name starts at `i` (`egui::FontId`) starts: over every `::` and the
/// name before it.
fn path_start(chars: &[char], mut i: usize) -> usize {
    loop {
        let Some(at) = chars.get(..i).and_then(|c| c.iter().rposition(|c| !c.is_whitespace())) else { return i };
        if at == 0 || chars.get(at) != Some(&':') || chars.get(at - 1) != Some(&':') {
            return i;
        }
        let Some(name_end) = chars.get(..at - 1).and_then(|c| c.iter().rposition(|c| !c.is_whitespace())).map(|j| j + 1) else { return at - 1 };
        i = chars.get(..name_end).unwrap_or_default().iter().rposition(|c| !is_ident(*c)).map_or(0, |j| j + 1);
    }
}

/// Whether `c` belongs in a name.
fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The name that ends at `end` (exclusive) and where it starts.
fn word_before(chars: &[char], end: usize) -> (String, usize) {
    let start = chars.get(..end).unwrap_or_default().iter().rposition(|c| !is_ident(*c)).map_or(0, |j| j + 1);
    (chars.get(start..end).unwrap_or_default().iter().collect(), start)
}

/// Whether the type whose path starts at `start` is in a place where the `{` after it opens a body,
/// not a literal: a function's return type (`-> &'a mut egui::FontId {`, `Fn() -> FontId {`) or
/// the type an impl is for (`impl Trait for &FontId {`); a for loop's pattern (`for FontId { .. }
/// in`) is no literal either. References, lifetimes and raw pointers in front are passed over.
fn opens_a_body(chars: &[char], start: usize) -> bool {
    let mut i = start;
    loop {
        let Some(at) = chars.get(..i).and_then(|c| c.iter().rposition(|c| !c.is_whitespace())) else { return false };
        match chars.get(at).copied() {
            Some('&' | '*') => i = at,
            Some('>') => return at > 0 && chars.get(at - 1) == Some(&'-'),
            Some(c) if is_ident(c) => {
                let (word, from) = word_before(chars, at + 1);
                if from > 0 && chars.get(from - 1) == Some(&'\'') {
                    i = from - 1;
                } else if word == "mut" || word == "const" {
                    i = from;
                } else {
                    return word == "for" || word == "impl";
                }
            }
            _ => return false,
        }
    }
}

/// The index of the `}` that closes the `{` at `open`; none when it is never closed.
fn closing(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, c) in chars.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// From `i`, past one item of a list (a tuple's or an argument's), to the `,`, the `;` or the
/// closing bracket that ends it at its own depth; none at the end of the text.
fn past_item(chars: &[char], i: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, c) in chars.iter().enumerate().skip(i) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return Some(j),
            ')' | ']' | '}' => depth -= 1,
            ',' | ';' if depth == 0 => return Some(j),
            _ => {}
        }
    }
    None
}

/// Whether the braces from `open` to `close` after a type's name are a struct pattern, which takes
/// a value apart, rather than a literal, which makes one. A body that ends in the rest `..` is a
/// pattern. Otherwise the place decides: past the brackets that close around it and the items
/// beside it in a tuple (`let (FontId { size, family }, b) = …`), a pattern is followed by `=` (a
/// let, an if let, a destructuring assignment), `=>` (a match arm), `:` (a parameter), `|` (a
/// closure's parameter or another alternative), `if` (a match guard) or `in` (a for loop); a
/// literal by anything else. Undecided is a literal, so the check fails closed.
fn is_pattern(chars: &[char], open: usize, close: usize) -> bool {
    let inner = chars.get(open + 1..close).unwrap_or_default().iter().rposition(|c| !c.is_whitespace());
    if let Some(at) = inner
        && at >= 1
        && chars.get(open + 1 + at) == Some(&'.')
        && chars.get(open + at) == Some(&'.')
        && matches!(chars.get(..open + at).and_then(|c| c.iter().rposition(|c| !c.is_whitespace())).and_then(|j| chars.get(j)), Some(',' | '{'))
    {
        return true;
    }
    let mut j = close + 1;
    let (at, c) = loop {
        let Some((at, c)) = next_char(chars, j) else { return false };
        match c {
            ')' | ']' => j = at + 1,
            ',' => match past_item(chars, at + 1) {
                Some(end) => j = end,
                None => return false,
            },
            _ => break (at, c),
        }
    };
    let next = chars.get(at + 1).copied();
    match c {
        '=' => next != Some('='),
        ':' => next != Some(':'),
        '|' => next != Some('|'),
        c if is_ident(c) => {
            let end = chars.iter().skip(at).position(|c| !is_ident(*c)).map_or(chars.len(), |n| at + n);
            let word: String = chars.get(at..end).unwrap_or_default().iter().collect();
            word == "if" || word == "in"
        }
        _ => false,
    }
}

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

/// The watched type the `type` alias named by the word at `k` of `words` stands for: `type Ink =
/// egui::Color32;`, a plain path ending in a watched name. An alias of a type built on one
/// (`Option<Color32>`) has no constants or literal of its own and is none.
fn aliased<'a>(chars: &[char], words: &'a [Word], k: usize) -> Option<&'a str> {
    let alias = words.get(k)?;
    let (mut at, mut c) = next_char(chars, alias.end)?;
    // Generic parameters of the alias, if any.
    if c == '<' {
        let mut depth = 0usize;
        let close = chars.iter().enumerate().skip(at).find_map(|(j, c)| {
            match c {
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                _ => {}
            }
            (depth == 0).then_some(j)
        })?;
        (at, c) = next_char(chars, close + 1)?;
    }
    if c != '=' {
        return None;
    }
    // The path runs to the first character that is no part of one.
    let end =
        chars.iter().enumerate().skip(at + 1).find(|(_, c)| !(c.is_whitespace() || is_ident(**c) || **c == ':')).map_or(chars.len(), |(j, _)| j);
    let last = words.iter().skip(k + 1).take_while(|w| w.end <= end).last()?;
    WATCHED.contains(&last.name.as_str()).then_some(last.name.as_str())
}

/// The colours and fonts made outside ux in one source file's text, each `line N: …`.
pub fn findings(source: &str) -> Vec<String> {
    let chars: Vec<char> = blank(source).chars().collect();
    let words = words(&chars);
    let mut out = Vec::new();
    for (k, w) in words.iter().enumerate() {
        let after = next_char(&chars, w.end);
        let next = words.get(k + 1);
        let follows = |n: &Word| after.map(|(j, _)| j) == Some(n.start);
        if w.name == "type"
            && let Some(name) = aliased(&chars, &words, k + 1)
        {
            let alias = words.get(k + 1).map_or("", |a| a.name.as_str());
            out.push(format!("line {}: type {alias} = {name}, an alias that hides its uses from this check: {}", w.line, instead(name)));
            continue;
        }
        if !WATCHED.contains(&w.name.as_str()) {
            continue;
        }
        // `FontId as Face` in an import, or `<Color32 as From<_>>` in a path.
        if next.is_some_and(|n| n.name == "as" && follows(n)) {
            out.push(format!("line {}: {} renamed, which hides its uses from this check: {}", w.line, w.name, instead(&w.name)));
            continue;
        }
        if w.name == "Color32"
            && let Some((j, _)) = after.filter(|(j, c)| *c == ':' && chars.get(j + 1) == Some(&':'))
        {
            match next_char(&chars, j + 2) {
                // `Color32::RED`.
                Some((m, _)) if next.is_some_and(|n| n.start == m && constant_case(&n.name)) => {
                    let name = next.map_or("", |n| n.name.as_str());
                    out.push(format!("line {}: Color32::{name}, a colour constant: {USE_COLOUR}", w.line));
                }
                // `use egui::Color32::*;` or `use egui::Color32::{RED, WHITE};`.
                Some((_, '*')) => out.push(format!("line {}: Color32::*, which brings its colour constants in: {USE_COLOUR}", w.line)),
                Some((open, '{')) => {
                    let close = closing(&chars, open).unwrap_or(chars.len());
                    for n in words.iter().skip(k + 1).take_while(|n| n.start < close).filter(|n| constant_case(&n.name)) {
                        out.push(format!("line {}: Color32::{}, a colour constant: {USE_COLOUR}", n.line, n.name));
                    }
                }
                _ => {}
            }
            continue;
        }
        if LITERALS.contains(&w.name.as_str())
            && let Some((open, '{')) = after
            && !opens_a_body(&chars, path_start(&chars, w.start))
            && !closing(&chars, open).is_some_and(|close| is_pattern(&chars, open, close))
        {
            out.push(format!("line {}: {} {{ .. }}, built by hand: {}", w.line, w.name, instead(&w.name)));
        }
    }
    out
}

/// Every `.rs` file under `dir`, sorted, leaving out `skip`, build output and hidden folders.
fn sources(dir: &Path, skip: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let path = entry.map_err(|e| format!("{}: {e}", d.display()))?.path();
            let hidden = path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.') || n == "target");
            if path.is_dir() {
                if !hidden && path != skip {
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

    /// Color32's constants brought in by a glob or a group are found, each by its name.
    #[test]
    fn color32_constants_brought_in_are_found() {
        let found = findings("use egui::Color32::*;");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("line 1: Color32::*, which brings its colour constants in: "), "{found:?}");
        let found = findings("use egui::Color32::{\n    RED,\n    WHITE,\n};");
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("line 2: Color32::RED, a colour constant: ") && found[1].starts_with("line 3: Color32::WHITE, "), "{found:?}");
        assert_eq!(findings("use egui::Color32::{self};"), Vec::<String>::new());
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
            "if a == FontId { size, family } {}",
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
