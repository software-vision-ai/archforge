//! The skills check.
//!
//! Every folder under `skills/` is one skill: its `SKILL.md` opens with a header whose `name` is the
//! folder's and whose `description` is not empty. The table in `skills/README.md` and the routing
//! table in `AGENTS.md` list exactly those folders. In those files and every `SKILL.md`, each skill
//! named (`skills/<name>/SKILL.md`) is there, each `cargo xtask` verb named is one the task runner
//! knows, and each path in backticks whose first part is at the root of the repository is there.
//! Paths under the ignored `plan/` folder are output, not checked.

use std::collections::BTreeSet;

use crate::util;

const DIR: &str = "skills";
const README: &str = "skills/README.md";

pub fn run() -> Result<(), String> {
    let root = util::root();
    let mut folders = Vec::new();
    let entries = std::fs::read_dir(root.join(DIR)).map_err(|e| format!("{DIR}/: {e}"))?;
    for e in entries.flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            folders.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    folders.sort();
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).map_err(|e| format!("{rel}: {e}"));
    let mut files = vec![("AGENTS.md".to_string(), read("AGENTS.md")?), (README.to_string(), read(README)?)];
    for f in &folders {
        let rel = format!("{DIR}/{f}/SKILL.md");
        files.push((rel.clone(), read(&rel)?));
    }
    let problems = check(&files, &folders, &verbs(crate::USAGE), |p| root.join(p).exists());
    if problems.is_empty() {
        println!("skills: {} skills, each routed, every file, verb and skill they name is there", folders.len());
        Ok(())
    } else {
        Err(format!("{} problems:\n  {}", problems.len(), problems.join("\n  ")))
    }
}

/// The verbs the task runner's usage text lists: the first word of each indented line.
pub fn verbs(usage: &str) -> BTreeSet<String> {
    usage.lines().skip(1).filter_map(|l| l.split_whitespace().next()).map(str::to_string).collect()
}

