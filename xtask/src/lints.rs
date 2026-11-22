//! The lints check (AR-775): every workspace member takes the workspace's lints.
//!
//! The never-crash deny list (`AGENTS.md`) lives in `[workspace.lints.clippy]` of the root
//! `Cargo.toml`, and reaches a crate only when its own manifest says so. A member without it would
//! pass the lint step with `unwrap()` in its production code, so the lint step runs this check
//! first.
//!
//! The members come from `cargo metadata`, as in the layer check. Each manifest must hold the table
//! `[lints]` with `workspace = true` and nothing else in it, the form every member uses; anything
//! else that sets lints (`workspace = false`, a `[lints.clippy]` table, a `lints` key in the
//! manifest's top-level table) is refused, and so is a manifest with no lints at all. The check
//! reads only what decides this, exactly: TOML's comments and strings, multi-line ones included, so
//! a `[lints]` inside a string or after a `#` never counts. Another spelling cargo would read the
//! same way (`lints.workspace = true`, `lints = { workspace = true }`) is refused and named, never
//! guessed at.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::util;

/// The table a member's manifest holds, as the refusals name it.
const WANTED: &str = "the table `[lints]` with `workspace = true`";

/// Why a manifest does not take the workspace's lints.
#[derive(Debug, PartialEq)]
pub enum Gap {
    /// Nothing in the manifest sets lints.
    Missing,
    /// The manifest sets lints, but not as [`WANTED`] alone; the line (from 1) where it does.
    Other { line: usize },
}

/// Whether the manifest `text` takes the workspace's lints, as [`WANTED`] and nothing else that
/// sets lints.
pub fn check(text: &str) -> Result<(), Gap> {
    // The table the line is in: `None` before the first header, the manifest's top-level table.
    let mut table: Option<String> = None;
    // The line of the `[lints]` header.
    let mut header = None;
    let mut inherits = false;
    for (n, code) in code_lines(text).into_iter().enumerate() {
        let line = n + 1;
        let Some(code) = code else { continue };
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        if code.starts_with('[') {
            let name = key(code.trim_start_matches('[').trim_end_matches(']'));
            if code == "[lints]" && header.is_none() {
                header = Some(line);
            } else if is_lints(&name) {
                return Err(Gap::Other { line });
            }
            table = Some(name);
            continue;
        }
        let in_lints = table.as_deref() == Some("lints");
        if in_lints && !inherits && code.split_whitespace().collect::<String>() == "workspace=true" {
            inherits = true;
        } else if in_lints || (table.is_none() && is_lints(&key(code.split('=').next().unwrap_or_default()))) {
            return Err(Gap::Other { line });
        }
    }
    match header {
        Some(_) if inherits => Ok(()),
        Some(line) => Err(Gap::Other { line }),
        None => Err(Gap::Missing),
    }
}

/// A dotted key with its whitespace and quotes taken out: `"lints" . clippy` reads `lints.clippy`.
fn key(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'').collect()
}

/// Whether a key names the manifest's lints or a part of them.
fn is_lints(key: &str) -> bool {
    key == "lints" || key.starts_with("lints.")
}

/// Where the reader is in TOML's lexical structure.
#[derive(Clone, Copy, PartialEq)]
enum Lex {
    Code,
    Comment,
    Basic,
    Literal,
    MultiBasic,
    MultiLiteral,
}

