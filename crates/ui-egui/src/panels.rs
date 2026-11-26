//! The right panels (slice W3 of the workspace chrome, from the design's pages 03 and 04):
//! Properties shows the selection under its heading and description — a diagram's name and the
//! element it is of, an element's body as its notation draws it, and the problems the rules find —
//! and Skills shows the skills on the selection and the ones it inherits, each opening in its tab. Every change goes through the
//! engine's commands, so undo covers it; a refusal shows at the panel's foot as the engine's
//! reason, without the command's id, until the selection moves on.
//! Agent and Appearance say what is true until their slices.
//!
//! The geometry is the design's: a section is 16 px in from the panel's edges with a full-width
//! line over it, a field is 28 px tall and filled, its label 11 px above, a row 36 px, an editor
//! a bordered card.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

pub use archforge_shell_api::notation_ui::MenuId;
pub(crate) use archforge_shell_api::notation_ui::{Editor, OpenEditor, Subject};

use crate::home::control;
use crate::i18n::{fmt, tr, tr_ctx};
use crate::notation_ui::Words;
use crate::state::Panel;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

pub(crate) const PAD: f32 = 16.0;
pub(crate) const FIELD: f32 = 28.0;
pub(crate) const ROW: f32 = 36.0;
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
    pub(crate) open_editors: Vec<(Editor, OpenEditor)>,
    pub(crate) reveal: Vec<(Editor, bool)>,
    pub(crate) drawn: f32,
    pub body: Option<Rect>,
    pub held: Option<egui::Id>,
}

/// What is left under the last control of a scrolled body, for its trailing text.
const SCROLL_TAIL: f32 = 48.0;

/// The editors of `now` to bring into view: one opened or moved to another place since `was`, or,
/// true beside it, one whose card shows a new refusal (AR-469). A refusal that goes or stays, and an
/// editor that changed nothing, bring nothing into view, so a body scrolled away stays put.
fn to_reveal(now: &[(Editor, OpenEditor)], was: &[(Editor, OpenEditor)]) -> Vec<(Editor, bool)> {
    let before = |editor: Editor| was.iter().find(|(e, _)| *e == editor).and_then(|(_, open)| open.as_ref());
    let reveal = |(editor, now): &(Editor, OpenEditor)| match (now, before(*editor)) {
        (Some((place, why)), Some((was, said))) if place == was => (why.is_some() && why != said).then_some((*editor, true)),
        (Some(_), _) => Some((*editor, false)),
        (None, _) => None,
    };
    now.iter().filter_map(reveal).collect()
}

/// Whether a name is being typed in `s`, or something of a notation's `notations` is being typed
/// or drawn or one of their editor cards is open ([`crate::notation_ui::NotationState::editing`]):
/// the fields that only show what is stored are not counted.
pub(crate) fn editing(s: &State, notations: &crate::notation_ui::NotationStates) -> bool {
    s.name.is_some() || notations.editing()
}

/// Brings the card of `editor`, once drawn, into the body's view when [`to_reveal`] asks: an opened
/// card's foot if it fits, else its top; a refused card's foot, with its Apply, Cancel and refusal,
/// also when the card is taller than the body. A card drawn after another's reveal counts the scroll
/// that one moved, so the last reveal of a draw is the one in view.
pub(crate) fn reveal(shell: &mut Shell, ui: &egui::Ui, editor: Editor, card: Rect) {
    let Some(body) = shell.panels.body else { return };
    let Some(at) = shell.panels.reveal.iter().position(|(e, _)| *e == editor) else { return };
    let (_, refused) = shell.panels.reveal.remove(at);
    let part = if refused { Rect::from_min_max(pos2(card.left(), card.top().max(card.bottom() - body.height() + 16.0)), card.max) } else { card };
    shell.panels.scroll += into_view(body, part.translate(vec2(0.0, shell.panels.drawn - shell.panels.scroll)));
    ui.ctx().request_repaint();
}

/// How far a body showing `body` scrolls on to bring `part` into view: its foot if it fits, else
/// its top, each 8 points clear of the edge; nothing for a part already in view.
pub(crate) fn into_view(body: Rect, part: Rect) -> f32 {
    let down = (part.bottom() + 8.0 - body.bottom()).min(part.top() - 8.0 - body.top());
    let up = body.top() + 8.0 - part.top();
    if down > 0.0 { down } else { -up.max(0.0) }
}

/// The body follows the keyboard (AR-470). Before it draws: while one of its controls has the
/// keyboard, no text is being typed and no menu is open, Page Up, Page Down, Home and End scroll
/// it, as they scroll the lists of versions.
fn body_keys(shell: &mut Shell, ui: &egui::Ui, body: Rect) {
    let held = ui.memory(|m| m.focused());
    if held.is_some() && held == shell.panels.held && shell.panels.menu.is_none() && !ui.ctx().text_edit_focused() {
        crate::version_details::page_keys(ui, &mut shell.panels.scroll, (body.height() - ROW).max(ROW));
    }
}

/// After the body has drawn at the scroll `drawn`: a control of the body the keyboard has just
/// moved to, by Tab or any way but the pointer, is brought into view, so Tab goes on past the
/// fold. The toolkit hands the keyboard to a control that is laid out, in view or not.
fn follow_keys(shell: &mut Shell, ui: &egui::Ui, body: Rect, drawn: f32) {
    let now = ui.memory(|m| m.focused()).and_then(|id| Some((id, shell.registry.laid_out(id)?)));
    let was = std::mem::replace(&mut shell.panels.held, now.map(|(id, _)| id));
    let pointer = ui.input(|i| i.pointer.any_down() || i.pointer.any_released());
    if let Some((_, at)) = now.filter(|(id, _)| Some(*id) != was && !pointer) {
        // An editor brought into view this frame has moved the scroll already.
        shell.panels.scroll += into_view(body, at.translate(vec2(0.0, drawn - shell.panels.scroll)));
        ui.ctx().request_repaint();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEdit {
    pub target: Uuid,
    pub text: String,
}

/// A description being typed under the heading for the element `target`, written through
/// `element.describe` on Enter, when the keyboard leaves the field, or when the selection moves on
/// to another element ([`flush_description`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionDraft {
    pub target: Uuid,
    pub text: String,
    /// Its write was refused: it is kept, for the user to see again on `target` beside the
    /// refusal, and not written again until its text changes.
    pub refused: bool,
}

fn subject(shell: &Shell, project: &str) -> Subject {
    let Some(s) = shell.session.as_ref() else {
        return Subject::root(project);
    };
    let store = s.doc();
    match s.view().selection.first().copied().filter(|o| store.is_alive(*o)) {
        Some(o) => {
            let class = store.class_of(o).ok();
            let words = match class {
                Some(c) if c == archforge_core::SKILL => Words {
                    kind: tr("Skill · model"),
                    word: tr("Skill"),
                    this: tr("This skill"),
                    on_this: tr("On this skill · {n}"),
                    name: archforge_core::skills::name_of(store, o),
                },
                Some(c) if c == archforge_diagram::DIAGRAM => {
                    let schema = crate::diagrams::schema_of(store, o).map(|s| archforge_core::name_of(store, s)).unwrap_or_default();
                    Words {
                        kind: fmt(&tr("Diagram · of {schema}"), &[("schema", &schema)]),
                        word: tr("Diagram"),
                        this: tr("This diagram"),
                        on_this: tr("On this diagram · {n}"),
                        name: crate::diagrams::diagram_name(store, o),
                    }
                }
                _ => crate::notation_ui::of(shell.notation_uis, store, o).and_then(|ui| ui.words(store, o)).unwrap_or_else(|| Words {
                    kind: tr("Element · model"),
                    word: tr("Element"),
                    this: tr("This element"),
                    on_this: tr("On this element · {n}"),
                    name: archforge_core::name_of(store, o),
                }),
            };
            Subject::of(o, store.uuid(o).ok(), class, words)
        }
        None => Subject::root(project),
    }
}

/// Runs a command on the open project's session; a refusal comes back as the panel says it
/// ([`refusal`]).
pub(crate) fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| refusal(&e))
}

