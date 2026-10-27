//! The portable-target check: every crate up to and including the shell builds for
//! `wasm32-unknown-unknown`. A source scan (a test below, run by the gate's test step) keeps
//! wall-clock, process, network and environment access out of those crates except in the files
//! listed as platform seams, each of which guards its platform code with
//! `cfg(not(target_arch = "wasm32"))`.

use crate::layers::{self, Class};
use crate::util;

pub const TARGET: &str = "wasm32-unknown-unknown";

/// The crates the portable target must build: every layered or standalone crate below the shell,
/// and the shell itself.
pub fn portable_crates() -> Vec<String> {
    let shell_layer = layers::class_of(layers::SHELL).and_then(|c| if let Class::Layer(n) = c { Some(n) } else { None }).unwrap_or(u8::MAX);
    layers::TABLE
        .iter()
        .filter(|(name, class)| match class {
            Class::Layer(n) => *n < shell_layer || *name == layers::SHELL,
            Class::Standalone => true,
            _ => false,
        })
        .map(|(name, _)| format!("{}{name}", layers::PREFIX))
        .collect()
}

pub fn run() -> Result<(), String> {
    let mut cmd = util::cargo();
    cmd.args(["check", "--target", TARGET]);
    for c in portable_crates() {
        cmd.args(["-p", &c]);
    }
    util::run(cmd).map_err(|e| format!("{e}\nportable: if the target is missing, install it with `rustup target add {TARGET}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Files below the shell that may touch the platform, each behind a target guard.
    pub const PLATFORM_SEAMS: &[&str] = &[];

    /// Calls that reach the clock, processes, the network or the environment.
    pub const PLATFORM_CALLS: &[&str] = &["std::fs", "std::process", "std::net", "std::env", "SystemTime", "Instant::now", "std::thread::sleep"];

    #[test]
    fn crates_below_the_shell_reach_the_platform_only_through_their_seams() {
        let root = util::root();
        let mut offenders = Vec::new();
        for krate in portable_crates() {
            let short = krate.strip_prefix(layers::PREFIX).unwrap_or(&krate).to_string();
            for file in util::walk(&root.join("crates").join(&short).join("src")) {
                let rel = file.to_string_lossy().replace('\\', "/");
                if rel.ends_with("tests.rs") || rel.contains("/tests/") {
                    continue;
                }
                let text = std::fs::read_to_string(root.join(&file)).unwrap_or_default();
                // Test modules come last in a file, so the scan stops at the first one.
                let production = text.split("#[cfg(test)]").next().unwrap_or_default();
                let seam = PLATFORM_SEAMS.contains(&rel.as_str());
                for call in PLATFORM_CALLS {
                    if production.contains(call) && !seam {
                        offenders.push(format!("{rel} uses {call}"));
                    }
                }
                if seam && !text.contains("cfg(not(target_arch = \"wasm32\"))") && !text.contains("cfg(target_arch = \"wasm32\")") {
                    offenders.push(format!("{rel} is a platform seam without a target guard"));
                }
            }
        }
        assert!(offenders.is_empty(), "platform code outside the seams:\n{}", offenders.join("\n"));
    }

    #[test]
    fn the_portable_set_ends_with_the_shell() {
        let set = portable_crates();
        assert!(set.iter().any(|c| c.ends_with(layers::SHELL)));
    }
}
