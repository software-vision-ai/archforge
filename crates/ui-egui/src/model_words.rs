//! Words for what the metamodel names: a property, an enumeration literal and the kind of an
//! element, made from the metamodel's own names by [`words`] and looked up through the catalogues.
//! The translation is the interface thread's language, so a view makes these words while drawing,
//! never on another thread.
//!
//! The catalogues hold a row for every property a card shows and every literal in the model area
//! (the core, ER and UML; a diagram's own are no model words). Kind words go only to the
//! [`KINDS`] the notations create, as the catalogues stay smaller that way; the rest say Element.

use archforge_mof::{ClassId, DataId, DataKind, PropId, Universe};

use crate::i18n::{tr, tr_ctx};

/// The classes a notation creates, and so the only ones that get a kind word of their own. A new
/// `create` call by constant name fails the catalogue test until its class is here, a diagram's
/// node, edge and label aside; one that picks the class through a variable (the classifier kinds,
/// a default value) is listed by hand.
pub(crate) const KINDS: &[ClassId] = &[
    archforge_core::LINK,
    archforge_core::MODEL,
    archforge_core::SKILL,
    archforge_core::TAG,
    archforge_er::ATTRIBUTE,
    archforge_er::CHECK_CONSTRAINT,
    archforge_er::DOMAIN,
    archforge_er::DOMAIN_CHECK,
    archforge_er::ENTITY,
    archforge_er::ENUM_TYPE,
    archforge_er::FOREIGN_KEY,
    archforge_er::GENERALIZATION,
    archforge_er::INDEX_COLUMN,
    archforge_er::PRIMARY_KEY,
    archforge_er::RELATIONSHIP,
    archforge_er::SCHEMA,
    archforge_er::SEQUENCE,
    archforge_er::TABLE_INDEX,
    archforge_er::UNIQUE_CONSTRAINT,
    archforge_er::VIEW,
    archforge_uml::ASSOCIATION,
    archforge_uml::ASSOCIATION_CLASS,
    archforge_uml::CLASS,
    archforge_uml::CLASSIFIER_TEMPLATE_PARAMETER,
    archforge_uml::COMMENT,
    archforge_uml::CONSTRAINT,
    archforge_uml::DATA_TYPE,
    archforge_uml::DEPENDENCY,
    archforge_uml::ENUMERATION,
    archforge_uml::ENUMERATION_LITERAL,
    archforge_uml::EXTENSION,
    archforge_uml::EXTENSION_END,
    archforge_uml::GENERALIZATION,
    archforge_uml::GENERALIZATION_SET,
    archforge_uml::INSTANCE_SPECIFICATION,
    archforge_uml::INSTANCE_VALUE,
    archforge_uml::INTERFACE,
    archforge_uml::INTERFACE_REALIZATION,
    archforge_uml::LITERAL_BOOLEAN,
    archforge_uml::LITERAL_INTEGER,
    archforge_uml::LITERAL_REAL,
    archforge_uml::LITERAL_STRING,
    archforge_uml::LITERAL_UNLIMITED_NATURAL,
    archforge_uml::MODEL,
    archforge_uml::OPAQUE_EXPRESSION,
    archforge_uml::OPERATION,
    archforge_uml::PACKAGE,
    archforge_uml::PARAMETER,
    archforge_uml::PRIMITIVE_TYPE,
    archforge_uml::PROFILE,
    archforge_uml::PROFILE_APPLICATION,
    archforge_uml::PROPERTY,
    archforge_uml::REDEFINABLE_TEMPLATE_SIGNATURE,
    archforge_uml::SLOT,
    archforge_uml::STEREOTYPE,
    archforge_uml::TEMPLATE_BINDING,
    archforge_uml::TEMPLATE_PARAMETER_SUBSTITUTION,
];

/// Whether `metamodel` is in the model area, the one these words cover: not the diagrams'.
fn in_the_model(metamodel: u16) -> bool {
    metamodel != archforge_diagram::INDEX
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

/// The label of `prop` in this thread's language; none when it is not a property of the model area.
pub fn property_label(universe: &Universe, prop: PropId) -> Option<String> {
    Some(tr(&words(&universe.prop(prop).filter(|_| in_the_model(prop.metamodel()))?.name)))
}

/// The label of literal `index` of the enumeration `ty` in this thread's language; none when `ty`
/// is no enumeration of the model area or has no such literal.
pub fn literal_label(universe: &Universe, ty: DataId, index: u32) -> Option<String> {
    let DataKind::Enumeration(literals) = &universe.data_type(ty).filter(|_| in_the_model(ty.metamodel()))?.kind else { return None };
    Some(tr(&words(literals.get(usize::try_from(index).ok()?)?)))
}

/// The kind word of `class` in this thread's language: its own for one of the [`KINDS`], otherwise
/// Element. UML's Profile has a row of its own, as the plain word is a person's profile.
pub fn kind_label(universe: &Universe, class: ClassId) -> String {
    if class == archforge_uml::PROFILE {
        return tr_ctx("uml", "Profile");
    }
    match universe.class(class).filter(|_| KINDS.contains(&class)) {
        Some(known) => tr(&words(&known.name)),
        None => tr("Element"),
    }
}

/// Every key these labels look up, for the catalogue tests: each property a card shows and each
/// literal of the model area, and the [`KINDS`] but Profile, which has a row of its own.
#[cfg(test)]
pub(crate) fn texts() -> Vec<String> {
    let Ok(universe) = archforge_notations::universe() else { return Vec::new() };
    let mut all = Vec::new();
    for metamodel in universe.metamodels().filter(|m| in_the_model(m.index)) {
        all.extend(metamodel.props.iter().filter(|p| archforge_notations::differences::on_a_card(p)).map(|p| words(&p.name)));
        for ty in &metamodel.data_types {
            if let DataKind::Enumeration(literals) = &ty.kind {
                all.extend(literals.iter().map(|l| words(l)));
            }
        }
    }
    all.extend(KINDS.iter().filter(|c| **c != archforge_uml::PROFILE).filter_map(|c| universe.class(*c)).map(|c| words(&c.name)));
    all
}
