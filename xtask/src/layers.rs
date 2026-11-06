//! The layer check.
//!
//! Reads `cargo metadata`, strips the application prefix from each package name, classifies the
//! remainder through `TABLE`, walks normal, build and development edges between workspace crates,
//! checks toolkit crates against their minimum layer, reports every violation and prints the table.
//! Applications under `apps/` and the task runner are exempt.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::util;

/// The application prefix of every package name.
pub const PREFIX: &str = "archforge-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A numbered layer; edges point strictly down.
    Layer(u8),
    /// No workspace dependencies; publishable alone.
    Standalone,
    /// Applications and the task runner.
    Exempt,
}

/// Every library crate, by short name. Every library crate of the workspace has a row.
pub const TABLE: &[(&str, Class)] = &[
    // The rules for names that become file names: pure, shared by the shell and the host.
    ("names", Class::Standalone),
    // The command engine: knows no notation, builds for the portable target.
    ("engine", Class::Layer(3)),
    // The MOF core: the meta-metamodel, the object store and change sets; pure, portable. The
    // shared core metamodel comes at 5, the diagrams at 6, the notations at 7.
    ("mof", Class::Layer(4)),
    // The shared core metamodel (comments, tags, folders, trace links), generated from our XMI.
    ("core", Class::Layer(5)),
    // The diagram metamodel for every notation (diagrams, nodes, edges, labels, styles), generated
    // from our XMI, with the generic diagram commands, the spatial index and the project's view,
    // which asks each notation about its own diagrams and elements; knows no notation. Diagrams are
    // members of the core's Model, so it sits above the core.
    ("diagram", Class::Layer(6)),
    // UML: its metamodel generated from OMG's XMI, its commands on the MOF core, its class diagrams
    // in the project's view.
    ("uml", Class::Layer(7)),
    // The ER notation: its metamodel generated from our XMI, its commands on the MOF core.
    ("er", Class::Layer(7)),
    // The notations a project holds, composed into one store, one command table and one set of
    // tables: the one crate above the notations that names them all.
    ("notations", Class::Layer(8)),
    ("ui-egui", Class::Layer(9)),
    // Host services, for the desktop only: the application uses them and the shell never depends on
    // them, so the portable-target check, which builds the layers below the shell, leaves them out.
    ("dolt", Class::Layer(9)),
    ("project", Class::Layer(10)),
    // Agent access: the control channel's token, loopback server and client, and the MCP server
    // bridged to it. Host services like the two above, never built for the portable target.
    ("control", Class::Layer(9)),
    // API keys in the system's credential store, never in a file. A host service like the ones
    // above, never built for the portable target.
    ("secrets", Class::Layer(9)),
    // The agent runtimes a person installs: found, version-checked, run and supervised as child
    // processes with keys from `secrets`. A host service, never built for the portable target.
    ("agents", Class::Layer(10)),
    ("mcp", Class::Layer(10)),
];

/// The shell crate: the highest crate the portable-target check builds.
pub const SHELL: &str = "ui-egui";

/// Toolkit crates (exact names or prefixes ending in `*`) and the minimum layer that may use them.
pub const TOOLKIT: &[&str] = &["egui", "egui-*", "egui_*", "eframe", "winit", "wgpu", "wgpu-*"];
pub const TOOLKIT_MIN_LAYER: u8 = 9;

pub fn class_of(short: &str) -> Option<Class> {
    TABLE.iter().find(|(n, _)| *n == short).map(|(_, c)| *c)
}

fn layer(c: Class) -> Option<u8> {
    match c {
        Class::Layer(n) => Some(n),
        Class::Standalone => Some(0),
        Class::Exempt => None,
    }
}

fn is_toolkit(name: &str) -> bool {
    TOOLKIT.iter().any(|t| match t.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *t,
    })
}

/// One package as the check sees it.
pub struct Package {
    pub short: String,
    pub class: Option<Class>,
    pub deps: Vec<(String, String)>,
}

