//! The ER notation: a schema of entities with ordered attributes, relationships with a
//! cardinality at each end, primary keys, unique constraints, foreign keys and mapping tables, as a metamodel over the MOF core (`er.xmi`, generated into
//! `generated/` by `cargo xtask generate er`) with the rules and commands written here. The schema
//! is a member of the project's Model (the shared core); comments on ER elements are the core's;
//! its diagrams are the diagram metamodel's, with the ER policy in [`ErView`]. Pure: builds for
//! the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod commands;
mod generated;
pub mod rules;

use std::sync::Arc;

use archforge_diagram::DiagramView;
use archforge_mof::{ClassId, CompositeSlabs, MetaError, Metamodel, Obj, PropId, Store, Universe, Uuid};

pub use commands::{NO_ENTITIES, NO_SCHEMAS};
pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::ErSlabs;

/// The view: the selection and the open diagram, which undo never touches.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ErView {
    pub selection: Vec<Obj>,
    pub open_diagram: Option<Obj>,
}

impl archforge_engine::View<Store> for ErView {
    fn retain_valid(&mut self, doc: &Store) {
        self.selection.retain(|o| doc.is_alive(*o));
        if self.open_diagram.is_some_and(|d| !doc.is_alive(d)) {
            self.open_diagram = None;
        }
    }
}

/// The ER diagram kind: a diagram of a schema that shows entities as nodes and relationships as
/// edges.
pub const DIAGRAM_KIND: &str = "er";

/// An entity node's width and height when the user names none.
pub const ENTITY_SIZE: (f64, f64) = (160.0, 100.0);

/// The ER policy for the diagram commands (gm2.29).
impl DiagramView for ErView {
    fn open_diagram(&self) -> Option<Obj> {
        self.open_diagram
    }

    fn set_open_diagram(&mut self, diagram: Option<Obj>) {
        self.open_diagram = diagram;
    }

    fn selection_mut(&mut self) -> &mut Vec<Obj> {
        &mut self.selection
    }

    fn kinds() -> &'static [&'static str] {
        &[DIAGRAM_KIND]
    }

    /// Diagrams are members of the project's Model, beside the schemas.
    fn home(store: &Store) -> Result<Option<(Obj, PropId)>, String> {
        store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_core::MODEL))
            .map(|m| Some((m, archforge_core::model::MEMBER)))
            .ok_or_else(|| "the project has no model root".to_string())
    }

    fn is_of(_kind: &str, class: ClassId) -> bool {
        class == SCHEMA
    }

    /// An ER diagram is always of a schema.
    fn needs_element(_kind: &str) -> bool {
        true
    }

    fn shows(_kind: &str, class: ClassId) -> bool {
        class == ENTITY
    }

    /// A schema's diagram shows any table of the model (slice S7: one of another schema draws
    /// dimmed, with its schema named).
    fn may_show(store: &Store, diagram: Obj, element: Obj) -> bool {
        let of = store.get(diagram, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
        of.is_some_and(|s| store.class_of(s).ok() == Some(SCHEMA)) && rules::schema_of(store, element).is_some()
    }

    /// Relationships, and generalizations (slice S6) drawn from the subtype to the supertype.
    fn connects(_kind: &str, class: ClassId) -> bool {
        class == RELATIONSHIP || class == GENERALIZATION
    }

    fn ends(store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        let end = |p| store.get(element, p).ok().and_then(|v| v.as_ref());
        match store.class_of(element).ok() {
            Some(c) if c == RELATIONSHIP => Some((end(relationship::FROM)?, end(relationship::TO)?)),
            Some(c) if c == GENERALIZATION => Some((end(generalization::SUBTYPE)?, end(generalization::SUPERTYPE)?)),
            _ => None,
        }
    }

    fn text_slots(class: ClassId) -> &'static [&'static str] {
        if class == RELATIONSHIP { &["name", "from_end", "to_end", "from_role", "to_role"] } else { &[] }
    }

    fn default_size(_class: ClassId) -> (f64, f64) {
        ENTITY_SIZE
    }
}

/// What may carry a skill in ER and what a skill may aim at (spec archforge-zer.2): a skill sits
/// on a schema, an entity or an attribute; it aims at every table, every relationship or every
/// column below its subject, and a column skill may name a type.
impl archforge_core::skills::SkillHost for ErView {
    fn selection_mut(&mut self) -> &mut Vec<Obj> {
        &mut self.selection
    }

    fn skill_subjects() -> &'static [ClassId] {
        &[SCHEMA, ENTITY, ATTRIBUTE]
    }

    fn skill_kinds() -> &'static [(ClassId, &'static str)] {
        &[(ENTITY, "table"), (RELATIONSHIP, "relationship"), (ATTRIBUTE, "column")]
    }

    fn typed_class() -> Option<(ClassId, PropId)> {
        Some((ATTRIBUTE, attribute::TYPE))
    }
}

/// The ER notation's session: the store of the project, the selection, the ER commands.
pub type Session = archforge_engine::Session<Store, ErView>;

/// The ER notation's command registry: the edit area, the ER commands, the diagram commands and
/// the skill commands.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, ErView>>, String> {
    let mut specs = commands::specs();
    specs.extend(archforge_diagram::commands::specs::<ErView>());
    specs.extend(archforge_core::skills::specs::<ErView>());
    archforge_engine::Registry::new(specs)
}

/// The id counter a new project's session starts at: ids below it are the store's own.
const FIRST_COUNTER: u64 = 1;

/// The session of a project that has nothing yet: a store over the shared core, the diagram
/// metamodel and ER, the project's Model root named `name`, and the ER registry. `ids` mints the
/// elements' ids; the host supplies it, since only the host may read the clock.
pub fn new_project_session(name: &str, ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Session, String> {
    let registry = registry()?;
    let mut st = store(ids)?;
    let model = st.create(archforge_core::MODEL).map_err(|e| e.to_string())?;
    st.set(model, archforge_core::named_element::NAME, archforge_mof::Value::str(name)).map_err(|e| e.to_string())?;
    Ok(Session::new(registry, st, FIRST_COUNTER, archforge_engine::Options::default()))
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
