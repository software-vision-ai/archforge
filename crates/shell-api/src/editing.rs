//! What editing on the canvas keeps between frames, which names no shell: the name typed in a
//! node, a note's text typed in place, the pointer gesture under way with the corner a resize
//! holds, and the element menu. The canvas's state holds them (`canvas::State`); the shell edits
//! with them (`archforge_ui_egui::editing` re-exports them under their old paths), and so will a
//! notation's screens, without the shell.

use archforge_mof::Uuid;
use egui::{Pos2, Rect};

/// A name being typed in a node's header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRename {
    pub node: Uuid,
    pub entity: Uuid,
    pub text: String,
    /// True until the field has taken the keyboard once.
    pub opened: bool,
    pub refused: Option<String>,
}

/// A note's text being typed in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEdit {
    pub node: Uuid,
    pub text: String,
    pub opened: bool,
    pub refused: Option<String>,
}

/// A corner handle of a selected node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A pointer gesture in progress, previewed on the canvas and committed on release.
#[derive(Debug, Clone, PartialEq)]
pub enum Gesture {
    /// The nodes moved together, from the screen point the drag started at.
    Move { nodes: Vec<Uuid>, from: Pos2 },
    /// A node resized by a corner, from its rectangle in diagram units.
    Resize { node: Uuid, corner: Corner, base: Rect, from: Pos2 },
    /// A selection rectangle from a screen point.
    Marquee { from: Pos2 },
    /// A line of a kind's connect tool drawn from a node, from the screen point the drag started
    /// at (ER's relationship, slice E2).
    Connect { tool: &'static str, from_node: Uuid, from: Pos2 },
    /// A note link drawn from a note's handle to a table or a relationship's line.
    LinkNote { note: Uuid },
    /// A cardinality text dragged off its place: its line, its end, the screen point the drag
    /// started at, its centre then and the end of the line it belongs to.
    EndText { edge: Uuid, end: u8, from: Pos2, centre: Pos2, anchor: Pos2 },
    /// A segment of a right-angled line slid across: the line, the segment's index in the route,
    /// the route as it was in diagram units, and the screen point the drag started at.
    Segment { edge: Uuid, index: usize, route: Vec<(f64, f64)>, from: Pos2 },
    /// A selected line's end dragged towards another element: the line's
    /// element, whether it is the source end, the screen point of the end that stays, and the
    /// revision of the model the drag began on, which the drop changes only if it still holds (AR-697).
    Reconnect { element: Uuid, source: bool, fixed: Pos2, at: archforge_engine::Revision },
    /// A label of the figures of no one node or line dragged where no node holds it yet (a UML
    /// generalization set's, AR-129): the element it stands for, the screen point the drag started
    /// at and the label's rectangle then.
    ExtraLabel { element: Uuid, from: Pos2, rect: Rect },
}

/// The entity menu, open at a screen point.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityMenu {
    pub node: Uuid,
    pub entity: Uuid,
    pub at: Pos2,
}
