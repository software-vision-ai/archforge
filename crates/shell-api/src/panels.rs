//! What the right panels keep between frames (`State`: a name or description being typed, the
//! open menu, the body's scroll and the last refusal), running a command through the narrow
//! context, and the refusals in the user's words: an engine's refusal as the panels say it and a
//! typed refusal, asking the notations the shell registered for their own.

use archforge_mof::Uuid;
use egui::Rect;

use crate::cx::Ctx;
use crate::i18n::{fmt, tr, tr_ctx};
use crate::notation_ui::{Editor, MenuId, NotationUi, OpenEditor};
use crate::state::Panel;

/// The row of the open menu the arrow keys reached and the rows drawn above and below it, each as
/// its text and note (empty for none).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReachedRows {
    pub menu: String,
    pub row: (String, String),
    pub above: Option<(String, String)>,
    pub below: Option<(String, String)>,
}

/// A menu's rows numbered among the rows of their text and note, by the menu's id and a hash of
/// the rows they were numbered for, with the texts and notes more than one row shares.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MenuOrdinals {
    pub menu: String,
    pub rows: u64,
    pub nth: Vec<usize>,
    pub twins: Vec<(String, String)>,
}

/// What the panels keep between frames: a name or description being typed, the open menu, the
/// body's scroll and the last refusal. The notations' own editors are their own
/// ([`crate::notation_ui::NotationState`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// A name field being typed: the element and the text so far.
    pub name: Option<NameEdit>,
    /// The select menu that is open.
    pub menu: Option<MenuId>,
    /// How far the open menu's rows are scrolled, by the menu's id: a menu longer than the window
    /// has room for scrolls within it.
    pub menu_scroll: Option<(String, f32)>,
    /// The row of the open menu the arrow keys have reached, by the menu's id: Enter picks it.
    pub menu_reached: Option<(String, usize)>,
    /// That row and its neighbours as last drawn, so the reach stays on its row when rows come or
    /// go, and an arrow with Enter in its frame runs only a row that was in sight.
    pub menu_reached_rows: Option<ReachedRows>,
    /// The open menu's rows numbered among the rows of their text and note, made once per set of
    /// rows, so a row's toolkit id never hangs on how far the menu is scrolled.
    pub menu_ordinals: Option<MenuOrdinals>,
    /// The texts and notes of twin rows whose rows changed lately, by the menu's id, with the time
    /// until which a click on one of them picks nothing (longer while the pointer is held).
    pub menu_unsure: Option<(String, Vec<(String, String)>, f64)>,
    /// The item of an editable select's open menu the arrow keys have reached.
    pub highlight: Option<usize>,
    /// The Appearance panel's colour picker, while it is open.
    pub picker: Option<crate::appearance::Picker>,
    /// The last refusal of a change, shown at the panel's foot until the next change or another
    /// selection, and the selection it was given under.
    pub notice: Option<String>,
    pub notice_for: Option<Uuid>,
    /// The selection the open editors belong to: selecting anything else closes them.
    pub editors_for: Option<Uuid>,
    /// A description being typed under the heading (slice S2).
    pub description: Option<DescriptionDraft>,
    /// The body's scroll: how far it is scrolled, how tall its content was last
    /// frame, what it was scrolled for (a new panel or selection starts at the top), the editors
    /// open last frame with the refusal each showed, the editors that just opened or were refused
    /// and are still to be brought into view (true for a refusal), the scroll the body is drawn at,
    /// the body's visible rectangle, and the control of the body that had the keyboard last frame.
    pub scroll: f32,
    pub content: f32,
    pub scrolled_for: Option<(Panel, Option<Uuid>)>,
    pub open_editors: Vec<(Editor, OpenEditor)>,
    pub reveal: Vec<(Editor, bool)>,
    pub drawn: f32,
    pub body: Option<Rect>,
    pub held: Option<egui::Id>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEdit {
    pub target: Uuid,
    pub text: String,
}

/// A description being typed under the heading for the element `target`, written through
/// `element.describe` on Enter, when the keyboard leaves the field, or when the selection moves on
/// to another element (the shell's `panels::flush_description`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionDraft {
    pub target: Uuid,
    pub text: String,
    /// Its write was refused: it is kept, for the user to see again on `target` beside the
    /// refusal, and not written again until its text changes.
    pub refused: bool,
}

