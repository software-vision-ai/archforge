//! Shared helpers: the workspace root, running tools, a small TOML reader, file walking.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace root: the parent of this crate's manifest directory, wherever the target
/// directory is.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// The cargo that runs this task runner.
pub fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    c.current_dir(root());
    c
}

/// Runs a command, inheriting the terminal; Err when it fails or cannot start.
pub fn run(mut cmd: Command) -> Result<(), String> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().map_err(|e| format!("{shown}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{shown} exited with {status}")) }
}

/// One table of a TOML file: its name, whether it is an array entry, and its key-value pairs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub name: String,
    pub array: bool,
    pub values: BTreeMap<String, String>,
}

/// Reads the subset of TOML these files use: comments, `[table]`, `[[array]]` and
/// `key = value` with string, number or boolean values. Anything else is an error naming the line.
pub fn toml(text: &str) -> Result<Vec<Table>, String> {
    let mut tables = vec![Table::default()];
    for (n, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
            tables.push(Table { name: name.trim().to_string(), array: true, values: BTreeMap::new() });
        } else if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            tables.push(Table { name: name.trim().to_string(), array: false, values: BTreeMap::new() });
        } else if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let value = if let Some(s) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                s.replace("\\\"", "\"").replace("\\\\", "\\")
            } else {
                v.to_string()
            };
            if let Some(t) = tables.last_mut() {
                t.values.insert(k.trim().trim_matches('"').to_string(), value);
            }
        } else {
            return Err(format!("line {}: cannot read {raw:?}", n + 1));
        }
    }
    Ok(tables)
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Every regular file under `dir`, sorted, as paths relative to the workspace root. Symbolic links
/// are not followed, so a link cycle cannot make the walk endless.
pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let p = e.path();
            if kind.is_dir() {
                stack.push(p);
            } else if kind.is_file() {
                out.push(p.strip_prefix(root()).map(Path::to_path_buf).unwrap_or(p));
            }
        }
    }
    out.sort();
    out
}

/// The files git tracks, or every file under the root when git is unavailable.
pub fn tracked() -> Vec<PathBuf> {
    let out = Command::new("git").args(["ls-files", "--cached", "--others", "--exclude-standard"]).current_dir(root()).output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().map(PathBuf::from).collect(),
        _ => walk(&root()).into_iter().filter(|p| !p.starts_with("target") && !p.starts_with(".git")).collect(),
    }
}

/// The variable that names the trademark list: names the project must not use, from a list
/// maintainers keep outside the repository.
pub const BRANDS_VAR: &str = "FORGE_BRANDS";
/// Set to `1` where the trademark list must be there, so a missing list fails the gate.
pub const BRANDS_REQUIRED_VAR: &str = "FORGE_BRANDS_REQUIRED";

/// The trademark list's path: the file `FORGE_BRANDS` names, when that file is there. Without the
/// list the trademark check is skipped.
pub fn brand_list_path() -> Option<PathBuf> {
    let path = root().join(std::env::var_os(BRANDS_VAR).filter(|v| !v.is_empty())?);
    path.is_file().then_some(path)
}

/// Whether the trademark list must be there (`FORGE_BRANDS_REQUIRED=1`).
pub fn brand_list_required() -> bool {
    std::env::var(BRANDS_REQUIRED_VAR).is_ok_and(|v| v == "1")
}

/// Lines of the trademark list, lower-cased, without comments. A list that is named but cannot be read
/// is an error, so the check never passes for want of its list.
pub fn brands() -> Result<Vec<String>, String> {
    let path = brand_list_path().ok_or_else(|| format!("no trademark list to check against: {BRANDS_VAR} names no file that is here"))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("the trademark list {}: {e}", path.display()))?;
    Ok(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_lowercase).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toml_reader_reads_tables_arrays_and_comments() {
        let t = toml("a = \"x # not a comment\" # comment\n[t]\nb = 2\n[[row]]\nc = true\n[[row]]\nc = false\n").unwrap();
        assert_eq!(t[0].values["a"], "x # not a comment");
        assert_eq!(t[1].name, "t");
        assert_eq!(t.iter().filter(|x| x.array && x.name == "row").count(), 2);
        assert!(toml("nonsense").is_err());
    }
}