/// Every problem with `files` (path and text) for the skill `folders`, given the task runner's
/// `verbs` and whether a path relative to the root `exists`.
pub fn check(files: &[(String, String)], folders: &[String], verbs: &BTreeSet<String>, exists: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    let expected: BTreeSet<&str> = folders.iter().map(String::as_str).collect();
    for (path, text) in files {
        if let Some(folder) = path.strip_prefix(&format!("{DIR}/")).and_then(|p| p.strip_suffix("/SKILL.md")) {
            match header(text) {
                None => out.push(format!("{path}: no header with a name and a description")),
                Some((name, _)) if name != folder => out.push(format!("{path}: its name is {name:?}, not its folder's {folder:?}")),
                Some((_, description)) if description.is_empty() => out.push(format!("{path}: its description is empty")),
                Some(_) => {}
            }
        }
        let table: Option<BTreeSet<&str>> = match path.as_str() {
            "AGENTS.md" => Some(skills_named(text).into_iter().collect()),
            README => Some(readme_rows(text).into_iter().collect()),
            _ => None,
        };
        if let Some(table) = table {
            for missing in expected.difference(&table) {
                out.push(format!("{path}: does not list the skill {missing}"));
            }
        }
        for skill in skills_named(text) {
            if !expected.contains(skill) {
                out.push(format!("{path}: names the skill {skill}, which has no folder in {DIR}/"));
            }
        }
        for verb in verbs_named(text) {
            if !verbs.contains(verb) {
                out.push(format!(
                    "{path}: names `cargo xtask {verb}`, which is not a verb; the verbs are {}",
                    verbs.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
        }
        for p in paths(text) {
            let first = p.split('/').next().unwrap_or_default();
            if first != "plan" && exists(first) && !exists(p.trim_end_matches('/')) {
                out.push(format!("{path}: names {p}, which is not there"));
            }
        }
    }
    out
}

/// The `name` and `description` of a skill's header, the lines between its two `---` lines.
fn header(text: &str) -> Option<(String, String)> {
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    let (mut name, mut description) = (None, None);
    for line in lines.take_while(|l| *l != "---") {
        let unquote = |v: &str| v.trim().trim_matches('"').trim().to_string();
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(unquote(v));
        } else if let Some(v) = line.strip_prefix("description:") {
            description = Some(unquote(v));
        }
    }
    Some((name?, description?))
}

/// The skills a text names as `skills/<name>/SKILL.md`, in order, each once.
fn skills_named(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("skills/") {
        let after = &rest[at + "skills/".len()..];
        let before = rest[..at].chars().last();
        rest = after;
        if before.is_some_and(|c| c.is_alphanumeric() || c == '/') {
            continue;
        }
        if let Some((name, tail)) = after.split_once('/')
            && tail.starts_with("SKILL.md")
            && !name.is_empty()
            && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !out.contains(&name)
        {
            out.push(name);
        }
    }
    out
}

/// The skills the README's table lists: the names in its rows' first cells, ``| [`name`](…)``.
fn readme_rows(text: &str) -> Vec<&str> {
    text.lines().filter_map(|l| l.strip_prefix("| [`")).filter_map(|l| l.split_once('`')).map(|(name, _)| name).collect()
}

/// The verbs a text names as `cargo xtask <verb>`.
fn verbs_named(text: &str) -> Vec<&str> {
    text.match_indices("cargo xtask ")
        .filter_map(|(at, m)| text[at + m.len()..].split(|c: char| !(c.is_alphanumeric() || c == '-')).next())
        .filter(|v| !v.is_empty())
        .collect()
}

/// The spans in backticks outside fenced blocks that read as paths: a slash, and no spaces,
/// placeholders, wildcards or addresses.
fn paths(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        for span in line.split('`').skip(1).step_by(2) {
            let span = span.trim_end_matches(':');
            if span.contains('/') && !span.contains(|c: char| c.is_whitespace() || "<>*{}()[]:\\".contains(c)) && !span.starts_with('/') {
                out.push(span);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: \"Use when testing.\"\n---\n\n{body}\n")
    }

    fn files(agents: &str, readme: &str, skills: &[(&str, String)]) -> Vec<(String, String)> {
        let mut f = vec![("AGENTS.md".to_string(), agents.to_string()), ("skills/README.md".to_string(), readme.to_string())];
        f.extend(skills.iter().map(|(n, t)| (format!("skills/{n}/SKILL.md"), t.clone())));
        f
    }

    #[test]
    fn verbs_are_read_from_the_usage_text() {
        assert_eq!(verbs(crate::USAGE).into_iter().take(3).collect::<Vec<_>>(), ["assets", "brands", "ci"]);
        assert!(verbs(crate::USAGE).contains("skills"));
    }

    #[test]
    fn a_routed_skill_naming_real_files_and_verbs_passes() {
        let agents = "| Tests | `skills/a-tests/SKILL.md` |\nRun `cargo xtask ci`.\n";
        let readme = "| [`a-tests`](a-tests/SKILL.md) | tests |\n";
        let a = skill("a-tests", "See `crates/ui/src/lib.rs`, `logs/app.log`, `plan/shots/` and `i18n/<code>.tsv`.\n```sh\ncargo xtask shot\n```");
        let exists = |p: &str| ["crates", "crates/ui/src/lib.rs", "plan"].contains(&p);
        let verbs = BTreeSet::from(["ci".to_string(), "shot".to_string()]);
        assert_eq!(check(&files(agents, readme, &[("a-tests", a)]), &["a-tests".to_string()], &verbs, exists), Vec::<String>::new());
    }

    #[test]
    fn every_kind_of_drift_is_reported() {
        let agents = "Read `skills/gone/SKILL.md`, then `cargo xtask nope`.\n";
        let readme = "nothing listed\n";
        let a = "---\nname: other\ndescription: x\n---\nSee `crates/ui/src/missing.rs`.\n".to_string();
        let b = "no header".to_string();
        let exists = |p: &str| p == "crates";
        let verbs = BTreeSet::from(["ci".to_string()]);
        let problems = check(&files(agents, readme, &[("a", a), ("b", b)]), &["a".to_string(), "b".to_string()], &verbs, exists);
        let said = problems.join("\n");
        for expected in [
            "AGENTS.md: does not list the skill a",
            "skills/README.md: does not list the skill b",
            "names the skill gone",
            "`cargo xtask nope`, which is not a verb",
            "its name is \"other\", not its folder's \"a\"",
            "skills/b/SKILL.md: no header",
            "names crates/ui/src/missing.rs, which is not there",
        ] {
            assert!(said.contains(expected), "{expected:?} missing from:\n{said}");
        }
    }
}
