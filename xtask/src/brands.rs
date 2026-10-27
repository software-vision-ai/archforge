//! The optional trademark check: names the project must not use, from a list maintainers keep
//! outside the repository (`FORGE_BRANDS`), may not appear in code, comments, strings, test names,
//! manifest descriptions, packaging sources or non-Markdown documentation. Markdown is not checked.
//! Without the list the check is skipped.

use crate::util;

pub fn run() -> Result<(), String> {
    let names = util::brands()?;
    if names.is_empty() {
        println!("brands: the trademark list is empty; nothing to check");
        return Ok(());
    }
    let mut hits = Vec::new();
    for file in util::tracked() {
        let rel = file.to_string_lossy().replace('\\', "/");
        if rel.ends_with(".md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(util::root().join(&file)) else { continue };
        for (n, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if let Some(name) = names.iter().find(|b| lower.contains(b.as_str())) {
                hits.push(format!("{rel}:{}: {name:?}", n + 1));
            }
        }
    }
    if hits.is_empty() {
        println!("brands: no listed name outside Markdown");
        Ok(())
    } else {
        for h in &hits {
            eprintln!("brands: {h}");
        }
        Err(format!("{} trademark hit(s)", hits.len()))
    }
}
