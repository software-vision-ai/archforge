//! The keys of effective results, built from authored source UUIDs, never from display names, an
//! order or a bare hash. [`LocalKey`] is the one structural key within a result, which merge
//! results reuse (AR-615); [`ResultKey`] adds the context. Neither converts to a `Uuid` or `Obj`.

use std::sync::Arc;

use archforge_mof::{ClassId, Uuid};

/// The revision a result was resolved from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Inspected {
    /// A point in one session's undo history (`Session::token()`). A number that recurs after Undo
    /// gives the same key again, so working results never outlive a change of the token.
    Working { history: u64, revision: u64 },
    /// A saved version, by its commit.
    Version { commit: Arc<str> },
}

/// Where a result was resolved: the project (its core Model root's UUID), the revision and the
/// receiving package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResultContext {
    pub project: Uuid,
    pub inspected: Inspected,
    pub package: Uuid,
}

/// Which occurrence a result is: one authored source can be an element, a member and a
/// relationship's end at once, and each is its own result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// An element of the receiving package: a classifier or a package.
    Element,
    /// A member of an effective element: an attribute, an operation or a literal.
    Member,
    /// A relationship between results.
    Relationship,
    /// An end of a relationship.
    End,
}

/// A result's structural key within its context: its role, metatype, the key of the result that
/// holds it, and the sorted, unique UUIDs of the authored sources that contribute to it. Equality,
/// order and hash take the whole tuple, parent chain included, so a hash table that hashes two
/// keys alike still tells them apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalKey {
    role: Role,
    metatype: ClassId,
    parent: Option<Arc<LocalKey>>,
    sources: Box<[Uuid]>,
}

impl LocalKey {
    /// The key of a result with these sources, in any order and with repeats: they are sorted and
    /// each kept once, so the order a resolver met them in never changes the key.
    pub fn new(role: Role, metatype: ClassId, parent: Option<Arc<LocalKey>>, sources: impl IntoIterator<Item = Uuid>) -> LocalKey {
        let mut sources: Vec<Uuid> = sources.into_iter().collect();
        sources.sort_unstable();
        sources.dedup();
        LocalKey { role, metatype, parent, sources: sources.into_boxed_slice() }
    }

    /// The key of the result that holds this one; none for an element of the receiving package.
    pub fn parent(&self) -> Option<&Arc<LocalKey>> {
        self.parent.as_ref()
    }

    /// The authored sources that contribute to the result, sorted and each once.
    pub fn sources(&self) -> &[Uuid] {
        &self.sources
    }

    /// The bytes this key adds to a result: its record and sources; its parent counts as its own.
    pub fn accounted_bytes(&self) -> usize {
        size_of::<LocalKey>().saturating_add(self.sources.len().saturating_mul(size_of::<Uuid>()))
    }
}

/// A result's key with the context it was resolved in. It is not a model element's id: there is
/// no conversion to a `Uuid` or an `Obj`.
///
/// ```
/// fn local(key: archforge_uml::effective::ResultKey) -> std::sync::Arc<archforge_uml::effective::LocalKey> { key.local }
/// ```
///
/// ```compile_fail
/// fn id(key: archforge_uml::effective::ResultKey) -> archforge_mof::Uuid { key.into() } // no model element's id
/// ```
///
/// ```compile_fail
/// fn handle(key: archforge_uml::effective::ResultKey) -> archforge_mof::Obj { key.into() } // nor a handle to one
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResultKey {
    pub context: ResultContext,
    pub local: Arc<LocalKey>,
}
