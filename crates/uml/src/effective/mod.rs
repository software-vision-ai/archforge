//! Effective results (Decision 13): a receiving package's own, imported and merged contents as
//! read-only records apart from the model, keyed by [`LocalKey`] within one [`ResultContext`].
//! Display paths are values from the same snapshot, never identities, so the interface and agents
//! never read one from another revision. Provenance is a graph of contributions and route edges.

pub mod keys;

use std::{collections::HashSet, iter::successors, sync::Arc};

use archforge_mof::Uuid;

pub use keys::{Inspected, LocalKey, ResultContext, ResultKey, Role};

/// Whether the whole package was resolved; an incomplete result never passes for the whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completeness {
    Whole,
    Incomplete,
}

/// What the rule checks found for the result: not checked yet, no violations in the checked
/// scope, not fully verified (a check could not be finished or evaluated), or violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NotChecked,
    NoViolations,
    NotFullyVerified,
    Violations,
}

/// Where a result comes from. Not exclusive: a merged member can also be local or imported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Origins {
    pub local: bool,
    pub imported: bool,
    pub merged: bool,
}

/// A result's context, completeness and status, and the receiving package's display path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub context: ResultContext,
    pub completeness: Completeness,
    pub status: Status,
    pub path: Box<str>,
}

/// An element of the receiving package, or a member of one (its key's parent), with the display
/// path of what owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub key: Arc<LocalKey>,
    pub name: Box<str>,
    pub path: Box<str>,
    pub origins: Origins,
}

/// A relationship between results, its ends given as their keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub key: Arc<LocalKey>,
    pub ends: Box<[Arc<LocalKey>]>,
    pub origins: Origins,
}

/// An authored source: its UUID, package and revision, the import or merge it enters the receiving
/// package by (none for the package's own), and its display path from the same snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub source: Uuid,
    pub package: Uuid,
    pub inspected: Inspected,
    pub route: Option<Uuid>,
    pub path: Box<str>,
}

/// One source's contribution to one result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    pub result: Arc<LocalKey>,
    pub source: SourceRef,
}

/// The kind of relationship a route edge follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    Import,
    Merge,
}

/// An authored import or merge relationship, from the importing or receiving package to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteEdge {
    pub relationship: Uuid,
    pub kind: RouteKind,
    pub from: Uuid,
    pub to: Uuid,
}

/// Why each result exists: what contributes to it, and the edges the routes follow.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provenance {
    pub contributions: Vec<Contribution>,
    pub routes: Vec<RouteEdge>,
}

/// A finding about the result: none yet. 398-D2 adds import findings, counted in the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {}

/// One package's effective contents at one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveResult {
    pub header: Header,
    pub elements: Vec<Record>,
    pub members: Vec<Record>,
    pub relationships: Vec<Relationship>,
    pub provenance: Provenance,
    pub findings: Vec<Finding>,
}

impl EffectiveResult {
    /// Every result's key: the elements', the members' and the relationships'.
    pub fn keys(&self) -> impl Iterator<Item = &Arc<LocalKey>> {
        let elements = self.elements.iter().map(|e| &e.key);
        elements.chain(self.members.iter().map(|m| &m.key)).chain(self.relationships.iter().map(|r| &r.key))
    }

    /// The bytes the result holds, for its cache's and its cap's account: every record, string and
    /// nested collection, an inspected version's commit wherever it is named, and each key allocation
    /// once by its address, however many record keys, ends, contribution results and parents share it.
    pub fn accounted_bytes(&self) -> usize {
        let commit = |i: &Inspected| match i {
            Inspected::Working { .. } => 0,
            Inspected::Version { commit } => commit.len(),
        };
        let records = self.elements.iter().chain(&self.members);
        let records = records.map(|r| sum(&[size_of::<Record>(), r.name.len(), r.path.len()]));
        let relationships = self.relationships.iter().map(|r| sum(&[size_of::<Relationship>(), size_of_val(&*r.ends)]));
        let sources =
            self.provenance.contributions.iter().map(|c| sum(&[size_of::<Contribution>(), commit(&c.source.inspected), c.source.path.len()]));
        let header = sum(&[size_of::<Self>(), commit(&self.header.context.inspected), self.header.path.len()]);
        let routes = self.provenance.routes.len().saturating_mul(size_of::<RouteEdge>());
        // A walk up from a key stops at the first allocation already counted, whose parents are too.
        let (mut seen, mut keys, ends) = (HashSet::new(), 0usize, self.relationships.iter().flat_map(|r| &r.ends));
        for key in self.keys().chain(ends).chain(self.provenance.contributions.iter().map(|c| &c.result)) {
            let fresh = successors(Some(key), |&k| k.parent()).take_while(|k| seen.insert(Arc::as_ptr(*k)));
            keys = fresh.fold(keys, |total, k| total.saturating_add(k.accounted_bytes()));
        }
        records.chain(relationships).chain(sources).fold(sum(&[header, routes, keys]), usize::saturating_add)
    }
}

/// The sum of `parts`, at most `usize::MAX`.
fn sum(parts: &[usize]) -> usize {
    parts.iter().fold(0, |total, part| total.saturating_add(*part))
}

/// The one result in `new` with the same role, metatype, parent chain and sources as `old`, from
/// an earlier result, which keeps a selection across revisions. None when there is no such result,
/// or more than one: a selection is only kept when it cannot be mistaken.
pub fn counterpart<'a>(old: &LocalKey, new: &'a EffectiveResult) -> Option<&'a Arc<LocalKey>> {
    let mut matching = new.keys().filter(|key| ***key == *old);
    let found = matching.next()?;
    matching.next().is_none().then_some(found)
}
