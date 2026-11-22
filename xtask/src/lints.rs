//! The lints check (AR-775): every workspace member takes the workspace's lints.
//!
//! The never-crash deny list (`AGENTS.md`) lives in `[workspace.lints.clippy]` of the root
//! `Cargo.toml`, and reaches a crate only when its own manifest says so. Without it, clippy passes
//! `unwrap()` in a member's production code, so the lint step runs this check before clippy.
//!
//! The members come from `cargo metadata`, as in the layer check. Each manifest is read with a TOML
//! parser (the `toml` crate), and its `lints` must be exactly `{ workspace = true }`: the table
//! `[lints]` with `workspace = true` and nothing else in it, however it is written (`lints.workspace
//! = true` and `lints = { workspace = true }` are the same table). Anything else under `lints`
//! (`workspace = false`, an empty table, a `[lints.clippy]` table) is refused at the line of the key
//! that sets it, and so is a manifest with no `lints` at all. A manifest that does not parse is
//! refused with the parser's reason, never passed.

use std::ops::Range;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toml::de::{DeTable, DeValue};

use crate::util;

/// The table a member's manifest holds, as the refusals name it.
const WANTED: &str = "the table `[lints]` with `workspace = true`";

/// Why a manifest does not take the workspace's lints.
#[derive(Debug, PartialEq)]
pub enum Gap {
    /// Nothing in the manifest sets lints.
    Missing,
    /// The manifest sets lints, but not as [`WANTED`] alone; the line (from 1) of the key that does.
    Other { line: usize },
    /// The manifest is not TOML the parser reads: its reason, and the line (from 1) where it applies
    /// when it names one.
    Unreadable { line: Option<usize>, reason: String },
}

/// Whether the manifest `text` takes the workspace's lints: its `lints` is exactly
/// `{ workspace = true }`.
pub fn check(text: &str) -> Result<(), Gap> {
    let line = |span: Range<usize>| line_at(text, span.start);
    let document = DeTable::parse(text).map_err(|e| Gap::Unreadable { line: e.span().map(line), reason: e.message().to_string() })?;
    let Some((key, lints)) = document.get_ref().iter().find(|(k, _)| k.get_ref() == "lints") else {
        return Err(Gap::Missing);
    };
    let DeValue::Table(table) = lints.get_ref() else {
        return Err(Gap::Other { line: line(key.span()) });
    };
    let mut inherits = false;
    for (k, v) in table.iter() {
        if k.get_ref() == "workspace" && matches!(v.get_ref(), DeValue::Boolean(true)) {
            inherits = true;
        } else {
            return Err(Gap::Other { line: line(k.span()) });
        }
    }
    if inherits { Ok(()) } else { Err(Gap::Other { line: line(key.span()) }) }
}

/// The line (from 1) of byte `offset` in `text`.
fn line_at(text: &str, offset: usize) -> usize {
    text.bytes().take(offset).filter(|b| *b == b'\n').count() + 1
}