/// Runs a command on the open project's session; a refusal comes back as the panel says it
/// ([`refusal`]).
pub fn run_cx(cx: &mut Ctx, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = cx.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| refusal(cx.notation_uis, &e))
}

/// An engine's refusal as the panel says it: the reason alone, without the command's id or the
/// parameter it is about, as a sentence. A typed reason, a reason a command is not available and
/// the engine's own errors are said in the user's language ([`crate::engine_said`]), the notations
/// of `uis` asked for their own; a handler's
/// refusal not typed yet, or a reason not available the catalogues don't hold, in the engine's
/// English.
pub fn refusal(uis: &[&'static dyn NotationUi], e: &archforge_engine::EngineError) -> String {
    let reason = match e {
        archforge_engine::EngineError::Failed { error, .. } => match error.reason.as_ref().and_then(|r| said(uis, r)) {
            Some(words) => return words,
            None => error.message.clone(),
        },
        archforge_engine::EngineError::Disabled { reason, .. } => match crate::engine_said::disabled(uis, reason) {
            Some(words) => return words,
            None => (*reason).to_string(),
        },
        archforge_engine::EngineError::ReadOnly => return tr("The model cannot be changed at the moment; nothing was changed."),
        other => return crate::engine_said::said(other),
    };
    let mut chars = reason.chars();
    let first: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    format!("{first}{}{}", chars.as_str(), if reason.ends_with(['.', '!', '?']) { "" } else { "." })
}

/// A typed refusal in the user's words: the shell's own, which the engine, the diagrams or both
/// notations give (a name or a text refused, an id of no element or none given, an item's key no
/// list takes, an expression or a description refused, a coordinate, a default that is none of an
/// enumeration's literals), then the one notation of `uis` whose commands give it
/// ([`NotationUi::said`](crate::notation_ui::NotationUi::said)); none for a reason or a kind of
/// element the catalogues don't name, which the engine's sentence then says.
pub fn said(uis: &[&'static dyn NotationUi], reason: &archforge_engine::Reason) -> Option<String> {
    use archforge_engine::{DefaultKind, Reason};
    Some(match reason {
        Reason::NameTaken { kind, name } => fmt(&tr("There is {what} named {name} already."), &[("what", &a_kind(uis, kind)?), ("name", name)]),
        Reason::NameEmpty => tr("A name can't be empty."),
        Reason::TextEmpty => tr("The text can't be empty."),
        Reason::TooLong { max } => fmt(&tr("It can't be longer than {max} characters."), &[("max", &max.to_string())]),
        Reason::ControlCharacter => tr("It can't hold a control or hidden character."),
        Reason::EdgeSpace => tr("It can't start or end with a space."),
        Reason::IdEmpty { kind: archforge_diagram::project::ELEMENT } => tr("No element is given. Give its id."),
        Reason::NoElement { id } => fmt(&tr("There is no element with the id {id}."), &[("id", id)]),
        Reason::UnknownKey { key, takes } => {
            let template = if key.is_empty() {
                tr("An item holds a key without a name. It takes {takes}.")
            } else {
                tr("An item takes no {key}. It takes {takes}.")
            };
            fmt(&template, &[("key", key), ("takes", &takes.join(", "))])
        }
        Reason::NoExpression => tr("Type an expression. A constraint without one states no rule."),
        Reason::TakesNoDescription => tr("This element takes no description."),
        Reason::CoordinateNotANumber { axis } => fmt(&tr("The {axis} is not a number."), &[("axis", axis)]),
        Reason::CoordinateTooFar { axis } => {
            let max = archforge_diagram::rules::MAX_COORD.to_string();
            fmt(&tr("The {axis} is farther than {max} from the origin."), &[("axis", axis), ("max", &max)])
        }
        Reason::DefaultUnread { value, kind: DefaultKind::Literal(en) } => {
            fmt(&tr("{value} is not a literal of {enumeration}."), &[("value", value), ("enumeration", en)])
        }
        other => {
            return diagram_said(other).or_else(|| crate::engine_said::reason(other)).or_else(|| uis.iter().find_map(|ui| ui.said(other)));
        }
    })
}

/// A diagram command's refusal: an id or an element of the wrong kind where a diagram, a box, a line
/// or a label is taken, an element a diagram's kind cannot be of, a box's size, an element a diagram
/// does not show or shows already, a box on another diagram or nested too deep, a note's text or
/// link, too many bends, elements a notation does not remove yet and one element it does not, a
/// kind word that is no plain word, a diagram kind or a label's text none
/// takes, a list of boxes, or of what a style is set on, that is empty, no list of ids or names
/// one twice, a line connected against a relationship's rules, a note's link refused, a label on a
/// note's link or shown already, a style's colour not of the form #rrggbb, and a diagram arranged
/// that shows no box or would put one out of reach; and the shared commands' list of elements that
/// is empty, no list of ids or names one twice, an element of no notation, and a notation's elements
/// it does not describe yet; none for another reason. A coordinate, an id of no element and an
/// element not given are said with the shell's other own refusals ([`said`]).
fn diagram_said(reason: &archforge_engine::Reason) -> Option<String> {
    use archforge_diagram::commands::{ARRANGED, DIAGRAM_ELEMENT, NOTE, NOTE_LINK, STYLED};
    use archforge_diagram::project::{ELEMENT, NOTATION_ELEMENT};
    use archforge_diagram::rules::{MAX_COORD, MAX_SIZE, MIN_SIZE};
    use archforge_engine::{End, Reason};
    let between = |template: String| fmt(&template, &[("min", &MIN_SIZE.to_string()), ("max", &MAX_SIZE.to_string())]);
    Some(match reason {
        Reason::IdEmpty { kind: "diagram" } => tr("No diagram is given. Give its id."),
        Reason::IdEmpty { kind: "node" } => tr("No box is given. Give its id."),
        Reason::IdEmpty { kind: "edge" } => tr("No line is given. Give its id."),
        Reason::IdEmpty { kind: "label" } => tr("No label is given. Give its id."),
        Reason::IdEmpty { kind: STYLED } => tr("No diagram, box or line is given. Give its id."),
        Reason::NotA { kind: "diagram", name } => fmt(&tr("{name} is not a diagram."), &[("name", name)]),
        Reason::NotA { kind: "node", name } => fmt(&tr("{name} is not a box on a diagram."), &[("name", name)]),
        Reason::NotA { kind: "edge", name } => fmt(&tr("{name} is not a line on a diagram."), &[("name", name)]),
        Reason::NotA { kind: "label", name } => fmt(&tr("{name} is not a label on a line."), &[("name", name)]),
        Reason::NotA { kind: STYLED, name } => fmt(&tr("{name} is not a diagram, a box or a line."), &[("name", name)]),
        Reason::NotA { kind: DIAGRAM_ELEMENT, name } => fmt(&tr("{name} cannot have a diagram of this kind."), &[("name", name)]),
        Reason::SizeOutOfRange { side: "width" } => between(tr("The width is not between {min} and {max}.")),
        Reason::SizeOutOfRange { side: "height" } => between(tr("The height is not between {min} and {max}.")),
        Reason::NotShown { name } => fmt(&tr("A diagram of this kind does not show {name}."), &[("name", name)]),
        Reason::OutsideDiagram { name } => fmt(&tr("{name} is outside what this diagram shows."), &[("name", name)]),
        Reason::ShownAlready { name } => fmt(&tr("This diagram shows {name} already."), &[("name", name)]),
        Reason::OtherDiagram => tr("That box is on another diagram."),
        Reason::TooFew { kind: "node", min: 1 } => tr("Give at least one box."),
        Reason::TooFew { kind: STYLED, min: 1 } => tr("Give at least one diagram, box or line."),
        Reason::IdsExpected { kind: "node" } => tr("Give the boxes as a list of ids."),
        Reason::IdsExpected { kind: STYLED } => tr("Give the diagrams, boxes and lines as a list of ids."),
        Reason::NamedTwice { kind: "node" | STYLED, .. } => tr("An element is named twice. Name each element once."),
        Reason::TooMany { kind: "nesting level", max } => fmt(&tr("Boxes nest at most {max} deep."), &[("max", &max.to_string())]),
        Reason::ShowsElement => tr("This box shows an element and draws its text. Only a note takes a text of its own."),
        Reason::StraightLink => tr("A note's link is a straight line and does not bend."),
        Reason::NoRelationship => tr("A line between two elements shows a relationship. Name the relationship."),
        Reason::LacksEnd => tr("This element lacks an end, so no line shows it."),
        Reason::NotAtEnd { end: End::Source } => tr("The first box does not show the relationship's first end."),
        Reason::NotAtEnd { end: End::Target } => tr("The second box does not show the relationship's second end."),
        Reason::NotA { kind: NOTE, name } => fmt(&tr("This box shows {name}. Only a note links to a box or a line."), &[("name", name)]),
        Reason::Itself { relationship: NOTE_LINK, .. } => tr("A note does not link to itself."),
        Reason::NotLinkable => tr("A note links to a box that shows an element or to a relationship's line."),
        Reason::TargetOtherDiagram => tr("What the note links to is on another diagram."),
        Reason::Again { relationship: NOTE_LINK } => tr("The note is linked to it already."),
        Reason::LinkUnlabelled => tr("A note's link shows no element, so it takes no label."),
        Reason::Labelled { slot } => fmt(&tr("This line has the label text {slot} already."), &[("slot", slot)]),
        Reason::NotAColour { part: "fill" } => tr("Give the fill colour as #rrggbb, such as #3366cc."),
        Reason::NotAColour { part: "stroke" } => tr("Give the border or line colour as #rrggbb, such as #3366cc."),
        Reason::NotAColour { part: "text" } => tr("Give the text colour as #rrggbb, such as #3366cc."),
        Reason::TooFew { kind: ARRANGED, min: 1 } => tr("This diagram shows no box to arrange."),
        Reason::ArrangedTooFar => {
            let far = tr("Arranged, a box would land farther than {max} from the origin. Move the boxes nearer to it first.");
            fmt(&far, &[("max", &MAX_COORD.to_string())])
        }
        Reason::TooMany { kind: "waypoint", max } => fmt(&tr("A line bends at most {max} times."), &[("max", &max.to_string())]),
        Reason::NotRemovable { notation } => fmt(&tr("{notation} elements cannot be removed yet."), &[("notation", notation)]),
        Reason::ElementNotRemovable { name } => fmt(&tr("{name} cannot be removed yet."), &[("name", name)]),
        Reason::NotDescribable { notation } => fmt(&tr("{notation} elements cannot be described yet."), &[("notation", notation)]),
        Reason::TooFew { kind: ELEMENT, min: 1 } => tr("Give at least one element."),
        Reason::IdsExpected { kind: ELEMENT } => tr("Give the elements as a list of ids."),
        Reason::NamedTwice { kind: ELEMENT, .. } => tr("An element is named twice. Name each element once."),
        Reason::NotA { kind: NOTATION_ELEMENT, name } => fmt(&tr("{name} is not an element of any notation in this project."), &[("name", name)]),
        Reason::NotAPlainWord => tr("Use a plain word: lower case letters, digits, _, - and ., starting with a letter."),
        Reason::NotOneOf(no) => return not_one_of_said(no),
        _ => return None,
    })
}

/// A word none of its place's words: a diagram kind no notation knows, with the kinds there are,
/// and a label's text a line's element lacks, with those it has; none for another place.
fn not_one_of_said(no: &archforge_engine::NotOneOf) -> Option<String> {
    use archforge_engine::WordPlace;
    let known = no.known.join(", ");
    let args = [("word", no.word.as_str()), ("kinds", known.as_str()), ("slots", known.as_str())];
    Some(match (no.what, no.known.is_empty()) {
        (WordPlace::DiagramKind, false) => fmt(&tr("{word} is not a diagram kind. The kinds are {kinds}."), &args),
        (WordPlace::DiagramKind, true) => tr("This project holds no notation, so it knows no diagram kind."),
        (WordPlace::TextSlot, false) => fmt(&tr("This line has no label text {word}. It has {slots}."), &args),
        (WordPlace::TextSlot, true) => fmt(&tr("This line has no label text {word}. It has none."), &args),
        _ => return None,
    })
}

/// A kind of element as a refusal names it, with its article where the language has one: the
/// shell's own for the kinds the core, the diagrams or both notations name (an element, a
/// constraint, a skill), then the one notation of `uis` whose commands alone name it
/// ([`NotationUi::a_kind`](crate::notation_ui::NotationUi::a_kind)); none for a kind the
/// catalogues don't name.
pub fn a_kind(uis: &[&'static dyn NotationUi], kind: &str) -> Option<String> {
    Some(match kind {
        "element" => tr_ctx("refusal", "an element"),
        "constraint" => tr_ctx("refusal", "a constraint"),
        "skill" => tr_ctx("refusal", "a skill"),
        _ => return uis.iter().find_map(|ui| ui.a_kind(kind)),
    })
}
