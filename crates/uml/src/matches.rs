//! The relationships of the model that a new one between the same two elements would repeat
//! (AR-699): drawing a relationship again reuses the one there is rather than adding a parallel one.
//!
//! A relationship repeats another when it is of exactly the same metaclass and joins the same two
//! elements in the same order, by their identities, never by a name: the pair the other way round
//! is another relationship, and a realization or a usage, a package import or a merge, is never an
//! ordinary dependency, though its ends are the same. Reusing a repeat is this product's rule, not
//! UML's, which allows parallel relationships.
//!
//! The matching is shared by every kind a gesture can draw again: a dependency's here, and the
//! package import's and merge's as their commands arrive, each candidate with its own `detail`.

use archforge_engine::CommandError;
use archforge_mof::{Obj, Store, Value};
use serde_json::{Map, Value as Json, json};

use crate::commands::{name_of, uuid_of};
use crate::{DEPENDENCY, dependency};

/// The kind of relationship a repeat is matched for; a kind is matched only by exactly its own
/// metaclass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An ordinary dependency, client to supplier: exactly the metaclass Dependency, of one client
    /// and one supplier.
    Dependency,
}

impl Kind {
    /// The relationship's two ends, from and to, when `r` is one of this kind that joins exactly two
    /// live elements; none for another metaclass, a specialised one included, or for one of several
    /// clients or suppliers, which joins no single pair.
    pub fn ends(self, store: &Store, r: Obj) -> Option<(Obj, Obj)> {
        match self {
            Kind::Dependency => {
                if store.class_of(r).ok() != Some(DEPENDENCY) {
                    return None;
                }
                Some((only_live(store, r, dependency::CLIENT)?, only_live(store, r, dependency::SUPPLIER)?))
            }
        }
    }

    /// What else tells a candidate of this kind apart, beside its name and ends: nothing for a
    /// dependency.
    fn detail(self, _store: &Store, _r: Obj) -> Map<String, Json> {
        match self {
            Kind::Dependency => Map::new(),
        }
    }
}

/// The one live element `o` lists under `prop`; none when it lists no live one or more than one,
/// read only as far as the second.
fn only_live(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    let values = store.list(o, prop).ok()?;
    let mut live = values.iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x));
    match (live.next(), live.next()) {
        (Some(one), None) => Some(one),
        _ => None,
    }
}

/// The live relationships of `kind` that join `from` to `to`, in that order, each once and in a
/// stable order; none when nothing does. Only identities count, never names.
pub fn existing_matches(store: &Store, kind: Kind, from: Obj, to: Obj) -> Vec<Obj> {
    let mut out: Vec<Obj> =
        store.referrers(from).into_iter().map(|(r, _)| r).filter(|r| store.is_alive(*r) && kind.ends(store, *r) == Some((from, to))).collect();
    out.sort();
    out.dedup();
    out
}

/// A candidate as a command's result gives it: its id, its name (empty for none), its two ends'
/// ids under the names `ends` gives them, such as `client` and `supplier`, and its `detail`; an id
/// that cannot be read, as only a damaged store has, is refused.
pub fn candidate_json(store: &Store, kind: Kind, r: Obj, ends: [&str; 2]) -> Result<Json, CommandError> {
    let (from, to) = kind.ends(store, r).ok_or_else(|| CommandError::new("the relationship no longer joins two elements"))?;
    let mut out = json!({ "id": uuid_of(store, r)?, "name": name_of(store, r), "detail": Json::Object(kind.detail(store, r)) });
    if let Some(fields) = out.as_object_mut() {
        fields.insert(ends[0].to_string(), Json::String(uuid_of(store, from)?));
        fields.insert(ends[1].to_string(), Json::String(uuid_of(store, to)?));
    }
    Ok(out)
}
