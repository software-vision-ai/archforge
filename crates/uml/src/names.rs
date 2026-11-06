//! Which UML elements may share a name in one owner (owner decision, 2026-11-06, AR-208): as UML
//! has it, a namespace tells its members apart by kind first and by name within a kind, so only
//! two elements of one kind clash, whatever the case of their names. The kinds:
//!
//! - every classifier is one kind: a class, an interface, an enumeration, a data type, a primitive
//!   type and an association class, since a type is picked by its name and must never be in
//!   doubt;
//! - a package; a property (an attribute, an association's end and a qualifier: an association
//!   class's attributes and its ends' roles are all its properties); an operation; a parameter;
//!   an enumeration literal; a generalization set: each a kind of its own;
//! - any other metaclass is a kind of its own, as UML's default rule has it;
//! - a profile is a package's kind (AR-134); a stereotype, the hidden UML library's metaclass
//!   classes (each in the library alone, which is unnamed), a stereotype's `base_` attributes
//!   (properties, so a stereotype's own property never takes one's name), an extension's end, and a
//!   stereotype application's instance specification are each their metaclass's kind; a core link
//!   is no UML element and has no name;
//! - a relationship (an association, a dependency, a realization) and a constraint carry their
//!   names as labels and are told apart by what they join or constrain: their names never clash.
//!
//! [`kind`] is the one rule. The project's check as it opens (`archforge_notations`) asks it for
//! every UML element, and every command that names, adds or moves a UML element asks [`clash`],
//! which asks the same rule of one name, so an edit refused by neither leaves a project that opens
//! again.

use archforge_mof::{ClassId, NameKind, Obj, Store};

use crate::{
    ABSTRACTION, ASSOCIATION, COMMUNICATION_PATH, COMPONENT_REALIZATION, CONSTRAINT, DEPENDENCY, DEPLOYMENT, EXTEND, EXTENSION, GENERALIZATION,
    INCLUDE, INFORMATION_FLOW, INTERFACE_REALIZATION, MANIFESTATION, PACKAGE, PROFILE, REALIZATION, SUBSTITUTION, USAGE,
};

/// The kind every classifier is, whatever its metaclass.
pub const CLASSIFIER: NameKind = NameKind::Group("classifier");

/// The relationships, whose names are labels: told apart by what they join, never by name.
const LABELLED: [ClassId; 16] = [
    ASSOCIATION,
    COMMUNICATION_PATH,
    EXTENSION,
    DEPENDENCY,
    ABSTRACTION,
    REALIZATION,
    USAGE,
    COMPONENT_REALIZATION,
    INTERFACE_REALIZATION,
    SUBSTITUTION,
    MANIFESTATION,
    DEPLOYMENT,
    INFORMATION_FLOW,
    INCLUDE,
    EXTEND,
    GENERALIZATION,
];

/// The kind a UML element of `class` is told apart within by its name, among its owner's
/// elements; none when its name is a label that never clashes (a relationship, a constraint).
pub fn kind(class: ClassId) -> Option<NameKind> {
    if crate::is_classifier(class) {
        return Some(CLASSIFIER);
    }
    if class == CONSTRAINT || LABELLED.contains(&class) {
        return None;
    }
    // A profile is a package (AR-134): the two never share a name in one owner, as UML's
    // distinguishability has it for a kind and its specializations.
    if class == PROFILE {
        return Some(NameKind::Class(PACKAGE));
    }
    // A package, a property, an operation, a parameter, a literal, a generalization set, and any
    // other metaclass: each a kind of its own.
    Some(NameKind::Class(class))
}

/// The element `owner` holds, other than `except`, that an element of `class` named `name` would
/// clash with there by [`kind`]: one of the same kind whose name differs only in case. None for an
/// empty name and for a kind whose names never clash.
pub fn clash(store: &Store, owner: Obj, class: ClassId, name: &str, except: Option<Obj>) -> Option<Obj> {
    store.name_clash(owner, class, name, except, kind)
}
