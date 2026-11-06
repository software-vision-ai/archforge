//! The UML 2.5.1 metamodel as a metamodel of the MOF core: 242 metaclasses, their properties,
//! enumerations and the five primitive types, generated from OMG's UML.xmi by `cargo xtask
//! generate uml`. Nothing here is written by hand except this file: the generated code under
//! `generated/` is what the XMI gives, checked by the gate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

/// The parameter lines `uml.attribute.add` takes for a new attribute's modifiers.
macro_rules! modifier_lines {
    () => {
        concat!(
            "default: string = \"\"; the default value as typed, read by the type: an Integer's a whole number, a Real's a number, a Boolean's true or false, an UnlimitedNatural's a whole number of 0 or more or *, a String's any text, an enumeration's one of its literals' names; any other type's, or no type's, an expression's text; nothing for none\n",
            "derived: bool = false; its value is worked out from others, written / before its name\n",
            "read_only: bool = false; it is not changed once set, written {readOnly}\n",
            "ordered: bool = false; its values have an order, written {ordered} when it holds more than one\n",
            "unique: bool = true; its values are all different; written {nonunique} when false and it holds more than one\n",
            "leaf: bool = false; nothing may redefine it (UML's isLeaf), written {leaf}\n",
            "subsets: array = []; the properties it subsets, each an id or an {id, name} object as the reads give them: of its classifier (an end's: the class at the other end) or one that classifier specializes, named otherwise, whose type its type conforms to, whose upper bound is not below its own, and not subsetting it already; at most 32\n",
            "redefines: array = []; the properties it redefines, each an id or an {id, name} object as the reads give them: of a classifier its classifier specializes, whose type its type conforms to, whose bounds hold its own, not composite unless it is, and not leaf; at most 32"
        )
    };
}

/// The parameter lines `uml.attribute.set` and `uml.association.set-end` take for the modifiers:
/// each may be left out, which keeps what the property has; clearing is said explicitly.
macro_rules! kept_modifier_lines {
    () => {
        concat!(
            "default: string?; the default value as typed, read by the type: an Integer's a whole number, a Real's a number, a Boolean's true or false, an UnlimitedNatural's a whole number of 0 or more or *, a String's any text, an enumeration's one of its literals' names; any other type's, or no type's, an expression's text; \"\" clears it; left out, the default is kept (and read again by a new type, which it must fit)\n",
            "derived: bool?; its value is worked out from others, written / before its name; left out, kept\n",
            "read_only: bool?; it is not changed once set, written {readOnly}; left out, kept\n",
            "ordered: bool?; its values have an order, written {ordered} when it holds more than one; left out, kept\n",
            "unique: bool?; its values are all different; written {nonunique} when false and it holds more than one; left out, kept\n",
            "leaf: bool?; nothing may redefine it (UML's isLeaf), written {leaf}: made leaf only while no property redefines it; cleared, it may be redefined again; left out, kept\n",
            "subsets: array?; the properties it subsets, each an id or an {id, name} object as the reads give them: of its classifier (an end's: the class at the other end) or one that classifier specializes, named otherwise, whose type its type conforms to, whose upper bound is not below its own, and not subsetting it already; at most 32; [] clears them; left out, kept; only properties not there before are checked\n",
            "redefines: array?; the properties it redefines, each an id or an {id, name} object as the reads give them: of a classifier its classifier specializes, whose type its type conforms to, whose bounds hold its own, not composite unless it is, and not leaf; at most 32; [] clears them; left out, kept; only properties not there before are checked"
        )
    };
}

pub mod commands;
pub mod constraints;
pub mod descriptions;
pub mod figures;
pub mod generalization_sets;
mod generated;
pub mod members;
pub mod modifiers;
pub mod nesting;
pub mod qualifiers;
pub mod relations;

use std::sync::Arc;

use archforge_diagram::figure::{Card, Line};
use archforge_diagram::{Notation, ProjectView};
use archforge_mof::{ClassId, MetaError, Obj, Store, Universe};

pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::UmlSlabs;

/// A universe of UML alone.
pub fn universe() -> Result<Arc<Universe>, MetaError> {
    Ok(Arc::new(Universe::new(vec![metamodel()?])?))
}

/// The class diagram kind: a diagram of a package that shows its classes as cards.
pub const CLASS_DIAGRAM: &str = "uml.class";

/// The kinds of classifier a class diagram places, each a card with its members: a class, an
/// interface, an enumeration, whose members are its literals, a data type, a value without
/// identity with attributes and operations as a class has, a primitive type, a value with no
/// members at all (AR-124), and an association class, a class and an association at once, whose
/// card sits above its line (AR-125). Each is matched by its exact metaclass, never by conformance: an
/// enumeration and a primitive type are DataTypes in UML's metamodel, but the data type kind is the
/// DataType metaclass alone, and a skill for every data type reaches neither (a primitive type is a
/// skill kind of its own). The built-in types, Integer, String and the others kept in the package
/// Types, are primitive types too ([`members::is_built_in`]).
pub const CLASSIFIERS: [ClassId; 6] = [CLASS, INTERFACE, ENUMERATION, DATA_TYPE, PRIMITIVE_TYPE, ASSOCIATION_CLASS];

