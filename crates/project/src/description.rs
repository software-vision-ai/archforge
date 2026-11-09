//! The description a person gives a saved version. It stays in the project's history, so its
//! shape is checked before anything is committed; checking it against the kept API keys needs the
//! secure store and is the host's part.

/// The most characters a description may hold.
pub const DESCRIPTION_CHARS: usize = 200;

/// Why a text cannot be a description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptionProblem {
    /// Nothing but white space: a save without a description is the way to say nothing.
    Empty,
    /// It holds a line break or another control character.
    NotOneLine,
    /// It holds more than [`DESCRIPTION_CHARS`] characters, `chars` of them.
    TooLong { chars: usize },
}

/// Checks that `text` can be a description: not empty, one line, at most [`DESCRIPTION_CHARS`]
/// characters. It is refused rather than changed, so what is saved is what was typed.
pub fn check_description(text: &str) -> Result<(), DescriptionProblem> {
    if text.trim().is_empty() {
        return Err(DescriptionProblem::Empty);
    }
    if text.chars().any(breaks_the_line) {
        return Err(DescriptionProblem::NotOneLine);
    }
    let chars = text.chars().count();
    if chars > DESCRIPTION_CHARS {
        return Err(DescriptionProblem::TooLong { chars });
    }
    Ok(())
}

/// Whether `c` has no place in one line of text: the control characters, and the Unicode line and
/// paragraph separators, which are not control characters but do break a line.
pub(crate) fn breaks_the_line(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The limits hold at their edges, counted in characters.
    #[test]
    fn a_description_is_one_line_of_at_most_two_hundred_characters() {
        assert_eq!(check_description("Renamed Customer to Client"), Ok(()));
        assert_eq!(check_description(&"é".repeat(DESCRIPTION_CHARS)), Ok(()), "characters, not bytes");
        assert_eq!(check_description(&"é".repeat(DESCRIPTION_CHARS + 1)), Err(DescriptionProblem::TooLong { chars: 201 }));
        for empty in ["", "   ", "\n", "\u{2003}"] {
            assert_eq!(check_description(empty), Err(DescriptionProblem::Empty), "{empty:?}");
        }
        for broken in ["a\nb", "a\rb", "a\tb", "a\0b", "a\u{85}b", "a\u{2028}b", "a\u{2029}b", "a\n"] {
            assert_eq!(check_description(broken), Err(DescriptionProblem::NotOneLine), "{broken:?}");
        }
    }
}
