//! Words for what the metamodel names: a property, an enumeration literal and the kind of an
//! element, made from the metamodel's own names by [`words`] and looked up through the catalogues.
//! The translation is the interface thread's language, so a view makes these words while drawing,
//! never on another thread.
//!
//! The catalogues hold a row for every property a card shows and every literal in the model area:
//! the core's in the shell's catalogue, and those of each notation's metamodel
//! ([`NotationUi::metamodel`](crate::notation_ui::NotationUi::metamodel)) in that notation's own
//! ([`NotationUi::catalogues`](crate::notation_ui::NotationUi::catalogues)), unless the core or
//! the other notation names them too; a diagram's own are no model words. So the words are said
//! for the core and the notations the shell was given, whose catalogues it registered, and for no
//! other metamodel. Kind words go only to the classes
//! the core and the notations the shell was given create ([`NotationUi::created_classes`](crate::notation_ui::NotationUi::created_classes)), as the catalogues stay
//! smaller that way, and to the default-diagram binding, the one class of the diagrams a
//! comparison lists as the model's; the rest say Element.

use archforge_mof::{ClassId, DataId, DataKind, PropId, Universe};

use crate::i18n::tr;
use crate::notation_ui::{NotationUi, of_class};

/// The classes the core creates, the only ones beside those the notations create
/// ([`NotationUi::created_classes`](crate::notation_ui::NotationUi::created_classes)) that get a kind word of their own. A new `create` call by
/// constant name fails the catalogue test until its class is here or its notation's, a diagram's
/// node, edge and label aside; one that picks the class through a variable (the classifier kinds,
/// a default value) is listed by hand.
const CORE_KINDS: &[ClassId] = &[archforge_core::LINK, archforge_core::MODEL, archforge_core::SKILL, archforge_core::TAG];

/// Whether `class` gets a kind word of its own: the core creates it, or its notation of `uis` does.
fn is_kind(uis: &[&'static dyn NotationUi], class: ClassId) -> bool {
    CORE_KINDS.contains(&class) || of_class(uis, class).is_some_and(|ui| ui.created_classes().contains(&class))
}

/// Every class that gets a kind word of its own, the core's and each notation's of the app.
#[cfg(test)]
pub(crate) fn kinds() -> Vec<ClassId> {
    CORE_KINDS.iter().chain(crate::tests::SCREENS.notation_uis().iter().flat_map(|ui| ui.created_classes())).copied().collect()
}

/// Whether `metamodel` is one these words cover: the core's, or the metamodel of a notation of `uis`
/// (the shell's), whose rows its catalogue holds; never the diagrams'.
fn in_the_model(uis: &[&'static dyn NotationUi], metamodel: u16) -> bool {
    metamodel == archforge_core::INDEX || uis.iter().any(|ui| ui.metamodel() == metamodel)
}

/// A metamodel name as words, the catalogue key: `isAbstract` is "Is abstract", `InterfaceRealization`
/// is "Interface realization". A run of capitals stays one word as written (`isID` is "Is ID").
pub fn words(name: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut chars = name.chars().peekable();
    let mut previous: Option<char> = None;
    while let Some(c) = chars.next() {
        let starts = c.is_uppercase() && previous.is_none_or(|p| !p.is_uppercase() || chars.peek().is_some_and(|n| n.is_lowercase()));
        match parts.last_mut() {
            Some(part) if !starts => part.push(c),
            _ => parts.push(c.to_string()),
        }
        previous = Some(c);
    }
    let cased = parts.iter().enumerate().map(|(i, part)| {
        if part.chars().count() > 1 && part.chars().all(char::is_uppercase) {
            part.clone()
        } else if i == 0 {
            let mut letters = part.chars();
            letters.next().map(|first| first.to_uppercase().chain(letters).collect()).unwrap_or_default()
        } else {
            part.to_lowercase()
        }
    });
    cased.collect::<Vec<String>>().join(" ")
}

/// The label of `prop` in this thread's language, as the notations of `uis` (the shell's) word it;
/// none when it is not a property of the core or of one of their metamodels.
pub fn property_label(uis: &[&'static dyn NotationUi], universe: &Universe, prop: PropId) -> Option<String> {
    Some(tr(&words(&universe.prop(prop).filter(|_| in_the_model(uis, prop.metamodel()))?.name)))
}

/// The label of literal `index` of the enumeration `ty` in this thread's language, as the
/// notations of `uis` (the shell's) word it; none when `ty` is no enumeration of the core or of one
/// of their metamodels, or has no such literal.
pub fn literal_label(uis: &[&'static dyn NotationUi], universe: &Universe, ty: DataId, index: u32) -> Option<String> {
    let DataKind::Enumeration(literals) = &universe.data_type(ty).filter(|_| in_the_model(uis, ty.metamodel()))?.kind else { return None };
    Some(tr(&words(literals.get(usize::try_from(index).ok()?)?)))
}

/// The word Models and Properties use for `class` where it isn't its metamodel's name: its
/// notation's own, of `uis` ([`NotationUi::kind_word`](crate::notation_ui::NotationUi::kind_word)),
/// and the word a comparison says for the default-diagram binding.
fn notation_word(uis: &[&'static dyn NotationUi], class: ClassId) -> Option<String> {
    if class == archforge_diagram::DEFAULT_DIAGRAM_BINDING {
        return Some(tr("Default diagram"));
    }
    of_class(uis, class)?.kind_word(class)
}

/// The kind word of `class` in this thread's language, as the notations of `uis` (the shell's)
/// say it: its notation's own where it has one, the metamodel's name for the rest of the kinds the
/// core and the notations create, otherwise Element.
pub fn kind_label(uis: &[&'static dyn NotationUi], universe: &Universe, class: ClassId) -> String {
    if let Some(word) = notation_word(uis, class) {
        return word;
    }
    match universe.class(class).filter(|_| is_kind(uis, class)) {
        Some(known) => tr(&words(&known.name)),
        None => tr("Element"),
    }
}

/// Every key these labels look up, for the catalogue tests, each with the index of the metamodel
/// that names it, so the tests can tell whose catalogue holds its row: each property a card shows
/// and each literal of the core and of each notation's metamodel the tests register as the app
/// does, and the kinds ([`kinds`]) but those with a notation's word, whose rows the scan reads from
/// each notation's [`NotationUi::kind_word`](crate::notation_ui::NotationUi::kind_word).
#[cfg(test)]
pub(crate) fn texts() -> Vec<(u16, String)> {
    let Ok(universe) = archforge_notations::universe() else { return Vec::new() };
    let uis = crate::tests::SCREENS.notation_uis();
    let mut all = Vec::new();
    for metamodel in universe.metamodels().filter(|m| in_the_model(uis, m.index)) {
        let named = |name: &str| (metamodel.index, words(name));
        all.extend(metamodel.props.iter().filter(|p| archforge_notations::differences::on_a_card(p)).map(|p| named(&p.name)));
        for ty in &metamodel.data_types {
            if let DataKind::Enumeration(literals) = &ty.kind {
                all.extend(literals.iter().map(|l| named(l)));
            }
        }
    }
    let worded = kinds().into_iter().filter(|c| notation_word(uis, *c).is_none());
    all.extend(worded.filter_map(|c| universe.class(c).map(|known| (c.metamodel(), words(&known.name)))));
    all
}