/// An engine's refusal as the panel says it: the reason alone, without the command's id or the
/// parameter it is about, as a sentence. A typed reason, a reason a command is not available and
/// the engine's own errors are said in the user's language ([`crate::engine_said`]); a handler's
/// refusal not typed yet, or a reason not available the catalogues don't hold, in the engine's
/// English.
pub(crate) fn refusal(e: &archforge_engine::EngineError) -> String {
    let reason = match e {
        archforge_engine::EngineError::Failed { error, .. } => match error.reason.as_ref().and_then(said) {
            Some(words) => return words,
            None => error.message.clone(),
        },
        archforge_engine::EngineError::Disabled { reason, .. } => match crate::engine_said::disabled(reason) {
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
/// enumeration's literals), then the one notation's whose commands give it
/// ([`NotationUi::said`](crate::notation_ui::NotationUi::said)); none for a reason or a kind of
/// element the catalogues don't name, which the engine's sentence then says.
pub(crate) fn said(reason: &archforge_engine::Reason) -> Option<String> {
    use archforge_engine::{DefaultKind, Reason};
    Some(match reason {
        Reason::NameTaken { kind, name } => fmt(&tr("There is {what} named {name} already."), &[("what", &a_kind(kind)?), ("name", name)]),
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
            return diagram_said(other)
                .or_else(|| crate::engine_said::reason(other))
                .or_else(|| crate::notation_ui::UIS.iter().find_map(|ui| ui.said(other)));
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
/// constraint, a skill), then the one notation's whose commands alone name it
/// ([`NotationUi::a_kind`](crate::notation_ui::NotationUi::a_kind)); none for a kind the
/// catalogues don't name.
pub(crate) fn a_kind(kind: &str) -> Option<String> {
    Some(match kind {
        "element" => tr_ctx("refusal", "an element"),
        "constraint" => tr_ctx("refusal", "a constraint"),
        "skill" => tr_ctx("refusal", "a skill"),
        _ => return crate::notation_ui::UIS.iter().find_map(|ui| ui.a_kind(kind)),
    })
}

/// The string `prop` of `o`; empty when it has none.
pub(crate) fn string_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Drops a name or description being typed whose element is gone; the notations drop their own
/// editors ([`crate::notation_ui::NotationState::retain_valid`]).
pub fn retain_valid(state: &mut State, store: &Store) {
    if state.name.as_ref().is_some_and(|n| store.by_uuid(n.target).is_none()) {
        state.name = None;
    }
    if state.description.as_ref().is_some_and(|d| store.by_uuid(d.target).is_none()) {
        state.description = None;
    }
}

// Shared drawing.

/// A section's full-width top line and its title row with an optional `+`; gives the y under it,
/// or none when the `+` was clicked this frame.
pub(crate) fn section(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, title: &str, add: Option<(&str, &str)>) -> Option<f32> {
    let paint = ui.painter();
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)), 0.0, p.border);
    ux::text(paint, pos2(r.left() + PAD, y + 17.0), title, types::UI, p.text, Some((r.width() - 2.0 * PAD - 40.0).max(0.0)));
    if let Some((id, hint)) = add {
        let b = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
        ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
        if control(shell, ui, id, "button", b, true).on_hover_text(tr(hint)).clicked() {
            return None;
        }
    }
    Some(y + 50.0)
}

/// What a text field that writes when it is left asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldOut {
    Idle,
    /// The text as typed is to be written: Enter, or the field left another way.
    Commit,
    /// Escape: the edit is given up and the value as last written comes back; nothing is written.
    Cancel,
}

/// A label over a filled text field that commits on Enter, and when it loses the keyboard if
/// `commit_on_blur`; gives whether it was committed this frame ([`edit_field`] says more).
pub(crate) fn text_field(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    text: &mut String,
    limit: usize,
    take_focus: bool,
    commit_on_blur: bool,
) -> bool {
    edit_field(shell, ui, p, at, width, id, label, text, limit, take_focus, commit_on_blur) == FieldOut::Commit
}

/// Records which widget held the keyboard as each frame begins, before the toolkit takes it away
/// on Escape from a field that does not keep Escape (AR-417).
struct KeysAtStart;

impl egui::Plugin for KeysAtStart {
    /// The recorder's name, for the toolkit's profiling.
    fn debug_name(&self) -> &'static str {
        "archforge.keys-at-start"
    }

    /// Runs before the frame's input reaches the toolkit's focus: the keyboard is still where it was.
    fn input_hook(&mut self, ctx: &egui::Context, _input: &mut egui::RawInput) {
        let held = ctx.memory(|m| m.focused());
        ctx.data_mut(|d| d.insert_temp(widgets::egui_id("archforge.keys-at-start"), held));
    }
}

/// The widget that held the keyboard as this frame began, so Escape goes to what the person was
/// in even when the toolkit has already let go of it; none when nothing held it. The recorder is
/// registered on first use (later calls are ignored) and answers from the next frame on.
pub(crate) fn keys_at_start(ui: &egui::Ui) -> Option<egui::Id> {
    ui.ctx().add_plugin(KeysAtStart);
    ui.data(|d| d.get_temp::<Option<egui::Id>>(widgets::egui_id("archforge.keys-at-start"))).flatten()
}

/// [`text_field`], saying what it asked for. One that commits on blur is written in place, as in
/// Models: Enter or leaving it commits, Escape gives the edit up (AR-417); others leave Escape be.
pub(crate) fn edit_field(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    text: &mut String,
    limit: usize,
    take_focus: bool,
    commit_on_blur: bool,
) -> FieldOut {
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, id, "field", field);
    let inner = field_inner(ui, field);
    let eid = widgets::egui_id(id);
    // Escape stays with a field that commits on blur and cancels there (checklist 118).
    let cancel = commit_on_blur && ui.memory(|m| m.has_focus(eid)) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    // The toolkit still reports the field as left on the frame after, which is no commit.
    let just_cancelled = ui.data_mut(|d| d.remove_temp::<bool>(eid.with("cancelled"))).unwrap_or(false);
    if cancel {
        ui.memory_mut(|m| m.surrender_focus(eid));
        ui.data_mut(|d| d.insert_temp(eid.with("cancelled"), true));
    }
    let filter = egui::EventFilter { escape: commit_on_blur, horizontal_arrows: true, vertical_arrows: true, tab: false };
    let edit = egui::TextEdit::singleline(text)
        .id(eid)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text.into())
        .char_limit(limit)
        .event_filter(filter)
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(label);
    if take_focus {
        out.response.request_focus();
    }
    if out.response.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    // A one-line field gives up the keyboard on Enter in the same frame.
    let enter = (out.response.has_focus() || out.response.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if cancel {
        FieldOut::Cancel
    } else if enter || (commit_on_blur && out.response.lost_focus() && !just_cancelled) {
        FieldOut::Commit
    } else {
        FieldOut::Idle
    }
}

/// Where a field's one line of text goes: 8 points in from the sides, its row centred on the
/// field's middle line, as a select's value is.
pub(crate) fn field_inner(ui: &egui::Ui, field: Rect) -> Rect {
    let row = ui.fonts_mut(|f| f.row_height(&types::SMALL.font()));
    Rect::from_min_max(
        pos2(field.left() + 8.0, field.center().y - row / 2.0),
        pos2((field.right() - 8.0).max(field.left() + 8.0), field.center().y + row / 2.0),
    )
}

/// A filled select showing `value`; a click opens or closes `menu`.
pub(crate) fn select_field(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    value: &str,
    menu: MenuId,
) -> Rect {
    let paint = ui.painter();
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(paint, field.left() + 8.0, field.center().y, value, types::SMALL, p.text, Some((width - 36.0).max(0.0)));
    ux::icon_mid(paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    // Its label and what it shows, for assistive technology.
    if control(shell, ui, id, "select", field, true).accessible_name(format!("{label}: {value}")).clicked() {
        shell.panels.menu = if shell.panels.menu == Some(menu) { None } else { Some(menu) };
    }
    field
}

/// [`select_field`] for a package's path, its names from the top in `parts` (`Commerce platform
/// › Orders`): too long for the field, it is cut from its front, so the package's own name shows
/// (`… › Orders`); the whole path is its tooltip and accessible name.
pub(crate) fn path_select_field(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    parts: &[String],
    menu: MenuId,
) -> Rect {
    let paint = ui.painter();
    let path = parts.join(PATH_SEP);
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid_front(paint, field.left() + 8.0, field.center().y, parts, PATH_SEP, types::SMALL, p.text, (width - 36.0).max(0.0));
    ux::icon_mid(paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    let resp = control(shell, ui, id, "select", field, true).accessible_name(format!("{label}: {path}"));
    let resp = if path.is_empty() { resp } else { resp.on_hover_text(&path) };
    if resp.clicked() {
        shell.panels.menu = if shell.panels.menu == Some(menu) { None } else { Some(menu) };
    }
    field
}

/// What joins a package's path: `Commerce platform › Orders`.
pub(crate) const PATH_SEP: &str = " › ";

/// An editable select: what it shows and offers.
pub(crate) struct Combo<'a> {
    pub id: &'a str,
    pub label: &'a str,
    /// Its accessible name, which tells it from its namesakes.
    pub name: &'a str,
    /// Shown, muted, while the text is empty; never a value.
    pub placeholder: &'a str,
    pub options: &'a [&'a str],
    /// The option the model's value is, ticked in the menu.
    pub current: Option<usize>,
    /// The chevron's tooltip and its accessible name, which names the field too; the line under
    /// the options, and the last row, which empties it.
    pub pick: &'a str,
    pub pick_name: &'a str,
    pub hint: &'a str,
    pub clear: &'a str,
    /// Why the text was refused: ringed in the danger colour, and told to assistive technology;
    /// the caller draws it under the field.
    pub refused: Option<&'a str>,
    pub menu: MenuId,
    pub limit: usize,
}

