//! The UML 2.5.1 metamodel as a metamodel of the MOF core: 242 metaclasses, their properties,
//! enumerations and the five primitive types, generated from OMG's UML.xmi by `cargo xtask
//! generate uml`. Nothing here is written by hand except this file: the generated code under
//! `generated/` is what the XMI gives, checked by the gate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod generated;

use std::sync::Arc;

use archforge_mof::{MetaError, Universe};

pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::UmlSlabs;

/// A universe of UML alone.
pub fn universe() -> Result<Arc<Universe>, MetaError> {
    Ok(Arc::new(Universe::new(vec![metamodel()?])?))
}
