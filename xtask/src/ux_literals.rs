//! The ux part of the layer check: the interface draws through `ux` (AGENTS.md), so a colour or a
//! font is made there alone. The compiler holds that rule: `clippy.toml` bans egui's colour and
//! font types and their constructors outside `ux` (`disallowed-types`, `disallowed-methods`), so
//! naming one there, its constants, a renamed import of it and an `impl` for it included, fails the
//! lint step. The ban does not see a struct expression or an enum variant's path, through which a
//! type with public fields or variants is built (`FontId { size, family }`, `HexColor::Hex6 { .. }`),
//! even by a macro that brings the braces from elsewhere. So outside `ux` this check refuses the
//! name of every type `clippy.toml` bans ([`banned`]: the last segment of each path, read from the
//! file at check time, so the two never drift), with the reason the file gives. Naming one is the
//! lint's error anyway, so the check refuses nothing the lint allows.
//!
//! Every Rust source under `crates/` and `apps/`, tests included, is read as Rust tokens
//! (`proc-macro2`): every identifier is seen, in code, in a macro's input, in a `macro_rules!`
//! matcher and transcriber and in an attribute's arguments alike, and none is resolved. A string
//! is one literal token and a comment no token (a doc comment is a string attribute), so neither
//! is read. A source that is not Rust tokens is a finding. A finding's line is its line in the
//! file. `ux` itself ([`UX`]) is exempt.
//!
//! The walk is strict ([`util::walk_strict_all`]): `crates` and `apps` are judged by their own
//! kind, never through a link; a folder that cannot be opened stops the check; a link or an entry
//! that is no plain file is a finding, never followed or read; and a source over a byte cap, its
//! own or the total's, is a finding, known from its size before it is read.

use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use toml::de::{DeTable, DeValue};

use crate::util;

/// The `ux` module, from the repository's root: where colours and fonts are made.
pub const UX: [&str; 4] = ["crates", "ui-egui", "src", "ux"];

/// The folders, from the root, whose sources are scanned.
const SCANNED: [&str; 2] = ["crates", "apps"];

/// A type `clippy.toml` bans: the last segment of its path, as code writes it, and the reason the
/// file gives for the ban.
#[derive(Debug, Clone, PartialEq)]
pub struct Banned {
    pub name: String,
    pub reason: String,
}

/// The types the `clippy.toml` text `text` bans (`disallowed-types`), each by the last segment of
/// its path, with its reason. An entry is a path, or a table with `path` and an optional `reason`.
/// A text that is no TOML, has no `disallowed-types` array, bans no type or holds an entry without
/// a path is refused, so the check never runs knowing fewer names than the lint bans.
pub fn banned(text: &str) -> Result<Vec<Banned>, String> {
    let document = DeTable::parse(text).map_err(|e| format!("clippy.toml: not read as TOML ({}), so the banned types are unknown", e.message()))?;
    let Some((_, list)) = document.get_ref().iter().find(|(k, _)| k.get_ref() == "disallowed-types") else {
        return Err("clippy.toml: no disallowed-types, so no banned type is known".to_string());
    };
    let DeValue::Array(list) = list.get_ref() else {
        return Err("clippy.toml: disallowed-types is no list, so the banned types are unknown".to_string());
    };
    let mut out = Vec::new();
    for entry in list {
        let (path, reason) = match entry.get_ref() {
            DeValue::String(path) => (Some(path.as_ref()), None),
            DeValue::Table(t) => {
                let field = |name: &str| t.iter().find(|(k, _)| k.get_ref() == name).and_then(|(_, v)| v.get_ref().as_str());
                (field("path"), field("reason"))
            }
            _ => (None, None),
        };
        let name = path.and_then(|p| p.rsplit("::").next()).filter(|n| !n.is_empty());
        let Some(name) = name else {
            return Err("clippy.toml: a disallowed-types entry names no path, so the banned types are unknown".to_string());
        };
        out.push(Banned { name: name.to_string(), reason: reason.unwrap_or("banned in clippy.toml").to_string() });
    }
    if out.is_empty() {
        return Err("clippy.toml: disallowed-types bans no type, so no banned type is known".to_string());
    }
    Ok(out)
}

