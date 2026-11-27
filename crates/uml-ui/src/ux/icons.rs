//! The design's icons as UML's screens draw them: the icons the shell and both notations draw,
//! from the shell's interfaces (`archforge_shell_api::ux::icons`), and the icons only UML draws.

pub use archforge_shell_api::ux::icons::*;

use super::{Icon, Seg::C, Seg::L};

/// A chevron pointing up and one pointing down: move a row up or down its list.
pub const MOVE_UP: Icon =
    Icon { size: [16.0, 16.0], stroke: 1.5, standard_size: None, segments: &[L([3.0, 10.5], [8.0, 5.5]), L([8.0, 5.5], [13.0, 10.5])] };
pub const MOVE_DOWN: Icon =
    Icon { size: [16.0, 16.0], stroke: 1.5, standard_size: None, segments: &[L([3.0, 5.5], [8.0, 10.5]), L([8.0, 10.5], [13.0, 5.5])] };

/// The class diagram's Class tool: a class box in its three compartments (name, attributes,
/// operations), drawn at the Entity tool's size.
pub const TOOL_CLASS: Icon = Icon {
    size: [14.0, 11.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        L([0.0, 0.0], [14.0, 0.0]),
        L([14.0, 0.0], [14.0, 11.0]),
        L([14.0, 11.0], [0.0, 11.0]),
        L([0.0, 11.0], [0.0, 0.0]),
        L([0.0, 3.5], [14.0, 3.5]),
        L([0.0, 7.25], [14.0, 7.25]),
    ],
};
/// The classifier tool's face while it places an interface: a lollipop, a ring on a stem that
/// stands on a short foot.
pub const TOOL_INTERFACE: Icon = Icon {
    size: [10.0, 14.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        C([5.0, 0.5], [6.657, 0.5], [8.0, 1.843], [8.0, 3.5]),
        C([8.0, 3.5], [8.0, 5.157], [6.657, 6.5], [5.0, 6.5]),
        C([5.0, 6.5], [3.343, 6.5], [2.0, 5.157], [2.0, 3.5]),
        C([2.0, 3.5], [2.0, 1.843], [3.343, 0.5], [5.0, 0.5]),
        L([5.0, 6.5], [5.0, 13.0]),
        L([1.5, 13.0], [8.5, 13.0]),
    ],
};
/// The classifier tool's face while it places an enumeration (the design's Tool · Enumeration): a
/// list of three rows, each a short mark before a longer line.
pub const TOOL_ENUMERATION: Icon = Icon {
    size: [14.0, 10.0],
    stroke: 1.2,
    standard_size: None,
    segments: &[
        L([0.0, 0.0], [2.0, 0.0]),
        L([5.0, 0.0], [14.0, 0.0]),
        L([0.0, 5.0], [2.0, 5.0]),
        L([5.0, 5.0], [14.0, 5.0]),
        L([0.0, 10.0], [2.0, 10.0]),
        L([5.0, 10.0], [14.0, 10.0]),
    ],
};
/// The classifier tool's face while it places a data type (the design's Tool · Data type): a pair
/// of braces, a value's fields between them.
pub const TOOL_DATA_TYPE: Icon = Icon {
    size: [14.0, 13.0],
    stroke: 1.2,
    standard_size: None,
    segments: &[
        C([4.0, 0.0], [2.5, 0.0], [2.0, 1.0], [2.0, 2.5]),
        L([2.0, 2.5], [2.0, 5.0]),
        C([2.0, 5.0], [2.0, 6.0], [1.2, 6.5], [0.0, 6.5]),
        C([0.0, 6.5], [1.2, 6.5], [2.0, 7.0], [2.0, 8.0]),
        L([2.0, 8.0], [2.0, 10.5]),
        C([2.0, 10.5], [2.0, 12.0], [2.5, 13.0], [4.0, 13.0]),
        C([10.0, 0.0], [11.5, 0.0], [12.0, 1.0], [12.0, 2.5]),
        L([12.0, 2.5], [12.0, 5.0]),
        C([12.0, 5.0], [12.0, 6.0], [12.8, 6.5], [14.0, 6.5]),
        C([14.0, 6.5], [12.8, 6.5], [12.0, 7.0], [12.0, 8.0]),
        L([12.0, 8.0], [12.0, 10.5]),
        C([12.0, 10.5], [12.0, 12.0], [11.5, 13.0], [10.0, 13.0]),
    ],
};
/// The classifier tool's face while it places a primitive type (the design's Tool · Primitive type):
/// a plain box with a small ring at its middle, one value and nothing inside it.
pub const TOOL_PRIMITIVE_TYPE: Icon = Icon {
    size: [13.0, 13.0],
    stroke: 1.2,
    standard_size: None,
    segments: &[
        L([0.0, 0.0], [13.0, 0.0]),
        L([13.0, 0.0], [13.0, 13.0]),
        L([13.0, 13.0], [0.0, 13.0]),
        L([0.0, 13.0], [0.0, 0.0]),
        C([4.5, 6.5], [4.5, 7.605], [5.395, 8.5], [6.5, 8.5]),
        C([6.5, 8.5], [7.605, 8.5], [8.5, 7.605], [8.5, 6.5]),
        C([8.5, 6.5], [8.5, 5.395], [7.605, 4.5], [6.5, 4.5]),
        C([6.5, 4.5], [5.395, 4.5], [4.5, 5.395], [4.5, 6.5]),
    ],
};
/// A class's attribute in the Models tree: a small field.
pub const ATTRIBUTE: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[L([4.0, 5.0], [12.0, 5.0]), L([12.0, 5.0], [12.0, 11.0]), L([12.0, 11.0], [4.0, 11.0]), L([4.0, 11.0], [4.0, 5.0])],
};
/// A class's operation in the Models tree: a pair of brackets.
pub const OPERATION: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        L([6.0, 3.0], [4.0, 6.0]),
        L([4.0, 6.0], [4.0, 10.0]),
        L([4.0, 10.0], [6.0, 13.0]),
        L([10.0, 3.0], [12.0, 6.0]),
        L([12.0, 6.0], [12.0, 10.0]),
        L([12.0, 10.0], [10.0, 13.0]),
    ],
};
/// A class in the Models tree and the search: its box in three compartments.
pub const CLASS: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        L([1.0, 2.0], [15.0, 2.0]),
        L([15.0, 2.0], [15.0, 14.0]),
        L([15.0, 14.0], [1.0, 14.0]),
        L([1.0, 14.0], [1.0, 2.0]),
        L([1.0, 6.0], [15.0, 6.0]),
        L([1.0, 10.0], [15.0, 10.0]),
    ],
};
/// A UML interface in the Models tree and the classifier kinds: the lollipop, a ring on a stem
/// that stands on a short foot.
pub const INTERFACE: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        C([8.0, 1.5], [9.933, 1.5], [11.5, 3.067], [11.5, 5.0]),
        C([11.5, 5.0], [11.5, 6.933], [9.933, 8.5], [8.0, 8.5]),
        C([8.0, 8.5], [6.067, 8.5], [4.5, 6.933], [4.5, 5.0]),
        C([4.5, 5.0], [4.5, 3.067], [6.067, 1.5], [8.0, 1.5]),
        L([8.0, 8.5], [8.0, 14.0]),
        L([4.0, 14.0], [12.0, 14.0]),
    ],
};
/// A UML enumeration in the Models tree and the classifier kinds: the tool's list of three rows,
/// drawn at the tree's size.
pub const ENUMERATION: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        L([2.0, 4.0], [4.0, 4.0]),
        L([6.5, 4.0], [14.0, 4.0]),
        L([2.0, 8.0], [4.0, 8.0]),
        L([6.5, 8.0], [14.0, 8.0]),
        L([2.0, 12.0], [4.0, 12.0]),
        L([6.5, 12.0], [14.0, 12.0]),
    ],
};
/// A UML data type in the Models tree and the classifier kinds: the tool's braces, drawn at the
/// tree's size.
pub const DATA_TYPE: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        C([5.0, 1.5], [3.5, 1.5], [3.0, 2.5], [3.0, 4.0]),
        L([3.0, 4.0], [3.0, 6.5]),
        C([3.0, 6.5], [3.0, 7.5], [2.2, 8.0], [1.0, 8.0]),
        C([1.0, 8.0], [2.2, 8.0], [3.0, 8.5], [3.0, 9.5]),
        L([3.0, 9.5], [3.0, 12.0]),
        C([3.0, 12.0], [3.0, 13.5], [3.5, 14.5], [5.0, 14.5]),
        C([11.0, 1.5], [12.5, 1.5], [13.0, 2.5], [13.0, 4.0]),
        L([13.0, 4.0], [13.0, 6.5]),
        C([13.0, 6.5], [13.0, 7.5], [13.8, 8.0], [15.0, 8.0]),
        C([15.0, 8.0], [13.8, 8.0], [13.0, 8.5], [13.0, 9.5]),
        L([13.0, 9.5], [13.0, 12.0]),
        C([13.0, 12.0], [13.0, 13.5], [12.5, 14.5], [11.0, 14.5]),
    ],
};
/// A UML primitive type in the Models tree and the classifier kinds: the tool's box with its ring,
/// drawn at the tree's size.
pub const PRIMITIVE_TYPE: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        L([1.5, 1.5], [14.5, 1.5]),
        L([14.5, 1.5], [14.5, 14.5]),
        L([14.5, 14.5], [1.5, 14.5]),
        L([1.5, 14.5], [1.5, 1.5]),
        C([6.0, 8.0], [6.0, 9.105], [6.895, 10.0], [8.0, 10.0]),
        C([8.0, 10.0], [9.105, 10.0], [10.0, 9.105], [10.0, 8.0]),
        C([10.0, 8.0], [10.0, 6.895], [9.105, 6.0], [8.0, 6.0]),
        C([8.0, 6.0], [6.895, 6.0], [6.0, 6.895], [6.0, 8.0]),
    ],
};
/// A UML association class in the Models tree and the search: a small class box over the middle of
/// a line, joined to it by a short tie, as the Relationship menu's row draws it.
pub const ASSOCIATION_CLASS: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        L([4.0, 1.5], [12.0, 1.5]),
        L([12.0, 1.5], [12.0, 7.0]),
        L([12.0, 7.0], [4.0, 7.0]),
        L([4.0, 7.0], [4.0, 1.5]),
        L([4.0, 4.25], [12.0, 4.25]),
        L([8.0, 8.5], [8.0, 9.5]),
        L([8.0, 11.0], [8.0, 12.0]),
        L([1.0, 13.5], [15.0, 13.5]),
    ],
};
/// A UML profile (AR-134, the design's Icon / Profile): a folder with guillemets on it, in the
/// Models tree, the New menu and the search.
pub const PROFILE: Icon = Icon {
    size: [14.0, 14.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        L([1.5, 3.5], [1.5, 12.5]),
        L([1.5, 12.5], [12.5, 12.5]),
        L([12.5, 12.5], [12.5, 5.0]),
        L([12.5, 5.0], [6.5, 5.0]),
        L([6.5, 5.0], [5.5, 3.5]),
        L([5.5, 3.5], [1.5, 3.5]),
        L([6.0, 7.0], [4.5, 8.75]),
        L([4.5, 8.75], [6.0, 10.5]),
        L([9.5, 7.0], [8.0, 8.75]),
        L([8.0, 8.75], [9.5, 10.5]),
    ],
};

