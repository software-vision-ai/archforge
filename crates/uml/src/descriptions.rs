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
//! itself alone. Describing makes exactly such a Comment, so an element has one description at
//! most. A Comment the element owns that annotates other elements, or none, is some other note:
//! it is never taken for the description, and describing never changes or removes it. The notes
//! drawn on a diagram are no Comments at all but the diagram's own nodes, their text on the node
//! (`archforge_diagram::commands::NOTE`), so they can never be taken for a description either.

use archforge_engine::CommandError;
use archforge_mof::{ClassId, Obj, Store, Value};

use crate::commands::{fail, refuse, short, uuid_of};
use crate::{COMMENT, ENUMERATION_LITERAL, OPERATION, PACKAGE, PROPERTY, comment, element};

/// The most characters in a description: as many as an ER element's (`archforge_er::rules::MAX_TEXT`),
/// one rule across the notations.
pub const MAX_DESCRIPTION: usize = 4096;

/// The most Comments of an element looked through for its description: reading one is bounded
/// work, done each frame for the element Properties shows. An element that owns this many
/// Comments, none of them its description, is refused a new one.
pub const MAX_COMMENTS: usize = 64;

/// The kinds of UML element that take a description, as a refusal names them.
const DESCRIBABLE: &str = "a package, a class, an interface, an enumeration, a data type, a primitive type, an association class, an attribute or association end, an operation, a literal, an association, a generalization, a dependency or an interface realization";

/// Whether an element of `class` takes a description: a package, a classifier, a member (an
/// attribute or association end, an operation, a literal) or a relationship; what the model shows
/// and Properties describes. A Comment, a parameter and the values inside a member take none.
pub fn describable(class: ClassId) -> bool {
    crate::is_classifier(class) || crate::is_relationship(class) || [PACKAGE, PROPERTY, OPERATION, ENUMERATION_LITERAL].contains(&class)
}

/// The Comments `element` owns, borrowed, at most [`MAX_COMMENTS`] of them; none for an object
/// that owns none or is no UML element.
fn owned_comments(store: &Store, element: Obj) -> impl Iterator<Item = Obj> + '_ {
    let listed = store.reflect(element).ok().and_then(|r| r.list(element::OWNED_COMMENT)).unwrap_or(&[]);
    listed.iter().take(MAX_COMMENTS).filter_map(Value::as_ref)
}

/// Whether `c` is a live Comment that annotates `element` and nothing else.
fn is_about_alone(store: &Store, c: Obj, element: Obj) -> bool {
    store.is_alive(c)
        && store.class_of(c).ok() == Some(COMMENT)
        && store.reflect(c).ok().and_then(|r| r.list(comment::ANNOTATED_ELEMENT)).is_some_and(|a| a == [Value::Ref(element)])
}

/// The Comment that holds `element`'s description, when it has one: the first Comment it owns
/// that annotates it alone (see the module's note).
pub fn description_comment(store: &Store, element: Obj) -> Option<Obj> {
    owned_comments(store, element).find(|c| is_about_alone(store, *c, element))
}

/// `element`'s description, or nothing.
pub fn description_of(store: &Store, element: Obj) -> String {
    description_comment(store, element)
        .and_then(|c| store.get(c, comment::BODY).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Why `text` is no description, when it is not: too long, holding a control character other than
/// a line break or a tab, or starting or ending with a space. An empty text clears one and is fine.
fn text_problem(text: &str) -> Option<&'static str> {
    if text.chars().count() > MAX_DESCRIPTION {
        Some("is longer than 4096 characters")
    } else if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        Some("holds a control character")
    } else if text.trim() != text {
        Some("starts or ends with a space")
    } else {
        None
    }
}

/// Sets the description of `o`, a UML element (`element.describe`, through
/// [`crate::UmlNotation`]): a new text makes its description Comment, a changed one changes the
/// Comment's body, and an empty one removes the Comment; one undo step each, all or nothing. Any
/// other Comment it owns is left as it is. Refused, naming the value and what is accepted: an
/// element of a kind that takes no description, a text that is no description, and an element
/// that owns [`MAX_COMMENTS`] Comments already, none of them its description.
pub fn describe_element(store: &mut Store, o: Obj, text: &str) -> Result<(), CommandError> {
    let class = store.class_of(o).map_err(fail)?;
    if !describable(class) {
        return Err(refuse("element", format!("{} takes no description; {DESCRIBABLE} does", uuid_of(store, o)?)));
    }
    if let Some(problem) = text_problem(text) {
        return Err(refuse(
            "text",
            format!(
                "the text {:?} {problem}; a description is up to {MAX_DESCRIPTION} characters with no control character but line breaks and tabs, and no space at either end, or nothing to clear it",
                short(text)
            ),
        ));
    }
    let held = description_comment(store, o);
    match (held, text.is_empty()) {
        (None, true) => Ok(()),
        (Some(c), true) => store.destroy(c).map(|_| ()).map_err(fail),
        (Some(c), false) => store.set(c, comment::BODY, Value::str(text)).map_err(fail),
        (None, false) => {
            if owned_comments(store, o).count() >= MAX_COMMENTS {
                return Err(refuse(
                    "element",
                    format!("{} owns {MAX_COMMENTS} comments already, none of them its description; it takes no more", uuid_of(store, o)?),
                ));
            }
            store
                .transaction(|st| {
                    let c = st.create(COMMENT)?;
                    st.set(c, comment::BODY, Value::str(text))?;
                    st.push(o, element::OWNED_COMMENT, Value::Ref(c))?;
                    st.push(c, comment::ANNOTATED_ELEMENT, Value::Ref(o))?;
                    Ok(())
                })
                .map_err(fail)
        }
    }
}
