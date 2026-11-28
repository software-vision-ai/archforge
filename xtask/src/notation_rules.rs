//! The notation rules (ADRs 0002, 0004 and 0005), part of the layer check.
//!
//! Read from `cargo metadata`, over every kind of dependency (normal, build and development), so a
//! test's link counts as a production one does:
//!
//! 1. only `archforge-notations`, a notation's own screens crate and the app link a notation's
//!    model crate;
//! 2. only the app links a notation's screens crate;
//! 3. the shared crates (`ui-egui`, `shell-api`, `project` and the command-line tool) link neither;
//! 4. every model crate has its screens crate (`<model>-ui`), and every screens crate its model
//!    crate.
//!
//! And a plain text scan of every file of the shared crates for a notation's command names
//! (rule 5): a command id under a model crate's name (`<model>.…`) and the Model menu rows a
//! screens crate declares (its `"model.…"` literals).
//!
//! A notation's model crates are the crates on [`MODEL_LAYER`] of the layer table, its screens
//! crates those on [`SCREENS_LAYER`]. Every refusal names its rule and the records it comes from.
//! There is no exception list.

use std::path::Path;

use crate::layers::{Class, Package};
use crate::util;

/// The layer of the notations' model crates in the layer table.
pub const MODEL_LAYER: u8 = 7;

/// The layer of the notations' screens crates in the layer table.
pub const SCREENS_LAYER: u8 = 10;

/// The crate that composes the notations a project holds, the one library crate that names every
/// model crate (ADR 0004).
pub const NOTATIONS: &str = "notations";

/// The desktop app, the one crate that registers every notation (ADR 0005), by its package name.
pub const APP: &str = "archforge";

/// The shared crates, by short name: the shell, its interfaces, the project store and the
/// command-line tool, which name no notation (ADR 0004 rule 1).
pub const SHARED: [&str; 4] = ["ui-egui", "shell-api", "project", "cli"];

/// The model crates and the screens crates among `packages`, by short name.
fn notation_crates(packages: &[Package]) -> (Vec<&str>, Vec<&str>) {
    let on = |layer: u8| packages.iter().filter(move |p| p.class == Some(Class::Layer(layer))).map(|p| p.short.as_str()).collect::<Vec<_>>();
    (on(MODEL_LAYER), on(SCREENS_LAYER))
}

/// The screens crate of the model crate `model`.
fn screens_of(model: &str) -> String {
    format!("{model}-ui")
}

/// Every refusal of rules 1 to 4 for the given packages (pure, so it is unit-tested without cargo).
pub fn violations(packages: &[Package]) -> Vec<String> {
    let (models, screens) = notation_crates(packages);
    let mut out = Vec::new();
    for p in packages {
        let here = p.short.as_str();
        for (dep, kind) in &p.deps {
            let (model, screen) = (models.contains(&dep.as_str()), screens.contains(&dep.as_str()));
            if dep == here || !(model || screen) {
                continue;
            }
            if SHARED.contains(&here) {
                out.push(format!(
                    "{here} -> {dep} ({kind}): rule 3 (ADR 0004, ADR 0005): ui-egui, shell-api, project and the command-line tool link no notation crate, in production or in tests; they reach the notations through the shell's interfaces and archforge-notations, and their tests use the fake notation (archforge-notations' `testing` feature)"
                ));
            } else if model && here != NOTATIONS && here != APP && here != screens_of(dep) {
                out.push(format!(
                    "{here} -> {dep} ({kind}): rule 1 (ADR 0004, ADR 0005): only archforge-notations, the notation's own screens crate {} and the app link the model crate {dep}, in production or in tests; code that needs it belongs in {}, a test of the whole app in the app's tests",
                    screens_of(dep),
                    screens_of(dep)
                ));
            } else if screen && here != APP {
                out.push(format!(
                    "{here} -> {dep} ({kind}): rule 2 (ADR 0004, ADR 0005): only the app links a notation's screens crate, in production or in tests; other crates reach the notations through the shell's interfaces, and a test of the whole app belongs in the app's tests"
                ));
            }
        }
    }
    for model in &models {
        if !screens.contains(&screens_of(model).as_str()) {
            out.push(format!("{model}: rule 4 (ADR 0002): the model crate has no screens crate {}; a notation is a model part and a screens part", screens_of(model)));
        }
    }
    for screen in &screens {
        let model = screen.strip_suffix("-ui").unwrap_or(screen);
        if model == *screen || !models.contains(&model) {
            out.push(format!("{screen}: rule 4 (ADR 0002): the screens crate has no model crate {model}; a notation is a model part and a screens part"));
        }
    }
    out
}