/// What an editable select asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComboOut {
    Idle,
    /// The text as typed is to be written: Enter, or the field left.
    Commit,
    /// A row of the menu was picked: an option's index, or the options' count for Clear.
    Pick(usize),
    /// Escape with the menu closed: the value as last written comes back.
    Cancel,
}

/// The editable select's menu width, its widest, and its height for `n` options: the options, the
/// hint under them and Clear.
const COMBO_W: f32 = 160.0;
const COMBO_MAX_W: f32 = 264.0;
fn combo_height(n: usize) -> f32 {
    n as f32 * 30.0 + 78.0
}

/// A label over a filled field that takes typing, with a chevron that opens its options above or
/// under it. The keys go to the menu while it is open (the arrows move, Enter picks, Escape
/// closes it) and to the field otherwise (Alt+Down opens the menu, Enter commits, Escape cancels).
/// A refused field is ringed in the danger colour and its reason is the field's description;
/// the caller draws the reason under it. Leaving the field closes its menu.
pub(crate) fn editable_select(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    text: &mut String,
    c: &Combo,
) -> ComboOut {
    let paint = ui.painter().clone();
    ux::text(&paint, at, c.label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, c.id, "field", field);
    let eid = widgets::egui_id(c.id);
    let rows = c.options.len() + 1;
    // The menu is as wide as the design's, or wider when its hint needs it (a longer language).
    let hint_w = ux::galley(&paint, c.hint, types::CAPTION, p.text_secondary, None).size().x;
    let list_w = (hint_w + 24.0).clamp(COMBO_W, COMBO_MAX_W);
    let list = ux::drop_rect(ui, field, field.right() - list_w, vec2(list_w, combo_height(c.options.len())));
    let was_open = shell.panels.menu == Some(c.menu);
    let focused = ui.memory(|m| m.has_focus(eid));
    let mut out = ComboOut::Idle;
    // The keys first, before the text field takes them. The open menu's arrows and Escape act pressed
    // alone: with a modifier they are the field's, as Shift with Down selects to its end. Its Enter
    // keeps Shift and Alt on purpose, as the field would end its input on them too.
    if was_open {
        if ui.input_mut(|i| crate::actions::take_alone(i, egui::Key::ArrowDown)) {
            shell.panels.highlight = Some(shell.panels.highlight.map_or(0, |h| (h + 1).min(rows - 1)));
        }
        if ui.input_mut(|i| crate::actions::take_alone(i, egui::Key::ArrowUp)) {
            shell.panels.highlight = Some(shell.panels.highlight.map_or(0, |h| h.saturating_sub(1)));
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            out = shell.panels.highlight.map_or(ComboOut::Commit, ComboOut::Pick);
            shell.panels.menu = None;
        }
        if ui.input_mut(|i| crate::actions::take_alone(i, egui::Key::Escape)) {
            shell.panels.menu = None;
        }
    } else if focused {
        // Alt+Down exactly (`consume_key` lets Shift through): with Shift more the press stays
        // for the field, which selects to its end, as Shift with Down does.
        let open = crate::actions::Keys { modifiers: egui::Modifiers::ALT, key: egui::Key::ArrowDown };
        if ui.input_mut(|i| crate::actions::take(i, &open)) {
            shell.panels.menu = Some(c.menu);
            shell.panels.highlight = c.current;
        } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            out = ComboOut::Cancel;
            ui.memory_mut(|m| m.surrender_focus(eid));
        }
    }
    let chevron = Rect::from_min_size(pos2(field.right() - 28.0, field.top()), vec2(28.0, FIELD));
    let inner = field_inner(ui, field);
    let inner = Rect::from_min_max(inner.min, pos2((chevron.left() - 2.0).max(inner.left()), inner.bottom()));
    // Escape stays with the field, so the menu or the edit can take it.
    let filter = egui::EventFilter { escape: true, horizontal_arrows: true, vertical_arrows: true, tab: false };
    let edit = egui::TextEdit::singleline(text)
        .id(eid)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text.into())
        .char_limit(c.limit)
        .hint_text(egui::RichText::new(c.placeholder).color(p.text_muted).font(types::SMALL.font()))
        .event_filter(filter)
        .desired_width(inner.width());
    let resp = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner))).response;
    // Typing takes over from the row the keys reached: Enter then writes the typing.
    if resp.changed() && shell.panels.menu == Some(c.menu) {
        shell.panels.highlight = None;
    }
    // The row the keys reached is the field's active descendant, so a screen reader says which
    // one Enter picks as the arrows move.
    let open = shell.panels.menu == Some(c.menu);
    let reached = shell.panels.highlight.filter(|_| open).map(|h| {
        let row = if h < c.options.len() { format!("{}.menu.{h}", c.id) } else { format!("{}.menu.clear", c.id) };
        widgets::egui_id(&row).accesskit_id()
    });
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label(c.name);
        node.set_expanded(open);
        if let Some(row) = reached {
            node.set_active_descendant(row);
        }
        if let Some(why) = c.refused {
            node.set_description(why);
            node.set_invalid(egui::accesskit::Invalid::True);
        }
    });
    // Leaving the field writes what was typed, unless the press that left it is on the field's own
    // chevron or its open menu: then the typing stays a draft and the field takes the keyboard back,
    // so a pick replaces the draft and Escape still brings the written value back.
    // The field gives the keyboard up on the press or on the click, so either edge counts.
    let pressed_in =
        |r: Rect| ui.input(|i| (i.pointer.any_pressed() || i.pointer.any_released()) && i.pointer.interact_pos().is_some_and(|o| r.contains(o)));
    let own = pressed_in(chevron) || (was_open && pressed_in(list));
    if out == ComboOut::Idle && resp.lost_focus() {
        if own {
            ui.memory_mut(|m| m.request_focus(eid));
        } else {
            // Left another way (a click elsewhere, Tab): its menu goes too, so its keys stop.
            out = ComboOut::Commit;
            if shell.panels.menu == Some(c.menu) {
                shell.panels.menu = None;
            }
        }
    }
    let ring = if c.refused.is_some() { Some(p.danger) } else { (resp.has_focus() || shell.panels.menu == Some(c.menu)).then_some(p.focus) };
    if let Some(ink) = ring {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    }
    ux::icon_mid(&paint, pos2(field.right() - 16.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    if control(shell, ui, &format!("{}.menu", c.id), "button", chevron, true).on_hover_text(c.pick).accessible_name(c.pick_name).clicked() {
        if shell.panels.menu == Some(c.menu) {
            shell.panels.menu = None;
        } else {
            shell.panels.menu = Some(c.menu);
            shell.panels.highlight = c.current;
            // The field keeps the keyboard, so the keys reach the menu and not the canvas.
            ui.memory_mut(|m| m.request_focus(eid));
        }
    }
    if shell.panels.menu == Some(c.menu)
        && let Some(k) = ux::overlay(ui, widgets::egui_id(&format!("{}.menu.layer", c.id)), list, |ui| combo_menu(shell, ui, p, field, list, c))
    {
        out = ComboOut::Pick(k);
    }
    out
}

/// The editable select's open menu over `r`: its options, the current one ticked, the hint, then
/// Clear; gives the row picked and closes on a pick or a press elsewhere.
fn combo_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, r: Rect, c: &Combo) -> Option<usize> {
    let paint = ui.painter();
    ux::shadow(paint, r, 8.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let n = c.options.len();
    let rows = c.options.iter().copied().enumerate().map(|(i, o)| (i, o.to_string(), 4.0 + i as f32 * 30.0, format!("{}.menu.{i}", c.id)));
    let clear = (n, c.clear.to_string(), 4.0 + n as f32 * 30.0 + 40.0, format!("{}.menu.clear", c.id));
    let mut picked = None;
    for (i, word, top, id) in rows.chain([clear]) {
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + top), vec2(r.width() - 8.0, 30.0));
        let resp = control(shell, ui, &id, "menu-item", row, true).accessible_name(word.as_str());
        let on = c.current == Some(i);
        ui.ctx().accesskit_node_builder(resp.id, |node| node.set_selected(on));
        if on {
            paint.rect_filled(row, 5.0, p.accent_tint);
        } else if resp.hovered() || shell.panels.highlight == Some(i) {
            paint.rect_filled(row, 5.0, p.page);
        }
        if on && shell.panels.highlight == Some(i) {
            paint.rect_stroke(row, 5.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
        }
        ux::text(paint, row.min + vec2(10.0, 7.5), &word, types::SMALL, p.text, Some((row.width() - 40.0).max(0.0)));
        if on {
            ux::icon_in(paint, Rect::from_center_size(pos2(row.right() - 16.0, row.center().y), Vec2::splat(14.0)), &icons::TICK, p.accent);
        }
        if resp.clicked() {
            picked = Some(i);
        }
    }
    ux::text(paint, pos2(r.left() + 12.0, r.top() + 4.0 + n as f32 * 30.0 + 10.0), c.hint, types::CAPTION, p.text_secondary, Some(r.width() - 24.0));
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside {
        shell.panels.menu = None;
    }
    picked
}

