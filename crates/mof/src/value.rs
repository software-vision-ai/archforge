//! A property's value, and the kinds of value a data type takes.

use std::fmt;
use std::sync::Arc;

use crate::store::Obj;

/// The most characters a string value may hold (CMOF constraint 14.4 [4]).
pub const MAX_STRING: usize = 65_535;

/// One value of a property: a data value, a reference, or nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    /// Always finite.
    Real(f64),
    Str(Arc<str>),
    /// None is `*`.
    Unlimited(Option<u64>),
    /// The index of a literal in its enumeration.
    Enum(u32),
    Ref(Obj),
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Arc::from(s))
    }

    pub fn as_ref(&self) -> Option<Obj> {
        if let Value::Ref(o) = self { Some(*o) } else { None }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self { Some(s) } else { None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(b) = self { Some(*b) } else { None }
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(i) = self { Some(*i) } else { None }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// What the value is, for error messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Null => "nothing",
            Value::Bool(_) => "a boolean",
            Value::Int(_) => "an integer",
            Value::Real(_) => "a real",
            Value::Str(_) => "a string",
            Value::Unlimited(_) => "an unlimited natural",
            Value::Enum(_) => "an enumeration literal",
            Value::Ref(_) => "a reference",
        }
    }

    /// An estimate of the bytes the value holds beyond its own size.
    pub fn heap_bytes(&self) -> usize {
        match self {
            Value::Str(s) => s.len(),
            _ => 0,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Real(r) => write!(f, "{r}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Unlimited(None) => write!(f, "*"),
            Value::Unlimited(Some(n)) => write!(f, "{n}"),
            Value::Enum(i) => write!(f, "literal {i}"),
            Value::Ref(o) => write!(f, "{o}"),
        }
    }
}