/// A UML stereotype (AR-134, the design's Icon / Stereotype): a pair of guillemets, «», the
/// design's glyph centred in its box with the 1.5 margin the profile's icon keeps.
pub const STEREOTYPE: Icon = Icon {
    size: [14.0, 14.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        L([4.0, 3.5], [1.5, 7.0]),
        L([1.5, 7.0], [4.0, 10.5]),
        L([6.5, 3.5], [4.0, 7.0]),
        L([4.0, 7.0], [6.5, 10.5]),
        L([7.5, 3.5], [10.0, 7.0]),
        L([10.0, 7.0], [7.5, 10.5]),
        L([10.0, 3.5], [12.5, 7.0]),
        L([12.5, 7.0], [10.0, 10.5]),
    ],
};

/// A UML generalization set (AR-129, the design's Icon / Generalization set): a hollow triangle
/// over a dashed set line, in the Models tree, the menu and Properties.
pub const GENERALIZATION_SET: Icon = Icon {
    size: [14.0, 14.0],
    stroke: 1.1,
    standard_size: None,
    segments: &[
        L([7.0, 1.5], [12.0, 8.0]),
        L([12.0, 8.0], [2.0, 8.0]),
        L([2.0, 8.0], [7.0, 1.5]),
        L([1.0, 11.5], [3.0, 11.5]),
        L([4.5, 11.5], [6.5, 11.5]),
        L([8.0, 11.5], [10.0, 11.5]),
        L([11.5, 11.5], [13.0, 11.5]),
    ],
};
/// An enumeration's literal in the Models tree: one row of the enumeration's list.
pub const LITERAL: Icon =
    Icon { size: [16.0, 16.0], stroke: 1.3, standard_size: None, segments: &[L([3.0, 8.0], [5.0, 8.0]), L([7.5, 8.0], [13.0, 8.0])] };
