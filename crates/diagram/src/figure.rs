//! What a notation draws, described without a toolkit: a node of an element is
//! a [`Card`] (an eyebrow, a name and compartments of rows), and an edge of an element a [`Line`]
//! (marks and texts at its ends, its dashes, its status, a note). The notation describes; the
//! canvas draws every description alike, with the routing, the hover, the labels, the drags, the
//! notes and the links, which stay shared.

/// A text on the canvas: words the shell translates, an English template with named arguments, or
/// the model's own text, shown as it is.
#[derive(Debug, Clone, PartialEq)]
pub enum Text {
    Said(&'static str, Vec<(&'static str, String)>),
    Plain(String),
}

impl Text {
    /// Words without arguments.
    pub fn said(template: &'static str) -> Text {
        Text::Said(template, Vec::new())
    }
}

/// A node's card: its eyebrow (what kind of element it is), its name and its compartments, each a
/// run of rows under a dividing line.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub eyebrow: Text,
    pub name: String,
    pub compartments: Vec<Vec<Row>>,
    /// The words for the rows past the card's height, one and many, each with `{n}`.
    pub more: (&'static str, &'static str),
    /// Rows the notation left out of its compartments, past the most it lists (a package holding
    /// many elements): counted with those past the card's height.
    pub hidden: usize,
    /// An element of another scope than the diagram's (a table of another schema): drawn dimmed,
    /// its outline dashed.
    pub elsewhere: bool,
    pub tone: Tone,
    /// Its name slanted: an abstract class.
    pub slanted: bool,
    /// What its name row says in braces after the name, as notation, never translated: `{final}`
    /// for a classifier nothing may specialize. None for nothing.
    pub braces: Option<String>,
}

/// The colours a card is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    /// The warm tint of a table.
    #[default]
    Tinted,
    /// The plain surface with the plain border, its eyebrow muted: a class.
    Plain,
    /// [`Tone::Plain`] with a tab at its top left, as a folder: a package.
    Tabbed,
    /// A constraint note (AR-133): the surface with the purple border and a folded corner, its
    /// eyebrow purple, its name over its expression in braces, linked to what it constrains.
    Constraint,
    /// A constraint's braces beside its element (AR-133): its one row, `{expression}`, in purple,
    /// with no card around it.
    Braces,
}

/// A row of a compartment: a mark before it (such as `PK`, shown while the status marks are on),
/// its text, and whether it is linked to another element.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub mark: Option<&'static str>,
    pub text: String,
    pub linked: bool,
    /// Slanted: an abstract operation.
    pub slanted: bool,
    /// Underlined: a static attribute or operation (UML's notation for a feature of the classifier
    /// itself, not of its instances).
    pub underlined: bool,
}

/// A mark at a line's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndMark {
    None,
    /// Crow's foot: a bar and a bar.
    ExactlyOne,
    /// Crow's foot: a bar and a circle.
    ZeroOrOne,
    /// Crow's foot: a crow's foot and a circle.
    ZeroOrMore,
    /// Crow's foot: a crow's foot and a bar.
    OneOrMore,
    /// A hollow triangle, the line stopping at its base: a generalization's general end.
    Triangle,
    /// An open arrowhead at the end: a navigable association end, a dependency's supplier.
    OpenArrow,
    /// A diamond, the line stopping at its far corner: the whole of a shared aggregation
    /// (hollow) or of a composition (filled).
    HollowDiamond,
    FilledDiamond,
}

impl EndMark {
    /// How far from its end the line stops, in diagram units, so the mark draws on its own.
    pub fn set_back(self) -> f32 {
        match self {
            EndMark::Triangle => 12.0,
            EndMark::HollowDiamond | EndMark::FilledDiamond => 20.0,
            _ => 0.0,
        }
    }
}

/// One end of a line: its mark, the text beside it, which the user may move (kept in a label of
/// the kind `slot`), and a role written above the line near it.
#[derive(Debug, Clone, PartialEq)]
pub struct End {
    pub mark: EndMark,
    pub text: Option<String>,
    pub slot: &'static str,
    pub role: Option<String>,
}

impl End {
    /// An end with a mark and nothing written.
    pub fn marked(mark: EndMark) -> End {
        End { mark, text: None, slot: "", role: None }
    }
}

/// A line of an element: its source end and its target end, and how it draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub ends: [End; 2],
    /// A relationship of the model: its own look (colour, width) applies, its labels show, and
    /// selected it shows handles at its ends. Otherwise it draws thin in the muted ink.
    pub styled: bool,
    pub dashed: bool,
    /// Whether it is complete, while the status marks are on: green when it is, orange when not;
    /// None draws it in its own colour.
    pub status: Option<bool>,
    /// A text beside the target's triangle, such as a generalization set's facts.
    pub note: Option<Vec<Text>>,
    /// Whether the element's problems show as a badge on the line, and whether the badge offers to
    /// resolve it (a many-to-many relationship waiting for its mapping table).
    pub badge: bool,
    pub resolvable: bool,
}

impl Default for Line {
    /// A plain solid line of a relationship, without marks.
    fn default() -> Line {
        Line {
            ends: [End::marked(EndMark::None), End::marked(EndMark::None)],
            styled: true,
            dashed: false,
            status: Some(true),
            note: None,
            badge: false,
            resolvable: false,
        }
    }
}
