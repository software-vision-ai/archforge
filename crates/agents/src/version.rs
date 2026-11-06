//! Runtime versions: read strictly from `--version`, and judged against the range Archforge
//! accepts.

use std::fmt;

use crate::RuntimeKind;

/// A release version, `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// The version `major.minor.patch`.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Version {
        Version { major, minor, patch }
    }

    /// Reads exactly `X.Y.Z`: three parts of ASCII digits, each within `u32`, and nothing else (no
    /// sign, no suffix, no fourth part). Anything else is `None`.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.').map(number);
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(Some(major)), Some(Some(minor)), Some(Some(patch)), None) => Some(Version { major, minor, patch }),
            _ => None,
        }
    }
}

/// One version part: 1 to 9 ASCII digits (so it always fits `u32`), else `None`.
fn number(part: &str) -> Option<u32> {
    if part.is_empty() || part.len() > 9 || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

impl fmt::Display for Version {
    /// Writes `major.minor.patch`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The versions of a runtime Archforge accepts: from `floor` up to, not including, `below`, and the
/// one it was tested with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VersionRange {
    /// The oldest version accepted.
    pub floor: Version,
    /// The first version no longer accepted (the next major version).
    pub below: Version,
    /// The newest version Archforge was tested with; a newer one in the range is accepted with a
    /// warning.
    pub tested: Version,
}

/// Where a version falls in a [`VersionRange`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fit {
    /// In the range, and no newer than the tested version.
    Tested,
    /// In the range but newer than the tested version: accepted, with "not yet tested with this
    /// version".
    NewerThanTested,
    /// Below the floor: counts as missing until it is updated.
    TooOld,
    /// At or past `below`: a version Archforge does not support.
    TooNew,
}

impl VersionRange {
    /// Where `version` falls in this range.
    pub fn fit(&self, version: Version) -> Fit {
        if version < self.floor {
            Fit::TooOld
        } else if version >= self.below {
            Fit::TooNew
        } else if version > self.tested {
            Fit::NewerThanTested
        } else {
            Fit::Tested
        }
    }
}

/// The version in a runtime's `--version` output, read from its first non-blank line in the form
/// that runtime prints: `2.1.317 (Claude Code)` for Claude, `codex-cli 0.166.0` for Codex.
pub(crate) fn read_version(kind: RuntimeKind, output: &str) -> Option<Version> {
    let line = output.lines().map(str::trim).find(|l| !l.is_empty())?;
    let token = match kind {
        RuntimeKind::Claude => line.split_whitespace().next()?,
        RuntimeKind::Codex => line.strip_prefix("codex-cli ")?.split_whitespace().next()?,
    };
    Version::parse(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a plain `X.Y.Z` of digits is a version.
    #[test]
    fn versions_are_read_strictly() {
        assert_eq!(Version::parse("2.1.317"), Some(Version::new(2, 1, 317)));
        assert_eq!(Version::parse("0.166.0"), Some(Version::new(0, 166, 0)));
        for bad in ["", "2.1", "2.1.3.4", "2.1.x", "+2.1.3", "2.1.3-beta", "2..3", "2.1.9999999999", " 2.1.3"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
    }

    /// Each runtime's own `--version` line is read, and nothing looser.
    #[test]
    fn each_runtime_s_version_line_is_read_in_its_own_form() {
        assert_eq!(read_version(RuntimeKind::Claude, "2.1.317 (Claude Code)\n"), Some(Version::new(2, 1, 317)));
        assert_eq!(read_version(RuntimeKind::Claude, "\n  2.2.0\n"), Some(Version::new(2, 2, 0)));
        assert_eq!(read_version(RuntimeKind::Codex, "codex-cli 0.166.0\n"), Some(Version::new(0, 166, 0)));
        assert_eq!(read_version(RuntimeKind::Codex, "0.166.0\n"), None);
        assert_eq!(read_version(RuntimeKind::Claude, "Claude Code 2.1.317"), None);
        assert_eq!(read_version(RuntimeKind::Claude, ""), None);
    }

    /// The owner's ranges: tested, newer in the range, too old, and the next major refused.
    #[test]
    fn the_ranges_decide_tested_newer_too_old_and_too_new() {
        let claude = RuntimeKind::Claude.range();
        assert_eq!(claude.fit(Version::new(2, 1, 317)), Fit::Tested);
        assert_eq!(claude.fit(Version::new(2, 1, 286)), Fit::Tested);
        assert_eq!(claude.fit(Version::new(2, 1, 318)), Fit::NewerThanTested);
        assert_eq!(claude.fit(Version::new(2, 9, 0)), Fit::NewerThanTested);
        assert_eq!(claude.fit(Version::new(2, 1, 285)), Fit::TooOld);
        assert_eq!(claude.fit(Version::new(1, 9, 999)), Fit::TooOld);
        assert_eq!(claude.fit(Version::new(3, 0, 0)), Fit::TooNew);
        let codex = RuntimeKind::Codex.range();
        assert_eq!(codex.fit(Version::new(0, 166, 0)), Fit::Tested);
        assert_eq!(codex.fit(Version::new(0, 167, 2)), Fit::NewerThanTested);
        assert_eq!(codex.fit(Version::new(0, 165, 9)), Fit::TooOld);
        assert_eq!(codex.fit(Version::new(1, 0, 0)), Fit::TooNew);
    }
}
