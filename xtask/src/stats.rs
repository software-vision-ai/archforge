//! `cargo xtask stats`: tests and lines per crate.

use crate::util;

/// Prints the tests and lines of each crate. A listing that fails part way is an error.
pub fn run() -> Result<(), String> {
    let root = util::root();
    println!("{:<24} {:>7} {:>8}", "crate", "tests", "lines");
    for base in ["crates", "apps", "xtask"] {
        let dirs: Vec<std::path::PathBuf> = if base == "xtask" {
            vec![root.join("xtask")]
        } else {
            // A listing that fails part way fails the run rather than leaving crates out.
            let listed = std::fs::read_dir(root.join(base)).map(|r| r.map(|e| e.map(|e| e.path())).collect::<Result<Vec<_>, _>>());
            listed
                .unwrap_or(Ok(Vec::new()))
                .map_err(|e| format!("{base}/: the listing stopped part way: {e}"))?
                .into_iter()
                .filter(|p| p.is_dir())
                .collect()
        };
        for dir in dirs {
            let (mut tests, mut lines) = (0usize, 0usize);
            for file in util::walk(&dir)?.into_iter().filter(|f| f.extension().is_some_and(|e| e == "rs")) {
                let text = std::fs::read_to_string(root.join(&file)).unwrap_or_default();
                tests += text.matches("#[test]").count();
                lines += text.lines().count();
            }
            let name = dir.strip_prefix(&root).map(|p| p.display().to_string()).unwrap_or_default();
            println!("{name:<24} {tests:>7} {lines:>8}");
        }
    }
    Ok(())
}
