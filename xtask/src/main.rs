//! The repository task runner: the gate, its checks, and the metamodel generator.
//!
//! `cargo xtask ci` is the gate: it first moves the shared crates to the newest commit of their
//! main branch, then runs every check in order, stops at the first failure and prints a summary of
//! step, result and duration.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod assets;
mod brands;
mod generate;
mod ico;
mod layers;
mod manual;
mod resize;
mod shot;
mod skills;
mod stats;
mod util;
mod version;
mod wasm;

use std::time::Instant;

/// The cargo-deny version the gate is tested with, which reads deny.toml's keys as written.
const DENY_VERSION: &str = "0.20.2";

const USAGE: &str = "usage: cargo xtask <verb>
  ci                 the gate: every check, in order, stopping at the first failure
  layers             the layer check
  wasm               the portable-target check
  assets [--check]   the attribution check; regenerates ATTRIBUTION.md unless --check
  skills             the skills check: the routing tables and every file, verb and skill they name
  brands             the optional trademark check against the list FORGE_BRANDS names
  generate [name] [--docs]  regenerate a metamodel crate's code from its XMI (all when no name)
  generated          the check that the generated code is what the XMI gives
  version [x.y.z]    print or set the workspace version
  ico <out> <png>... pack PNGs into a Windows icon
  resize <in> <out> <size> [--grid]  scale a square PNG down, on the icon grid with --grid
  shot [light|dark] [--size WxH] [--max-edge N]  a screenshot and a small copy under plan/shots
  manual             the user manual with its pictures, rendered from the interface, under plan/manual
  stats              tests and lines per crate";

type Step = (&'static str, Box<dyn Fn() -> Result<(), String>>);

fn cargo_step(args: &'static [&'static str]) -> Box<dyn Fn() -> Result<(), String>> {
    Box::new(move || {
        let mut cmd = util::cargo();
        cmd.args(args);
        util::run(cmd)
    })
}

/// The git source whose crates the gate moves to their newest commit before it checks anything,
/// so the repository always builds against the latest main of the shared crates.
const LATEST: &str = "git+https://github.com/storytold/craft-libs";

/// The packages of `lock` (a `Cargo.lock`) whose source is `source`, each once, in lock order.
fn packages_from(lock: &str, source: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut name = None;
    for line in lock.lines().map(str::trim) {
        if line == "[[package]]" {
            name = None;
        } else if let Some(n) = line.strip_prefix("name = \"").and_then(|r| r.strip_suffix('"')) {
            name = Some(n.to_string());
        } else if let Some(s) = line.strip_prefix("source = \"")
            && s.starts_with(source)
            && let Some(n) = name.take()
            && !out.contains(&n)
        {
            out.push(n);
        }
    }
    out
}

/// Moves the shared crates in `Cargo.lock` to the newest commit of their branch.
fn update_latest() -> Result<(), String> {
    let lock = std::fs::read_to_string(util::root().join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    let packages = packages_from(&lock, LATEST);
    if packages.is_empty() {
        println!("update: no crate comes from {LATEST}");
        return Ok(());
    }
    let mut cmd = util::cargo();
    cmd.arg("update");
    for p in &packages {
        cmd.args(["-p", p]);
    }
    util::run(cmd).map_err(|e| format!("{e}\nupdate: moving {} to the newest main needs the network", packages.join(", ")))
}

fn steps() -> Vec<Step> {
    let mut s: Vec<Step> = vec![
        ("update", Box::new(update_latest)),
        ("format", cargo_step(&["fmt", "--all", "--", "--check"])),
        ("lint", cargo_step(&["clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"])),
        ("test", cargo_step(&["test", "--workspace"])),
        ("assets", Box::new(|| assets::run(true))),
        ("skills", Box::new(skills::run)),
        ("layers", Box::new(layers::run)),
        ("portable", Box::new(wasm::run)),
        ("generated", Box::new(generate::check)),
    ];
    // The trademark list is kept outside the repository: without it the check is skipped and said
    // so, or fails where the list is required.
    if util::brand_list_path().is_some() || util::brand_list_required() {
        s.push(("brands", Box::new(brands::run)));
    }
    let deny = cargo_step(&["deny", "check"]);
    s.push((
        "deny",
        Box::new(move || {
            deny().map_err(|e| {
                format!("{e}\ndeny: if cargo-deny is missing, install it with `cargo install --locked cargo-deny@{DENY_VERSION}`, the version the gate is tested with; it fetches the advisory database, so it needs the network")
            })
        }),
    ));
    s
}

fn ci() -> Result<(), String> {
    let mut table: Vec<(&str, &str, f64)> = Vec::new();
    let mut failure = None;
    for (name, step) in steps() {
        println!("== {name}");
        let started = Instant::now();
        let result = step();
        let seconds = started.elapsed().as_secs_f64();
        match result {
            Ok(()) => table.push((name, "ok", seconds)),
            Err(e) => {
                table.push((name, "FAILED", seconds));
                failure = Some(format!("{name}: {e}"));
                break;
            }
        }
    }
    println!("\n{:<10} {:<7} {:>9}", "step", "result", "seconds");
    for (name, result, seconds) in &table {
        println!("{name:<10} {result:<7} {seconds:>9.1}");
    }
    if util::brand_list_path().is_none() && !util::brand_list_required() {
        println!("skipped: brands, the optional trademark check, as {} names no list here", util::BRANDS_VAR);
    }
    match failure {
        Some(f) => Err(f),
        None => Ok(()),
    }
}

/// A task runner built from another checkout must not act on this one: two checkouts sharing a
/// build directory would run each other's binary.
fn same_checkout() -> Result<(), String> {
    let (Ok(cwd), Ok(root)) = (std::env::current_dir().and_then(std::fs::canonicalize), std::fs::canonicalize(util::root())) else {
        return Ok(());
    };
    if cwd.starts_with(&root) {
        Ok(())
    } else {
        Err(format!("this task runner was built for {}, not for {}; give each checkout its own CARGO_TARGET_DIR", root.display(), cwd.display()))
    }
}

fn main() {
    if let Err(e) = same_checkout() {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    let check = rest.iter().any(|a| a == "--check");
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("layers") => layers::run(),
        Some("wasm") => wasm::run(),
        Some("assets") => assets::run(check),
        Some("skills") => skills::run(),
        Some("brands") => brands::run(),
        Some("generate") => generate::run(rest),
        Some("generated") => generate::check(),
        Some("version") => version::run(rest.first().map(String::as_str)),
        Some("ico") => ico::run(rest),
        Some("resize") => resize::run(rest),
        Some("shot") => shot::run(rest),
        Some("manual") => manual::run(),
        Some("stats") => stats::run(),
        Some("help" | "--help" | "-h") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown verb {other:?}\n{USAGE}")),
        None => Err(USAGE.to_string()),
    };
    if let Err(e) = result {
        eprintln!("xtask: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_crates_are_read_from_the_lock_once_each() {
        let lock = "version = 4\n\n[[package]]\nname = \"craft-atomic\"\nversion = \"0.1.0\"\nsource = \"git+https://github.com/storytold/craft-libs?branch=main#d40c2be\"\n\n\
                    [[package]]\nname = \"egui\"\nversion = \"0.36.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n\
                    [[package]]\nname = \"local\"\nversion = \"0.1.0\"\n";
        assert_eq!(packages_from(lock, LATEST), ["craft-atomic"]);
        assert!(packages_from("", LATEST).is_empty());
    }
}