/// A menu under `anchor` with `items`; gives the index picked, closing the menu on a pick or a
/// click elsewhere.
pub(crate) fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String]) -> Option<usize> {
    menu_frame(shell, ui, p, anchor, id, MenuRows { items, notes: &[], item_parts: &[], note_parts: &[] }, 0.0)
}

/// [`menu`] with a muted note after an item's text where `notes` has one (empty for none), such as
/// the package that tells two types of one name apart; the note joins the row's accessible name.
/// `note_parts` holds a note's parts when it is a package's path: too long for its row, it folds
/// from its front (`… › Orders › EU`) and the row's tooltip says it whole.
pub(crate) fn menu_noted(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    anchor: Rect,
    id: &str,
    items: &[String],
    notes: &[String],
    note_parts: &[Vec<String>],
) -> Option<usize> {
    let width = noted_width(ui, p, items, notes);
    menu_frame(shell, ui, p, anchor, id, MenuRows { items, notes, item_parts: &[], note_parts }, width)
}

/// [`menu`] of package paths, each given by its names from the top (`paths`): as wide as the
/// longest needs, inside the window; a path still too long folds from its front, so the packages
/// at its end, the ones that tell it apart, show (`… › Orders › EU`), and its row's tooltip says
/// it whole.
pub(crate) fn menu_paths(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, paths: &[Vec<String>]) -> Option<usize> {
    let items: Vec<String> = paths.iter().map(|parts| parts.join(PATH_SEP)).collect();
    let width = {
        let paint = ui.painter();
        items.iter().take(MAX_MEASURED).map(|t| 8.0 + 10.0 + ux::galley(paint, t, types::SMALL, p.text, None).size().x + 10.0).fold(0.0, f32::max)
    };
    menu_frame(shell, ui, p, anchor, id, MenuRows { items: &items, notes: &[], item_parts: paths, note_parts: &[] }, width)
}

/// What a menu's rows show: each item, its muted note (empty for none), and, where the item or the
/// note is a package's path, its parts, which fold from the front when the row is too narrow.
#[derive(Clone, Copy)]
struct MenuRows<'a> {
    items: &'a [String],
    notes: &'a [String],
    item_parts: &'a [Vec<String>],
    note_parts: &'a [Vec<String>],
}

/// The open menu under `anchor`, at least `wanted` wide and as wide as the anchor, inside the
/// window: under the field, else above it; longer than either side has room for, it takes the
/// side with more and its rows scroll.
fn menu_frame(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, rows: MenuRows<'_>, wanted: f32) -> Option<usize> {
    let full = 8.0 + rows.items.len() as f32 * MENU_ROW;
    let window = ui.ctx().content_rect();
    let (below, above) = (window.bottom() - 8.0 - (anchor.bottom() + 4.0), anchor.top() - 4.0 - (window.top() + 8.0));
    let width = wanted.max(anchor.width()).min((window.width() - 16.0).max(anchor.width()));
    let left = anchor.left().min(window.right() - 8.0 - width).max(window.left() + 8.0);
    let r = if full <= below.max(0.0) || (full > above && below >= above) {
        Rect::from_min_size(pos2(left, anchor.bottom() + 4.0), vec2(width, full.min(below.max(8.0 + MENU_ROW))))
    } else {
        let height = full.min(above.max(8.0 + MENU_ROW));
        Rect::from_min_size(pos2(left, anchor.top() - 4.0 - height), vec2(width, height))
    };
    ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), r, |ui| menu_body(shell, ui, p, anchor, r, id, rows))
}

/// A menu row's height.
const MENU_ROW: f32 = 30.0;

/// How long, in seconds, a click on a twin row picks nothing after the menu's rows changed.
const TWIN_HOLD: f64 = 0.5;
/// The most noted rows a menu measures for its width: a note marks a name several types share,
/// which few do, and the rest fit or are cut as any row is.
const MAX_NOTED: usize = 64;
/// The most path rows a menu of paths measures for its width; the rest fold to fit.
const MAX_MEASURED: usize = 256;

/// How wide a menu is to show its noted rows whole, each item, a gap and its muted note inside the
/// row's insets; nothing without a note. Only the first [`MAX_NOTED`] noted rows are measured.
fn noted_width(ui: &egui::Ui, p: &Palette, items: &[String], notes: &[String]) -> f32 {
    let paint = ui.painter();
    let measure = |t: &str| ux::galley(paint, t, types::SMALL, p.text, None).size().x;
    items
        .iter()
        .zip(notes)
        .filter(|(_, n)| !n.is_empty())
        .take(MAX_NOTED)
        .map(|(item, note)| 8.0 + 10.0 + measure(item) + 6.0 + measure(note) + 10.0)
        .fold(0.0, f32::max)
}

/// A type menu's choices kept between frames: whose and which state of it they were built for (a
/// revision means something only in its session's history, as another session of the same model,
/// a version shown or a replacement, starts its revisions anew), in which interface language (the
/// labels are translated), their labels, the values the commands take and the muted notes that
/// tell same-named ones apart, in the same order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TypeChoices {
    pub project: Option<Uuid>,
    pub history: u64,
    pub revision: u64,
    pub language: &'static str,
    /// The classifier whose member is typed, whose template parameters the menu offers (AR-130).
    pub within: Option<Uuid>,
    pub labels: Vec<String>,
    pub values: Vec<String>,
    pub notes: Vec<String>,
    /// Each note's parts, a package's path by its names, folded from its front when its row is
    /// too narrow; empty for none.
    pub note_parts: Vec<Vec<String>>,
}