/// Whether `b` may stand next to a name: anything but a letter, a digit, `_`, `.` or `-`.
fn apart(b: Option<u8>) -> bool {
    b.is_none_or(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-')))
}

/// The notation command names in `text`, each with its line (from 1), in order: every name that
/// starts with one of `prefixes` and a lower-case letter, and every one of `names`, each standing
/// apart from the text around it ([`apart`]).
pub fn names_in(text: &str, prefixes: &[String], names: &[String]) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut line = 1;
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            continue;
        }
        if !apart(i.checked_sub(1).map(|j| bytes[j])) {
            continue;
        }
        let rest = &bytes[i..];
        let id = prefixes.iter().any(|p| rest.starts_with(p.as_bytes()) && rest.get(p.len()).is_some_and(u8::is_ascii_lowercase));
        if id {
            let end = rest.iter().position(|b| !(b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-'))).unwrap_or(rest.len());
            out.push((line, String::from_utf8_lossy(&rest[..end]).trim_end_matches('.').to_string()));
            continue;
        }
        if let Some(n) = names.iter().find(|n| rest.starts_with(n.as_bytes()) && apart(rest.get(n.len()).copied())) {
            out.push((line, n.clone()));
        }
    }
    out
}

/// The Model menu rows the screens crate at `dir` declares: every `"model.…"` literal in its
/// sources, without the quotes, sorted and once each.
fn row_keys(dir: &Path) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    for file in util::walk_strict(dir, &[])? {
        let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let text = String::from_utf8_lossy(&text);
        for piece in text.split('"').skip(1).step_by(2) {
            if piece.strip_prefix("model.").is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_lowercase() || matches!(b, b'.' | b'-'))) {
                keys.push(piece.to_string());
            }
        }
    }
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// Every notation command name in the files of the shared crates (rule 5), each named with its
/// crate, file and line. The crates' folders are `packages`' own, under the workspace root `root`.
pub fn command_names(root: &Path, packages: &[Package]) -> Result<Vec<String>, String> {
    let (models, screens) = notation_crates(packages);
    let prefixes: Vec<String> = models.iter().map(|m| format!("{m}.")).collect();
    let mut names = Vec::new();
    for screen in &screens {
        let Some(p) = packages.iter().find(|p| p.short == *screen) else { continue };
        names.extend(row_keys(&root.join(&p.dir).join("src"))?);
    }
    let mut out = Vec::new();
    for p in packages.iter().filter(|p| SHARED.contains(&p.short.as_str())) {
        let dir = root.join(&p.dir);
        for file in util::walk_strict(&dir, &["target"])? {
            let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            let rel = file.strip_prefix(&dir).unwrap_or(&file).display().to_string();
            for (line, name) in names_in(&String::from_utf8_lossy(&text), &prefixes, &names) {
                out.push(format!(
                    "{}: {rel} line {line}: {name}: rule 5 (ADR 0004, ADR 0005): a shared crate names no notation command, in production or in tests; it asks the notations through their interfaces, and its tests use the fake notation's",
                    p.short
                ));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A package of the given class: the tests describe their own notations, not the repository's.
    fn pkg(short: &str, class: Class, deps: &[(&str, &str)]) -> Package {
        Package { short: short.into(), class: Some(class), deps: deps.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect(), dir: short.into() }
    }

    /// A workspace of one notation, `flow` with its screens crate, composed, registered by the app.
    fn workspace(extra: Vec<Package>) -> Vec<Package> {
        let mut all = vec![
            pkg("flow", Class::Layer(MODEL_LAYER), &[]),
            pkg("flow-ui", Class::Layer(SCREENS_LAYER), &[("flow", "normal")]),
            pkg(NOTATIONS, Class::Layer(8), &[("flow", "normal")]),
            pkg(APP, Class::Exempt, &[("flow-ui", "normal"), ("flow", "dev")]),
        ];
        all.extend(extra);
        all
    }

    #[test]
    fn a_workspace_that_holds_to_the_rules_has_no_refusal() {
        assert_eq!(violations(&workspace(vec![pkg("ui-egui", Class::Layer(11), &[(NOTATIONS, "normal")])])), Vec::<String>::new());
    }

    /// Rule 1: a model crate linked by any crate but the composition, its own screens crate and
    /// the app is refused, a test's link as a production one; another notation's screens crate too.
    #[test]
    fn rule_1_refuses_a_model_crate_linked_by_anyone_else() {
        let other = pkg("tree-ui", Class::Layer(SCREENS_LAYER), &[("flow", "normal")]);
        let v = violations(&workspace(vec![pkg("diagram", Class::Layer(6), &[("flow", "dev")]), other, pkg("tree", Class::Layer(MODEL_LAYER), &[])]));
        assert!(v.iter().any(|l| l.starts_with("diagram -> flow (dev): rule 1 (ADR 0004, ADR 0005)")), "{v:?}");
        assert!(v.iter().any(|l| l.starts_with("tree-ui -> flow (normal): rule 1")), "{v:?}");
        assert_eq!(v.len(), 2, "{v:?}");
    }

    /// Rule 2: a screens crate linked by any crate but the app is refused, the composition and
    /// another screens crate included.
    #[test]
    fn rule_2_refuses_a_screens_crate_linked_by_anyone_but_the_app() {
        let v = violations(&workspace(vec![pkg("dolt", Class::Layer(12), &[("flow-ui", "dev")]), pkg(NOTATIONS, Class::Layer(8), &[("flow-ui", "build")])]));
        assert!(v.iter().any(|l| l.starts_with("dolt -> flow-ui (dev): rule 2 (ADR 0004, ADR 0005)")), "{v:?}");
        assert!(v.iter().any(|l| l.starts_with("notations -> flow-ui (build): rule 2")), "{v:?}");
    }

    /// Rule 3: each shared crate is refused a link to a model crate or a screens crate, whatever its
    /// kind, under rule 3 alone.
    #[test]
    fn rule_3_refuses_every_shared_crate_any_notation_crate() {
        for shared in SHARED {
            for (dep, kind) in [("flow", "normal"), ("flow-ui", "dev")] {
                let v = violations(&workspace(vec![pkg(shared, Class::Layer(11), &[(dep, kind)])]));
                assert_eq!(v.len(), 1, "{shared} -> {dep}: {v:?}");
                assert!(v[0].starts_with(&format!("{shared} -> {dep} ({kind}): rule 3 (ADR 0004, ADR 0005)")), "{v:?}");
            }
        }
    }

    /// Rule 4: a model crate without its screens crate, and a screens crate without its model
    /// crate, are each refused.
    #[test]
    fn rule_4_refuses_a_notation_of_one_part() {
        let v = violations(&workspace(vec![pkg("tree", Class::Layer(MODEL_LAYER), &[]), pkg("chart-ui", Class::Layer(SCREENS_LAYER), &[])]));
        assert_eq!(
            v,
            [
                "tree: rule 4 (ADR 0002): the model crate has no screens crate tree-ui; a notation is a model part and a screens part",
                "chart-ui: rule 4 (ADR 0002): the screens crate has no model crate chart; a notation is a model part and a screens part"
            ]
        );
    }

    /// Rule 5's names: a command id under a model crate's name and a declared row, each standing
    /// apart, on its line; a longer word that holds one, or a row's key inside a longer key, is none.
    #[test]
    fn the_scan_finds_command_ids_and_rows_standing_apart() {
        let text = "a \"flow.step.add\" b\nflow.Step pageflow.x flow_x \"model.flow-step\"\nmodel.flow-steps\tmodel.flow-step\n";
        let found = names_in(text, &["flow.".to_string()], &["model.flow-step".to_string()]);
        assert_eq!(found, [(1, "flow.step.add".to_string()), (2, "model.flow-step".to_string()), (3, "model.flow-step".to_string())]);
    }

    /// Rule 5 reads every file of each shared crate, catalogues and tests included, with the rows
    /// the screens crates declare, and leaves the other crates alone.
    #[test]
    fn rule_5_reads_every_file_of_the_shared_crates() {
        let tree = util::Scratch::new("notation-names").unwrap();
        tree.file("flow-ui/src/kind.rs", "ModelRow { key: \"model.flow-step\" }\n").unwrap();
        tree.file("ui-egui/src/i18n/en.tsv", "str\taccess\tmodel.flow-step\tS\n").unwrap();
        tree.file("cli/tests/bridge.rs", "\n\"flow.step.add\"\n").unwrap();
        tree.file("diagram/src/lib.rs", "\"flow.step.add\"\n").unwrap();
        let packages = workspace(vec![pkg("ui-egui", Class::Layer(11), &[]), pkg("cli", Class::Exempt, &[]), pkg("diagram", Class::Layer(6), &[])]);
        let said = "rule 5 (ADR 0004, ADR 0005): a shared crate names no notation command, in production or in tests; it asks the notations through their interfaces, and its tests use the fake notation's";
        assert_eq!(
            command_names(&tree.path, &packages).unwrap(),
            [format!("ui-egui: src/i18n/en.tsv line 1: model.flow-step: {said}"), format!("cli: tests/bridge.rs line 2: flow.step.add: {said}")]
        );
    }
}
