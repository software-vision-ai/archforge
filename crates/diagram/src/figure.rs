//! What a notation draws, described without a toolkit (archforge-il6.3.4): a node of an element is
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
    /// An element of another scope than the diagram's (a table of another schema): drawn dimmed,
    /// its outline dashed.
    pub elsewhere: bool,
    pub tone: Tone,
}

/// The colours a card is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    /// The warm tint of a table.
    #[default]
    Tinted,
    /// The plain surface with the plain border, its eyebrow muted: a class.
    Plain,
}

/// A row of a compartment: a mark before it (such as `PK`, shown while the status marks are on),
/// its text, and whether it is linked to another element.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub mark: Option<&'static str>,
    pub text: String,
    pub linked: bool,
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
