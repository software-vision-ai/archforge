//! The shell's one reading of the wall clock, and so its one platform seam for the time (the gate's
//! portable check lists this file in `xtask/src/wasm.rs`): Home's recent list records when a
//! project was last opened in whole seconds since 1970, and every place that records it asks
//! [`now`]. The desktop asks the system's clock. The web build has no wall clock without a
//! dependency of its own, so there the time reads 0, where the system's clock would stop the
//! window.

/// The time now, in whole seconds since 1970, from the system's clock; 0 for a clock set before
/// 1970.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The time now on the web, where the shell has no wall clock: always 0, the start of 1970.
#[cfg(target_arch = "wasm32")]
pub(crate) fn now() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The three places that record the time ask [`now`](super::now) and none reads the system's
    /// clock itself: Home's `now`, and the shell opening a project and showing one already open.
    #[test]
    fn the_shell_reads_the_wall_clock_only_here() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for (file, callers) in [("home.rs", 1), ("lib.rs", 2)] {
            let text = std::fs::read_to_string(src.join(file)).unwrap();
            assert_eq!(text.matches("crate::clock::now()").count(), callers, "{file} records the time through the clock");
            assert!(!text.contains("SystemTime"), "{file} reads the system's clock itself");
        }
    }
}
