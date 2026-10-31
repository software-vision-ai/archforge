//! The MOF core: the meta-metamodel a metamodel is described in, the store that holds objects of
//! any metamodel in one id space, change sets with exact inverses for undo, the semantics the
//! store enforces on every change (types, multiplicities, containment, opposites, subsets), the
//! indexes it keeps (who refers to whom, names), and the reflective access generic tools use.
//!
//! Generated code (one crate per metamodel) brings the typed slabs and the `Reflect` for each
//! metaclass; this crate knows no metamodel of its own. Pure: it builds for the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod id;
pub mod meta;
pub mod rows;
pub mod schema;
pub mod store;
#[cfg(feature = "testing")]
pub mod toy;
mod value;

pub use id::Uuid;
pub use meta::{
    Builder, Class, ClassId, ClassInfo, DataId, DataKind, DataType, MetaError, Metamodel, PropId, Property, PropertySpec, TypeRef, Universe,
};
pub use store::{CompositeSlabs, Delta, Invalid, Obj, Op, Problem, Reflect, Slabs, Store};
pub use value::{MAX_STRING, Value};
