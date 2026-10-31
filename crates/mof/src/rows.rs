//! A store as the rows of its metamodels' generated tables, and back: what a project writes to its
//! database as the user works and reads when it opens. One row per object in the table of its
//! class (its id and its single-valued stored properties), one row per value of a multi-valued
//! stored property in that property's junction (owner, position, value). Nothing here talks to a
//! database: the project crate moves these rows.
//!
//! Reading back trusts nothing: every row is checked against its table's shape, every value goes
//! through the store's own setters (types, multiplicities, containment, lengths), an id seen twice
//! or a reference to no object refuses the whole snapshot, and the store is left as it was.

use std::collections::HashMap;
use std::sync::Arc;

use crate::meta::{ClassId, DataKind, Property, TypeRef};
use crate::schema::{Junction, Table};
use crate::store::{Invalid, Obj, Store};
use crate::{Uuid, Value};

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
                (DataKind::Real, Cell::Int(i)) => Ok(Value::Real(*i as f64)),
                (DataKind::String, Cell::Text(t)) => Ok(Value::Str(t.clone())),
                (DataKind::UnlimitedNatural, Cell::Int(-1)) => Ok(Value::Unlimited(None)),
                (DataKind::UnlimitedNatural, Cell::Int(n)) if *n >= 0 => Ok(Value::Unlimited(Some(*n as u64))),
                (DataKind::Enumeration(literals), Cell::Int(i)) if *i >= 0 && (*i as usize) < literals.len() => Ok(Value::Enum(*i as u32)),
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
        for o in by_class.remove(&table.class).unwrap_or_default() {
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
    if let Some((class, _)) = by_class.iter().next() {
        let name = store.universe().class(*class).map(|c| c.name.to_string()).unwrap_or_default();
        return Err(refused(format!("objects of {name} have no table to be saved in")));
    }
    // Every list a saved object has must have its junction, or its values would be left out.
    let covered: std::collections::HashSet<(ClassId, crate::meta::PropId)> = junctions.iter().map(|j| (j.class, j.prop)).collect();
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
        for o in store.alive() {
            if store.class_of(o)? != j.class {
                continue;
            }
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
pub fn restore(store: &mut Store, rows: &Rows, tables: &[Table], junctions: &[Junction]) -> Result<(), Invalid> {
    if rows.tables.len() != tables.len() || rows.junctions.len() != junctions.len() {
        return Err(refused("the rows do not match the project's tables"));
    }
    store.transaction(|st| {
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
                for (k, (at, cell)) in items.iter().enumerate() {
                    if *at as usize != k {
                        return Err(refused(format!("{} misses position {k} of a list", j.name)));
                    }
                    let value = value_of(st, &prop, cell, &ids)?;
                    st.push(o, j.prop, value)?;
                }
            }
        }
        Ok(())
    })
}
