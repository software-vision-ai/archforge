//! What a comparison calls an element with no name of its own: what it joins or what it states, as
//! the notation writes it. These are the same in every language, so they can be made off the
//! interface thread; an element with nothing to say is left to go by its kind.

use archforge_core::name_of;
use archforge_er as er;
use archforge_mof::{ClassId, Obj, PropId, Store};
use archforge_uml as uml;

/// The most characters a label holds; the cards say the rest in their fields.
const LONGEST: usize = 80;

/// The words for the nameless `obj`, on one line and cut to [`LONGEST`]; none when it has none.
pub(crate) fn unnamed(store: &Store, obj: Obj) -> Option<String> {
    let class = store.class_of(obj).ok()?;
    let text = match class.metamodel() {
        er::INDEX => er_words(store, obj, class),
        uml::INDEX => uml_words(store, obj, class),
        _ => None,
    }?;
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(LONGEST) {
        _ if flat.is_empty() => None,
        Some((end, _)) => Some(format!("{}…", &flat[..end])),
        None => Some(flat),
    }
}

/// The live element `prop` of `obj` refers to.
fn target(store: &Store, obj: Obj, prop: PropId) -> Option<Obj> {
    store.get(obj, prop).ok()?.as_ref().filter(|o| store.is_alive(*o))
}

/// The text `prop` of `obj` holds; empty when it holds none.
fn text(store: &Store, obj: Obj, prop: PropId) -> String {
    store.get(obj, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// `from → to` as a line between them runs.
fn arrow(store: &Store, from: Obj, to: Obj) -> Option<String> {
    Some(format!("{} → {}", name_of(store, from), name_of(store, to)))
}

/// An ER element's words: a line by its tables, a key or index as the export names it, a part by what it states.
fn er_words(store: &Store, obj: Obj, class: ClassId) -> Option<String> {
    match class {
        c if c == er::RELATIONSHIP => arrow(store, target(store, obj, er::relationship::FROM)?, target(store, obj, er::relationship::TO)?),
        c if c == er::GENERALIZATION => {
            let (supertype, subtype, _) = er::rules::generalization_parts(store, obj);
            arrow(store, subtype?, supertype?)
        }
        c if c == er::FOREIGN_KEY => {
            arrow(store, er::rules::entity_of(store, obj)?, er::rules::entity_of(store, target(store, obj, er::foreign_key::REFERENCES)?)?)
        }
        c if [er::PRIMARY_KEY, er::UNIQUE_CONSTRAINT, er::CHECK_CONSTRAINT, er::TABLE_INDEX].contains(&c) => {
            er::rules::entity_of(store, obj).map(|_| er::rules::display_name(store, obj))
        }
        c if c == er::INDEX_COLUMN => Some(
            target(store, obj, er::index_column::ATTRIBUTE).map_or_else(|| text(store, obj, er::index_column::EXPRESSION), |a| name_of(store, a)),
        ),
        c if c == er::DOMAIN_CHECK => Some(text(store, obj, er::domain_check::EXPRESSION)),
        _ => None,
    }
}

/// A UML element's words: a line by its ends, a comment or constraint by what it says, a value as it is typed.
fn uml_words(store: &Store, obj: Obj, class: ClassId) -> Option<String> {
    match class {
        c if c == uml::COMMENT => Some(text(store, obj, uml::comment::BODY)),
        c if c == uml::CONSTRAINT => Some(uml::constraints::braces(store, obj)),
        c if c == uml::PROFILE_APPLICATION => target(store, obj, uml::profile_application::APPLIED_PROFILE).map(|p| name_of(store, p)),
        // A value specification's text; empty for anything else, which then goes by its kind.
        _ => uml::figures::ends(store, obj).and_then(|(from, to)| arrow(store, from, to)).or_else(|| Some(uml::modifiers::spec_text(store, obj))),
    }
}