/// The open menu's frame inside `r`: its rows, those in sight laid out, each with its note muted
/// after its text where `notes` has one; the wheel and the arrow keys scroll, Enter or a click
/// picks, Escape or a press elsewhere closes.
#[allow(clippy::too_many_arguments, reason = "a menu is drawn for its anchor, frame, id, items and notes")]
fn menu_body(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, r: Rect, id: &str, rows_of: MenuRows<'_>) -> Option<usize> {
    let MenuRows { items, notes, item_parts, note_parts } = rows_of;
    let paint = ui.painter();
    ux::shadow(paint, r, 8.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    // Its rows scroll with the wheel over it, as far as the last.
    let most = (8.0 + items.len() as f32 * MENU_ROW - r.height()).max(0.0);
    let mut offset = shell.panels.menu_scroll.as_ref().filter(|(of, _)| of == id).map_or(0.0, |(_, o)| *o);
    if ui.input(|i| i.pointer.hover_pos().is_some_and(|at| r.contains(at))) {
        offset -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    // The arrow keys reach a row, scrolled into sight; Enter picks it. The row is kept by its text
    // and note too: rows that come or go before it (Open, while a folder pick waits) move the reach
    // with it, never onto another row, and a row gone, or there twice, is reached no more (AR-749).
    let key_of = |i: usize| items.get(i).map(|text| (text.as_str(), notes.get(i).map_or("", String::as_str)));
    /// A row's text and note as kept, borrowed.
    fn as_key(k: &(String, String)) -> (&str, &str) {
        (k.0.as_str(), k.1.as_str())
    }
    let drawn = shell.panels.menu_reached_rows.clone().filter(|d| d.menu == id);
    let mut reached = shell.panels.menu_reached.as_ref().filter(|(of, _)| of == id).map(|(_, i)| *i);
    // When the rows change, a reach whose text and note more than one row shares, before or after,
    // is dropped: a twin come in may stand where it was. Unchanged rows keep it, twins and all.
    let fingerprint = egui::Id::unique((id, items, notes)).value();
    let before = shell.panels.menu_ordinals.as_ref().filter(|o| o.menu == id && o.rows != fingerprint);
    if let (Some(before), Some(d)) = (before, &drawn) {
        let twin_before = before.twins.iter().any(|twin| as_key(twin) == as_key(&d.row));
        let twin_now = (0..items.len()).filter(|k| key_of(*k) == Some(as_key(&d.row))).nth(1).is_some();
        if twin_before || twin_now {
            reached = None;
        }
    }
    // A row scrolled into sight: the menu's offset moved only as far as it takes.
    let into_sight = |row: usize, offset: f32| {
        let (top, bottom) = (4.0 + row as f32 * MENU_ROW, 4.0 + (row + 1) as f32 * MENU_ROW);
        if top < offset + 4.0 {
            top - 4.0
        } else if bottom > offset + r.height() - 4.0 {
            bottom - r.height() + 4.0
        } else {
            offset
        }
    };
    if let Some(d) = &drawn
        && reached.is_some_and(|i| key_of(i) != Some(as_key(&d.row)))
    {
        let mut same = (0..items.len()).filter(|k| key_of(*k) == Some(as_key(&d.row)));
        reached = match (same.next(), same.next()) {
            (Some(i), None) => Some(i),
            _ => None,
        };
        // Moved by rows that came or went before it, it is kept in sight.
        if let Some(i) = reached {
            offset = into_sight(i, offset);
        }
    }
    let from = reached;
    let (down, up, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            reached.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    if !items.is_empty() && (down || up) {
        let next = match reached {
            None => 0,
            Some(i) if down => (i + 1).min(items.len() - 1),
            Some(i) => i.saturating_sub(1),
        };
        reached = Some(next);
        offset = into_sight(next, offset);
    }
    // An arrow with Enter in its frame picks the row it moved to only if that row was drawn there
    // beside the row in sight: one that came with a change of the rows this frame is drawn first.
    let enter = enter
        && match (from, reached, &drawn) {
            (Some(from), Some(to), Some(d)) if to != from => (if to < from { &d.above } else { &d.below }).as_ref().map(as_key) == key_of(to),
            _ => true,
        };
    let owned = |(text, note): (&str, &str)| (text.to_string(), note.to_string());
    shell.panels.menu_reached = reached.map(|i| (id.to_string(), i));
    shell.panels.menu_reached_rows = reached.and_then(|i| {
        let (above, below) = (i.checked_sub(1).and_then(key_of).map(owned), i.checked_add(1).and_then(key_of).map(owned));
        Some(ReachedRows { menu: id.to_string(), row: owned(key_of(i)?), above, below })
    });
    let offset = offset.clamp(0.0, most);
    shell.panels.menu_scroll = Some((id.to_string(), offset));
    let rows = r.shrink2(vec2(0.0, 4.0));
    let paint = paint.with_clip_rect(rows);
    let mut picked = None;
    // Only the rows in sight are laid out, however many the menu holds.
    let first = ((offset / MENU_ROW).floor().max(0.0) as usize).min(items.len());
    let in_sight = ((r.height() / MENU_ROW).ceil().max(0.0) as usize).saturating_add(1);
    // Rows of one text and note (two template parameters `T`, say) are told apart by their order
    // in the whole menu, numbered once for each set of rows: never by the rows in sight, which
    // scrolling changes (AR-749).
    // For a moment after the rows change, a click or Enter on a row whose text and note another
    // row shares, before or after, runs nothing: it may have been meant for a twin that just went,
    // by a screen reader still reading the rows before, and the menu stays for a click on what now
    // shows. A press held over the change is released on such a row for nothing too.
    let (now, held, released) = ui.input(|i| (i.time, i.pointer.primary_down(), i.pointer.primary_released()));
    let kept = shell.panels.menu_unsure.take().filter(|(of, _, until)| of == id && (held || released || now < *until));
    let mut until = kept.as_ref().map_or(now, |(.., until)| *until);
    let mut unsure: Vec<(String, String)> = kept.map(|(_, twins, _)| twins).unwrap_or_default();
    if shell.panels.menu_ordinals.as_ref().is_none_or(|o| o.menu != id || o.rows != fingerprint) {
        let mut nth_of: std::collections::HashMap<(&str, &str), usize> = std::collections::HashMap::new();
        let nth = (0..items.len())
            .filter_map(key_of)
            .map(|key| {
                let seen = nth_of.entry(key).or_insert(0);
                *seen += 1;
                *seen - 1
            })
            .collect::<Vec<_>>();
        let twins: Vec<(String, String)> =
            nth_of.into_iter().filter(|(_, n)| *n > 1).map(|((text, note), _)| (text.to_string(), note.to_string())).collect();
        if let Some(before) = shell.panels.menu_ordinals.take().filter(|o| o.menu == id) {
            unsure.extend(before.twins.into_iter().chain(twins.iter().cloned()));
            until = now + TWIN_HOLD;
        }
        shell.panels.menu_ordinals = Some(MenuOrdinals { menu: id.to_string(), rows: fingerprint, nth, twins });
    }
    if !unsure.is_empty() && (held || now < until) {
        shell.panels.menu_unsure = Some((id.to_string(), unsure.clone(), until));
    }
    let shown = first..first.saturating_add(in_sight).min(items.len());
    let nths = shell.panels.menu_ordinals.as_ref().and_then(|o| o.nth.get(shown)).map(<[usize]>::to_vec).unwrap_or_default();
    for (i, item) in items.iter().enumerate().skip(first).take(in_sight) {
        let key = key_of(i).unwrap_or((item.as_str(), ""));
        let nth = nths.get(i.saturating_sub(first)).copied().unwrap_or(0);
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * MENU_ROW - offset), vec2(r.width() - 8.0, MENU_ROW));
        if !rows.intersects(row) {
            continue;
        }
        let note = notes.get(i).filter(|n| !n.is_empty());
        // Automation finds the row by its place; the toolkit knows it by its text, so a press, a
        // release or a screen reader's click meant for a row that moved or went (Open, as a folder
        // pick ends or starts) never lands on the row now in its place (AR-749).
        widgets::record_visible(ui, &mut shell.registry, &format!("{id}.{i}"), "menu-item", row.intersect(rows));
        let resp = ui.interact(row.intersect(rows), menu_row_id(id, key, nth), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let resp = match note {
            Some(n) => resp.accessible_name(fmt(&tr("{name} · {where}"), &[("name", item), ("where", n)])),
            None => resp,
        };
        if resp.hovered() || reached == Some(i) {
            paint.rect_filled(row, 5.0, p.page);
        }
        let room = (row.width() - 20.0).max(0.0);
        // A path too long for its room folds from its front, its end, the packages that tell it
        // apart, kept; the row's tooltip says it whole.
        let fits = |t: &str, width: f32| ux::galley(&paint, t, types::SMALL, p.text, None).size().x <= width;
        let mut folded = false;
        let shown = match item_parts.get(i).filter(|parts| !parts.is_empty()) {
            Some(parts) => {
                let (shown, cut) = ux::cut_front(parts, PATH_SEP, |t| fits(t, room));
                folded |= cut;
                shown
            }
            None => item.clone(),
        };
        let drawn = ux::text(&paint, row.min + vec2(10.0, 7.5), &shown, types::SMALL, p.text, Some(room));
        if let Some(n) = note {
            let left = drawn.right() + 6.0;
            let rest = (row.left() + 10.0 + room - left).max(0.0);
            let n = match note_parts.get(i).filter(|parts| !parts.is_empty()) {
                Some(parts) => {
                    let (shown, cut) = ux::cut_front(parts, PATH_SEP, |t| fits(t, rest));
                    folded |= cut;
                    shown
                }
                None => n.clone(),
            };
            ux::text(&paint, pos2(left, row.top() + 7.5), &n, types::SMALL, p.text_muted, Some(rest));
        }
        let resp = if folded {
            let whole = match note {
                Some(n) => fmt(&tr("{name} · {where}"), &[("name", item), ("where", n)]),
                None => item.clone(),
            };
            resp.on_hover_text(whole)
        } else {
            resp
        };
        if (resp.clicked() || (enter && reached == Some(i))) && !unsure.iter().any(|twin| as_key(twin) == key) {
            picked = Some(i);
        }
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
        shell.panels.menu_reached = None;
        shell.panels.menu_reached_rows = None;
        shell.panels.menu_unsure = None;
    }
    picked
}

/// The toolkit's id for the menu `id`'s row of `key`, its text and note (empty for none), the `nth`
/// of the menu's rows with that key: never its place among rows that can come and go.
pub(crate) fn menu_row_id(id: &str, key: (&str, &str), nth: usize) -> egui::Id {
    widgets::egui_id(id).with(("row", key, nth))
}

/// A check box with its label; a click flips `on`.
pub(crate) fn check(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, id: &str, label: &str, on: &mut bool) -> f32 {
    let paint = ui.painter();
    let b = Rect::from_min_size(at, Vec2::splat(16.0));
    paint.rect_filled(b, 4.0, if *on { p.accent_fill } else { p.surface });
    paint.rect_stroke(b, 4.0, Stroke::new(1.0, if *on { p.accent_fill } else { p.border }), StrokeKind::Inside);
    if *on {
        ux::icon_in(paint, b.shrink(3.0), &icons::TICK, p.on_accent);
    }
    let g = ux::text(paint, at + vec2(24.0, -1.0), label, types::SMALL, p.text, None);
    let hit = Rect::from_min_max(b.min, pos2(g.right(), b.bottom()));
    if control(shell, ui, id, "check", hit, true).accessible_name(label).clicked() {
        *on = !*on;
    }
    g.right() + 20.0
}

/// An icon button of 28 px: quiet, bordered or primary.
pub(crate) fn icon_button(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    id: &str,
    icon: &ux::Icon,
    ink: ux::Colour,
    fill: Option<ux::Colour>,
    hint: &str,
) -> bool {
    let paint = ui.painter();
    let b = Rect::from_min_size(at, Vec2::splat(FIELD));
    match fill {
        Some(f) => {
            paint.rect_filled(b, 6.0, f);
        }
        None => {
            paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
    }
    ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), icon, ink);
    control(shell, ui, id, "button", b, true).on_hover_text(tr(hint)).clicked()
}

/// A quiet button of an icon and a label (the design's Button / Ghost); gives whether it was clicked.
pub(crate) fn ghost_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, id: &str, icon: &ux::Icon, label: &str) -> bool {
    let paint = ui.painter();
    let g = ux::galley(paint, label, types::UI, p.text, None);
    let b = Rect::from_min_size(at, vec2(6.0 + 14.0 + 7.0 + g.size().x + 8.0, 24.0));
    ux::icon_in(paint, Rect::from_min_size(b.min + vec2(6.0, 5.0), Vec2::splat(14.0)), icon, p.text);
    paint.galley(b.min + vec2(27.0, 2.5), g, p.text.into());
    let resp = control(shell, ui, id, "button", b, true);
    if resp.hovered() {
        paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    resp.clicked()
}

// Properties.

/// Draws the Properties panel's body under the header, from `y`.
fn properties(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    // A notation may title the first section otherwise (a column's panel is its table's).
    let notation = shell.session.as_ref().zip(subject.obj).and_then(|(s, o)| crate::notation_ui::of(shell.notation_uis, s.doc(), o));
    let heading = shell.session.as_ref().zip(subject.obj).zip(notation).and_then(|((s, o), n)| n.heading(s.doc(), o));
    let word = heading.unwrap_or_else(|| subject.word.clone());
    y = section(shell, ui, p, r, y, &word, None).unwrap_or(y + 50.0);
    if shell.panels.editors_for != subject.id {
        shell.panels.editors_for = subject.id;
        close_editors(shell);
    }
    match subject.class {
        None => {
            // The model root: its name, which the project's name is.
            ux::text(&paint, pos2(r.left() + PAD, y), &tr("Name"), types::CAPTION, p.text_secondary, Some(width.max(0.0)));
            let field = Rect::from_min_size(pos2(r.left() + PAD, y + 17.0), vec2(width.max(0.0), FIELD));
            paint.rect_filled(field, 6.0, p.field);
            ux::text_mid(&paint, field.left() + 8.0, field.center().y, &subject.name, types::SMALL, p.text, Some((width - 16.0).max(0.0)));
            control(shell, ui, "workspace.properties.name", "button", field, false);
        }
        Some(c) if c == archforge_diagram::DIAGRAM => {
            // A diagram: its name, and the schema it is of, which does not change.
            let half = (width - 16.0) / 2.0;
            name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, "diagram.rename", "diagram");
            // The element it is of, named by its notation's word (ER's Schema).
            let (schema, word) = shell
                .session
                .as_ref()
                .and_then(|s| {
                    let store = s.doc();
                    let of = crate::diagrams::schema_of(store, subject.obj?)?;
                    let word = crate::notation_ui::of(shell.notation_uis, store, of).and_then(|ui| ui.words(store, of)).map(|w| w.word);
                    Some((archforge_core::name_of(store, of), word))
                })
                .unwrap_or_default();
            let at = pos2(r.left() + PAD + half + 16.0, y);
            ux::text(&paint, at, &word.unwrap_or_else(|| tr("Schema")), types::CAPTION, p.text_secondary, Some(half.max(0.0)));
            let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(half.max(0.0), FIELD));
            paint.rect_filled(field, 6.0, p.field);
            ux::text_mid(&paint, field.left() + 8.0, field.center().y, &schema, types::SMALL, p.text, Some((half - 36.0).max(0.0)));
            ux::icon_mid(&paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_muted);
            control(shell, ui, "workspace.properties.schema", "select", field, false);
            if let Some(o) = subject.obj {
                problems_section(shell, ui, p, r, y + 17.0 + FIELD + 16.0, o);
            }
        }
        // An element of a notation: its notation's sections.
        Some(_) => {
            if let Some(notation) = notation {
                notation.body(shell, ui, p, r, y, subject);
            }
        }
    }
}

