//! The repository task runner: the gate, its checks, and the metamodel generator.
//!
//! `cargo xtask ci` is the gate: it runs every check in order, stops at the first failure and
//! prints a summary of step, result and duration. It never changes `Cargo.lock`: every cargo
//! command it runs that resolves dependencies takes `--locked`, as the `xtask` alias does, so a
//! manifest changed without its lock fails the gate. `cargo xtask bump` alone moves the shared
//! crates to the newest commit of their main branch.

mod assets;
mod bump;
mod generate;
mod ico;
mod kind_literals;
mod layers;
mod lints;
mod manual;
mod resize;
mod screens;
mod shell_io;
mod shot;
mod skills;
mod stats;
mod util;
mod ux_literals;
mod version;
mod wasm;

use std::time::Instant;

/// The cargo-deny version the gate is tested with, which reads deny.toml's keys as written.
const DENY_VERSION: &str = "0.20.2";

const USAGE: &str = "usage: cargo xtask <verb>
  ci                 the gate: every check, in order, stopping at the first failure
  bump               move the craft-libs crates in Cargo.lock to the newest main, for a pull request of its own
  layers             the layer check
  wasm               the portable-target check
  assets [--check]   the attribution check; regenerates ATTRIBUTION.md unless --check
  skills             the skills check: the routing tables and every file, verb and skill they name
  generate [name] [--docs]  regenerate a metamodel crate's code from its XMI (all when no name)
  generated          the check that the generated code is what the XMI gives
  kinds              the check that a diagram kind is written out only as its crate's constant
  version [x.y.z]    print or set the workspace version
  ico <out> <png>... pack PNGs into a Windows icon
  resize <in> <out> <size> [--grid]  scale a square PNG down, on the icon grid with --grid
  shot [light|dark] [--size WxH] [--max-edge N]  a screenshot and a small copy under plan/shots
  screens [--gallery <dir>]  every screen in both appearances, with a contact sheet each, under
                     plan/screens; or the README's gallery and the wiki's guide screens alone,
                     into <dir>
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

/// The gate's steps in order. Each cargo command that resolves dependencies takes `--locked`, so
/// none of them writes `Cargo.lock`; `cargo fmt` reads no lock file.
///
/// The lint step first checks that every member takes the workspace's lints (`lints`), where the
/// never-crash deny list lives: clippy's verdict says nothing about a crate without them.
fn steps() -> Vec<Step> {
    let mut s: Vec<Step> = vec![
        ("format", cargo_step(&["fmt", "--all", "--", "--check"])),
        (
            "lint",
            Box::new(|| {
                lints::run()
                    .and_then(|()| cargo_step(&["clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"])())
            }),
        ),
        ("test", cargo_step(&["test", "--locked", "--workspace"])),
        ("assets", Box::new(|| assets::run(true))),
        ("skills", Box::new(skills::run)),
        ("layers", Box::new(layers::run)),
        ("portable", Box::new(wasm::run)),
        ("generated", Box::new(generate::check)),
        ("kinds", Box::new(kind_literals::run)),
    ];
    let deny = cargo_step(&["deny", "--locked", "check"]);
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
        Some("bump") => bump::run(),
        Some("layers") => layers::run(),
        Some("wasm") => wasm::run(),
        Some("assets") => assets::run(check),
        Some("skills") => skills::run(),
        Some("generate") => generate::run(rest),
        Some("generated") => generate::check(),
        Some("kinds") => kind_literals::run(),
        Some("version") => version::run(rest.first().map(String::as_str)),
        Some("ico") => ico::run(rest),
        Some("resize") => resize::run(rest),
        Some("shot") => shot::run(rest),
        Some("screens") => screens::run(rest),
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
