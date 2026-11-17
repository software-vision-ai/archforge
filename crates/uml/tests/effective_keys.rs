//! Effective result keys and records (AR-642): the whole tuple decides, even when every hash
//! collides; source order, repeats and display paths do not; a counterpart is the one match or
//! none; accounted bytes count every string and collection.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

use archforge_mof::Uuid;
use archforge_uml::effective::{
    Completeness, Contribution, EffectiveResult, Header, Inspected, LocalKey, Origins, Provenance, Record, Relationship, ResultContext, ResultKey,
    Role, RouteEdge, RouteKind, SourceRef, Status, counterpart,
};
use archforge_uml::{CLASS, OPERATION, PROPERTY};

/// A UUID whose bytes are all `n`.
fn id(n: u8) -> Uuid {
    Uuid([n; 16])
}

/// A class element's key with these sources.
fn class(sources: &[u8]) -> Arc<LocalKey> {
    Arc::new(LocalKey::new(Role::Element, CLASS, None, sources.iter().map(|n| id(*n))))
}

/// A hasher that gives every key the same hash.
#[derive(Default)]
struct Collide;

impl Hasher for Collide {
    /// The same hash for every key.
    fn finish(&self) -> u64 {
        7
    }
    /// Ignores what a key writes.
    fn write(&mut self, _: &[u8]) {}
}

/// A working-revision context of package 2 in project 1.
fn working(history: u64, revision: u64) -> ResultContext {
    ResultContext { project: id(1), inspected: Inspected::Working { history, revision }, package: id(2) }
}

/// A result holding these elements, each named "Order" and owned at the given display path.
fn result(elements: &[(&Arc<LocalKey>, &str)]) -> EffectiveResult {
    let origins = Origins { local: true, ..Origins::default() };
    let element = |(key, path): &(&Arc<LocalKey>, &str)| Record { key: Arc::clone(key), name: "Order".into(), path: (*path).into(), origins };
    EffectiveResult {
        header: Header { context: working(1, 1), completeness: Completeness::Whole, status: Status::NotChecked, path: "Sales".into() },
        elements: elements.iter().map(element).collect(),
        members: Vec::new(),
        relationships: Vec::new(),
        provenance: Provenance::default(),
        findings: Vec::new(),
    }
}

/// Each part of a key alone tells it apart in a table whose hashes all collide, and an equal key
/// built again, or with its sources shuffled and repeated, finds its entry.
#[test]
fn the_whole_tuple_decides_even_when_every_hash_collides() {
    let member = |role, metatype, parent: Option<&Arc<LocalKey>>, sources: &[u8]| {
        LocalKey::new(role, metatype, parent.cloned(), sources.iter().map(|n| id(*n)))
    };
    let (order, invoice) = (class(&[10]), class(&[11]));
    let keys = [
        member(Role::Member, PROPERTY, Some(&order), &[20]),
        member(Role::End, PROPERTY, Some(&order), &[20]),
        member(Role::Member, OPERATION, Some(&order), &[20]),
        member(Role::Member, PROPERTY, Some(&invoice), &[20]),
        member(Role::Member, PROPERTY, None, &[20]),
        member(Role::Member, PROPERTY, Some(&order), &[21]),
        member(Role::Member, PROPERTY, Some(&order), &[20, 21]),
    ];
    let mut table: HashMap<LocalKey, usize, BuildHasherDefault<Collide>> = HashMap::default();
    for (at, key) in keys.iter().enumerate() {
        assert_eq!(table.insert(key.clone(), at), None, "key {at} was taken for another");
    }
    assert!(keys.iter().enumerate().all(|(at, key)| table.get(key) == Some(&at)));
    assert_eq!(table.get(&member(Role::Member, PROPERTY, Some(&class(&[10])), &[20])), Some(&0), "an equal key built again");
    let shuffled = member(Role::Member, PROPERTY, Some(&order), &[21, 20, 21, 20]);
    assert_eq!((&shuffled, shuffled.sources()), (&keys[6], &[id(20), id(21)][..]), "source order and repeats leave the key alone");
}

/// The project, the revision (its history included) and the package each decide a result key.
#[test]
fn the_context_is_part_of_a_result_key() {
    let key = |context| ResultKey { context, local: class(&[10]) };
    assert_eq!(key(working(1, 5)), key(working(1, 5)));
    assert_ne!(key(working(1, 5)), key(working(2, 5)), "a revision number from another history is another revision");
    assert_ne!(key(working(1, 5)), key(working(1, 6)));
    assert_ne!(key(working(1, 5)), key(ResultContext { package: id(3), ..working(1, 5) }));
    let version = |commit: &str| ResultContext { inspected: Inspected::Version { commit: commit.into() }, ..working(1, 5) };
    assert_ne!(key(version("abc")), key(version("abd")));
}

/// A counterpart among elements, members and relationships is the one match, whatever its display
/// path, and never a guess.
#[test]
fn a_counterpart_is_the_one_match_or_none() {
    let (order, invoice) = (class(&[10]), class(&[11]));
    let moved = result(&[(&invoice, "Sales"), (&class(&[10]), "Commerce::Sales")]);
    assert_eq!(counterpart(&order, &moved), Some(&moved.elements[1].key), "display paths take no part in the key");
    assert_eq!(counterpart(&order, &result(&[(&invoice, "Sales")])), None, "no result has its sources");
    assert_eq!(counterpart(&order, &result(&[(&class(&[10, 12]), "Sales")])), None, "a grown source set is another result");
    assert_eq!(counterpart(&order, &result(&[(&order, "Sales"), (&class(&[10]), "Shared")])), None, "two matches");
    let (mut held, line) = (result(&[(&invoice, "Sales")]), class(&[13]));
    held.members.push(Record { key: Arc::clone(&order), ..held.elements[0].clone() });
    held.relationships.push(Relationship { key: Arc::clone(&line), ends: Box::new([]), origins: Origins::default() });
    assert_eq!((counterpart(&order, &held), counterpart(&line, &held)), (Some(&order), Some(&line)), "members and relationships");
}

/// Every string, source, record, end and route edge adds its bytes to the account.
#[test]
fn accounted_bytes_grow_with_every_string() {
    let base = result(&[(&class(&[10]), "Sales")]);
    let mut longer = base.clone();
    longer.elements[0].path = "Commerce::Sales".into();
    longer.header.path = "Commerce::Sales".into();
    longer.elements[0].name = "Order line".into();
    longer.elements[0].key = class(&[10, 11, 12]);
    longer.members.push(base.elements[0].clone());
    let commit = || Inspected::Version { commit: "abcd".into() };
    longer.header.context.inspected = commit();
    let source = SourceRef { source: id(10), package: id(2), inspected: commit(), route: None, path: "Sales::Order".into() };
    longer.provenance.contributions.push(Contribution { result: class(&[10]), source });
    longer.relationships.push(Relationship { key: class(&[13]), ends: Box::new([class(&[10])]), origins: Origins::default() });
    longer.provenance.routes.push(RouteEdge { relationship: id(14), kind: RouteKind::Import, from: id(2), to: id(3) });
    let (strings, sources) = (10 + 10 + 5 + 4 + 4 + 12, 2 * size_of::<Uuid>());
    let records = size_of::<Record>() + class(&[10]).accounted_bytes() + 5 + 5 + size_of::<Contribution>() + size_of::<RouteEdge>();
    let records = records + size_of::<Relationship>() + class(&[13]).accounted_bytes() + size_of::<Arc<LocalKey>>();
    assert_eq!(longer.accounted_bytes(), base.accounted_bytes() + strings + sources + records);
}
