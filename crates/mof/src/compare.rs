//! The rows of two states of a model compared by identity: each row is matched by its id, so a
//! rename stays one change and where a row stands on either side plays no part. Only the tables are
//! compared here, not the values of multi-valued properties. A comparison too large for its budget
//! stops and says so: it never gives part of an answer.

use std::collections::BTreeMap;
use std::fmt;

use crate::Uuid;
use crate::rows::{Cell, Rows};
use crate::schema::Table;

/// The most cells a comparison of two whole models may read: both sides at the most cells a model
/// read back may hold.
pub const MAX_COMPARED: usize = 16_000_000;

/// How one row differs between the two states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowChange {
    Added(Uuid),
    Removed(Uuid),
    /// The row is on both sides; `columns` are the table's columns that differ.
    Changed {
        id: Uuid,
        columns: Vec<usize>,
    },
}

/// How the rows differ, per table in the order the tables were given: within a table the rows on
/// the before side by id, then those only on the after side by id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowDifferences {
    pub tables: Vec<Vec<RowChange>>,
}

impl RowDifferences {
    /// Whether every table's rows are the same on both sides.
    pub fn is_empty(&self) -> bool {
        self.tables.iter().all(Vec::is_empty)
    }
}

/// Why two states could not be compared. There is never a partial answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareError {
    /// The two sides hold `cells` cells, more than the `max` allowed.
    TooLarge { cells: usize, max: usize },
    /// `name` should have `expected` tables or columns and has `found`.
    Shape { name: &'static str, expected: usize, found: usize },
    /// A row of `table` has no id.
    RowWithoutId { table: &'static str },
    /// Two rows of `table` have the id `id`.
    DuplicateRow { table: &'static str, id: Uuid },
}

impl fmt::Display for CompareError {
    /// The reason in English, as the store's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareError::TooLarge { cells, max } => write!(f, "comparing these reads {cells} cells, more than the {max} allowed"),
            CompareError::Shape { name, expected, found } => write!(f, "{name} has {found} parts where {expected} are expected"),
            CompareError::RowWithoutId { table } => write!(f, "a row of {table} has no id"),
            CompareError::DuplicateRow { table, id } => write!(f, "two rows of {table} have the id {id}"),
        }
    }
}

impl std::error::Error for CompareError {}

/// Compares the rows of `tables` in `before` with those in `after`, reading at most `max` cells.
/// The values of multi-valued properties, in the junctions of the two states, are not looked at.
pub fn compare_rows(before: &Rows, after: &Rows, tables: &[Table], max: usize) -> Result<RowDifferences, CompareError> {
    let mut cells = 0usize;
    for side in [before, after] {
        check_count("the tables", tables.len(), side.tables.len())?;
        cells = side.tables.iter().flatten().map(Vec::len).fold(cells, usize::saturating_add);
    }
    if cells > max {
        return Err(CompareError::TooLarge { cells, max });
    }
    let mut differences = RowDifferences::default();
    for (table, (old, new)) in tables.iter().zip(before.tables.iter().zip(&after.tables)) {
        differences.tables.push(table_changes(table, old, new)?);
    }
    Ok(differences)
}

/// Refuses a side that does not have the parts its schema names.
fn check_count(name: &'static str, expected: usize, found: usize) -> Result<(), CompareError> {
    if expected == found { Ok(()) } else { Err(CompareError::Shape { name, expected, found }) }
}

/// The rows of one side of `table` by their id, each checked against the table's shape.
fn indexed<'a>(table: &Table, rows: &'a [Vec<Cell>]) -> Result<BTreeMap<Uuid, &'a [Cell]>, CompareError> {
    let at = table.columns.iter().position(|c| c.prop.is_none());
    let mut by_id = BTreeMap::new();
    for row in rows {
        check_count(table.name, table.columns.len(), row.len())?;
        let Some(Cell::Id(id)) = at.and_then(|at| row.get(at)) else { return Err(CompareError::RowWithoutId { table: table.name }) };
        if by_id.insert(*id, row.as_slice()).is_some() {
            return Err(CompareError::DuplicateRow { table: table.name, id: *id });
        }
    }
    Ok(by_id)
}

