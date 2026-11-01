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

/// A class card's width and height when the user names none: the header, an attribute row, an
/// operation row and the line between them.
pub const CLASS_SIZE: (f64, f64) = (240.0, 120.0);

/// UML in a project's view (archforge-3c5.1.1, 3c5.1.2): class diagrams of a package showing its
/// classes as plain cards with their attributes and operations, and the associations,
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

    fn shows(&self, _kind: &str, class: ClassId) -> bool {
        class == CLASS
    }

    /// A package's diagram shows the package's own classes.
    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let of = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
        of.is_some() && commands::package_of(store, element) == of
    }

    fn default_size(&self, _class: ClassId) -> (f64, f64) {
        CLASS_SIZE
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

    /// A class: its name, slanted when abstract, its attributes, then its operations.
    fn card(&self, store: &Store, element: Obj, _diagram: Obj) -> Option<Card> {
        figures::card(store, element)
    }

    fn line(&self, store: &Store, element: Obj) -> Option<Line> {
        figures::line(store, element)
    }

    fn slot_text(&self, store: &Store, element: Obj, slot: &str) -> String {
        figures::slot_text(store, element, slot)
    }

    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<usize, archforge_engine::CommandError> {
        commands::remove_elements(store, objs)
    }
}

/// A project's notations when it holds UML alone.
pub static ALONE: &[&dyn Notation] = &[&UmlNotation];

/// The words UML's figures say, for the shell's catalogue: the texts, and the plural.
pub const WORDS: &[&str] = &["Class"];
pub const PLURALS: &[&str] = &["+ {n} more member"];

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
