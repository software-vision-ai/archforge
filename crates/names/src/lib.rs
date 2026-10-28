//! The rules for a name that becomes a file name, such as a project's: one set, used by the
//! interface to explain a name as it is typed and by the host to refuse one, so the two never
//! disagree. Pure: no file system, no toolkit.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

/// A project folder's extension.
pub const EXTENSION: &str = "archforge";

/// The longest name in bytes: with `.archforge` after it, it still fits the 255-byte file name
/// every common file system allows.
pub const MAX_BYTES: usize = 245;

/// The characters a file name cannot hold on some platform.
pub const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The names Windows keeps for devices, in any case and with any extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5",
    "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Why a name cannot be a file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// No name yet: nothing to explain, only nothing to create.
    Empty,
    ControlCharacter,
    Forbidden(char),
    TooLong {
        max_bytes: usize,
    },
    /// A space at the start or the end.
    EdgeSpace,
    LeadingDot,
    TrailingDot,
    /// A name Windows keeps for a device.
    Reserved,
}

/// What is wrong with `name`, the first rule it breaks, or `None` when it can be a file name on
/// every platform.
pub fn problem(name: &str) -> Option<Problem> {
    if name.is_empty() {
        return Some(Problem::Empty);
    }
    if name.chars().any(char::is_control) {
        return Some(Problem::ControlCharacter);
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN.contains(c)) {
        return Some(Problem::Forbidden(c));
    }
    if name.len() > MAX_BYTES {
        return Some(Problem::TooLong { max_bytes: MAX_BYTES });
    }
    if name.starts_with(char::is_whitespace) || name.ends_with(char::is_whitespace) {
        return Some(Problem::EdgeSpace);
    }
    if name.starts_with('.') {
        return Some(Problem::LeadingDot);
    }
    if name.ends_with('.') {
        return Some(Problem::TrailingDot);
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Some(Problem::Reserved);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_has_its_case() {
        let cases: &[(&str, Option<Problem>)] = &[
            ("Commerce platform", None),
            ("Ünïcødé ✓", None),
            ("v2.1 model", None),
            ("CONSOLE", None),
            ("Com10", None),
            ("", Some(Problem::Empty)),
            ("a\tb", Some(Problem::ControlCharacter)),
            ("a/b", Some(Problem::Forbidden('/'))),
            ("a:b", Some(Problem::Forbidden(':'))),
            (" lead", Some(Problem::EdgeSpace)),
            ("trail ", Some(Problem::EdgeSpace)),
            (".hidden", Some(Problem::LeadingDot)),
            ("ends.", Some(Problem::TrailingDot)),
            ("con", Some(Problem::Reserved)),
            ("LPT1.notes", Some(Problem::Reserved)),
            ("nul ", Some(Problem::EdgeSpace)),
        ];
        for (name, want) in cases {
            assert_eq!(problem(name), *want, "{name:?}");
        }
        assert_eq!(problem(&"x".repeat(MAX_BYTES)), None);
        assert_eq!(problem(&"x".repeat(MAX_BYTES + 1)), Some(Problem::TooLong { max_bytes: MAX_BYTES }));
        // The limit counts bytes: 62 four-byte emoji are 248 bytes.
        assert_eq!(problem(&"😀".repeat(62)), Some(Problem::TooLong { max_bytes: MAX_BYTES }));
        assert!(format!("{}.archforge", "x".repeat(MAX_BYTES)).len() <= 255);
    }
}
