//! A store as the rows of its metamodels' generated tables, and back: what a project writes to its
//! database as the user works and reads when it opens. One row per object in the table of its
//! class (its id and its single-valued stored properties), one row per value of a multi-valued
//! stored property in that property's junction (owner, position, value). Nothing here talks to a
//! database: the project crate moves these rows.
//!
//! Reading back trusts nothing: every row is checked against its table's shape, every value goes
//! through the store's own setters (types, multiplicities, containment, lengths), an id seen twice,
//! a reference to no object, an object listed as a part twice, a value twice in a unique list or a
//! nesting deeper than [`MAX_NESTING`] refuses the whole snapshot, and the store is left as it was.
//! The work is linear in the rows: depths are found once, and a unique list is checked for repeats
//! once before it is built.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::meta::{ClassId, DataKind, Property, TypeRef};
use crate::schema::{Junction, Table};
use crate::store::{Invalid, Obj, Store};
use crate::{Uuid, Value};

/// The deepest an object may be nested in its containers when read back: every containment step
/// walks the chain above it, so the depth is checked before anything is built.
pub const MAX_NESTING: usize = 256;

/// One stored value as a database cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Bool(bool),
    /// An integer, an enumeration literal's index, or an unlimited natural (-1 for `*`).
    Int(i64),
    /// Always finite.
    Real(f64),
    Text(Arc<str>),
    /// An object's id: a row's own, or a reference.
    Id(Uuid),
}

/// The rows of a store: per table (in the order the tables were given) its rows, each with its
/// columns in the table's order; per junction (likewise) its (owner, position, value) rows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rows {
    pub tables: Vec<Vec<Vec<Cell>>>,
    pub junctions: Vec<Vec<(Uuid, u32, Cell)>>,
}

