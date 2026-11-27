//! Checks UML's catalogues at build time, so a malformed row fails the build rather than a lookup.

use std::path::Path;

/// The row check of the shell's interfaces' translations machinery (`archforge_shell_api::i18n::check`).
#[path = "../shell-api/src/i18n/check.rs"]
mod check;

/// Fails the build at a catalogue row under `src/i18n` (UML's are in its folder `uml`) that does
/// not have four columns, at a catalogue that cannot be read as text, or at a listing of the
/// catalogues that fails part way.
fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let i18n = Path::new(&manifest).join("src").join("i18n");
    println!("cargo:rerun-if-changed={}", i18n.display());
    catalogues(&i18n);
}

/// Checks every catalogue in `dir` and in its folders, and exits the build at the first malformed
/// row, at a catalogue that cannot be read as text, or at a listing that fails part way.
fn catalogues(dir: &Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => listing_failed(dir, &e),
    };
    for entry in entries {
        // The standard library does not promise that a listing ends after an error. A folder is
        // told by the entry's own type, which does not follow a link.
        let (path, folder) = match entry.and_then(|entry| Ok((entry.path(), entry.file_type()?.is_dir()))) {
            Ok(found) => found,
            Err(e) => listing_failed(dir, &e),
        };
        if folder {
            catalogues(&path);
            continue;
        }
        if path.extension().is_none_or(|e| e != "tsv") {
            continue;
        }
        // A catalogue that cannot be read as text, invalid UTF-8 among it, cannot be checked.
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                println!("cargo:warning={}: the catalogue could not be read, so it cannot be checked: {e}", path.display());
                std::process::exit(1);
            }
        };
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
