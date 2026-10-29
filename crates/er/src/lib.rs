//! The ER notation: a schema of entities with ordered attributes, and relationships with a
//! cardinality at each end, as a metamodel over the MOF core (`er.xmi`, generated into
//! `generated/` by `cargo xtask generate er`) with the rules and commands written here. The schema
//! is a member of the project's Model (the shared core); comments on ER elements are the core's.
//! Pure: builds for the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod commands;
mod generated;
pub mod rules;

use std::sync::Arc;

use archforge_mof::{CompositeSlabs, MetaError, Metamodel, Obj, Store, Universe, Uuid};

pub use commands::{NO_ENTITIES, NO_SCHEMAS};
pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::ErSlabs;

/// The view: the selection, which undo never touches. The open diagram comes with the diagram
/// metamodel (gm2.29).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ErView {
    pub selection: Vec<Obj>,
}

impl archforge_engine::View<Store> for ErView {
    fn retain_valid(&mut self, doc: &Store) {
        self.selection.retain(|o| doc.is_alive(*o));
    }
}

/// The ER notation's session: the store of the project, the selection, the ER commands.
pub type Session = archforge_engine::Session<Store, ErView>;

/// The ER notation's command registry: the edit area and the ER commands.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, ErView>>, String> {
    archforge_engine::Registry::new(commands::specs())
}

/// A universe of the shared core, ER and the metamodels given.
pub fn universe_with(others: Vec<Arc<Metamodel>>) -> Result<Arc<Universe>, MetaError> {
    let mut all = vec![metamodel()?];
    all.extend(others);
    archforge_core::universe_with(all)
}

/// The slabs of the shared core and ER, for a store over [`universe_with`] with no other
/// metamodel; more slabs join through [`CompositeSlabs::with`].
pub fn slabs() -> Result<CompositeSlabs, String> {
    CompositeSlabs::new().with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))?.with(INDEX, Box::new(ErSlabs::default()))
}

/// A store over the shared core and ER alone, with the host's ids.
pub fn store(ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Store, String> {
    let universe = universe_with(Vec::new()).map_err(|e| e.to_string())?;
    Ok(Store::new(universe, Box::new(slabs()?), ids))
}