impl Rows {
    /// How many rows there are in all.
    pub fn len(&self) -> usize {
        self.tables.iter().map(Vec::len).sum::<usize>() + self.junctions.iter().map(Vec::len).sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn refused(why: impl Into<String>) -> Invalid {
    Invalid::Refused(why.into())
}

/// A value as a key for finding repeats in a list: equal keys exactly for the values the store
/// calls equal (it is made from the value read, not the cell, so two cells read as one value are
/// one key).
#[derive(PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Null,
    Bool(bool),
    Int(i64),
    /// The bits of a real; both zeros are one key, as they are one value.
    Real(u64),
    Text(Arc<str>),
    Unlimited(Option<u64>),
    Enum(u32),
    Ref(Obj),
}

pub(crate) fn key(value: &Value) -> Key {
    match value {
        Value::Null => Key::Null,
        Value::Bool(b) => Key::Bool(*b),
        Value::Int(i) => Key::Int(*i),
        Value::Real(r) => Key::Real(if *r == 0.0 { 0 } else { r.to_bits() }),
        Value::Str(t) => Key::Text(t.clone()),
        Value::Unlimited(n) => Key::Unlimited(*n),
        Value::Enum(i) => Key::Enum(*i),
        Value::Ref(o) => Key::Ref(*o),
    }
}

/// A stored value as a cell.
fn cell_of(store: &Store, value: &Value) -> Result<Cell, Invalid> {
    Ok(match value {
        Value::Null => Cell::Null,
        Value::Bool(b) => Cell::Bool(*b),
        Value::Int(i) => Cell::Int(*i),
        Value::Real(r) => Cell::Real(*r),
        Value::Str(s) => Cell::Text(s.clone()),
        Value::Unlimited(None) => Cell::Int(-1),
        Value::Unlimited(Some(n)) => Cell::Int(i64::try_from(*n).map_err(|_| refused(format!("the unlimited natural {n} is too large to store")))?),
        Value::Enum(i) => Cell::Int(i64::from(*i)),
        Value::Ref(o) => Cell::Id(store.uuid(*o)?),
    })
}

/// A cell read back as a value of `prop`; a cell of the wrong kind is refused.
fn value_of(store: &Store, prop: &Property, cell: &Cell, ids: &HashMap<Uuid, Obj>) -> Result<Value, Invalid> {
    let wrong = || refused(format!("{} holds a value of the wrong kind", prop.name));
    if *cell == Cell::Null {
        return Ok(Value::Null);
    }
    match prop.ty {
        TypeRef::Class(_) | TypeRef::Any => match cell {
            Cell::Id(u) => {
                ids.get(u).copied().map(Value::Ref).ok_or_else(|| refused(format!("{} refers to {u}, which is not in the project", prop.name)))
            }
            _ => Err(wrong()),
        },
        TypeRef::Data(d) => {
            let kind =
                store.universe().data_type(d).map(|t| t.kind.clone()).ok_or_else(|| Invalid::Internal(format!("there is no data type {}", d.0)))?;
            match (kind, cell) {
                (DataKind::Boolean, Cell::Bool(b)) => Ok(Value::Bool(*b)),
                (DataKind::Boolean, Cell::Int(0)) => Ok(Value::Bool(false)),
                (DataKind::Boolean, Cell::Int(1)) => Ok(Value::Bool(true)),
                (DataKind::Integer, Cell::Int(i)) => Ok(Value::Int(*i)),
                (DataKind::Real, Cell::Real(r)) if r.is_finite() => Ok(Value::Real(*r)),
                // An integer cell for a real column is exact only within the 53 bits of an f64.
                (DataKind::Real, Cell::Int(i)) if i.unsigned_abs() <= 1 << 53 => Ok(Value::Real(*i as f64)),
                (DataKind::String, Cell::Text(t)) => Ok(Value::Str(t.clone())),
                (DataKind::UnlimitedNatural, Cell::Int(-1)) => Ok(Value::Unlimited(None)),
                (DataKind::UnlimitedNatural, Cell::Int(n)) => u64::try_from(*n).map(|n| Value::Unlimited(Some(n))).map_err(|_| wrong()),
                (DataKind::Enumeration(literals), Cell::Int(i)) => match (usize::try_from(*i), u32::try_from(*i)) {
                    (Ok(at), Ok(index)) if at < literals.len() => Ok(Value::Enum(index)),
                    _ => Err(wrong()),
                },
                _ => Err(wrong()),
            }
        }
    }
}

/// The store's rows for `tables` and `junctions`. An object whose class has no table is refused,
/// so nothing is ever left out of a save without a word.
pub fn snapshot(store: &Store, tables: &[Table], junctions: &[Junction]) -> Result<Rows, Invalid> {
    let mut by_class: HashMap<ClassId, Vec<Obj>> = HashMap::new();
    for o in store.alive() {
        by_class.entry(store.class_of(o)?).or_default().push(o);
    }
    let mut rows = Rows::default();
    for table in tables {
        let mut out = Vec::new();
        for &o in by_class.get(&table.class).map(Vec::as_slice).unwrap_or_default() {
            let mut row = Vec::with_capacity(table.columns.len());
            for c in table.columns {
                row.push(match c.prop {
                    None => Cell::Id(store.uuid(o)?),
                    Some(p) => cell_of(store, &store.get(o, p)?)?,
                });
            }
            out.push(row);
        }
        rows.tables.push(out);
    }
    let tabled: HashSet<ClassId> = tables.iter().map(|t| t.class).collect();
    if let Some(class) = by_class.keys().find(|c| !tabled.contains(c)) {
        let name = store.universe().class(*class).map(|c| c.name.to_string()).unwrap_or_default();
        return Err(refused(format!("objects of {name} have no table to be saved in")));
    }
    // Every list a saved object has must have its junction, or its values would be left out.
    let covered: HashSet<(ClassId, crate::meta::PropId)> = junctions.iter().map(|j| (j.class, j.prop)).collect();
    for table in tables {
        let info = store.universe().info(table.class).ok_or_else(|| Invalid::Internal(format!("there is no class {}", table.class.0)))?;
        for p in &info.stored {
            let many = store.universe().prop(*p).is_some_and(Property::is_many);
            if many && !covered.contains(&(table.class, *p)) {
                return Err(refused(format!(
                    "{}'s lists of {} have no table to be saved in",
                    table.name,
                    store.universe().prop(*p).map(|x| x.name.to_string()).unwrap_or_default()
                )));
            }
        }
    }
    for j in junctions {
        let mut out = Vec::new();
        // A junction is one concrete class's: two classes sharing a property each have their own.
        for &o in by_class.get(&j.class).map(Vec::as_slice).unwrap_or_default() {
            let owner = store.uuid(o)?;
            for (i, v) in store.list(o, j.prop)?.iter().enumerate() {
                let at = u32::try_from(i).map_err(|_| refused("a list is too long to save"))?;
                out.push((owner, at, cell_of(store, v)?));
            }
        }
        rows.junctions.push(out);
    }
    Ok(rows)
}

/// Builds the objects of `rows` into `store`, all of them or none: the objects first under their
/// ids, then their single values, then each list in its order. The caller validates the result.
/// It is one transaction, so a model past [`crate::store::MAX_OPS`] steps is refused; a project
/// being opened uses [`restore_into_empty`], which has no such bound.
pub fn restore(store: &mut Store, rows: &Rows, tables: &[Table], junctions: &[Junction]) -> Result<(), Invalid> {
    if rows.tables.len() != tables.len() || rows.junctions.len() != junctions.len() {
        return Err(refused("the rows do not match the project's tables"));
    }
    check_nesting(store, rows, tables, junctions)?;
    store.transaction(|st| build(st, rows, tables, junctions))
}

/// Builds the objects of `rows` into `store`, which must hold nothing yet, and gives it back built
/// (AR-30). No undo log is kept, so a model of any size the read budgets let through opens: all of
/// it or none comes from the store being given back only when everything was built, and dropped
/// otherwise. The caller validates the result.
pub fn restore_into_empty(mut store: Store, rows: &Rows, tables: &[Table], junctions: &[Junction]) -> Result<Store, Invalid> {
    if store.live() != 0 {
        return Err(Invalid::Internal("a model is built into an empty store only".into()));
    }
    if rows.tables.len() != tables.len() || rows.junctions.len() != junctions.len() {
        return Err(refused("the rows do not match the project's tables"));
    }
    check_nesting(&store, rows, tables, junctions)?;
    build(&mut store, rows, tables, junctions)?;
    Ok(store)
}

/// The objects of `rows`, their single values, then their lists, built into `st`.
fn build(st: &mut Store, rows: &Rows, tables: &[Table], junctions: &[Junction]) -> Result<(), Invalid> {
    {
        let mut ids: HashMap<Uuid, Obj> = HashMap::new();
        for (table, table_rows) in tables.iter().zip(&rows.tables) {
            let id_column =
                table.columns.iter().position(|c| c.prop.is_none()).ok_or_else(|| Invalid::Internal(format!("{} has no id column", table.name)))?;
            for row in table_rows {
                if row.len() != table.columns.len() {
                    return Err(refused(format!("a row of {} has {} columns, not {}", table.name, row.len(), table.columns.len())));
                }
                let Some(Cell::Id(uuid)) = row.get(id_column) else { return Err(refused(format!("a row of {} has no id", table.name))) };
                if ids.contains_key(uuid) {
                    return Err(refused(format!("the id {uuid} is there twice")));
                }
                let o = st.create_with_uuid(table.class, *uuid)?;
                ids.insert(*uuid, o);
            }
        }
        for (table, table_rows) in tables.iter().zip(&rows.tables) {
            for row in table_rows {
                let Some(Cell::Id(uuid)) = table.columns.iter().zip(row).find(|(c, _)| c.prop.is_none()).map(|(_, cell)| cell) else { continue };
                let o = ids.get(uuid).copied().ok_or_else(|| Invalid::Internal("a created object went missing".into()))?;
                for (column, cell) in table.columns.iter().zip(row) {
                    let Some(p) = column.prop else { continue };
                    let prop = st.universe().prop(p).cloned().ok_or_else(|| Invalid::Internal(format!("there is no property {}", p.0)))?;
                    let value = value_of(st, &prop, cell, &ids)?;
                    if st.get(o, p)? != value {
                        st.set(o, p, value)?;
                    }
                }
            }
        }
        for (j, junction_rows) in junctions.iter().zip(&rows.junctions) {
            let prop = st.universe().prop(j.prop).cloned().ok_or_else(|| Invalid::Internal(format!("there is no property {}", j.prop.0)))?;
            let mut lists: HashMap<Uuid, Vec<(u32, &Cell)>> = HashMap::new();
            for (owner, at, cell) in junction_rows {
                lists.entry(*owner).or_default().push((*at, cell));
            }
            let mut owners: Vec<&Uuid> = lists.keys().collect();
            owners.sort();
            for owner in owners {
                let o =
                    ids.get(owner).copied().ok_or_else(|| refused(format!("{} lists values for {owner}, which is not in the project", j.name)))?;
                let mut items = lists.get(owner).cloned().unwrap_or_default();
                items.sort_by_key(|(at, _)| *at);
                // A unique list is checked for repeats once, so each value is added without a
                // search of the list built so far. A unique list with an opposite may hold values
                // already, put there as their opposites were set from a column or another list:
                // read once, they are not added again, and the list then takes its saved order in
                // one step.
                let mut seen = HashSet::new();
                let paired = prop.opposite.is_some() && prop.is_unique;
                let present: HashSet<Key> = if paired { st.list(o, j.prop)?.iter().map(key).collect() } else { HashSet::new() };
                let mut order: Vec<Value> = if paired { Vec::with_capacity(items.len()) } else { Vec::new() };
                for (k, (at, cell)) in items.iter().enumerate() {
                    if *at as usize != k {
                        return Err(refused(format!("{} misses position {k} of a list", j.name)));
                    }
                    let value = value_of(st, &prop, cell, &ids)?;
                    if prop.is_unique {
                        if !seen.insert(key(&value)) {
                            return Err(refused(format!("{} holds a value twice in a list", j.name)));
                        }
                        if paired {
                            order.push(value.clone());
                            if present.contains(&key(&value)) {
                                continue;
                            }
                        }
                        st.push_new(o, j.prop, value)?;
                    } else {
                        st.push(o, j.prop, value)?;
                    }
                }
                // A value the list holds that its rows do not list makes the reorder refuse.
                if paired {
                    st.reorder(o, j.prop, order).map_err(|_| refused(format!("{} does not list every value its opposites give it", j.name)))?;
                }
            }
        }
        Ok(())
    }
}

/// Refuses rows whose containment nests deeper than [`MAX_NESTING`], or in a loop, or lists an
/// object as a part twice, before anything is built: each object's depth is found once, walking
/// up through its containers. (The store keeps an object's last container, so a second one would
/// make a different tree from the one checked.)
fn check_nesting(store: &Store, rows: &Rows, tables: &[Table], junctions: &[Junction]) -> Result<(), Invalid> {
    let composite = |p| store.universe().prop(p).is_some_and(|x| x.is_composite);
    // Each contained object's container, from the composite columns and lists.
    let mut container: HashMap<Uuid, Uuid> = HashMap::new();
    for (table, table_rows) in tables.iter().zip(&rows.tables) {
        let Some(id_column) = table.columns.iter().position(|c| c.prop.is_none()) else { continue };
        let parts: Vec<usize> = table.columns.iter().enumerate().filter(|(_, c)| c.prop.is_some_and(composite)).map(|(i, _)| i).collect();
        if parts.is_empty() {
            continue;
        }
        for row in table_rows {
            let Some(Cell::Id(owner)) = row.get(id_column) else { continue };
            for &i in &parts {
                if let Some(Cell::Id(part)) = row.get(i) {
                    contain(&mut container, *part, *owner)?;
                }
            }
        }
    }
    for (j, junction_rows) in junctions.iter().zip(&rows.junctions) {
        if composite(j.prop) {
            for (owner, _, cell) in junction_rows {
                if let Cell::Id(part) = cell {
                    contain(&mut container, *part, *owner)?;
                }
            }
        }
    }
    let mut depth: HashMap<Uuid, usize> = HashMap::with_capacity(container.len());
    let mut path = Vec::new();
    for start in container.keys() {
        path.clear();
        let mut at = *start;
        let base = loop {
            if let Some(d) = depth.get(&at) {
                break *d;
            }
            if path.len() > MAX_NESTING {
                return Err(refused(format!("the model nests deeper than {MAX_NESTING} levels, or in a loop")));
            }
            path.push(at);
            match container.get(&at) {
                Some(up) => at = *up,
                None => break 0,
            }
        };
        // The walk ended at a root (the last on the path is at depth 0) or at an object whose depth
        // was known.
        let known = depth.contains_key(&at);
        for (k, o) in path.iter().rev().enumerate() {
            let d = if known { base + k + 1 } else { base + k };
            if d > MAX_NESTING {
                return Err(refused(format!("the model nests deeper than {MAX_NESTING} levels, or in a loop")));
            }
            depth.insert(*o, d);
        }
    }
    Ok(())
}

/// Records `owner` as `part`'s container. The store gives an object one place in one container,
/// so a part listed a second time, by another container or by the same one in another list, is
/// refused: building it would move the part, searching the list it leaves.
fn contain(container: &mut HashMap<Uuid, Uuid>, part: Uuid, owner: Uuid) -> Result<(), Invalid> {
    match container.insert(part, owner) {
        Some(_) => Err(refused(format!("{part} is listed twice as a part"))),
        None => Ok(()),
    }
}
