//! Two states of a model compared by identity: each row is matched by its id, so a rename stays one
//! change and where a row stands on either side plays no part, and the values of a multi-valued
//! property are compared per owner, so an insertion at the front of a list does not report every
//! value after it. A comparison too large for its budget stops and says so: it never gives part of
//! an answer.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::Uuid;
use crate::rows::{Cell, Rows};
use crate::schema::{Junction, Table};

/// The most work a comparison of two whole models may do: both sides at the most cells a model
/// read back may hold.
pub const MAX_COMPARED: usize = 16_000_000;
/// What one row of a list costs in the budget: its owner, position and value.
const LIST_ROW_CELLS: usize = 3;

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

/// How one owner's list differs between the two states.
#[derive(Debug, Clone, PartialEq)]
pub struct ListChange {
    pub owner: Uuid,
    /// The values only the after side has, as many times as it has more of them.
    pub added: Vec<Cell>,
    /// The values only the before side has, likewise.
    pub removed: Vec<Cell>,
    /// Whether the values on both sides also stand in another order. Never for an unordered list.
    pub reordered: bool,
}

/// Everything that differs, per table and per junction in the order they were given. Within a
/// table the rows on the before side come by id, then those only on the after side by id; within
/// a junction the owners come by id.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Differences {
    pub tables: Vec<Vec<RowChange>>,
    pub lists: Vec<Vec<ListChange>>,
}

impl Differences {
    /// Whether the two states are the same.
    pub fn is_empty(&self) -> bool {
        self.tables.iter().all(Vec::is_empty) && self.lists.iter().all(Vec::is_empty)
    }
}

/// Why two states could not be compared. There is never a partial answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareError {
    /// The comparison would take `work` steps, more than the `max` allowed.
    TooLarge { work: usize, max: usize },
    /// `name` should have `expected` tables, junctions or columns and has `found`.
    Shape { name: &'static str, expected: usize, found: usize },
    /// A row of `table` has no id.
    RowWithoutId { table: &'static str },
    /// Two rows of `table` have the id `id`.
    DuplicateRow { table: &'static str, id: Uuid },
    /// Two values of `list` stand at the same `position` of `owner`'s list.
    DuplicatePosition { list: &'static str, owner: Uuid, position: u32 },
}

impl fmt::Display for CompareError {
    /// The reason in English, as the store's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareError::TooLarge { work, max } => write!(f, "comparing these takes {work} steps, more than the {max} allowed"),
            CompareError::Shape { name, expected, found } => write!(f, "{name} has {found} parts where {expected} are expected"),
            CompareError::RowWithoutId { table } => write!(f, "a row of {table} has no id"),
            CompareError::DuplicateRow { table, id } => write!(f, "two rows of {table} have the id {id}"),
            CompareError::DuplicatePosition { list, owner, position } => write!(f, "two values of {list} stand at {position} in the list of {owner}"),
        }
    }
}

impl std::error::Error for CompareError {}

/// A cell as something to count: `-0.0` and `0.0` are one value, as they are in a cell.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key<'a> {
    Null,
    Bool(bool),
    Int(i64),
    Real(u64),
    Text(&'a str),
    Id(Uuid),
}

/// The key of `cell`.
fn key(cell: &Cell) -> Key<'_> {
    match cell {
        Cell::Null => Key::Null,
        Cell::Bool(b) => Key::Bool(*b),
        Cell::Int(i) => Key::Int(*i),
        Cell::Real(r) => Key::Real((*r + 0.0).to_bits()),
        Cell::Text(t) => Key::Text(t),
        Cell::Id(id) => Key::Id(*id),
    }
}

/// The budget, spent as the work is found.
struct Budget {
    spent: usize,
    max: usize,
}

impl Budget {
    /// Spends `work`, or says the comparison is too large.
    fn charge(&mut self, work: usize) -> Result<(), CompareError> {
        self.spent = self.spent.saturating_add(work);
        if self.spent > self.max { Err(CompareError::TooLarge { work: self.spent, max: self.max }) } else { Ok(()) }
    }
}