/// The Problems section for a selection, when the rules find any; gives the y under it.
pub(crate) fn problems_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: Obj) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let problems: Vec<(Option<Uuid>, String)> = match shell.session.as_ref() {
        Some(s) => crate::problems::of(shell.notation_uis, s.doc(), &shell.problems, subject)
            .into_iter()
            .map(|pr| (s.doc().uuid(pr.element).ok(), pr.text))
            .collect(),
        None => return y,
    };
    if problems.is_empty() {
        return y;
    }
    let title = fmt(&tr("Problems · {n}"), &[("n", &problems.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
    let own = shell.session.as_ref().and_then(|s| s.doc().uuid(subject).ok());
    for (i, (element, text)) in problems.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 28.0));
        let live = element.is_some() && *element != own;
        let resp = control(shell, ui, &format!("workspace.properties.problem.{i}"), "row", row, live);
        if resp.hovered() && live {
            paint.rect_filled(row, 6.0, p.page);
        }
        paint.circle_stroke(pos2(row.left() + 10.0, row.center().y), 6.0, Stroke::new(1.2, p.key_foreign));
        ux::text(&paint, pos2(row.left() + 8.0, row.top() + 6.0), "!", types::TINY, p.key_foreign, None);
        ux::text(&paint, row.min + vec2(26.0, 6.0), text, types::SMALL, p.text, Some((width - 30.0).max(0.0)));
        if resp.clicked()
            && live
            && let Some(id) = element
            && let Err(e) = run(shell, "view.select", json!({ "ids": [id.to_string()] }))
        {
            shell.panels.notice = Some(e);
        }
        y += 28.0;
    }
    y + 8.0
}

/// Refuses a name of nothing but spaces for an element that needs one, before anything is sent:
/// the panel says so in plain words, and the name as written stays. A diagram may be left without a
/// name, as may what a notation's rename command `cmd` leaves unnamed
/// ([`NotationUi::may_be_unnamed`](crate::notation_ui::NotationUi::may_be_unnamed)). Gives whether
/// it refused.
pub(crate) fn refuse_empty_name(shell: &mut Shell, cmd: &str, name: &str) -> bool {
    let may_be_unnamed = cmd == "diagram.rename" || crate::notation_ui::UIS.iter().any(|ui| ui.may_be_unnamed(cmd));
    let refused = name.trim().is_empty() && !may_be_unnamed;
    if refused {
        shell.panels.notice = Some(tr("A name can't be empty."));
    }
    refused
}

/// The name field of an element, committed through its rename command on Enter or when it is
/// left; Escape brings the name back, and an empty name is refused before it is sent.
pub(crate) fn name_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject, cmd: &str, key: &str) {
    let Some(id) = subject.id else { return };
    let mut text = match &shell.panels.name {
        Some(n) if n.target == id => n.text.clone(),
        _ => subject.name.clone(),
    };
    let limit = crate::notation_ui::name_limit_of(shell, id);
    let out = edit_field(shell, ui, p, at, width, "workspace.properties.name", &tr("Name"), &mut text, limit, false, true);
    if out == FieldOut::Cancel {
        shell.panels.name = None;
    } else if out == FieldOut::Commit {
        shell.panels.name = None;
        if text.trim() != subject.name && !refuse_empty_name(shell, cmd, &text) {
            match run(shell, cmd, json!({ key: id.to_string(), "name": text.trim() })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    } else if text != subject.name {
        shell.panels.name = Some(NameEdit { target: id, text });
    } else if shell.panels.name.as_ref().is_some_and(|n| n.target == id) {
        shell.panels.name = None;
    }
}

/// The description of the element `obj` as its notation keeps it, or nothing.
pub(crate) fn description_now(shell: &Shell, obj: Obj) -> String {
    shell.session.as_ref().and_then(|s| s.view().of_element(s.doc(), obj).map(|n| n.description(s.doc(), obj))).unwrap_or_default()
}

/// Writes `text` as the description of `target` through `element.describe` when it differs from
/// what the element has, and keeps the draft until the write succeeds: done, the draft and the
/// notice clear; refused, the draft stays, marked refused, and the refusal is the notice.
fn write_description(shell: &mut Shell, target: Uuid, text: String) {
    let obj = shell.session.as_ref().and_then(|s| s.doc().by_uuid(target));
    if obj.is_none_or(|o| text.trim() == description_now(shell, o)) {
        shell.panels.description = None;
        return;
    }
    match run(shell, "element.describe", json!({ "element": target.to_string(), "text": text.trim() })) {
        Ok(_) => {
            shell.panels.description = None;
            shell.panels.notice = None;
        }
        Err(e) => {
            shell.panels.description = Some(DescriptionDraft { target, text, refused: true });
            shell.panels.notice = Some(refused_on(shell, target, e));
        }
    }
}

/// A refusal `why` of what was typed on `target`, as the panel's notice: when `target` is not the
/// element selected, as when its draft is written because the selection moved on, it names the
/// element, so it is not read as about the one shown (AR-417).
pub(crate) fn refused_on(shell: &Shell, target: Uuid, why: String) -> String {
    let Some(s) = shell.session.as_ref() else { return why };
    let store = s.doc();
    let Some(o) = store.by_uuid(target).filter(|o| s.view().selection.first() != Some(o)) else { return why };
    match archforge_core::name_of(store, o) {
        name if name.is_empty() => fmt(&tr("Not kept on the element selected before: {why}"), &[("why", &why)]),
        name => fmt(&tr("Not kept on {name}: {why}"), &[("name", &name), ("why", &why)]),
    }
}

/// A description draft left behind when the selection moved on to `subject` (another element, a
/// diagram or nothing) is written to the element it was typed on, as Enter would, before the
/// panel shows the new selection: the draft is never lost and never lands on another element. A
/// draft already refused is not written again until its text changes.
pub(crate) fn flush_description(shell: &mut Shell, subject: Option<Uuid>) {
    let Some(d) = shell.panels.description.clone() else { return };
    if Some(d.target) != subject && !d.refused {
        write_description(shell, d.target, d.text);
    }
}

/// Writes the description being typed, on whichever element, as Enter would, before a save, a close
/// or a quit takes stock of what the project holds; a draft refused before is tried again, so its
/// refusal shows. Gives whether a draft is left: one whose write was refused, kept with the
/// refusal as the notice, which the project counts as unsaved work.
pub(crate) fn commit_description(shell: &mut Shell) -> bool {
    if let Some(d) = shell.panels.description.clone() {
        write_description(shell, d.target, d.text);
    }
    shell.panels.description.is_some()
}

/// Whether `panels` keep a description draft whose write was refused: a save stops on it.
pub(crate) fn refused_description(panels: &State) -> bool {
    panels.description.as_ref().is_some_and(|d| d.refused)
}

/// Whether `panels` keep a description draft not yet written, being typed or refused: unsaved work
/// the status bar, the tab, `document.inspect` and a close or a quit count.
pub(crate) fn description_pending(panels: &State) -> bool {
    panels.description.is_some()
}

/// The description under the heading: typed in place, committed through `element.describe` on Enter
/// or when the keyboard leaves it, and when the selection moves on ([`flush_description`]); Escape
/// gives the typing up, a refused draft too, and the description as written comes back. It
/// shows what the element's notation keeps: an ER element's description, a UML element's
/// description Comment (AR-186).
fn description_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject) {
    let (Some(id), Some(obj)) = (subject.id, subject.obj) else { return };
    let current = description_now(shell, obj);
    let Some(limit) = shell.session.as_ref().map(|s| crate::notation_ui::description_limit(shell.notation_uis, s.doc(), obj)) else { return };
    let mut text = match &shell.panels.description {
        Some(d) if d.target == id => d.text.clone(),
        _ => current.clone(),
    };
    let out = edit_field(shell, ui, p, at, width, "workspace.properties.description", &tr("Description"), &mut text, limit, false, true);
    if out == FieldOut::Cancel {
        shell.panels.description = None;
    } else if out == FieldOut::Commit {
        write_description(shell, id, text);
    } else if text != current {
        // The refused mark stays while the text does; a change makes it a draft to write again.
        let refused = shell.panels.description.as_ref().is_some_and(|d| d.target == id && d.text == text && d.refused);
        shell.panels.description = Some(DescriptionDraft { target: id, text, refused });
    } else if shell.panels.description.as_ref().is_some_and(|d| d.target == id) {
        shell.panels.description = None;
    }
}

/// The first selected element's id, which a list in Properties marks.
pub(crate) fn selected_id(shell: &Shell) -> Option<Uuid> {
    let s = shell.session.as_ref()?;
    s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok())
}

