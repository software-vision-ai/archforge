//! The data types the interface `DiagramKind` hands between the shell's canvas and a diagram kind,
//! which name no shell: what a kind's tool does, the tool itself, a press on the canvas as the
//! kind reads it, a line as the canvas drew it, and a label the kind drew of no one node or line.
//! The trait itself and the rows a kind adds to its element menu, which run on the `Shell`, stay
//! in the shell (`archforge_ui_egui::kinds` and `canvas` re-export these under their old paths).

use archforge_diagram::ProjectView;
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect};

use crate::ux::Icon;

/// What a kind's tool does on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolUse {
    /// A click on empty canvas places a new element there.
    Place,
    /// A drag from an element's node to another's connects them.
    Connect,
}

/// A tool a diagram kind brings to the toolbar, after Select and Pan.
pub struct KindTool {
    /// Its automation key (`workspace.tools.{key}`).
    pub key: &'static str,
    pub icon: &'static Icon,
    /// Its hover text, in the catalogue.
    pub hint: &'static str,
    pub use_: ToolUse,
    /// A chevron beside it that opens the kind's popover, with its key and hover text.
    pub chevron: Option<(&'static str, &'static str)>,
    /// Twice as wide, so a face that draws what the tool makes reads (a relationship's line); an
    /// icon's face is as wide as any tool's.
    pub wide: bool,
}

/// A press on the canvas as a diagram kind reads it (`DiagramKind::pointer`,
/// `DiagramKind::secondary_click`): the shared gestures' own view of it, read once.
pub struct Press<'a> {
    /// Where the pointer is on screen, when it is on the canvas.
    pub at: Option<Pos2>,
    /// Whether the primary button clicked, and whether as a double click (a third click in a row
    /// counts, as egui counts it a triple).
    pub clicked: bool,
    pub double: bool,
    pub store: &'a Store,
    pub notations: &'a ProjectView,
    /// The diagram the canvas shows, and what is selected.
    pub d: Obj,
    pub selection: &'a [Obj],
    /// The node under a screen point that takes the press: a card, or a package's tab, the one
    /// drawn on top.
    pub node_at: &'a dyn Fn(Pos2) -> Option<Obj>,
    /// The line under a screen point, nodes aside.
    pub line_at: &'a dyn Fn(Pos2) -> Option<Obj>,
}

/// A label of the figures of no one node or line as its diagram's kind drew it
/// (`DiagramKind::draw_extras`; a UML generalization set's, AR-129): the element it
/// stands for, where it was on screen, the node that holds where it was moved to on this diagram,
/// when it was moved, and what it says while the pointer is on it, its whole text in its
/// notation's words.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtraLabel {
    pub element: Uuid,
    pub rect: Rect,
    pub node: Option<Uuid>,
    pub hover: String,
}

/// A line as drawn this frame, for its diagram's kind to draw what goes with it
/// (`DiagramKind::line_extras`): the line, its element, its points on screen from
/// its source to its target, and whether it is drawn selected.
pub struct LineDrawn<'a> {
    pub line: Obj,
    pub element: Obj,
    pub points: &'a [Pos2],
    pub selected: bool,
}