/// A UML package: a box with its tab at the top left.
pub const PACKAGE: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.3,
    standard_size: None,
    segments: &[
        L([1.0, 2.0], [7.0, 2.0]),
        L([7.0, 2.0], [7.0, 5.0]),
        L([1.0, 2.0], [1.0, 14.0]),
        L([1.0, 14.0], [15.0, 14.0]),
        L([15.0, 14.0], [15.0, 5.0]),
        L([15.0, 5.0], [1.0, 5.0]),
    ],
};
/// A class diagram: two class boxes, the lower one generalizing the upper, as the ER diagram's
/// icon joins two tables.
pub const CLASS_DIAGRAM: Icon = Icon {
    size: [13.0, 12.0],
    stroke: 1.5,
    standard_size: None,
    segments: &[
        L([0.0, 0.0], [6.0, 0.0]),
        L([6.0, 0.0], [6.0, 5.0]),
        L([6.0, 5.0], [0.0, 5.0]),
        L([0.0, 5.0], [0.0, 0.0]),
        L([0.0, 2.5], [6.0, 2.5]),
        L([7.0, 7.0], [13.0, 7.0]),
        L([13.0, 7.0], [13.0, 12.0]),
        L([13.0, 12.0], [7.0, 12.0]),
        L([7.0, 12.0], [7.0, 7.0]),
        L([7.0, 9.5], [13.0, 9.5]),
        L([10.0, 7.0], [10.0, 3.0]),
        L([10.0, 3.0], [8.0, 3.0]),
        L([8.0, 3.0], [6.0, 2.0]),
        L([8.0, 3.0], [6.0, 4.0]),
        L([6.0, 2.0], [6.0, 4.0]),
    ],
};