/// Each line of `text` with its comment taken off, or `None` for a line that starts inside a
/// multi-line string. Strings that start on a line are kept in it, so a quoted key still reads.
fn code_lines(text: &str) -> Vec<Option<String>> {
    let mut lines = vec![Some(String::new())];
    let mut state = Lex::Code;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            state = match state {
                Lex::MultiBasic | Lex::MultiLiteral => state,
                _ => Lex::Code,
            };
            lines.push((state == Lex::Code).then(String::new));
            continue;
        }
        let quote = match state {
            Lex::Code | Lex::Comment => None,
            Lex::Basic | Lex::MultiBasic => Some('"'),
            Lex::Literal | Lex::MultiLiteral => Some('\''),
        };
        match state {
            Lex::Code if c == '#' => state = Lex::Comment,
            Lex::Code if c == '"' || c == '\'' => {
                let run = 1 + eat(&mut chars, c, 2);
                state = match (run, c) {
                    // An empty string: it opens and closes.
                    (2, _) => Lex::Code,
                    (1, '"') => Lex::Basic,
                    (1, _) => Lex::Literal,
                    (_, '"') => Lex::MultiBasic,
                    _ => Lex::MultiLiteral,
                };
                push(&mut lines, &c.to_string().repeat(run));
                continue;
            }
            Lex::Basic | Lex::MultiBasic if c == '\\' => {
                push(&mut lines, "\\");
                // The escaped character, unless it ends the line: a line-ending backslash in a
                // multi-line string still ends the line.
                if let Some(next) = chars.next_if(|n| *n != '\n') {
                    push(&mut lines, &next.to_string());
                }
                continue;
            }
            Lex::Basic | Lex::Literal if Some(c) == quote => state = Lex::Code,
            Lex::MultiBasic | Lex::MultiLiteral if Some(c) == quote => {
                // Three quotes close it; up to two more before them belong to the string.
                let run = 1 + eat(&mut chars, c, 4);
                if run >= 3 {
                    state = Lex::Code;
                }
                push(&mut lines, &c.to_string().repeat(run));
                continue;
            }
            _ => {}
        }
        if state != Lex::Comment {
            push(&mut lines, &c.to_string());
        }
    }
    lines
}

/// Takes up to `most` more of `c` from the front of `chars`; how many it took.
fn eat(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, c: char, most: usize) -> usize {
    let mut took = 0;
    while took < most && chars.next_if_eq(&c).is_some() {
        took += 1;
    }
    took
}

/// Adds `s` to the last line, when that line is read at all.
fn push(lines: &mut [Option<String>], s: &str) {
    if let Some(Some(line)) = lines.last_mut() {
        line.push_str(s);
    }
}

/// The refusal for the manifest at `path` (relative to the workspace root).
fn refusal(path: &Path, gap: &Gap) -> String {
    let why = "the workspace lints hold the never-crash deny list (AGENTS.md), and a crate without them passes the lint step with `unwrap()` in its production code";
    match gap {
        Gap::Missing => format!("{}: takes none of the workspace's lints; add {WANTED}: {why}", path.display()),
        Gap::Other { line } => {
            format!("{}:{line}: sets lints other than by {WANTED} alone; write that table, with nothing else under `lints`: {why}", path.display())
        }
    }
}

/// Every workspace member's manifest, through `cargo metadata`.
fn manifests() -> Result<Vec<PathBuf>, String> {
    let out =
        util::cargo().args(["metadata", "--format-version", "1", "--no-deps", "--locked"]).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let packages = meta["packages"].as_array().ok_or("cargo metadata: no package list")?;
    packages
        .iter()
        .map(|p| {
            p["manifest_path"].as_str().map(PathBuf::from).ok_or_else(|| format!("cargo metadata: a package without a manifest path: {}", p["name"]))
        })
        .collect()
}