/// Selects an element a Properties row lists, so the Models tree and the canvas mark it as they
/// would for a click there; the rows it sits under open in the tree, as its notation says
/// ([`NotationUi::parents`](crate::notation_ui::NotationUi::parents)), so its row shows.
pub(crate) fn select_element(shell: &mut Shell, id: Uuid) {
    if let Some(store) = shell.session.as_ref().map(|s| s.doc())
        && let Some(o) = store.by_uuid(id)
        && let Some(ui) = crate::notation_ui::of(shell.notation_uis, store, o)
    {
        shell.explorer.expanded.extend(ui.parents(store, o).into_iter().filter_map(|p| store.uuid(p).ok()));
    }
    if let Err(e) = run(shell, "view.select", json!({ "ids": [id.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// A list row's edit button at its right end. Only it, or a double click on the row, opens the
/// row's editor: selecting an element never does. True when it was pressed.
pub(crate) fn edit_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, row: Rect, id: &str, row_hovered: bool) -> bool {
    let at = Rect::from_center_size(pos2(row.right() - 16.0, row.center().y), vec2(24.0, 24.0));
    let resp = control(shell, ui, &format!("{id}.edit"), "button", at, true).on_hover_text(tr("Edit"));
    let paint = ui.painter();
    if resp.hovered() {
        paint.rect_filled(at, 6.0, p.border);
    }
    let ink = if resp.hovered() || row_hovered { p.text_secondary } else { p.text_muted };
    ux::icon_mid(paint, at.center(), &icons::EDIT, ink);
    resp.clicked()
}

/// Closes every Properties editor, as Cancel would, except one opened this frame and not yet
/// shown: an editor belongs to the selection it was opened on. Every editor is a notation's
/// ([`crate::notation_ui::NotationState::close_editors`]).
fn close_editors(shell: &mut Shell) {
    shell.notation_states.close_editors(&mut shell.panels.menu);
}

// Skills.

/// One skill as the panel lists it.
struct SkillRow {
    id: Uuid,
    name: String,
    /// What it aims at, or where it comes from.
    detail: String,
    /// The subject's own, which the row can remove; an inherited one is removed where it sits.
    own: bool,
}

/// What a skill aims at, as Properties says it: every class, every interface, every enumeration,
/// every data type, every primitive type, every association class, its type pattern, or nothing.
pub(crate) fn target_word(store: &Store, s: Obj) -> String {
    let applies = string_of(store, s, archforge_core::skill::APPLIES_TO);
    let pattern = string_of(store, s, archforge_core::skill::TYPE_PATTERN);
    match archforge_core::skills::kind_by_key(&archforge_notations::view(), store, &applies).map(|(_, w)| w) {
        Some("table") => tr("every table"),
        Some("relationship") => tr("every relationship"),
        Some("class") => tr("every class"),
        Some("interface") => tr("every interface"),
        Some("enumeration") => tr("every enumeration"),
        Some("data type") => tr("every data type"),
        Some("primitive type") => tr("every primitive type"),
        Some("association class") => tr("every association class"),
        Some("attribute") => tr("every attribute"),
        Some("operation") => tr("every operation"),
        Some("association") => tr("every association"),
        Some(_) if !pattern.is_empty() => pattern,
        Some("column") => tr("every column"),
        Some(_) | None => String::new(),
    }
}

/// The selection's own skills and those it inherits, each as its row says it.
fn skill_rows(store: &Store, subject: &Subject) -> (Vec<SkillRow>, Vec<SkillRow>) {
    let this = subject.this.clone();
    let subject = subject.obj;
    let own: Vec<SkillRow> = archforge_core::skills::skills(store)
        .into_iter()
        .filter(|s| archforge_core::skills::subject_of(store, *s) == subject)
        .filter_map(|s| {
            let target = target_word(store, s);
            Some(SkillRow {
                id: store.uuid(s).ok()?,
                name: archforge_core::skills::name_of(store, s),
                detail: if target.is_empty() { this.clone() } else { target },
                own: true,
            })
        })
        .collect();
    let inherited = match subject {
        Some(element) => archforge_core::skills::effective_skills(&archforge_notations::view(), store, element, None)
            .into_iter()
            .filter(|e| e.origin != Some(element))
            .filter_map(|e| {
                let origin = match e.origin {
                    Some(o) => crate::diagrams::subject_name(store, o),
                    None => tr("project"),
                };
                Some(SkillRow {
                    id: store.uuid(e.skill).ok()?,
                    name: archforge_core::skills::name_of(store, e.skill),
                    detail: if e.overridden_by.is_some() { tr("overridden below") } else { origin },
                    own: false,
                })
            })
            .collect(),
        None => Vec::new(),
    };
    (own, inherited)
}

/// Why the selection carries no skill, in the user's words, when it carries none: asked of the
/// model's own rule, so the panel offers to add a skill only where `skill.add` takes one.
fn no_skills(shell: &Shell, subject: &Subject) -> Option<String> {
    let (s, o) = (shell.session.as_ref()?, subject.obj?);
    let store = s.doc();
    archforge_core::skills::subject_problem(s.view(), store, o)?;
    let said = crate::notation_ui::of(shell.notation_uis, store, o).and_then(|ui| ui.no_skills(store, o));
    Some(said.unwrap_or_else(|| tr("This element carries no skills")))
}

/// Draws the Skills panel's body under the header, from `y`: the selection's own skills with `+`
/// to add one where it carries skills, else a line saying it carries none, then those it inherits.
fn skills(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (own, inherited) = match shell.session.as_ref() {
        Some(s) => skill_rows(s.doc(), subject),
        None => return,
    };
    let none = no_skills(shell, subject);
    let title = fmt(&subject.on_this, &[("n", &own.len().to_string())]);
    let add = none.is_none().then_some(("workspace.skills.add", "Add skill"));
    let Some(next) = section(shell, ui, p, r, y, &title, add) else {
        crate::skill_tab::create(shell, subject.id);
        return;
    };
    y = next - 6.0;
    if let Some(why) = none {
        ux::text(&paint, pos2(r.left() + PAD + 8.0, y + 7.0), &why, types::SMALL, p.text_muted, Some((width - 8.0).max(0.0)));
        y += 30.0;
    }
    for row in &own {
        y = skill_row(shell, ui, &paint, p, r, y, row, width);
    }
    y += 10.0;
    let title = fmt(&tr("Inherited · {n}"), &[("n", &inherited.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
    for row in &inherited {
        y = skill_row(shell, ui, &paint, p, r, y, row, width);
    }
}

fn skill_row(shell: &mut Shell, ui: &egui::Ui, paint: &egui::Painter, p: &Palette, r: Rect, y: f32, row: &SkillRow, width: f32) -> f32 {
    let rect = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 30.0));
    let resp = control(shell, ui, &format!("workspace.skills.row.{}", row.id), "row", rect, true).on_hover_text(tr("Open the skill"));
    // The subject's own skill can be removed from its row, the bin showing while the row is hovered.
    let bin = Rect::from_min_size(pos2(rect.right() - 28.0, rect.top() + 3.0), Vec2::splat(24.0));
    let bin_resp =
        row.own.then(|| control(shell, ui, &format!("workspace.skills.remove.{}", row.id), "button", bin, true).on_hover_text(tr("Remove")));
    let hovered = resp.hovered() || bin_resp.as_ref().is_some_and(|b| b.hovered());
    if shell.canvas.active == Some(row.id) {
        paint.rect_filled(rect, 5.0, p.accent_tint);
    } else if hovered {
        paint.rect_filled(rect, 5.0, p.page);
    }
    ux::icon_in(paint, Rect::from_min_size(rect.min + vec2(8.0, 6.0), Vec2::splat(18.0)), &icons::SKILL, p.text_secondary);
    let bin_w = if row.own { 26.0 } else { 0.0 };
    let detail = ux::galley(paint, &row.detail, types::CAPTION, p.text_muted, Some((width / 2.0).max(0.0)));
    let name_room = (width - 34.0 - detail.size().x - 16.0 - bin_w).max(0.0);
    ux::text(paint, rect.min + vec2(34.0, 6.5), &row.name, types::SMALL, p.text, Some(name_room));
    paint.galley(pos2(rect.right() - 8.0 - bin_w - detail.size().x, rect.top() + 8.0), detail, p.text_muted.into());
    if row.own && hovered {
        let ink = if bin_resp.as_ref().is_some_and(|b| b.hovered()) { p.danger } else { p.text_secondary };
        ux::icon_in(paint, Rect::from_center_size(bin.center(), Vec2::splat(14.0)), &icons::TRASH, ink);
    }
    if bin_resp.is_some_and(|b| b.clicked()) {
        if let Err(e) = run(shell, "skill.remove", json!({ "skill": row.id.to_string() })) {
            shell.panels.notice = Some(e);
        }
    } else if resp.clicked() {
        crate::canvas::open(shell, row.id);
    }
    y + 30.0
}

/// Every text that reaches `tr` through a variable: the hover hints of the icon buttons, for the
/// catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add skill"].into_iter()
}

// The panel.

/// Draws the panel body for `panel` under its header, from `y`: Properties and Skills follow the
/// selection; Chat holds the project's conversations; Appearance says what is true.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, panel: Panel, project: &str) {
    let paint = ui.painter().clone();
    let s = subject(shell, project);
    // A refusal is about what was selected: it goes with another selection. The drafts left on the
    // element selected before are written after, so a refusal of theirs shows, naming that element
    // ([`refused_on`]), until the selection moves on again.
    if shell.panels.notice_for != s.id {
        shell.panels.notice_for = s.id;
        shell.panels.notice = None;
    }
    // A description typed on the element selected before goes to it first.
    flush_description(shell, s.id);
    crate::uml_relate::finish_ends(shell, s.id);
    match panel {
        Panel::Agent => {
            crate::chat_view::draw(shell, ui, p, r, y);
            return;
        }
        Panel::Appearance => {
            crate::appearance::draw(shell, ui, p, r, y);
            return;
        }
        Panel::Properties | Panel::Skills => {}
    }
    // A template binding being drawn: its draft, nothing in the model yet (AR-131).
    if panel == Panel::Properties && crate::uml_bindings::draft_panel(shell, ui, p, r, y) {
        return;
    }
    // Several elements, generalizations among them: what a set may be made of them (AR-129).
    if panel == Panel::Properties
        && let Some(all) = crate::uml_sets::selected(shell).filter(|all| all.len() >= 2)
    {
        crate::uml_sets::selection_body(shell, ui, p, r, y, &all);
        return;
    }
    // An n-ary association being drawn: its draft, not yet in the model (AR-128).
    if panel == Panel::Properties && crate::uml_nary::draft_body(shell, ui, p, r, y) {
        return;
    }
    ux::text(&paint, r.min + vec2(PAD, y), &s.name, types::CARD_TITLE, p.text, Some((r.width() - 2.0 * PAD).max(0.0)));
    ux::text(&paint, r.min + vec2(PAD, y + 26.0), &s.kind, types::CAPTION, p.text_secondary, Some((r.width() - 2.0 * PAD).max(0.0)));
    // Every element of the model takes a description: an ER element's is the text DDL writes as its
    // comment, a UML element's a Comment it owns (AR-186).
    let mut body_y = r.top() + y + 51.0;
    if panel == Panel::Properties && s.obj.is_some() && s.class != Some(archforge_diagram::DIAGRAM) {
        description_field(shell, ui, p, pos2(r.left() + PAD, r.top() + y + 49.0), (r.width() - 2.0 * PAD).max(0.0), &s);
        body_y += 17.0 + FIELD + 14.0;
    }
    // The body scrolls under the pinned heading and description: a new panel
    // or selection starts at the top, an editor that opens or is refused is brought into view, the
    // wheel over the body moves it within its content.
    let body = Rect::from_min_max(pos2(r.left(), body_y), r.max);
    // A notation may show another element's page for its element (a column shows its table's), so
    // selecting one after the other in the same panel keeps the scroll ([`NotationUi::page`](crate::notation_ui::NotationUi::page)).
    let page = match (shell.session.as_ref(), s.obj) {
        (Some(session), Some(o)) => {
            let store = session.doc();
            match crate::notation_ui::of(shell.notation_uis, store, o) {
                Some(notation) => notation.page(store, o).and_then(|e| store.uuid(e).ok()),
                None => s.id,
            }
        }
        _ => s.id,
    };
    if shell.panels.scrolled_for != Some((panel, page)) {
        shell.panels.scrolled_for = Some((panel, page));
        shell.panels.scroll = 0.0;
    }
    let open = shell.notation_states.open_editors();
    if open != shell.panels.open_editors {
        shell.panels.reveal.extend(to_reveal(&open, &shell.panels.open_editors));
        shell.panels.open_editors = open;
    }
    let room = (shell.panels.content - body.height()).max(0.0);
    body_keys(shell, ui, body);
    shell.panels.scroll = shell.panels.scroll.clamp(0.0, room);
    shell.panels.body = Some(body);
    let drawn = shell.panels.scroll;
    shell.panels.drawn = drawn;
    let top = body_y - drawn;
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(body));
    clip.set_clip_rect(body);
    shell.registry.reach_from(top);
    match panel {
        Panel::Properties => properties(shell, &mut clip, p, r, top, &s),
        Panel::Skills => skills(shell, &mut clip, p, r, top, &s),
        _ => {}
    }
    shell.registry.reach_end();
    shell.panels.content = (shell.registry.reach() - top + SCROLL_TAIL).max(0.0);
    shell.panels.reveal.clear();
    follow_keys(shell, ui, body, drawn);
    if ui.rect_contains_pointer(body) && shell.panels.menu.is_none() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            shell.panels.scroll = (shell.panels.scroll - dy).clamp(0.0, (shell.panels.content - body.height()).max(0.0));
        }
    }
    // Content that shrank under the scroll, as a card does when its refusal goes, is followed on the
    // next frame, not on the next input: until then the body would show past its end.
    if shell.panels.scroll > (shell.panels.content - body.height()).max(0.0) {
        ui.ctx().request_repaint();
    }
    // A thin thumb at the right edge while the content overflows (the design has no scroll bar;
    // owner-delegated UX).
    if shell.panels.content > body.height() + 1.0 {
        let track = body.shrink2(vec2(0.0, 6.0));
        let h = (track.height() * body.height() / shell.panels.content).max(24.0).min(track.height());
        let at = shell.panels.scroll / (shell.panels.content - body.height()).max(1.0);
        let thumb = Rect::from_min_size(pos2(r.right() - 6.0, track.top() + at * (track.height() - h)), vec2(4.0, h));
        paint.rect_filled(thumb, 2.0, p.scroll_thumb);
    }
    if let Some(notice) = shell.panels.notice.clone() {
        let area =
            ux::text(&paint, pos2(r.left() + PAD, r.bottom() - 36.0), &notice, types::CAPTION, p.amber, Some((r.width() - 2.0 * PAD).max(0.0)));
        widgets::announce(ui, &mut shell.registry, "workspace.properties.notice", area, &notice);
    }
}
