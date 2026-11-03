//! The version tool: the version lives in one place, `[workspace.package]` in the
//! workspace manifest. `cargo xtask version` prints it; `cargo xtask version <x.y.z>` (or a pre-release, `<x.y.z>-rc.1`) sets it and
//! refreshes the lockfile.

use crate::util;

fn manifest() -> Result<String, String> {
    std::fs::read_to_string(util::root().join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))
}

/// The line index and value of the workspace version.
fn find(text: &str) -> Option<(usize, String)> {
    let mut in_package = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_package = t == "[workspace.package]";
        } else if in_package && let Some(v) = t.strip_prefix("version") {
            let v = v.trim().strip_prefix('=')?.trim().trim_matches('"');
            return Some((i, v.to_string()));
        }
    }
    None
}

/// `x.y.z`, or a pre-release of it, `x.y.z-rc.1`: the suffix is dot-separated letters, digits and
/// hyphens, as semantic versioning has it.
pub fn valid(v: &str) -> bool {
    let (numbers, pre) = match v.split_once('-') {
        Some((n, p)) => (n, Some(p)),
        None => (v, None),
    };
    let parts: Vec<&str> = numbers.split('.').collect();
    let numbers_ok =
        parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && (p.len() == 1 || !p.starts_with('0')));
    let pre_ok = pre.is_none_or(|p| p.split('.').all(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')));
    numbers_ok && pre_ok
}

pub fn current() -> Result<String, String> {
    find(&manifest()?).map(|(_, v)| v).ok_or_else(|| "no version in [workspace.package]".to_string())
}

pub fn run(new: Option<&str>) -> Result<(), String> {
    let Some(new) = new else {
        println!("{}", current()?);
        return Ok(());
    };
    if !valid(new) {
        return Err(format!("{new:?} is not a version of the form x.y.z or x.y.z-rc.1"));
    }
    let text = manifest()?;
    let (line, old) = find(&text).ok_or("no version in [workspace.package]")?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if let Some(l) = lines.get_mut(line) {
        *l = format!("version = \"{new}\"");
    }
    let mut out = lines.join("\n");
    out.push('\n');
    std::fs::write(util::root().join("Cargo.toml"), out).map_err(|e| format!("Cargo.toml: {e}"))?;
    let mut cmd = util::cargo();
    cmd.args(["update", "--workspace", "--offline"]);
    util::run(cmd)?;
    println!("version: {old} -> {new}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_read_from_the_workspace_package_table_only() {
        let text = "[package]\nversion = \"9.9.9\"\n[workspace.package]\nversion = \"1.2.3\"\n";
        assert_eq!(find(text), Some((3, "1.2.3".to_string())));
        assert!(valid("1.2.3") && !valid("1.2") && !valid("01.2.3") && !valid("1.2.x"));
        // A pre-release: dot-separated letters, digits and hyphens after the dash.
        assert!(valid("0.2.0-rc.1") && valid("1.0.0-beta") && !valid("0.2.0-") && !valid("0.2.0-rc..1") && !valid("0.2.0-rc_1"));
    }
}
