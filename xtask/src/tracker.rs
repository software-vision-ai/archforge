//! The public tree names no ticket of the project's tracker: a ticket is the owner's working
//! note, and the code, its comments, the skills and the manual say what a thing is and why, in
//! their own words. A test in the gate keeps it so.

#[cfg(test)]
mod tests {
    use crate::util;

    /// Where `text` names a ticket: the project's key, a hyphen and a number, standing alone.
    fn tickets(text: &str, key: &str) -> Vec<String> {
        let pattern = format!("{key}-");
        let mut found = Vec::new();
        for (at, _) in text.match_indices(&pattern) {
            let before = text.get(..at).and_then(|s| s.chars().next_back());
            let after = text.get(at + pattern.len()..).unwrap_or_default();
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            let alone = before.is_none_or(|c| !c.is_alphanumeric()) && !digits.is_empty();
            let ends = after.chars().nth(digits.chars().count()).is_none_or(|c| !c.is_alphanumeric());
            if alone && ends {
                found.push(format!("{pattern}{digits}"));
            }
        }
        found
    }

    #[test]
    fn a_ticket_is_found_only_where_it_stands_alone() {
        assert_eq!(tickets("fixed (XY-12), see XY-3.", "XY"), ["XY-12", "XY-3"]);
        assert!(tickets("AXY-12 XY-a XY-12b", "XY").is_empty());
    }

    #[test]
    fn no_tracked_file_names_a_ticket() {
        // The key is spelled in two halves so this file does not name a ticket itself.
        let key = format!("{}{}", "A", "R");
        let root = util::root();
        let listed = std::process::Command::new("git").arg("ls-files").current_dir(&root).output().unwrap();
        let mut named = Vec::new();
        for file in String::from_utf8_lossy(&listed.stdout).lines() {
            let Ok(text) = std::fs::read_to_string(root.join(file)) else { continue };
            for ticket in tickets(&text, &key) {
                named.push(format!("{file}: {ticket}"));
            }
        }
        assert!(named.is_empty(), "the tree names tracker tickets; say what the thing is instead:\n{}", named.join("\n"));
    }
}