/// The refusal for the manifest at `path` (relative to the workspace root).
fn refusal(path: &Path, gap: &Gap) -> String {
    let why =
        "the workspace lints hold the never-crash deny list (AGENTS.md), and without them clippy passes `unwrap()` in a crate's production code";
    match gap {
        Gap::Missing => format!("{}: takes none of the workspace's lints; add {WANTED}: {why}", path.display()),
        Gap::Other { line } => {
            format!("{}:{line}: sets lints other than by {WANTED} alone; write that table, with nothing else under `lints`: {why}", path.display())
        }
        Gap::Unreadable { line, reason } => {
            let at = line.map(|l| format!(":{l}")).unwrap_or_default();
            format!("{}{at}: cannot be read as TOML, so whether it takes the workspace's lints is unknown: {reason}", path.display())
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

    /// The table, with comments and any spacing around its key, is what a member holds.
    #[test]
    fn a_member_with_the_table_takes_the_workspaces_lints() {
        assert_eq!(check(&manifest("\n[lints]\nworkspace = true\n")), Ok(()));
        assert_eq!(check(&manifest("[lints] # the workspace's\n  workspace=true   # all of them\n\n[features]\n")), Ok(()));
    }

    /// No lints, or a `lints` key of another table, is no table.
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
        assert_eq!(check(&manifest("[lints]\nworkspaces = true\n")), Err(Gap::Other { line: 8 }));
    }

    /// A value that looks like a header (an array of arrays across lines) is a value, not a table
    /// (CodeRabbit on #577); a real `[lints.clippy]` table is still refused.
    #[test]
    fn an_array_element_is_no_table_header() {
        let groups = "[package.metadata]\ngroups = [\n  [\"lints\"],\n  [ 'lints.clippy' ]\n]\n";
        assert_eq!(check(&manifest(&format!("[lints]\nworkspace = true\n{groups}"))), Ok(()));
        assert_eq!(check(&manifest(&format!("{groups}[lints]\nworkspace = true\n"))), Ok(()));
        assert_eq!(
            check(&manifest(&format!("[lints]\nworkspace = true\n{groups}[lints.clippy]\nunwrap_used = \"allow\"\n"))),
            Err(Gap::Other { line: 14 })
        );
    }

    /// The same table written another way is the same table; a `lints` that is no table, or a
    /// `workspace` that is not the boolean `true`, is refused at its key.
    #[test]
    fn another_spelling_of_the_table_is_the_same_table() {
        assert_eq!(check("lints.workspace = true\n[package]\nname = \"m\"\n"), Ok(()));
        assert_eq!(check("\"lints\" = { workspace = true }\n[package]\n"), Ok(()));
        assert_eq!(check(&manifest("[ \"lints\" ]\n'workspace' = true\n")), Ok(()));
        assert_eq!(check("[package]\nname = \"m\"\n\n[lints]\nworkspace = \"true\"\n"), Err(Gap::Other { line: 5 }));
        assert_eq!(check("lints = { workspace = false }\n"), Err(Gap::Other { line: 1 }));
        assert_eq!(check("[package]\nlints = true\n"), Err(Gap::Missing), "a key of another table");
        assert_eq!(check("lints = true\n"), Err(Gap::Other { line: 1 }));
    }

    /// A manifest the parser cannot read is refused with its reason and line, never passed.
    #[test]
    fn a_manifest_that_does_not_parse_is_refused_with_the_reason() {
        let Err(Gap::Unreadable { line, reason }) = check("[package]\nname = \"m\"\n[lints\nworkspace = true\n") else {
            panic!("an unclosed header parses");
        };
        assert_eq!(line, Some(3));
        assert!(!reason.is_empty());
        let shown = refusal(Path::new("crates/new/Cargo.toml"), &Gap::Unreadable { line: Some(3), reason: "the reason".into() });
        assert_eq!(shown, "crates/new/Cargo.toml:3: cannot be read as TOML, so whether it takes the workspace's lints is unknown: the reason");
        let shown = refusal(Path::new("crates/new/Cargo.toml"), &Gap::Unreadable { line: None, reason: "the reason".into() });
        assert!(shown.starts_with("crates/new/Cargo.toml: cannot be read"), "{shown}");
    }

    /// A `[lints]` in a comment or a string, multi-line ones included, is no table.
    #[test]
    fn a_table_in_a_comment_or_a_string_does_not_count() {
        assert_eq!(check(&manifest("# [lints]\n# workspace = true\n")), Err(Gap::Missing));
        // A comment in the table sets nothing.
        assert_eq!(check(&manifest("[lints]\n# clippy = { unwrap_used = \"allow\" }\nworkspace = true # [lints.clippy]\n")), Ok(()));
        let basic = "[package]\ndescription = \"\"\"\n[lints]\nworkspace = true\n\"\"\"\n";
        assert_eq!(check(basic), Err(Gap::Missing));
        let literal = "[package]\ndescription = '''\n[lints]\nworkspace = true\n'''\n";
        assert_eq!(check(literal), Err(Gap::Missing));
        // A quote or an escaped quote inside does not close the string.
        let inner = "[package]\ndescription = \"\"\"a \\\"\"\" \"\" b\n[lints]\nworkspace = true\n\"\"\"\"\"\n";
        assert_eq!(check(inner), Err(Gap::Missing));
        // A line-ending backslash keeps the string open on the next line, and the lines after it
        // are counted.
        let continued = "[package]\ndescription = \"\"\"a \\\n[lints]\nworkspace = true\n\"\"\"\n";
        assert_eq!(check(continued), Err(Gap::Missing));
        assert_eq!(check(&format!("{continued}[lints]\nworkspace = false\n")), Err(Gap::Other { line: 7 }));
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
