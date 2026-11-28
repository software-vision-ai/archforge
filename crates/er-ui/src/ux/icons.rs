//! The design's icons as ER's screens draw them: the icons the shell and both notations draw,
//! from the shell's interfaces (`archforge_shell_api::ux::icons`), and the icons only ER draws.

pub use archforge_shell_api::ux::icons::*;

use super::{Icon, Seg::L};

/// A domain (the design's Icon / Domain): a typed box with a rule.
pub const DOMAIN: Icon = Icon {
    size: [11.0, 7.0],
    stroke: 1.1,
    standard_size: Some(11.0),
    segments: &[
        L([0.0, 0.0], [11.0, 0.0]),
        L([11.0, 0.0], [11.0, 7.0]),
        L([11.0, 7.0], [0.0, 7.0]),
        L([0.0, 7.0], [0.0, 0.0]),
        L([3.0, 3.5], [8.0, 3.5]),
    ],
};

/// A column of a table in the Models tree: a field with its leading cell.
pub const COLUMN: Icon = Icon {
    size: [12.0, 8.0],
    stroke: 1.2,
    standard_size: Some(11.0),
    segments: &[
        L([0.0, 0.0], [12.0, 0.0]),
        L([12.0, 0.0], [12.0, 8.0]),
        L([12.0, 8.0], [0.0, 8.0]),
        L([0.0, 8.0], [0.0, 0.0]),
        L([4.0, 0.0], [4.0, 8.0]),
    ],
};

/// A drag handle: two columns of three dots.
pub const GRIP: Icon = Icon {
    size: [4.0, 8.0],
    stroke: 1.6,
    standard_size: Some(8.0),
    segments: &[
        L([0.0, 0.0], [0.0, 0.0]),
        L([4.0, 0.0], [4.0, 0.0]),
        L([0.0, 4.0], [0.0, 4.0]),
        L([4.0, 4.0], [4.0, 4.0]),
        L([0.0, 8.0], [0.0, 8.0]),
        L([4.0, 8.0], [4.0, 8.0]),
    ],
};

/// A sequence (Icon / Sequence): lines of numbers and a plus.
pub const SEQUENCE: Icon = Icon {
    size: [10.0, 8.5],
    stroke: 1.1,
    standard_size: Some(10.0),
    segments: &[
        L([0.0, 0.0], [10.0, 0.0]),
        L([0.0, 3.5], [10.0, 3.5]),
        L([0.0, 7.0], [6.0, 7.0]),
        L([8.5, 5.5], [8.5, 8.5]),
        L([7.0, 7.0], [10.0, 7.0]),
    ],
};

/// An enum type (Icon / Enum): three marked lines.
pub const ENUM: Icon = Icon {
    size: [9.5, 7.0],
    stroke: 1.1,
    standard_size: Some(9.5),
    segments: &[
        L([0.0, 0.0], [0.5, 0.0]),
        L([3.0, 0.0], [9.5, 0.0]),
        L([0.0, 3.5], [0.5, 3.5]),
        L([3.0, 3.5], [9.5, 3.5]),
        L([0.0, 7.0], [0.5, 7.0]),
        L([3.0, 7.0], [9.5, 7.0]),
    ],
};

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

    /// Each icon here keeps the standard size it had where it was before: none for those the shell
    /// kept, as the table of sizes in `ux` named none of them before each icon carried its own
    /// (AR-842), and the domain's, the column's, the grip's, the sequence's and the enum type's
    /// from the shell's interfaces.
    #[test]
    fn every_icon_here_keeps_its_standard_size() {
        let icons: [(&str, Icon); 4] =
            [("SORT_ASCENDING", SORT_ASCENDING), ("SORT_DESCENDING", SORT_DESCENDING), ("MERGE_LINE", MERGE_LINE), ("TOOL_ENTITY", TOOL_ENTITY)];
        for (name, icon) in icons {
            assert_eq!(icon.standard_size, None, "{name}");
        }
        let sized: [(&str, Icon, f32); 5] =
            [("DOMAIN", DOMAIN, 11.0), ("COLUMN", COLUMN, 11.0), ("GRIP", GRIP, 8.0), ("SEQUENCE", SEQUENCE, 10.0), ("ENUM", ENUM, 9.5)];
        for (name, icon, size) in sized {
            assert_eq!(icon.standard_size, Some(size), "{name}");
        }
    }
}
