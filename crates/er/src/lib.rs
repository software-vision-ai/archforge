//! The ER notation: a schema of entities with ordered attributes, relationships with a
//! cardinality at each end, primary keys, unique constraints, foreign keys and mapping tables, as a metamodel over the MOF core (`er.xmi`, generated into
//! `generated/` by `cargo xtask generate er`) with the rules and commands written here. The schema
//! is a member of the project's Model (the shared core); comments on ER elements are the core's;
//! its diagrams are the diagram metamodel's, with the ER policy in [`ErNotation`]. Pure: builds for
//! the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod commands;
mod generated;
pub mod rules;

use std::sync::Arc;

use archforge_diagram::{Notation, ProjectView};
use archforge_mof::{ClassId, CompositeSlabs, MetaError, Metamodel, Obj, PropId, Store, Universe, Uuid};

pub use commands::{NO_ENTITIES, NO_SCHEMAS};
pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::ErSlabs;

/// The ER diagram kind: a diagram of a schema that shows entities as nodes and relationships as
/// edges.
pub const DIAGRAM_KIND: &str = "er";

/// An entity node's width and height when the user names none.
pub const ENTITY_SIZE: (f64, f64) = (160.0, 100.0);

/// The ER notation in a project's view: its diagrams (gm2.29), where skills sit (spec
/// archforge-zer.2), and how its elements are removed and described.
pub struct ErNotation;

/// A project's notations when it holds ER alone.
pub static ALONE: &[&dyn Notation] = &[&ErNotation];

impl Notation for ErNotation {
    fn name(&self) -> &'static str {
        "ER"
    }

    fn metamodel(&self) -> u16 {
        INDEX
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[DIAGRAM_KIND]
    }

    fn is_of(&self, _kind: &str, class: ClassId) -> bool {
        class == SCHEMA
    }

    /// An ER diagram is always of a schema.
    fn needs_element(&self, _kind: &str) -> bool {
        true
    }

    fn shows(&self, _kind: &str, class: ClassId) -> bool {
        class == ENTITY
    }

    /// A schema's diagram shows any table of the model (slice S7: one of another schema draws
    /// dimmed, with its schema named).
    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let of = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
        of.is_some_and(|s| store.class_of(s).ok() == Some(SCHEMA)) && rules::schema_of(store, element).is_some()
    }

    /// Relationships, and generalizations (slice S6) drawn from the subtype to the supertype.
    fn connects(&self, _kind: &str, class: ClassId) -> bool {
        class == RELATIONSHIP || class == GENERALIZATION
    }

    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        let end = |p| store.get(element, p).ok().and_then(|v| v.as_ref());
        match store.class_of(element).ok() {
            Some(c) if c == RELATIONSHIP => Some((end(relationship::FROM)?, end(relationship::TO)?)),
            Some(c) if c == GENERALIZATION => Some((end(generalization::SUBTYPE)?, end(generalization::SUPERTYPE)?)),
            _ => None,
        }
    }

    fn text_slots(&self, class: ClassId) -> &'static [&'static str] {
        if class == RELATIONSHIP { &["name", "from_end", "to_end", "from_role", "to_role"] } else { &[] }
    }

    fn default_size(&self, _class: ClassId) -> (f64, f64) {
        ENTITY_SIZE
    }

    /// A skill sits on a schema, an entity or an attribute; it aims at every table, every
    /// relationship or every column below its subject, and a column skill may name a type.
    fn skill_subjects(&self) -> &'static [ClassId] {
        &[SCHEMA, ENTITY, ATTRIBUTE]
    }

    fn skill_kinds(&self) -> &'static [(ClassId, &'static str)] {
        &[(ENTITY, "table"), (RELATIONSHIP, "relationship"), (ATTRIBUTE, "column")]
    }

    fn typed_class(&self) -> Option<(ClassId, PropId)> {
        Some((ATTRIBUTE, attribute::TYPE))
    }

    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<usize, archforge_engine::CommandError> {
        commands::remove_elements(store, objs)
    }

    fn describe(&self, store: &mut Store, obj: Obj, text: &str) -> Result<(), archforge_engine::CommandError> {
        commands::describe_element(store, obj, text)
    }
}

/// A session over a store of ER, the selection, the commands.
pub type Session = archforge_engine::Session<Store, ProjectView>;

/// The command registry of ER alone: the edit area, the shared element commands, the ER commands,
/// the diagram commands and the skill commands.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, ProjectView>>, String> {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend(archforge_diagram::project::specs());
    specs.extend(commands::specs());
    specs.extend(archforge_diagram::commands::specs::<ProjectView>());
    specs.extend(archforge_core::skills::specs::<ProjectView>());
    archforge_engine::Registry::new(specs)
}

/// The id counter a new session starts at: ids below it are the store's own.
const FIRST_COUNTER: u64 = 1;

/// A session over the shared core, the diagram metamodel and ER alone, with a Model root named
/// `name`: for ER's own tests and tools. A project's session holds every notation and is made by
/// `archforge-notations`. `ids` mints the elements' ids.
pub fn session_alone(name: &str, ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Session, String> {
    let registry = registry()?;
    let mut st = store(ids)?;
    let model = st.create(archforge_core::MODEL).map_err(|e| e.to_string())?;
    st.set(model, archforge_core::named_element::NAME, archforge_mof::Value::str(name)).map_err(|e| e.to_string())?;
    Ok(Session::with_view(registry, st, FIRST_COUNTER, archforge_engine::Options::default(), ProjectView::new(ALONE)))
}

/// The tables of the shared core, the diagram metamodel and ER, in that order: what a store of
/// [`store`] is written to.
pub fn storage() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    let tables = archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(TABLES).copied().collect();
    let junctions = archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(JUNCTIONS).copied().collect();
    (tables, junctions)
}

/// A universe of the shared core, the diagram metamodel, ER and the metamodels given.
pub fn universe_with(others: Vec<Arc<Metamodel>>) -> Result<Arc<Universe>, MetaError> {
    let mut all = vec![metamodel()?, archforge_diagram::metamodel()?];
    all.extend(others);
    archforge_core::universe_with(all)
}

/// The slabs of the shared core, the diagram metamodel and ER, for a store over [`universe_with`]
/// with no other metamodel; more slabs join through [`CompositeSlabs::with`].
pub fn slabs() -> Result<CompositeSlabs, String> {
    let core = CompositeSlabs::new().with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))?;
    archforge_diagram::with_slabs(core)?.with(INDEX, Box::new(ErSlabs::default()))
}

/// A store over the shared core, the diagram metamodel and ER alone, with the host's ids.
pub fn store(ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Store, String> {
    let universe = universe_with(Vec::new()).map_err(|e| e.to_string())?;
    Ok(Store::new(universe, Box::new(slabs()?), ids))
}
