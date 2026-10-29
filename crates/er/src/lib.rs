//! The ER notation: entities with their attributes, relationships with a cardinality at each end,
//! and diagrams that place entities, as a document, a view and commands for the engine. Pure: it
//! builds for the portable target, and the project crate stores it.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod commands;
pub mod model;

use std::sync::Arc;

pub use commands::{NO_DIAGRAMS, NO_ENTITIES};
pub use model::{Attribute, Cardinality, Diagram, Entity, ErModel, ErView, Id, Invalid, Kind, Place, Relationship};

/// The ER notation's session.
/// The ER model records its changes by copying, until it moves onto the MOF core.
pub type Doc = archforge_engine::Snapshotted<ErModel>;
pub type Session = archforge_engine::Session<Doc, ErView>;

/// The ER notation's command registry: the edit area and the ER commands.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Doc, ErView>>, String> {
    archforge_engine::Registry::new(commands::specs())
}
