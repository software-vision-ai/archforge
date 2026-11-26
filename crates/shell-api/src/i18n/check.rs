//! The catalogues' row check, run at build time so a malformed row fails the build rather than a
//! lookup (`parse` skips it). It needs nothing but the standard library, so the build script of a
//! crate that holds catalogues takes this file as a module of its own (`#[path]`) instead of
//! building this crate, with the toolkit, for the build machine.

/// The first row of a catalogue's text that does not have four tab-separated columns: its line
/// number, counted from 1, and the columns found. Blank lines and comments (`#`) are no rows.
pub fn malformed_row(text: &str) -> Option<(usize, usize)> {
    text.lines().enumerate().filter(|(_, l)| !l.is_empty() && !l.starts_with('#')).find_map(|(n, line)| {
        let cols = line.split('\t').count();
        (cols != 4).then_some((n.saturating_add(1), cols))
    })
}
