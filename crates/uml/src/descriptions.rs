//! A UML element's description (AR-186), as Properties' Description field and `element.describe`
//! set it: kept as a Comment the element owns (UML's `Element::ownedComment`), the description
//! being the Comment's `body` (owner decision, 2026-11-05). No table or column of its own: the
//! metamodel's Comment holds it, saved and read back with the rest of the model, and destroyed
//! with its element, which contains it.
//!
//! Which Comment is the description: UML's Comment has no field that says "description", but it
//! names the elements it is about (`Comment::annotatedElement`). The description is the first
//! Comment the element owns, in the order the store keeps (and a project saves) its owned
//! comments, that annotates the element itself and nothing else: a comment its owner holds about
//! itself alone. The description is recognised by that shape alone, so a Comment of that shape
//! read from a file counts as the description too. Describing makes exactly such a Comment, and
//! setting or clearing leaves none other of that shape behind, so an element has one description at
//! most. A Comment the element owns that annotates other elements, or none, is some other note:
//! it is never taken for the description, and describing never changes or removes it. The notes
//! drawn on a diagram are no Comments at all but the diagram's own nodes, their text on the node
//! (`archforge_diagram::commands::NOTE`), so they can never be taken for a description either.

use archforge_core::skills::Problem;
use archforge_engine::{CommandError, Reason};
use archforge_mof::{Obj, Store, Value};

use crate::commands::{fail, refuse, short, uuid_of};
use crate::{COMMENT, ENUMERATION_LITERAL, OPERATION, PACKAGE, PROPERTY, comment, element, property};

/// The most characters in a description: as many as an ER element's (`archforge_er::rules::MAX_TEXT`,
/// which a test holds it to), one rule across the notations.
pub const MAX_DESCRIPTION: usize = 4096;

/// The Comments of an element looked through for its description: reading one is bounded work,
/// done each frame for the element Properties shows. An element that owns this many Comments or
/// more (only a model from a file can hold them) is neither described nor cleared, since a
/// description past them could not be found.
pub const MAX_COMMENTS: usize = 64;

/// The kinds of UML element that take a description, as a refusal names them.
const DESCRIBABLE: &str = "a package, a class, an interface, an enumeration, a data type, a primitive type, an association class, an attribute, an operation, a literal, an association, a generalization, a dependency, an interface realization or a generalization set";

/// Whether `o` takes a description: a package, a classifier, a member (an attribute, an operation,
/// a literal) or a relationship; what the model shows and Properties describes. An association's
/// end goes with its association and takes none, as it is not removed on its own
/// ([`crate::commands::remove_elements`]); nor do a Comment, a parameter or the values inside a
/// member. Reading and describing agree on it.
pub fn describable(store: &Store, o: Obj) -> bool {
    let Ok(class) = store.class_of(o) else { return false };
    if class == PROPERTY {
        return store.get(o, property::ASSOCIATION).ok().and_then(|v| v.as_ref()).is_none();
    }
    crate::is_classifier(class)
        || crate::is_relationship(class)
        || [PACKAGE, OPERATION, ENUMERATION_LITERAL, crate::GENERALIZATION_SET].contains(&class)
}

/// The Comments `element` owns, borrowed, at most [`MAX_COMMENTS`] of them, and whether it owns
/// that many or more; none for an object that owns none or is no UML element.
fn owned_comments(store: &Store, element: Obj) -> (impl Iterator<Item = Obj> + '_, bool) {
    let listed = store.reflect(element).ok().and_then(|r| r.list(element::OWNED_COMMENT)).unwrap_or(&[]);
    (listed.iter().take(MAX_COMMENTS).filter_map(Value::as_ref), listed.len() >= MAX_COMMENTS)
}

/// Whether `c` is a live Comment that annotates `element` and nothing else.
fn is_about_alone(store: &Store, c: Obj, element: Obj) -> bool {
    store.is_alive(c)
        && store.class_of(c).ok() == Some(COMMENT)
        && store.reflect(c).ok().and_then(|r| r.list(comment::ANNOTATED_ELEMENT)).is_some_and(|a| a == [Value::Ref(element)])
}