/// Compares `before` with `after`, both snapshots of `tables` and `junctions`, within `max` steps:
/// one for each cell on either side, three for each row of a list, and the work of finding the
/// common order of a list whose values repeat.
pub fn compare(before: &Rows, after: &Rows, tables: &[Table], junctions: &[Junction], max: usize) -> Result<Differences, CompareError> {
    let mut budget = Budget { spent: 0, max };
    for side in [before, after] {
        check_count("the tables", tables.len(), side.tables.len())?;
        check_count("the lists", junctions.len(), side.junctions.len())?;
        let cells = side.tables.iter().flatten().map(Vec::len).fold(0, usize::saturating_add);
        let list_rows = side.junctions.iter().map(Vec::len).fold(0, usize::saturating_add);
        budget.charge(cells.saturating_add(list_rows.saturating_mul(LIST_ROW_CELLS)))?;
    }
    let mut differences = Differences::default();
    for (table, (old, new)) in tables.iter().zip(before.tables.iter().zip(&after.tables)) {
        differences.tables.push(table_changes(table, old, new)?);
    }
    for (list, (old, new)) in junctions.iter().zip(before.junctions.iter().zip(&after.junctions)) {
        differences.lists.push(list_changes(list, old, new, &mut budget)?);
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

/// One side of `list`: each owner's values in the order of their positions.
fn values_by_owner<'a>(list: &Junction, rows: &'a [(Uuid, u32, Cell)]) -> Result<BTreeMap<Uuid, Vec<&'a Cell>>, CompareError> {
    let mut by_owner: BTreeMap<Uuid, BTreeMap<u32, &Cell>> = BTreeMap::new();
    for (owner, position, value) in rows {
        if by_owner.entry(*owner).or_default().insert(*position, value).is_some() {
            return Err(CompareError::DuplicatePosition { list: list.name, owner: *owner, position: *position });
        }
    }
    Ok(by_owner.into_iter().map(|(owner, values)| (owner, values.into_values().collect())).collect())
}

/// How the owners' lists of `list` differ. An owner with no values on a side has an empty list there.
fn list_changes(
    list: &Junction,
    before: &[(Uuid, u32, Cell)],
    after: &[(Uuid, u32, Cell)],
    budget: &mut Budget,
) -> Result<Vec<ListChange>, CompareError> {
    let (old, new) = (values_by_owner(list, before)?, values_by_owner(list, after)?);
    let owners: BTreeSet<&Uuid> = old.keys().chain(new.keys()).collect();
    let mut changes = Vec::new();
    for owner in owners {
        let (was, is) = (old.get(owner).map_or(&[][..], Vec::as_slice), new.get(owner).map_or(&[][..], Vec::as_slice));
        let (added, removed) = (beyond(is, was), beyond(was, is));
        let reordered = list.ordered && reordered(was, is, was.len().saturating_sub(removed.len()), budget)?;
        if !added.is_empty() || !removed.is_empty() || reordered {
            changes.push(ListChange { owner: *owner, added, removed, reordered });
        }
    }
    Ok(changes)
}

/// The values of `these` left over once each value of `others` has been matched with one of them.
fn beyond<'a>(these: &[&'a Cell], others: &[&'a Cell]) -> Vec<Cell> {
    let mut left: BTreeMap<Key<'a>, usize> = BTreeMap::new();
    for cell in others {
        *left.entry(key(cell)).or_default() += 1;
    }
    let mut unmatched = Vec::new();
    for cell in these {
        match left.get_mut(&key(cell)) {
            Some(n) if *n > 0 => *n -= 1,
            _ => unmatched.push((*cell).clone()),
        }
    }
    unmatched
}

