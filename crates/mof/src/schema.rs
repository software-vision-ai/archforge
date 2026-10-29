//! The storage schema a metamodel's generated code describes, as data for the project crate: one
//! table per concrete metaclass, one junction table per multi-valued stored property. Nothing here
//! talks to a database.

use crate::meta::{ClassId, PropId};

/// A table with one row per object of a concrete metaclass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Table {
    pub name: &'static str,
    pub class: ClassId,
    pub columns: &'static [Column],
}

/// A column: the id, or one single-valued stored property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub name: &'static str,
    pub prop: Option<PropId>,
    /// The SQL type with its nullability, as the table is created.
    pub sql: &'static str,
}

/// A table with one row per value of a multi-valued stored property: owner, value and an order key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Junction {
    pub name: &'static str,
    pub class: ClassId,
    pub prop: PropId,
    pub ordered: bool,
    /// Whether the value column is a reference (BINARY(16)) or a data value.
    pub reference: bool,
}
