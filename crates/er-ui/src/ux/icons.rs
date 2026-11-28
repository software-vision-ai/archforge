//! The design's icons under the shell's old path: the icons the shell and both notations draw,
//! from the shell's interfaces (`archforge_shell_api::ux::icons`), and the icons only ER draws,
//! kept here until ER's screens crate takes them.

pub use archforge_shell_api::ux::icons::*;

use super::{Icon, Seg::L};

/// The sort arrows of an index column: ascending points up, descending down (thin, as the design's
/// Icon / Sort · Ascending and · Descending).
pub const SORT_ASCENDING: Icon = Icon {
    size: [7.0, 9.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[L([3.5, 9.0], [3.5, 0.0]), L([0.0, 3.5], [3.5, 0.0]), L([3.5, 0.0], [7.0, 3.5])],
};
pub const SORT_DESCENDING: Icon = Icon {
    size: [7.0, 9.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[L([3.5, 0.0], [3.5, 9.0]), L([0.0, 5.5], [3.5, 9.0]), L([3.5, 9.0], [7.0, 5.5])],
};

/// The mark of a mapping table that becomes a line again (the design's Icon / Collapse): two
/// arrows meeting at a bar.
pub const MERGE_LINE: Icon = Icon {
    size: [12.0, 9.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        L([0.0, 4.5], [4.0, 4.5]),
        L([2.0, 2.5], [4.0, 4.5]),
        L([4.0, 4.5], [2.0, 6.5]),
        L([12.0, 4.5], [8.0, 4.5]),
        L([10.0, 2.5], [8.0, 4.5]),
        L([8.0, 4.5], [10.0, 6.5]),
        L([6.0, 0.0], [6.0, 9.0]),
    ],
};

/// The entity tool: a table with its header and first column.
pub const TOOL_ENTITY: Icon = Icon {
    size: [14.0, 11.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        L([0.0, 0.0], [14.0, 0.0]),
        L([14.0, 0.0], [14.0, 11.0]),
        L([14.0, 11.0], [0.0, 11.0]),
        L([0.0, 11.0], [0.0, 0.0]),
        L([0.0, 3.5], [14.0, 3.5]),
        L([4.5, 3.5], [4.5, 11.0]),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    /// No icon only one notation draws here has a standard size, as the table of sizes in `ux`
    /// named none of them before each icon carried its own (AR-842): each keeps the size it is
    /// given.
    #[test]
    fn no_icon_here_has_a_standard_size() {
        let icons: [(&str, Icon); 4] =
            [("SORT_ASCENDING", SORT_ASCENDING), ("SORT_DESCENDING", SORT_DESCENDING), ("MERGE_LINE", MERGE_LINE), ("TOOL_ENTITY", TOOL_ENTITY)];
        for (name, icon) in icons {
            assert_eq!(icon.standard_size, None, "{name}");
        }
    }
}