/// The check: every member's manifest takes the workspace's lints; each one that does not is named
/// with what to write.
pub fn run() -> Result<(), String> {
    let root = util::root();
    let manifests = manifests()?;
    let mut refused = Vec::new();
    for manifest in &manifests {
        let shown = manifest.strip_prefix(&root).unwrap_or(manifest);
        // cargo has just read the whole manifest, so reading it again is bounded as cargo's was.
        let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", shown.display()))?;
        if let Err(gap) = check(&text) {
            refused.push(refusal(shown, &gap));
        }
    }
    if refused.is_empty() {
        println!("lints: {} members take the workspace's lints", manifests.len());
        Ok(())
    } else {
        for line in &refused {
            eprintln!("lints: {line}");
        }
        Err(format!("{} member(s) without the workspace's lints", refused.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A member's manifest: its package table, then `rest`.
    fn manifest(rest: &str) -> String {
        format!("[package]\nname = \"m\"\nedition.workspace = true\n\n[dependencies]\nlog.workspace = true\n{rest}")
    }

    #[test]
    fn a_member_with_the_table_takes_the_workspaces_lints() {
        assert_eq!(check(&manifest("\n[lints]\nworkspace = true\n")), Ok(()));
        assert_eq!(check(&manifest("[lints] # the workspace's\n  workspace=true   # all of them\n\n[features]\n")), Ok(()));
    }

    #[test]
    fn a_member_without_the_table_is_refused() {
        assert_eq!(check(&manifest("")), Err(Gap::Missing));
        // A `lints` key of another table is not the manifest's lints.
        assert_eq!(check(&manifest("[package.metadata]\nlints.workspace = true\n")), Err(Gap::Missing));
    }

    /// The table set to anything but `workspace = true` alone is refused at the line that does it.
    #[test]
    fn a_member_with_lints_of_its_own_is_refused_at_their_line() {
        assert_eq!(check(&manifest("[lints]\nworkspace = false\n")), Err(Gap::Other { line: 8 }));
        assert_eq!(check(&manifest("[lints]\n")), Err(Gap::Other { line: 7 }));
        assert_eq!(check(&manifest("[lints]\nworkspace = true\nclippy = { unwrap_used = \"allow\" }\n")), Err(Gap::Other { line: 9 }));
        assert_eq!(check(&manifest("[lints]\nworkspace = true\n[lints.clippy]\nunwrap_used = \"allow\"\n")), Err(Gap::Other { line: 9 }));
        assert_eq!(check(&manifest("[lints.rust]\nunsafe_code = \"allow\"\n")), Err(Gap::Other { line: 7 }));
    }

    /// Another spelling of the same table is named, not guessed at.
    #[test]
    fn another_spelling_of_the_table_is_refused() {
        assert_eq!(check("lints.workspace = true\n[package]\nname = \"m\"\n"), Err(Gap::Other { line: 1 }));
        assert_eq!(check("\"lints\" = { workspace = true }\n[package]\n"), Err(Gap::Other { line: 1 }));
        assert_eq!(check(&manifest("[ \"lints\" ]\nworkspace = true\n")), Err(Gap::Other { line: 7 }));
    }

    /// A `[lints]` in a comment or a string, multi-line ones included, is no table.
    #[test]
    fn a_table_in_a_comment_or_a_string_does_not_count() {
        assert_eq!(check(&manifest("# [lints]\n# workspace = true\n")), Err(Gap::Missing));
        let basic = "[package]\ndescription = \"\"\"\n[lints]\nworkspace = true\n\"\"\"\n";
        assert_eq!(check(basic), Err(Gap::Missing));
        let literal = "[package]\ndescription = '''\n[lints]\nworkspace = true\n'''\n";
        assert_eq!(check(literal), Err(Gap::Missing));
        // A quote or an escaped quote inside does not close the string; two extra quotes before
        // the closing three belong to it.
        let inner = "[package]\ndescription = \"\"\"a \\\"\"\" \"\" b\n[lints]\nworkspace = true\n\"\"\"\"\"\n";
        assert_eq!(check(inner), Err(Gap::Missing));
        // A line-ending backslash keeps the string open on the next line.
        let continued = "[package]\ndescription = \"\"\"a \\\n[lints]\nworkspace = true\n\"\"\"\n";
        assert_eq!(check(continued), Err(Gap::Missing));
        // After the string closes, the table counts again; a `#` inside a string is no comment.
        let after = "[package]\ndescription = '''x\n''' # [lints]\nname = \"a # b\"\n[lints]\nworkspace = true\n";
        assert_eq!(check(after), Ok(()));
        // An empty string closes at once.
        assert_eq!(check("[package]\nname = \"\"\n[lints]\nworkspace = true\n"), Ok(()));
    }

    /// The refusal says what to add and why.
    #[test]
    fn the_refusal_names_the_manifest_what_to_write_and_why() {
        let missing = refusal(Path::new("crates/new/Cargo.toml"), &Gap::Missing);
        assert!(
            missing.starts_with("crates/new/Cargo.toml: takes none of the workspace's lints; add the table `[lints]` with `workspace = true`"),
            "{missing}"
        );
        assert!(missing.contains("never-crash deny list"), "{missing}");
        let other = refusal(Path::new("crates/new/Cargo.toml"), &Gap::Other { line: 4 });
        assert!(other.starts_with("crates/new/Cargo.toml:4: sets lints other than by the table `[lints]` with `workspace = true` alone"), "{other}");
    }
}
