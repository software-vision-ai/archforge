//! The ux part of the layer check: the interface draws through `ux` (AGENTS.md), so a colour or a
//! font is made there alone. Clippy refuses egui's colour and font constructors everywhere else
//! (`disallowed-methods` and `disallowed-types` in `clippy.toml`); this scan refuses what clippy
//! cannot see: a colour constant (`Color32::RED`), a font or a colour built as a struct literal
//! (`FontId { size, family }`, `HsvaGamma { .. }`), and `Color32` renamed, which would hide its
//! constants from the scan.
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

/// The last character before the path whose last name starts at `i` (`egui::FontId`), white space
/// left out.
fn before_path(chars: &[char], mut i: usize) -> Option<char> {
    loop {
        let at = chars.get(..i)?.iter().rposition(|c| !c.is_whitespace())?;
        if chars.get(at) != Some(&':') || at == 0 || chars.get(at - 1) != Some(&':') {
            return chars.get(at).copied();
        }
        // Over the `::` and the name before it.
        let name_end = chars.get(..at - 1)?.iter().rposition(|c| !c.is_whitespace())? + 1;
        i = chars.get(..name_end)?.iter().rposition(|c| !(c.is_alphanumeric() || *c == '_')).map_or(0, |j| j + 1);
    }
}

/// Whether `name` is written as a constant: capitals, digits and underscores, a capital first.
fn constant_case(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// The colours and fonts made outside ux in one source file's text, each `line N: …`.
pub fn findings(source: &str) -> Vec<String> {
    let chars: Vec<char> = blank(source).chars().collect();
    let words = words(&chars);
    let mut out = Vec::new();
    for (k, w) in words.iter().enumerate() {
        let after = next_char(&chars, w.end);
        let next = words.get(k + 1);
        if w.name == "Color32" {
            // Where the name after `Color32::` starts, when a path follows.
            let member =
                after.filter(|(j, c)| *c == ':' && chars.get(j + 1) == Some(&':')).and_then(|(j, _)| next_char(&chars, j + 2)).map(|(j, _)| j);
            match next {
                Some(n) if member == Some(n.start) && constant_case(&n.name) => {
                    out.push(format!("line {}: Color32::{}, a colour constant: {USE_COLOUR}", w.line, n.name));
                }
                // `Color32 as C` in an import, or `<Color32 as From<_>>` in a path.
                Some(n) if n.name == "as" && after.map(|(j, _)| j) == Some(n.start) => {
                    out.push(format!("line {}: Color32 renamed, which hides its constants from this check: {USE_COLOUR}", w.line));
                }
                _ => {}
            }
        } else if LITERALS.contains(&w.name.as_str()) && after.is_some_and(|(_, c)| c == '{') && before_path(&chars, w.start) != Some('>') {
            // `-> FontId {` opens a function's body, not a literal.
            let instead = if w.name == "FontId" { USE_FONT } else { USE_DATA };
            out.push(format!("line {}: {} {{ .. }}, built by hand: {instead}", w.line, w.name));
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

    /// Renaming `Color32` would hide its constants, so it is a finding; `Color32::as_ref` is not.
    #[test]
    fn a_renamed_color32_is_found() {
        let found = findings("use egui::Color32 as C;\nlet x = C::RED;");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("line 1: Color32 renamed, which hides its constants from this check"), "{found:?}");
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