/// How the rows of `table` differ, by id: where a row stands on either side plays no part.
fn table_changes(table: &Table, before: &[Vec<Cell>], after: &[Vec<Cell>]) -> Result<Vec<RowChange>, CompareError> {
    let (old, new) = (indexed(table, before)?, indexed(table, after)?);
    let mut changes = Vec::new();
    for (id, row) in &old {
        match new.get(id) {
            None => changes.push(RowChange::Removed(*id)),
            Some(other) => {
                let columns: Vec<usize> = row.iter().zip(*other).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
                if !columns.is_empty() {
                    changes.push(RowChange::Changed { id: *id, columns });
                }
            }
        }
    }
    changes.extend(new.keys().filter(|id| !old.contains_key(id)).map(|id| RowChange::Added(*id)));
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{ClassId, PropId};
    use crate::schema::Column;

    // The id is not the first column, so only the schema can say where it is.
    const COLUMNS: &[Column] = &[Column { name: "name", prop: Some(PropId(1)), sql: "" }, Column { name: "id", prop: None, sql: "" }];
    const TABLES: &[Table] =
        &[Table { name: "one", class: ClassId(1), columns: COLUMNS }, Table { name: "two", class: ClassId(2), columns: COLUMNS }];

    /// The id spelled by `n`.
    fn id(n: u8) -> Uuid {
        Uuid([n; 16])
    }
    /// A row named `name` with the id spelled by `n`.
    fn row(name: &str, n: u8) -> Vec<Cell> {
        vec![Cell::Text(name.into()), Cell::Id(id(n))]
    }
    /// A state whose first table holds `rows`.
    fn table(rows: Vec<Vec<Cell>>) -> Rows {
        Rows { tables: vec![rows, vec![]], junctions: vec![] }
    }
    /// The comparison with the full budget.
    fn diff(before: &Rows, after: &Rows) -> Result<RowDifferences, CompareError> {
        compare_rows(before, after, TABLES, MAX_COMPARED)
    }

    /// Identity decides: rows swapped in place are no change, and ids swapped under the same places are.
    #[test]
    fn rows_are_matched_by_id_wherever_they_stand() {
        let before = table(vec![row("A", 1), row("B", 2)]);
        assert!(diff(&before, &table(vec![row("B", 2), row("A", 1)])).unwrap().is_empty());
        let swapped = diff(&before, &table(vec![row("A", 2), row("B", 1)])).unwrap();
        let changed = |n| RowChange::Changed { id: id(n), columns: vec![0] };
        assert_eq!(swapped.tables[0], [changed(1), changed(2)]);
    }

    /// One id on both sides is one change; in another table it is removed there and added here.
    #[test]
    fn an_id_is_added_or_removed_per_table() {
        let before = table(vec![row("A", 1), row("B", 2)]);
        let after = table(vec![row("C", 3), row("B", 2)]);
        assert_eq!(diff(&before, &after).unwrap().tables[0], [RowChange::Removed(id(1)), RowChange::Added(id(3))]);
        let moved = Rows { tables: vec![vec![], vec![row("A", 1)]], junctions: vec![] };
        let found = diff(&table(vec![row("A", 1)]), &moved).unwrap();
        assert_eq!(found.tables, [vec![RowChange::Removed(id(1))], vec![RowChange::Added(id(1))]]);
    }

    /// Exactly at the budget is an answer, and one short is a stop that says so.
    #[test]
    fn the_budget_edge_stops_with_no_partial_answer() {
        let (before, after) = (table(vec![row("A", 1)]), table(vec![row("A", 1), row("B", 2)]));
        assert_eq!(compare_rows(&before, &after, TABLES, 6).unwrap().tables[0], [RowChange::Added(id(2))]);
        assert_eq!(compare_rows(&before, &after, TABLES, 5), Err(CompareError::TooLarge { cells: 6, max: 5 }));
    }

    /// A state that cannot be a snapshot is refused for its own reason, as a standard error.
    #[test]
    fn a_malformed_state_is_refused_for_its_own_reason() {
        let refused = |before: &Rows| -> Result<(), Box<dyn std::error::Error>> {
            diff(before, &table(vec![]))?;
            Ok(())
        };
        let cases = [
            (Rows::default(), "the tables has 0 parts where 2 are expected"),
            (table(vec![vec![Cell::Null]]), "one has 1 parts where 2 are expected"),
            (table(vec![vec![Cell::Null, Cell::Null]]), "a row of one has no id"),
            (table(vec![row("A", 1), row("B", 1)]), "two rows of one have the id 01010101-0101-0101-0101-010101010101"),
        ];
        for (state, words) in cases {
            assert_eq!(refused(&state).unwrap_err().to_string(), words);
        }
        assert_eq!(CompareError::TooLarge { cells: 2, max: 1 }.to_string(), "comparing these reads 2 cells, more than the 1 allowed");
    }

    /// Cells compare by value and kind: zero has one value, and a whole number is not a real one.
    #[test]
    fn cells_compare_by_value_and_kind() {
        let one = |cell: Cell| table(vec![vec![cell, Cell::Id(id(1))]]);
        assert!(diff(&one(Cell::Real(0.0)), &one(Cell::Real(-0.0))).unwrap().is_empty());
        let kinds = diff(&one(Cell::Int(1)), &one(Cell::Real(1.0))).unwrap();
        assert_eq!(kinds.tables[0], [RowChange::Changed { id: id(1), columns: vec![0] }]);
    }
}