/// An issue: a ring with a tick, beside the refusal an editor's card shows.
pub const ISSUE: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.5,
    standard_size: Some(17.0),
    segments: &[
        C([8.0, 1.5], [11.59, 1.5], [14.5, 4.41], [14.5, 8.0]),
        C([14.5, 8.0], [14.5, 11.59], [11.59, 14.5], [8.0, 14.5]),
        C([8.0, 14.5], [4.41, 14.5], [1.5, 11.59], [1.5, 8.0]),
        C([1.5, 8.0], [1.5, 4.41], [4.41, 1.5], [8.0, 1.5]),
        L([5.3, 8.2], [7.2, 10.0]),
        L([7.2, 10.0], [10.8, 6.3]),
    ],
};

/// Open: an arrow out to the top right, into what it opens.
pub const OPEN_IN: Icon = Icon {
    size: [16.0, 16.0],
    stroke: 1.5,
    standard_size: Some(10.0),
    segments: &[L([3.0, 13.0], [13.0, 3.0]), L([6.0, 3.0], [13.0, 3.0]), L([13.0, 3.0], [13.0, 10.0])],
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Each icon here keeps the standard size it had where it was before: none for those the shell
    /// kept, as the table of sizes in `ux` named none of them before each icon carried its own
    /// (AR-842), and the issue's and Open's from the shell's interfaces.
    #[test]
    fn every_icon_here_keeps_its_standard_size() {
        let icons: [(&str, Icon); 21] = [
            ("MOVE_UP", MOVE_UP),
            ("MOVE_DOWN", MOVE_DOWN),
            ("TOOL_CLASS", TOOL_CLASS),
            ("TOOL_INTERFACE", TOOL_INTERFACE),
            ("TOOL_ENUMERATION", TOOL_ENUMERATION),
            ("TOOL_DATA_TYPE", TOOL_DATA_TYPE),
            ("TOOL_PRIMITIVE_TYPE", TOOL_PRIMITIVE_TYPE),
            ("ATTRIBUTE", ATTRIBUTE),
            ("OPERATION", OPERATION),
            ("CLASS", CLASS),
            ("INTERFACE", INTERFACE),
            ("ENUMERATION", ENUMERATION),
            ("DATA_TYPE", DATA_TYPE),
            ("PRIMITIVE_TYPE", PRIMITIVE_TYPE),
            ("ASSOCIATION_CLASS", ASSOCIATION_CLASS),
            ("PROFILE", PROFILE),
            ("STEREOTYPE", STEREOTYPE),
            ("GENERALIZATION_SET", GENERALIZATION_SET),
            ("LITERAL", LITERAL),
            ("PACKAGE", PACKAGE),
            ("CLASS_DIAGRAM", CLASS_DIAGRAM),
        ];
        for (name, icon) in icons {
            assert_eq!(icon.standard_size, None, "{name}");
        }
        assert_eq!(ISSUE.standard_size, Some(17.0));
        assert_eq!(OPEN_IN.standard_size, Some(10.0));
    }
}