/// Adds to `found` every name in `tokens` that `banned` holds, at any depth, with its line.
fn names(tokens: TokenStream, banned: &[Banned], found: &mut Vec<(usize, String)>) {
    for tree in tokens {
        match tree {
            TokenTree::Group(g) => names(g.stream(), banned, found),
            TokenTree::Ident(ident) => {
                let written = ident.to_string();
                let name = written.strip_prefix("r#").unwrap_or(&written);
                if let Some(b) = banned.iter().find(|b| b.name == name) {
                    found.push((ident.span().start().line, format!("{} named outside ux: {}", b.name, b.reason)));
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

/// The banned names one source file's text writes as code, each `line N: …`, in the order of their
/// lines. A text that is not Rust tokens is a finding, so the check fails closed.
pub fn findings(source: &str, banned: &[Banned]) -> Vec<String> {
    let mut found = Vec::new();
    match source.parse::<TokenStream>() {
        Ok(tokens) => names(tokens, banned, &mut found),
        Err(e) => found.push((e.span().start().line, format!("not read as Rust ({e}), so this check cannot see into it"))),
    }
    found.sort_by_key(|(line, _)| *line);
    found.into_iter().map(|(line, what)| format!("line {line}: {what}")).collect()
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

/// The types `root`'s `clippy.toml` bans ([`banned`]) named outside ux in every source under its
/// crates and applications, each named with its file from `root`; a `clippy.toml` that cannot be
/// read, or bans nothing, stops the check. What this check cannot read safely is a finding too: a
/// root (`crates`, `apps`) that is a link or no folder, a link or an entry that is no plain file
/// under them, a source over [`MAX_FILE`] bytes, and one past [`MAX_TOTAL`] in all. A root that is
/// missing or cannot be read, and a folder under one that cannot be opened ([`util::walk_strict`]),
/// stop the check with their paths, so no part of the scope passes unread.
pub fn scan(root: &Path) -> Result<Vec<String>, String> {
    let banned = rules_of(root, MAX_FILE)?;
    scan_within(root, &banned, MAX_FILE, MAX_TOTAL, |p| std::fs::symlink_metadata(p).map(|m| m.len())).map(|(found, _)| found)
}

/// The types `root`'s `clippy.toml` bans ([`banned`]), the file read up to `cap` bytes: a larger
/// one, one that cannot be read and one that is no text stop the check.
fn rules_of(root: &Path, cap: u64) -> Result<Vec<Banned>, String> {
    let bytes = read_capped(&root.join("clippy.toml"), cap)?;
    if bytes.len() as u64 > cap {
        return Err(format!("clippy.toml: more than {cap} bytes, so the banned types are not read"));
    }
    banned(&String::from_utf8(bytes).map_err(|e| format!("clippy.toml: {e}"))?)
}

/// [`scan`] with the banned types and its caps given, and a source's size in bytes from `size_of`, so a test can give
/// one that grows after it was measured; also how many bytes were read in all.
fn scan_within(
    root: &Path,
    banned: &[Banned],
    max_file: u64,
    max_total: u64,
    size_of: impl Fn(&Path) -> std::io::Result<u64>,
) -> Result<(Vec<String>, u64), String> {
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
        out.extend(findings(&text, banned).into_iter().map(|f| format!("{} {f}", rel(&file))));
    }
    Ok((out, read))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The types the repository's own `clippy.toml` bans.
    fn rules() -> Vec<Banned> {
        banned(&std::fs::read_to_string(crate::util::root().join("clippy.toml")).unwrap()).unwrap()
    }

    /// Puts the repository's own `clippy.toml` at the root of the scratch tree `s`.
    fn with_rules(s: &crate::util::Scratch) {
        s.file("clippy.toml", &std::fs::read_to_string(crate::util::root().join("clippy.toml")).unwrap()).unwrap();
    }

    /// A watched name written as code is found with its line, wherever it stands (a literal, a
    /// type, a pattern, a path), a raw identifier by its name; each finding says what to use.
    #[test]
    fn a_watched_name_in_code_is_found_with_its_line() {
        let source = "fn f(font: FontId) -> f32 {\n    let b = egui::ecolor::HsvaGamma { h, s, v, a: 1.0 };\n    let c = r#Hsva::new(h, s, v, a);\n    let FontId { size, .. } = font;\n    size\n}\n";
        let found = findings(source, &rules());
        let heads: Vec<String> = found.iter().map(|f| f.split(": ").take(2).collect::<Vec<_>>().join(": ")).collect();
        assert_eq!(
            heads,
            [
                "line 1: FontId named outside ux",
                "line 2: HsvaGamma named outside ux",
                "line 3: Hsva named outside ux",
                "line 4: FontId named outside ux"
            ],
            "{found:?}"
        );
        assert!(found[0].contains("ux::types") && found[1].contains("ux::from_hsv"), "{found:?}");
    }

    /// Only the watched names themselves are found: another type's struct expression or pattern,
    /// a name that holds a watched one, a string, a comment and a doc comment are not.
    #[test]
    fn other_names_strings_and_comments_are_not() {
        let source = "/// Not a FontId { size } of its own.\nfn f(s: Stroke) -> Stroke {\n    let Stroke { width, .. } = s;\n    let t = MyFontId { size: width };\n    let u = \"FontId { size }\";\n    // FontId { size }\n    /* Hsva { h } */\n    Stroke { width, color: t.colour(u) }\n}\n";
        assert_eq!(findings(source, &rules()), Vec::<String>::new());
    }

    /// A macro's input and an attribute's arguments are tokens too: a watched name there is found
    /// at any depth, whatever follows it, and a string there is not.
    #[test]
    fn macro_input_and_attribute_arguments_are_read_as_tokens() {
        let source = "#[case(egui::FontId { size: 1.0, family })]\n#[cfg_attr(test, case((Hsva::new(h, s, v, a))))]\n#[doc = text!(FontId)]\nfn f() {\n    let v = vec![[FontId::default()]];\n    m!((x, [HsvaGamma]));\n    m!(\"FontId { }\" Font { });\n}\n";
        let found = findings(source, &rules());
        let lines: Vec<&str> = found.iter().map(|f| f.split(" named").next().unwrap_or_default()).collect();
        let expected = ["line 1: FontId", "line 2: Hsva", "line 3: FontId", "line 5: FontId", "line 6: HsvaGamma"];
        assert_eq!(lines, expected, "{found:?}");
    }

    /// A watched name in a macro transcriber is a finding even when a metavariable brings the
    /// braces, so the expansion is a struct literal no source shows (CodeRabbit on #573).
    #[test]
    fn a_transcriber_naming_a_watched_type_is_found_whatever_brings_its_fields() {
        let source = "macro_rules! face {\n    ($fields:tt) => { egui::FontId $fields };\n}\nfn f() {\n    let _ = face!({\n        size: 12.0,\n        family: ux::regular(),\n    });\n}\n";
        let found = findings(source, &rules());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("line 2: FontId named outside ux: ") && found[0].contains("ux::types"), "{found:?}");
    }

    /// Every type `clippy.toml` bans is watched: `HexColor` too, whose public tuple variants make
    /// a colour through a path the type ban does not see (CodeRabbit on #573).
    #[test]
    fn a_banned_type_built_through_its_variant_is_found() {
        let found = findings("fn f() {\n    let c = egui::ecolor::HexColor::Hex6 { 0: ux::CLEAR.into() };\n}\n", &rules());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("line 2: HexColor named outside ux: ") && found[0].contains("ux::data_colour"), "{found:?}");
    }

    /// The banned types are `clippy.toml`'s own: each entry's last path segment with its reason,
    /// an entry written as a path alone too; other keys are not read. The repository's file bans
    /// egui's colour and font types.
    #[test]
    fn the_banned_types_are_read_from_clippy_toml_with_their_reasons() {
        let text = "allow-unwrap-in-tests = true\ndisallowed-methods = [{ path = \"egui::Color32::from_rgb\", reason = \"m\" }]\ndisallowed-types = [\n    { path = \"egui::Galley\", reason = \"a laid-out text is ux's\" },\n    \"egui::epaint::Mesh\",\n]\n";
        let expected = [
            Banned { name: "Galley".into(), reason: "a laid-out text is ux's".into() },
            Banned { name: "Mesh".into(), reason: "banned in clippy.toml".into() },
        ];
        assert_eq!(banned(text), Ok(expected.to_vec()));
        let names: Vec<String> = rules().into_iter().map(|b| b.name).collect();
        for name in ["Color32", "FontId", "Hsva", "HsvaGamma", "Rgba", "HexColor"] {
            assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
        }
    }

    /// A type added to `clippy.toml` is watched from then on, with the file's reason, and one the
    /// file does not ban is not.
    #[test]
    fn a_type_added_to_clippy_toml_is_watched() {
        let s = crate::util::Scratch::new("ux-added").unwrap();
        std::fs::create_dir(s.path.join("apps")).unwrap();
        s.file("clippy.toml", "disallowed-types = [{ path = \"egui::Galley\", reason = \"a laid-out text is ux's\" }]\n").unwrap();
        s.file("crates/a/src/lib.rs", "fn f(g: egui::Galley, font: egui::FontId) {}\n").unwrap();
        assert_eq!(scan(&s.path), Ok(vec!["crates/a/src/lib.rs line 1: Galley named outside ux: a laid-out text is ux's".to_string()]));
    }

    /// A `clippy.toml` that is missing, too large, no text or no TOML, bans no type or holds an
    /// entry without a path stops the check: it never runs knowing fewer names than the lint bans.
    #[test]
    fn a_clippy_toml_that_names_no_banned_type_stops_the_check() {
        let s = crate::util::Scratch::new("ux-no-rules").unwrap();
        s.file("crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        std::fs::create_dir(s.path.join("apps")).unwrap();
        let missing = scan(&s.path);
        assert!(missing.as_ref().is_err_and(|e| e.contains("clippy.toml")), "{missing:?}");
        for text in [
            "disallowed-types = [",
            "allow-unwrap-in-tests = true\n",
            "disallowed-types = \"egui::Color32\"\n",
            "disallowed-types = []\n",
            "disallowed-types = [{ reason = \"r\" }]\n",
            "disallowed-types = [{ path = 3 }]\n",
            "disallowed-types = [\"egui::\"]\n",
            "disallowed-types = [7]\n",
            "disallowed-types = [\"egui::Color32\", { reason = \"r\" }]\n",
        ] {
            s.file("clippy.toml", text).unwrap();
            let refused = scan(&s.path);
            assert!(refused.as_ref().is_err_and(|e| e.starts_with("clippy.toml: ")), "{text:?}: {refused:?}");
        }
        s.file("clippy.toml", "disallowed-types = [\"egui::Color32\"]\n").unwrap();
        assert_eq!(rules_of(&s.path, 37).map(|b| b.len()), Ok(1), "the file's 37 bytes fit a cap of 37");
        assert_eq!(rules_of(&s.path, 36), Err("clippy.toml: more than 36 bytes, so the banned types are not read".to_string()));
        // TOML apart from one byte that is no UTF-8, inside a string a lossy reading would accept.
        let mut bytes = b"disallowed-types = [{ path = \"egui::Color32\", reason = \"".to_vec();
        bytes.extend([0xff, b'"', b' ', b'}', b']', b'\n']);
        std::fs::write(s.path.join("clippy.toml"), &bytes).unwrap();
        assert!(rules_of(&s.path, 100).is_err_and(|e| e.starts_with("clippy.toml: ")), "no text");
    }

    /// A source that is not Rust is a finding, so nothing passes unread.
    #[test]
    fn a_source_not_read_as_rust_is_a_finding() {
        let found = findings("fn f( {\n", &rules());
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
        with_rules(&s);
        let outside = crate::util::Scratch::new("ux-roots-outside").unwrap();
        outside.file("a/src/lib.rs", "fn f() { egui::FontId { size: 1.0, family }; }\n").unwrap();
        std::os::unix::fs::symlink(&outside.path, s.path.join("crates")).unwrap();
        std::os::unix::fs::symlink(s.path.join("gone"), s.path.join("apps")).unwrap();
        assert_eq!(
            scan(&s.path),
            Ok(vec!["crates: a link, which this check does not follow".to_string(), "apps: a link, which this check does not follow".to_string()])
        );
        let s = crate::util::Scratch::new("ux-roots-file").unwrap();
        with_rules(&s);
        s.file("crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        s.file("apps", "").unwrap();
        assert_eq!(scan(&s.path), Ok(vec!["apps: no folder, so this check reads nothing there".to_string()]));
        let s = crate::util::Scratch::new("ux-roots-missing").unwrap();
        with_rules(&s);
        s.file("crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        let missing = scan(&s.path);
        assert!(missing.as_ref().is_err_and(|e| e.starts_with(&format!("{}: ", s.path.join("apps").display()))), "{missing:?}");
        let s = crate::util::Scratch::new("ux-roots-locked").unwrap();
        s.file("r/crates/a/src/lib.rs", "fn f() {}\n").unwrap();
        s.file("r/clippy.toml", &std::fs::read_to_string(crate::util::root().join("clippy.toml")).unwrap()).unwrap();
        s.file("r/apps/b/src/main.rs", "fn main() {}\n").unwrap();
        if s.lock("r/crates").unwrap() {
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
        with_rules(&s);
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
        let (found, read) = scan_within(&s.path, &rules(), 100, 150, size).unwrap();
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
        let (found, read) = scan_within(&s.path, &rules(), 100, 150, lying).unwrap();
        assert_eq!(found.len(), 4, "{found:?}");
        assert_eq!(found[0], format!("crates/a/a.rs: {more}"));
        // a.rs took 101 bytes of the 150, so b.rs (60) is past the total; c.rs (33) still fits.
        assert_eq!(found[1], format!("crates/a/b.rs: {past}"));
        assert!(found[2].starts_with("crates/a/c.rs line 1: FontId named outside ux"), "{found:?}");
        // 16 bytes are left for d.rs, which holds 40: 17 are read and put aside.
        assert_eq!(found[3], format!("crates/a/d.rs: {past}"));
        assert_eq!(read, 101 + 33 + 17);
    }

    /// `ux` is left out by its path, where its fonts are made, and the same literal anywhere else
    /// is found.
    #[test]
    fn ux_is_left_out_and_nothing_else_is() {
        let s = crate::util::Scratch::new("ux-exempt").unwrap();
        with_rules(&s);
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