/// Whether `class` is one of the [`CLASSIFIERS`].
pub fn is_classifier(class: ClassId) -> bool {
    CLASSIFIERS.contains(&class)
}

/// The relationships a class diagram draws as lines: associations, generalizations, dependencies,
/// interface realizations, and association classes, which have a card of their own besides
/// (AR-125): one element shown twice on a diagram, a node and an edge, so no view record is new.
pub const RELATIONSHIPS: [ClassId; 5] = [ASSOCIATION, GENERALIZATION, DEPENDENCY, INTERFACE_REALIZATION, ASSOCIATION_CLASS];

/// Whether `class` is an association or an association class: what has ends to set, roles,
/// multiplicities, navigability and aggregation.
pub fn is_association(class: ClassId) -> bool {
    class == ASSOCIATION || class == ASSOCIATION_CLASS
}

/// Whether `class` is one of the [`RELATIONSHIPS`].
pub fn is_relationship(class: ClassId) -> bool {
    RELATIONSHIPS.contains(&class)
}

/// A class card's width and height when the user names none: the header, an attribute row, an
/// operation row and the line between them.
pub const CLASS_SIZE: (f64, f64) = (240.0, 146.0);
/// A primitive type's card as it is placed: its header alone, the `«primitive»` eyebrow over its
/// name, with no compartment under it.
pub const PRIMITIVE_TYPE_SIZE: (f64, f64) = (240.0, 66.0);
/// A package's card as it is placed: its name, and a few of what it holds.
pub const PACKAGE_SIZE: (f64, f64) = (240.0, 148.0);

/// UML in a project's view: class diagrams of a package showing its
/// classes, interfaces and data types as plain cards with their attributes and operations, its
/// enumerations with their literals, its primitive types with their name alone, and the associations,
/// generalizations, dependencies and interface realizations between them as lines (`figures`); an
/// association class as a line and a card of its own; packages and classes removed with what they
/// hold.
pub struct UmlNotation;