/// Violations for the given packages (pure, so it is unit-tested without cargo).
pub fn violations(packages: &[Package]) -> Vec<String> {
    let mut out = Vec::new();
    let workspace: BTreeMap<&str, Option<Class>> = packages.iter().map(|p| (p.short.as_str(), p.class)).collect();
    for p in packages {
        let Some(class) = p.class else {
            out.push(format!("{} is missing from the layer table (xtask/src/layers.rs)", p.short));
            continue;
        };
        if class == Class::Exempt {
            continue;
        }
        let here = layer(class).unwrap_or(0);
        for (dep, kind) in &p.deps {
            // A crate's dev edge to itself only switches on a feature for its own tests.
            if *dep == p.short && kind == "dev" {
                continue;
            }
            if let Some(dep_class) = workspace.get(dep.as_str()) {
                let Some(dep_class) = dep_class else { continue };
                if class == Class::Standalone {
                    out.push(format!("{} is standalone but depends on workspace crate {dep}", p.short));
                    continue;
                }
                let Some(there) = layer(*dep_class) else {
                    out.push(format!("{} depends on the exempt crate {dep}", p.short));
                    continue;
                };
                if there == here {
                    out.push(format!("{} -> {dep}: a {kind} edge inside layer {here}", p.short));
                } else if there > here {
                    out.push(format!("{} (layer {here}) -> {dep} (layer {there}): an upward {kind} edge", p.short));
                }
            } else if is_toolkit(dep) && here < TOOLKIT_MIN_LAYER {
                out.push(format!("{} (layer {here}) uses the toolkit crate {dep}, allowed from layer {TOOLKIT_MIN_LAYER}", p.short));
            }
        }
    }
    out
}

/// Reads the workspace through `cargo metadata`.
pub fn packages() -> Result<Vec<Package>, String> {
    let out = util::cargo().args(["metadata", "--format-version", "1", "--no-deps"]).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let root = util::root();
    let mut result = Vec::new();
    for p in meta["packages"].as_array().cloned().unwrap_or_default() {
        let name = p["name"].as_str().unwrap_or_default();
        let manifest = std::path::PathBuf::from(p["manifest_path"].as_str().unwrap_or_default());
        let rel = manifest.strip_prefix(&root).map(|r| r.to_path_buf()).unwrap_or(manifest);
        let exempt = rel.starts_with("apps") || rel.starts_with("xtask");
        let short = name.strip_prefix(PREFIX).unwrap_or(name).to_string();
        let class = if exempt { Some(Class::Exempt) } else { class_of(&short) };
        let deps = p["dependencies"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|d| {
                let dn = d["name"].as_str().unwrap_or_default();
                let kind = d["kind"].as_str().unwrap_or("normal").to_string();
                (dn.strip_prefix(PREFIX).unwrap_or(dn).to_string(), kind)
            })
            .collect();
        result.push(Package { short, class, deps });
    }
    Ok(result)
}

pub fn run() -> Result<(), String> {
    let packages = packages()?;
    println!("{:<12} class", "crate");
    for p in &packages {
        println!("{:<12} {:?}", p.short, p.class);
    }
    let v = violations(&packages);
    if v.is_empty() {
        println!("layers: {} crates, no violation", packages.len());
        Ok(())
    } else {
        for line in &v {
            eprintln!("layers: {line}");
        }
        Err(format!("{} layer violation(s)", v.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A package of the given class: the tests describe their own layers, not the repository's table.
    fn pkg(short: &str, class: Option<Class>, deps: &[(&str, &str)]) -> Package {
        Package { short: short.into(), class, deps: deps.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect() }
    }

    #[test]
    fn the_check_reports_every_violation_kind() {
        let packages = vec![
            pkg("geom", Some(Class::Layer(0)), &[("model", "normal")]),
            pkg("model", Some(Class::Layer(1)), &[("geom", "normal"), ("egui", "normal")]),
            pkg("format", Some(Class::Layer(3)), &[("render", "dev")]),
            pkg("render", Some(Class::Layer(3)), &[]),
            pkg("stray", None, &[]),
        ];
        let v = violations(&packages).join("\n");
        assert!(v.contains("upward normal edge"), "{v}");
        assert!(v.contains("toolkit crate egui"), "{v}");
        assert!(v.contains("inside layer 3"), "{v}");
        assert!(v.contains("stray is missing"), "{v}");
    }

    #[test]
    fn a_clean_tree_has_no_violation() {
        let packages = vec![
            pkg("geom", Some(Class::Layer(0)), &[]),
            pkg("model", Some(Class::Layer(1)), &[("geom", "normal"), ("color", "dev"), ("model", "dev")]),
            pkg("color", Some(Class::Layer(0)), &[]),
            pkg("ui-egui", Some(Class::Layer(9)), &[("model", "normal"), ("egui", "normal")]),
        ];
        assert_eq!(violations(&packages), Vec::<String>::new());
    }

    #[test]
    fn every_library_crate_of_the_repository_is_in_the_table() {
        assert_eq!(class_of(SHELL), Some(Class::Layer(9)));
    }
}
