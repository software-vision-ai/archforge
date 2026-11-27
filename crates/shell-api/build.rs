//! Checks the interfaces' own catalogues at build time, so a malformed row fails the build rather
//! than a lookup.

use std::path::Path;

/// The catalogues' row check (`archforge_shell_api::i18n::check`).
#[path = "src/i18n/check.rs"]
mod check;

/// Fails the build at a catalogue row in `src/i18n` that does not have four columns, or at a
/// listing of the catalogues that fails part way.
fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let i18n = Path::new(&manifest).join("src").join("i18n");
    println!("cargo:rerun-if-changed={}", i18n.display());
    let entries = match std::fs::read_dir(&i18n) {
        Ok(entries) => entries,
        Err(e) => listing_failed(&i18n, &e),
    };
    for entry in entries {
        // The standard library does not promise that a listing ends after an error.
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(e) => listing_failed(&i18n, &e),
        };
        if path.extension().is_none_or(|e| e != "tsv") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if let Some((line, cols)) = check::malformed_row(&text) {
            println!("cargo:warning={}:{line}: a catalogue row has 4 tab-separated columns, found {cols}", path.display());
            std::process::exit(1);
        }
    }
}

/// Fails the build: the catalogues in `dir` were not all listed, so they cannot all be checked.
fn listing_failed(dir: &Path, e: &std::io::Error) -> ! {
    println!("cargo:warning={}: the catalogues were not all listed, so they cannot all be checked: {e}", dir.display());
    std::process::exit(1);
}
