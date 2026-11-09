//! Branch names. A person types ordinary text in any language, while Dolt takes only a narrow set
//! of characters and treats names that differ in case as one. So a name is stored through a
//! reversible encoding, and every stored name passes one check before it can reach a ``USE `…` ``.

use crate::chat::is_commit;
use crate::description::breaks_the_line;

/// The most characters a typed name may hold.
pub const BRANCH_NAME_CHARS: usize = 60;
/// The most bytes of a stored name: the longest typed name encodes to fewer.
const STORED_BYTES: usize = 512;
/// What starts a name stored as the hexadecimal of its UTF-8.
const MARKER: &str = "af-text-";

/// Why a typed text cannot be a branch name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchNameProblem {
    /// Nothing but white space.
    Empty,
    /// It holds a line break or another control character.
    NotOneLine,
    /// It starts or ends with white space, which would look like another name.
    EdgeSpace,
    /// It holds more than [`BRANCH_NAME_CHARS`] characters, `chars` of them.
    TooLong { chars: usize },
}

/// Why a name found in a project cannot be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsafeBranch {
    Empty,
    /// It is longer than any name Archforge stores, `bytes` long.
    TooLong {
        bytes: usize,
    },
    /// It holds this character, which is outside letters, digits, `-`, `_` and `.`.
    Character(char),
    /// It has the shape of a commit hash, which a `USE` would read as that commit.
    LooksLikeCommit,
    /// Dolt makes no branch of this shape: dots in the wrong places, a leading `-`, or `head`.
    NotABranchName,
}

/// A branch name that is safe to put in a statement: only [`SafeBranch::typed`] and
/// [`SafeBranch::found`] make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeBranch(String);

impl SafeBranch {
    /// The name to store for the text a person typed. Distinct texts give names distinct even to
    /// Dolt, which ignores case: only lower-case text can stay as it is, and the rest is encoded.
    pub fn typed(text: &str) -> Result<SafeBranch, BranchNameProblem> {
        if text.trim().is_empty() {
            return Err(BranchNameProblem::Empty);
        }
        if text.chars().any(breaks_the_line) {
            return Err(BranchNameProblem::NotOneLine);
        }
        if text.trim() != text {
            return Err(BranchNameProblem::EdgeSpace);
        }
        let chars = text.chars().count();
        if chars > BRANCH_NAME_CHARS {
            return Err(BranchNameProblem::TooLong { chars });
        }
        if stays_as_typed(text) {
            return Ok(SafeBranch(text.to_string()));
        }
        let hex: String = text.bytes().map(|b| format!("{b:02x}")).collect();
        Ok(SafeBranch(format!("{MARKER}{hex}")))
    }

    /// Checks a name found in a project, which may have been made elsewhere.
    pub fn found(stored: &str) -> Result<SafeBranch, UnsafeBranch> {
        if stored.is_empty() {
            return Err(UnsafeBranch::Empty);
        }
        if stored.len() > STORED_BYTES {
            return Err(UnsafeBranch::TooLong { bytes: stored.len() });
        }
        if let Some(c) = stored.chars().find(|c| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_' | '.')) {
            return Err(UnsafeBranch::Character(c));
        }
        if is_commit(&stored.to_ascii_lowercase()) {
            return Err(UnsafeBranch::LooksLikeCommit);
        }
        let misplaced = stored.starts_with(['.', '-']) || stored.ends_with('.') || stored.contains("..") || stored.ends_with(".lock");
        if misplaced || stored.eq_ignore_ascii_case("head") {
            return Err(UnsafeBranch::NotABranchName);
        }
        Ok(SafeBranch(stored.to_string()))
    }

    /// The name as stored, for a statement.
    pub fn stored(&self) -> &str {
        &self.0
    }

    /// The name to show: the text typed, or the name as it is when Archforge did not encode it.
    pub fn shown(&self) -> String {
        self.0
            .strip_prefix(MARKER)
            .and_then(text_of_hex)
            .filter(|text| SafeBranch::typed(text).is_ok_and(|again| again == *self))
            .unwrap_or_else(|| self.0.clone())
    }
}

/// Whether `text`, already a valid name, can be stored as it is: Dolt takes it, a statement cannot
/// mistake it for an option or a commit, and it cannot be taken for an encoded name.
fn stays_as_typed(text: &str) -> bool {
    let plain = text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    plain && !text.starts_with('-') && !text.starts_with(MARKER) && text != "head" && !is_commit(text)
}