/// Every Comment of the shape of a description `element` owns among its first [`MAX_COMMENTS`],
/// in its order: one, unless a file gave it more.
fn description_comments(store: &Store, element: Obj) -> Vec<Obj> {
    owned_comments(store, element).0.filter(|c| is_about_alone(store, *c, element)).collect()
}

/// The Comment that holds `element`'s description, when it has one: the first Comment it owns
/// that annotates it alone (see the module's note); none for an element that takes no description.
pub fn description_comment(store: &Store, element: Obj) -> Option<Obj> {
    if !describable(store, element) {
        return None;
    }
    owned_comments(store, element).0.find(|c| is_about_alone(store, *c, element))
}

/// `element`'s description, or nothing, also for an element that takes none.
pub fn description_of(store: &Store, element: Obj) -> String {
    description_comment(store, element)
        .and_then(|c| store.get(c, comment::BODY).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Why `text` is no description, when it is not: too long, holding a control character other than
/// a line break or a tab, or starting or ending with a space. An empty text clears one and is fine.
fn text_problem(text: &str) -> Option<Problem> {
    if text.chars().count() > MAX_DESCRIPTION {
        Some(Problem::too_long(MAX_DESCRIPTION))
    } else if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        Some(Problem::new(Reason::ControlCharacter, "holds a control character"))
    } else if text.trim() != text {
        Some(Problem::edge_space())
    } else {
        None
    }
}

/// Sets the description of `o`, a UML element (`element.describe`, through
/// [`crate::UmlNotation`]): a new text makes its description Comment, a changed one changes the
/// first such Comment's body, and an empty one removes the Comment; one undo step each, all or
/// nothing. Any further Comment of a description's shape (only a file can give one) goes in the
/// same step, so one description is left, or none once cleared. Any other Comment it owns is left
/// as it is. Refused, naming the value and what is accepted: an element of a kind that takes no
/// description, a text that is no description, and an element that owns [`MAX_COMMENTS`] Comments
/// or more.
pub fn describe_element(store: &mut Store, o: Obj, text: &str) -> Result<(), CommandError> {
    if !describable(store, o) {
        return Err(refuse("element", format!("{} takes no description; {DESCRIBABLE} does", uuid_of(store, o)?)).because(Reason::TakesNoDescription));
    }
    if let Some(problem) = text_problem(text) {
        return Err(refuse(
            "text",
            format!(
                "the text {:?} {problem}; a description is up to {MAX_DESCRIPTION} characters with no control character but line breaks and tabs, and no space at either end, or nothing to clear it",
                short(text)
            ),
        )
        .because(problem.reason));
    }
    if owned_comments(store, o).1 {
        return Err(refuse(
            "element",
            format!(
                "{} owns {MAX_COMMENTS} or more comments, which only a model read from a file holds; its description is looked for among its first {MAX_COMMENTS} alone, so it is neither described nor cleared here. Remove some of its comments in the file first",
                uuid_of(store, o)?
            ),
        )
        .because(Reason::TooManyComments { max: MAX_COMMENTS }));
    }
    let held = description_comments(store, o);
    store
        .transaction(|st| {
            let (first, rest) = match held.split_first() {
                Some((first, rest)) if !text.is_empty() => (Some(*first), rest),
                _ => (None, held.as_slice()),
            };
            for c in rest {
                st.destroy(*c)?;
            }
            match first {
                Some(c) => st.set(c, comment::BODY, Value::str(text)),
                None if text.is_empty() => Ok(()),
                None => {
                    let c = st.create(COMMENT)?;
                    st.set(c, comment::BODY, Value::str(text))?;
                    st.push(o, element::OWNED_COMMENT, Value::Ref(c))?;
                    st.push(c, comment::ANNOTATED_ELEMENT, Value::Ref(o)).map(|_| ())
                }
            }
        })
        .map_err(fail)
}
