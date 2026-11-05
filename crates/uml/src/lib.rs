//! The UML 2.5.1 metamodel as a metamodel of the MOF core: 242 metaclasses, their properties,
//! enumerations and the five primitive types, generated from OMG's UML.xmi by `cargo xtask
//! generate uml`. Nothing here is written by hand except this file: the generated code under
//! `generated/` is what the XMI gives, checked by the gate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod commands;
pub mod figures;
mod generated;
pub mod members;
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

/// The kinds of classifier a class diagram places, each a card with its members: a class and an
/// interface.
pub const CLASSIFIERS: [ClassId; 2] = [CLASS, INTERFACE];

/// Whether `class` is one of the [`CLASSIFIERS`].
pub fn is_classifier(class: ClassId) -> bool {
    CLASSIFIERS.contains(&class)
}

/// A class card's width and height when the user names none: the header, an attribute row, an
/// operation row and the line between them.
pub const CLASS_SIZE: (f64, f64) = (240.0, 136.0);
/// A package's card as it is placed: its name, and a few of what it holds.
pub const PACKAGE_SIZE: (f64, f64) = (240.0, 148.0);

/// UML in a project's view: class diagrams of a package showing its
/// classes and interfaces as plain cards with their attributes and operations, and the associations,
/// generalizations and dependencies between them as lines (`figures`); packages and classes removed
/// with what they hold.
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

    /// A class diagram shows classes, interfaces and packages as cards.
    fn shows(&self, _kind: &str, class: ClassId) -> bool {
        is_classifier(class) || class == PACKAGE
    }

    /// A package's diagram shows its own classes and, as outsiders, classes of other packages; and
    /// the packages it holds, each leading to its own diagram (AR-95).
    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let of = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
        match store.class_of(element).ok() {
            Some(c) if c == PACKAGE => of.is_some() && commands::package_of(store, element) == of,
            _ => of.is_some() && commands::package_of(store, element).is_some(),
        }
    }

    fn default_size(&self, class: ClassId) -> (f64, f64) {
        if class == PACKAGE { PACKAGE_SIZE } else { CLASS_SIZE }
    }

    /// Associations, generalizations and dependencies.
    fn connects(&self, _kind: &str, class: ClassId) -> bool {
        class == ASSOCIATION || class == GENERALIZATION || class == DEPENDENCY
    }

    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        figures::ends(store, element)
    }

    fn text_slots(&self, class: ClassId) -> &'static [&'static str] {
        figures::text_slots(class)
    }

    /// A class or an interface: its name, a class's slanted when abstract, its attributes, then its
    /// operations.
    fn card(&self, store: &Store, element: Obj, diagram: Obj) -> Option<Card> {
        figures::card(store, element, diagram)
    }

    fn line(&self, store: &Store, element: Obj) -> Option<Line> {
        figures::line(store, element)
    }

    fn slot_text(&self, store: &Store, element: Obj, slot: &str) -> String {
        figures::slot_text(store, element, slot)
    }

    /// A skill sits on a package, a class, an interface, an attribute or an operation; it aims at
    /// every class, interface, attribute, operation or association below its subject (owner
    /// decision: an interface carries skills as a class does).
    fn skill_subjects(&self) -> &'static [ClassId] {
        &[PACKAGE, CLASS, INTERFACE, PROPERTY, OPERATION]
    }

    /// The kinds a package skill may aim at, each with the word that names it.
    fn skill_kinds(&self) -> &'static [(ClassId, &'static str)] {
        &[(CLASS, "class"), (INTERFACE, "interface"), (PROPERTY, "attribute"), (OPERATION, "operation"), (ASSOCIATION, "association")]
    }

    /// An association's end is a property but no attribute: its association carries the skill.
    fn skill_problem(&self, store: &Store, element: Obj) -> Option<&'static str> {
        let end = store.class_of(element).ok() == Some(PROPERTY) && store.get(element, property::ASSOCIATION).ok().and_then(|v| v.as_ref()).is_some();
        end.then_some("an association's end carries no skill; its association's classes do")
    }

    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<usize, archforge_engine::CommandError> {
        commands::remove_elements(store, objs)
    }
}

/// A project's notations when it holds UML alone.
pub static ALONE: &[&dyn Notation] = &[&UmlNotation];

/// The words UML's figures say, for the shell's catalogue: the texts, and the plural.
pub const WORDS: &[&str] = &["Class", "Class · {package}", "«interface»", "«interface» · {package}", "Package"];
pub const PLURALS: &[&str] = &["+ {n} more member", "+ {n} more element"];

/// The command registry of UML alone: the edit area, the shared element commands, UML's, the
/// diagrams' and the skills'.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, ProjectView>>, String> {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend(archforge_diagram::project::specs());
    specs.extend(commands::specs());
    specs.extend(members::specs());
    specs.extend(relations::specs());
    specs.extend(archforge_diagram::commands::specs::<ProjectView>());
    specs.extend(archforge_core::skills::specs::<ProjectView>());
    archforge_engine::Registry::new(specs)
}