/// The text whose UTF-8 `hex` spells, when it spells one.
fn text_of_hex(hex: &str) -> Option<String> {
    if !hex.is_ascii() || !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect();
    String::from_utf8(bytes?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text that is not a valid name is refused, each for its own reason.
    #[test]
    fn a_typed_name_is_refused_for_its_own_reason() {
        use BranchNameProblem::*;
        let long = "é".repeat(BRANCH_NAME_CHARS + 1);
        for (text, problem) in [
            ("", Empty),
            ("   ", Empty),
            ("a\nb", NotOneLine),
            ("a\tb", NotOneLine),
            ("a\u{2028}b", NotOneLine),
            (" a", EdgeSpace),
            ("a ", EdgeSpace),
            (long.as_str(), TooLong { chars: 61 }),
        ] {
            assert_eq!(SafeBranch::typed(text), Err(problem), "{text:?}");
        }
        assert!(SafeBranch::typed(&"é".repeat(BRANCH_NAME_CHARS)).is_ok(), "characters, not bytes");
    }

    /// A name from elsewhere that could break out of the statement is refused, each for its reason.
    #[test]
    fn a_found_name_outside_the_safe_set_is_refused_for_its_own_reason() {
        use UnsafeBranch::*;
        let long = "a".repeat(STORED_BYTES + 1);
        for (name, problem) in [
            ("", Empty),
            (long.as_str(), TooLong { bytes: 513 }),
            ("a`; DROP DATABASE project; `", Character('`')),
            ("a'b", Character('\'')),
            ("a\"b", Character('"')),
            ("a;b", Character(';')),
            ("feature/x", Character('/')),
            ("a b", Character(' ')),
            ("é", Character('é')),
            ("0123456789abcdefghijklmnopqrstuv", LooksLikeCommit),
            ("0123456789ABCDEFGHIJKLMNOPQRSTUV", LooksLikeCommit),
            (".", NotABranchName),
            ("..", NotABranchName),
            (".a", NotABranchName),
            ("a.", NotABranchName),
            ("a..b", NotABranchName),
            ("a.lock", NotABranchName),
            ("-D", NotABranchName),
            ("HEAD", NotABranchName),
        ] {
            assert_eq!(SafeBranch::found(name), Err(problem), "{name:?}");
        }
        for name in ["main", "Feature-1.2_x", "af-text-zz", "af-text-616263"] {
            assert_eq!(SafeBranch::found(name).unwrap().stored(), name);
        }
    }

    /// Every text comes back as typed, and what Dolt would refuse or misread is encoded.
    #[test]
    fn a_typed_name_round_trips_and_only_plain_text_stays_as_it_is() {
        let emoji = "😀".repeat(BRANCH_NAME_CHARS);
        for text in [
            "Release 1",
            "李雷 Ü",
            "a'b`c;d/e",
            "a.b",
            "..",
            "-D",
            "--amend",
            "HEAD",
            "head",
            "Foo",
            "af-text-616263",
            "af-text-466f6f",
            emoji.as_str(),
        ] {
            let name = SafeBranch::typed(text).unwrap();
            assert!(name.stored().starts_with(MARKER), "{text:?} is encoded");
            assert_eq!(SafeBranch::found(name.stored()).as_ref(), Ok(&name), "{text:?} passes its own check");
            assert_eq!(name.shown(), text);
        }
        let hash = "0123456789abcdefghijklmnopqrstuv";
        assert_eq!(SafeBranch::typed(hash).unwrap().shown(), hash);
        assert_ne!(SafeBranch::typed(hash).unwrap().stored(), hash);
        for text in ["main", "release-1_x"] {
            assert_eq!(SafeBranch::typed(text).unwrap().stored(), text);
            assert_eq!(SafeBranch::typed(text).unwrap().shown(), text);
        }
    }

    /// No two texts share a stored name, even with case ignored as Dolt ignores it.
    #[test]
    fn two_names_never_encode_to_the_same_result() {
        let texts = [
            "foo",
            "Foo",
            "FOO",
            "a b",
            "a_b",
            "a-b",
            "a.b",
            "ab",
            "af-text-6162",
            "af-text-616263",
            "af-text-466f6f",
            "abc",
            "é",
            "e\u{301}",
            "head",
            "HEAD",
        ];
        let mut stored: Vec<String> = texts.iter().map(|t| SafeBranch::typed(t).unwrap().stored().to_ascii_lowercase()).collect();
        stored.sort();
        stored.dedup();
        assert_eq!(stored.len(), texts.len(), "{stored:?}");
    }

    /// What only looks encoded is shown as it is.
    #[test]
    fn a_name_from_elsewhere_shows_as_it_is() {
        // Not hexadecimal, an odd length, not UTF-8, upper-case digits, and text that would not be encoded.
        for name in ["af-text-zz", "af-text-616", "af-text-ff", "af-text-C3A9", "af-text-616263", "Feature-1"] {
            assert_eq!(SafeBranch::found(name).unwrap().shown(), name);
        }
    }
}