/// Whether the `shared` values on both sides cannot be kept in one order. A value repeated more on
/// one side can be matched in several ways, so only then is the longest common run worked out.
fn reordered(was: &[&Cell], is: &[&Cell], shared: usize, budget: &mut Budget) -> Result<bool, CompareError> {
    let (in_was, in_is): (BTreeSet<Key>, BTreeSet<Key>) = (was.iter().map(|c| key(c)).collect(), is.iter().map(|c| key(c)).collect());
    let a: Vec<Key> = was.iter().map(|c| key(c)).filter(|k| in_is.contains(k)).collect();
    let b: Vec<Key> = is.iter().map(|c| key(c)).filter(|k| in_was.contains(k)).collect();
    if a.len() == shared && b.len() == shared {
        return Ok(a != b);
    }
    budget.charge(a.len().saturating_mul(b.len()))?;
    Ok(longest_common(&a, &b) < shared)
}

/// The length of the longest run of values `a` and `b` have in the same order.
fn longest_common(a: &[Key], b: &[Key]) -> usize {
    let mut best = vec![0usize; b.len()];
    for x in a {
        let (mut diagonal, mut left) = (0, 0);
        for (cell, y) in best.iter_mut().zip(b) {
            let up = *cell;
            *cell = if x == y { diagonal + 1 } else { up.max(left) };
            diagonal = up;
            left = *cell;
        }
    }
    best.last().copied().unwrap_or(0)
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

    const LISTS: &[Junction] = &[
        Junction { name: "ordered", class: ClassId(1), prop: PropId(2), ordered: true, reference: true },
        Junction { name: "bag", class: ClassId(1), prop: PropId(3), ordered: false, reference: true },
    ];
    const OWNER: u8 = 90;

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
        Rows { tables: vec![rows, vec![]], junctions: vec![vec![], vec![]] }
    }
    /// An owner's list, its values at the positions given.
    fn entries(owner: u8, at: &[(u32, u8)]) -> Vec<(Uuid, u32, Cell)> {
        at.iter().map(|(position, value)| (id(owner), *position, Cell::Id(id(*value)))).collect()
    }
    /// A state whose ordered list holds [`OWNER`]'s values.
    fn list(at: &[(u32, u8)]) -> Rows {
        Rows { tables: vec![vec![], vec![]], junctions: vec![entries(OWNER, at), vec![]] }
    }
    /// The ids spelled by `values`, as cells.
    fn cells(values: &[u8]) -> Vec<Cell> {
        values.iter().map(|n| Cell::Id(id(*n))).collect()
    }
    /// The comparison with the full budget.
    fn diff(before: &Rows, after: &Rows) -> Result<Differences, CompareError> {
        compare(before, after, TABLES, LISTS, MAX_COMPARED)
    }
    /// The one change to [`OWNER`]'s ordered list, as added, removed and reordered.
    fn change(before: &[(u32, u8)], after: &[(u32, u8)]) -> (Vec<Cell>, Vec<Cell>, bool) {
        let found = diff(&list(before), &list(after)).unwrap().lists.remove(0);
        assert_eq!(found.len(), 1, "{found:?}");
        (found[0].added.clone(), found[0].removed.clone(), found[0].reordered)
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
        let moved = Rows { tables: vec![vec![], vec![row("A", 1)]], junctions: vec![vec![], vec![]] };
        let found = diff(&table(vec![row("A", 1)]), &moved).unwrap();
        assert_eq!(found.tables, [vec![RowChange::Removed(id(1))], vec![RowChange::Added(id(1))]]);
    }

    /// Exactly at the budget is an answer, and one short is a stop that says so.
    #[test]
    fn the_budget_edge_stops_with_no_partial_answer() {
        let (before, after) = (table(vec![row("A", 1)]), table(vec![row("A", 1), row("B", 2)]));
        assert_eq!(compare(&before, &after, TABLES, LISTS, 6).unwrap().tables[0], [RowChange::Added(id(2))]);
        assert_eq!(compare(&before, &after, TABLES, LISTS, 5), Err(CompareError::TooLarge { work: 6, max: 5 }));
        // 15 for the five list rows of the two sides, then 3 by 2 steps for the order of the repeated value.
        let (before, after) = (list(&[(0, 1), (1, 2), (2, 1)]), list(&[(0, 2), (1, 1)]));
        assert!(compare(&before, &after, TABLES, LISTS, 21).is_ok());
        assert_eq!(compare(&before, &after, TABLES, LISTS, 20), Err(CompareError::TooLarge { work: 21, max: 20 }));
    }

    /// A value keeps its place in the comparison however the positions around it shift.
    #[test]
    fn list_values_are_compared_per_owner_not_by_position() {
        assert_eq!(change(&[(0, 2), (1, 3)], &[(0, 1), (1, 2), (2, 3)]), (cells(&[1]), vec![], false));
        assert_eq!(change(&[(0, 1), (1, 2), (2, 3)], &[(0, 3), (1, 1), (2, 2)]), (vec![], vec![], true));
        // Positions with gaps, given out of order, are still the same list.
        assert!(diff(&list(&[(0, 1), (1, 2)]), &list(&[(9, 2), (4, 1)])).unwrap().is_empty());
        // An unordered list has no order to change, and its values still count.
        let bag = |at: &[(u32, u8)]| Rows { tables: vec![vec![], vec![]], junctions: vec![vec![], entries(OWNER, at)] };
        assert!(diff(&bag(&[(0, 1), (1, 2)]), &bag(&[(0, 2), (1, 1)])).unwrap().is_empty());
        let grown = diff(&bag(&[(0, 1)]), &bag(&[(0, 2), (1, 1)])).unwrap();
        assert_eq!(grown.lists[1], [ListChange { owner: id(OWNER), added: cells(&[2]), removed: vec![], reordered: false }]);
    }

    /// Repeated values are counted, and only a real change of order is called one.
    #[test]
    fn repeated_values_in_a_list() {
        assert_eq!(change(&[(0, 1), (1, 1), (2, 2)], &[(0, 1), (1, 2)]), (vec![], cells(&[1]), false));
        assert_eq!(change(&[(0, 1), (1, 2), (2, 1)], &[(0, 2), (1, 1)]), (vec![], cells(&[1]), false), "the first of the two went");
        assert_eq!(change(&[(0, 1), (1, 1), (2, 2)], &[(0, 1), (1, 2), (2, 1)]), (vec![], vec![], true));
        assert_eq!(change(&[(0, 1), (1, 2), (2, 1)], &[(0, 2), (1, 2), (2, 1), (3, 1)]), (cells(&[2]), vec![], true), "1, 2, 1 is not in 2, 2, 1, 1");
    }

    /// An owner with no values is an empty list on that side, and no difference when on both.
    #[test]
    fn an_empty_list_and_a_missing_owner_are_the_same() {
        let values = list(&[(0, 1), (1, 2)]);
        let other_owner = Rows { tables: vec![vec![], vec![]], junctions: vec![entries(7, &[(0, 1)]), vec![]] };
        assert!(diff(&list(&[]), &Rows { tables: vec![vec![], vec![]], junctions: vec![vec![], vec![]] }).unwrap().is_empty());
        let gone = diff(&values, &list(&[])).unwrap();
        assert_eq!(gone.lists[0], [ListChange { owner: id(OWNER), added: vec![], removed: cells(&[1, 2]), reordered: false }]);
        let came = diff(&list(&[]), &values).unwrap();
        assert_eq!(came.lists[0], [ListChange { owner: id(OWNER), added: cells(&[1, 2]), removed: vec![], reordered: false }]);
        // The same value under another owner is that owner's: nothing is matched across owners.
        let across = diff(&list(&[(0, 1)]), &other_owner).unwrap();
        assert_eq!(across.lists[0].iter().map(|c| c.owner).collect::<Vec<_>>(), [id(7), id(OWNER)]);
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
            (Rows { tables: vec![vec![], vec![]], junctions: vec![vec![]] }, "the lists has 1 parts where 2 are expected"),
            (list(&[(3, 1), (3, 2)]), "two values of ordered stand at 3 in the list of 5a5a5a5a-5a5a-5a5a-5a5a-5a5a5a5a5a5a"),
        ];
        for (state, words) in cases {
            assert_eq!(refused(&state).unwrap_err().to_string(), words);
        }
        assert_eq!(CompareError::TooLarge { work: 2, max: 1 }.to_string(), "comparing these takes 2 steps, more than the 1 allowed");
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