impl Notation for UmlNotation {
    fn name(&self) -> &'static str {
        "UML"
    }

    fn metamodel(&self) -> u16 {
        INDEX
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[CLASS_DIAGRAM]
    }

    fn is_of(&self, _kind: &str, class: ClassId) -> bool {
        class == PACKAGE
    }

    /// A class diagram is always of a package.
    fn needs_element(&self, _kind: &str) -> bool {
        true
    }

    /// A class diagram shows classes, interfaces, enumerations, data types, primitive types,
    /// association classes and packages as cards, and constraints as braces or notes (AR-133).
    fn shows(&self, _kind: &str, class: ClassId) -> bool {
        is_classifier(class) || class == PACKAGE || class == CONSTRAINT
    }

    /// A package's diagram shows its own classes and, as outsiders, classes of other packages; and
    /// the packages it holds, each leading to its own diagram (AR-95).
    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let of = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
        match store.class_of(element).ok() {
            Some(c) if c == PACKAGE => of.is_some() && commands::package_of(store, element) == of,
            // A constraint shows on any class diagram: its elements may be on several.
            Some(c) if c == CONSTRAINT => of.is_some() && constraints::in_model(store, element),
            // A nested classifier belongs in its owner's package (AR-132).
            _ => of.is_some() && nesting::home_package(store, element).is_some(),
        }
    }

    /// A node's size when none is named: a package's card, a primitive type's header alone, and a
    /// class's card for every other kind.
    fn default_size(&self, class: ClassId) -> (f64, f64) {
        match class {
            c if c == PACKAGE => PACKAGE_SIZE,
            c if c == PRIMITIVE_TYPE => PRIMITIVE_TYPE_SIZE,
            c if c == CONSTRAINT => (constraints::NOTE_WIDTH, 96.0),
            _ => CLASS_SIZE,
        }
    }

    /// Associations, generalizations, dependencies, interface realizations and association classes.
    fn connects(&self, _kind: &str, class: ClassId) -> bool {
        is_relationship(class)
    }

    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        figures::ends(store, element)
    }

    /// The texts a line of `class` places along it.
    fn text_slots(&self, class: ClassId) -> &'static [&'static str] {
        figures::text_slots(class)
    }

    /// A class, an interface or a data type: its name, a class's slanted when abstract, its
    /// attributes, then its operations, a static one underlined; an enumeration: its name and its literals; a primitive type:
    /// its name alone.
    fn card(&self, store: &Store, element: Obj, diagram: Obj) -> Option<Card> {
        figures::card(store, element, diagram)
    }

    fn line(&self, store: &Store, element: Obj) -> Option<Line> {
        figures::line(store, element)
    }

    /// A constraint note's dashed links to the nodes of the elements it constrains, worked out from
    /// the model on every frame (AR-133); braces have none.
    fn links(&self, store: &Store, element: Obj, diagram: Obj) -> Vec<Obj> {
        // Asked of every node each frame: only a constraint has links, so nothing else is looked up.
        if !constraints::is_constraint(store, element) {
            return Vec::new();
        }
        match constraints::shown_on(store, diagram, element) {
            Some((_, constraints::Form::Note)) => constraints::linked_nodes(store, diagram, element),
            _ => Vec::new(),
        }
    }

    /// A nested classifier's owner, tied to it on a diagram by a line ending in ⊕ at the owner
    /// (AR-132).
    fn nested_in(&self, store: &Store, element: Obj) -> Option<Obj> {
        nesting::classifier_owner(store, element)
    }

    /// The text of a line's slot `slot`: its name, an end's role or multiplicity.
    fn slot_text(&self, store: &Store, element: Obj, slot: &str) -> String {
        figures::slot_text(store, element, slot)
    }

    /// A skill sits on a package, a class, an interface, an enumeration, a data type, a primitive
    /// type of the model's own, an association class, an attribute or an operation; it aims at
    /// every class, interface, enumeration, data type, primitive type, association class,
    /// attribute, operation or association below its subject (owner decision: an interface carries
    /// skills as a class does, and an enumeration, a data type and a primitive type as an interface
    /// does; an association class carries them as the class it is, AR-125).
    fn skill_subjects(&self) -> &'static [ClassId] {
        &[PACKAGE, CLASS, INTERFACE, ENUMERATION, DATA_TYPE, PRIMITIVE_TYPE, ASSOCIATION_CLASS, PROPERTY, OPERATION]
    }

    /// The kinds a package skill may aim at, each with the word that names it. An association
    /// class is a Class and an Association alike in UML's metamodel, conforming to both equally, so
    /// it is a kind of its own (AR-125): the core's closer-kind rule then keeps a skill for every
    /// class and one for every association off it, as a skill for every data type misses an
    /// enumeration; a skill for every association class reaches it.
    fn skill_kinds(&self) -> &'static [(ClassId, &'static str)] {
        &[
            (CLASS, "class"),
            (INTERFACE, "interface"),
            (ENUMERATION, "enumeration"),
            (DATA_TYPE, "data type"),
            (PRIMITIVE_TYPE, "primitive type"),
            (ASSOCIATION_CLASS, "association class"),
            (PROPERTY, "attribute"),
            (OPERATION, "operation"),
            (ASSOCIATION, "association"),
        ]
    }

    /// An association's end is a property but no attribute: its association carries the skill. A
    /// built-in type (Integer, String and the others in the package Types) is the same in every
    /// project and carries none, nor is it aimed at by a skill for every primitive type; a primitive
    /// type of the model's own does.
    fn skill_problem(&self, store: &Store, element: Obj) -> Option<&'static str> {
        if members::is_built_in(store, element) {
            return Some("a built-in type carries no skill; a primitive type of the model's own does");
        }
        // A qualifier is its end's, not an attribute (AR-127).
        if qualifiers::is_qualifier(store, element) {
            return Some("a qualifier carries no skill; its association's classes do");
        }
        let end = store.class_of(element).ok() == Some(PROPERTY) && store.get(element, property::ASSOCIATION).ok().and_then(|v| v.as_ref()).is_some();
        end.then_some("an association's end carries no skill; its association's classes do")
    }

    /// Removes UML elements with what they hold and what shows them; an enumeration with its literals.
    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<usize, archforge_engine::CommandError> {
        commands::remove_elements(store, objs)
    }

    /// Sets a UML element's description, kept as a Comment the element owns (AR-186).
    fn describe(&self, store: &mut Store, obj: Obj, text: &str) -> Result<(), archforge_engine::CommandError> {
        descriptions::describe_element(store, obj, text)
    }

    /// A UML element's description: its description Comment's body (AR-186).
    fn description(&self, store: &Store, obj: Obj) -> String {
        descriptions::description_of(store, obj)
    }
}

/// A project's notations when it holds UML alone.
pub static ALONE: &[&dyn Notation] = &[&UmlNotation];

/// The words UML's figures say, for the shell's catalogue: the texts, and the plural.
pub const WORDS: &[&str] = &[
    "Class",
    "Class · {package}",
    "«interface»",
    "«interface» · {package}",
    "«enumeration»",
    "«enumeration» · {package}",
    "«dataType»",
    "«dataType» · {package}",
    "«primitive»",
    "«primitive» · {package}",
    "Association class",
    "Association class · {package}",
    "Package",
    "Constraint · not evaluated",
];
pub const PLURALS: &[&str] = &["+ {n} more member", "+ {n} more literal", "+ {n} more element", "+ {n} more line"];

/// The command registry of UML alone: the edit area, the shared element commands, UML's, the
/// diagrams' and the skills'.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, ProjectView>>, String> {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend(archforge_diagram::project::specs());
    specs.extend(commands::specs());
    specs.extend(members::specs());
    specs.extend(relations::specs());
    specs.extend(constraints::specs());
    specs.extend(generalization_sets::specs());
    specs.extend(qualifiers::specs());
    specs.extend(nesting::specs());
    specs.extend(archforge_diagram::commands::specs::<ProjectView>());
    specs.extend(archforge_core::skills::specs::<ProjectView>());
    archforge_engine::Registry::new(specs)
}
