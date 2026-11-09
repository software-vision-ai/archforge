//! What differs between two states of a project, classified and named for a person to follow: in
//! the model or a diagram, added, changed or removed, with the names and path of the item. Each
//! side is restored into a store of its own and repaired as an opening repairs it, so a box an
//! opening fits shows no false change. Nothing here writes: the stores are thrown away.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use archforge_mof::compare::{CompareError, MAX_COMPARED, RowChange, compare};
use archforge_mof::rows::{Cell, MAX_NESTING, Rows, restore_into_empty, snapshot};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{ClassId, Obj, Store, Uuid};

use crate::{storage, store};

/// Where a difference lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Model,
    /// A diagram, its layout or its style: what presents the model.
    Diagram,
}

/// What happened to an item between the two states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

/// One item that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub id: Uuid,
    pub class: ClassId,
    pub area: Area,
    pub kind: ChangeKind,
    /// The item's name, or for a box or line in a diagram that of the element it shows; none when
    /// it has neither. A removed item's comes from the before side, any other's from the after side.
    pub name: Option<String>,
    /// The names of what contains the item, outermost first, from the same side as its name.
    pub path: Vec<String>,
}

/// Why two states could not be classified. There is never a partial answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifyError {
    /// The rows of the `side` named could not be restored.
    Unreadable {
        side: &'static str,
        why: String,
    },
    Compare(CompareError),
}

impl fmt::Display for ClassifyError {
    /// The reason in English, as the store's own errors give it; a screen words it through `tr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassifyError::Unreadable { side, why } => write!(f, "the {side} side cannot be read: {why}"),
            ClassifyError::Compare(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ClassifyError {}

/// Classifies and names what differs between `before` and `after`, sorted by id. `repair` is what
/// an opening does to a store, applied to a private copy of each side.
pub fn classified(before: &Rows, after: &Rows, repair: impl Fn(&mut Store)) -> Result<Vec<Difference>, ClassifyError> {
    let (tables, junctions) = storage();
    let (old, old_rows) = repaired("before", before, &tables, &junctions, &repair)?;
    let (new, new_rows) = repaired("after", after, &tables, &junctions, &repair)?;
    let found = compare(&old_rows, &new_rows, &tables, &junctions, MAX_COMPARED).map_err(ClassifyError::Compare)?;
    // By id and class: an item that moved to another table is removed there and added here.
    let mut kinds: BTreeMap<(Uuid, ClassId), ChangeKind> = BTreeMap::new();
    let mut came_or_went = BTreeSet::new();
    for (table, changes) in tables.iter().zip(&found.tables) {
        for change in changes {
            let (id, kind) = match change {
                RowChange::Added(id) => (*id, ChangeKind::Added),
                RowChange::Removed(id) => (*id, ChangeKind::Removed),
                RowChange::Changed { id, .. } => (*id, ChangeKind::Changed),
            };
            if kind != ChangeKind::Changed {
                came_or_went.insert(id);
            }
            kinds.insert((id, table.class), kind);
        }
    }
    for (list, changes) in junctions.iter().zip(&found.lists) {
        for change in changes {
            // A value that only came or went with its own row is already listed: not an owner's change too.
            let own = change.added.iter().chain(&change.removed).any(|v| !matches!(v, Cell::Id(id) if came_or_went.contains(id)));
            if change.reordered || own {
                kinds.entry((change.owner, list.class)).or_insert(ChangeKind::Changed);
            }
        }
    }
    let named = |((id, class), kind): ((Uuid, ClassId), ChangeKind)| {
        let (name, path) = name_and_path(if kind == ChangeKind::Removed { &old } else { &new }, id);
        let area = if class.metamodel() == archforge_diagram::INDEX { Area::Diagram } else { Area::Model };
        Difference { id, class, area, kind, name, path }
    };
    Ok(kinds.into_iter().map(named).collect())
}

/// `rows` restored into a store of their own and repaired, with the rows of the repaired store.
fn repaired(
    side: &'static str,
    rows: &Rows,
    tables: &[Table],
    junctions: &[Junction],
    repair: &impl Fn(&mut Store),
) -> Result<(Store, Rows), ClassifyError> {
    let unreadable = |why: String| ClassifyError::Unreadable { side, why };
    // The same ids each time, so an object a repair makes is the same object on both sides.
    let mut next = 0u128;
    let ids = Box::new(move || {
        next += 1;
        Uuid(next.to_be_bytes())
    });
    let mut restored = restore_into_empty(store(ids).map_err(unreadable)?, rows, tables, junctions).map_err(|e| unreadable(e.to_string()))?;
    repair(&mut restored);
    let rows = snapshot(&restored, tables, junctions).map_err(|e| unreadable(e.to_string()))?;
    Ok((restored, rows))
}

/// The name of `obj`, or of the element it shows; none when there is neither.
fn shown_name(store: &Store, obj: Obj) -> Option<String> {
    let own = archforge_core::name_of(store, obj);
    if !own.is_empty() {
        return Some(own);
    }
    let shown = [archforge_diagram::node::ELEMENT, archforge_diagram::edge::ELEMENT].iter().find_map(|p| store.get(obj, *p).ok()?.as_ref())?;
    Some(archforge_core::name_of(store, shown)).filter(|name| !name.is_empty())
}

/// The name of the item `id` and the names of what contains it, outermost first.
fn name_and_path(store: &Store, id: Uuid) -> (Option<String>, Vec<String>) {
    let Some(obj) = store.by_uuid(id) else { return (None, Vec::new()) };
    let mut path = Vec::new();
    let mut at = obj;
    // Restoring refused anything nested deeper, so this always reaches the top.
    for _ in 0..MAX_NESTING {
        let Ok(Some((up, _))) = store.container(at) else { break };
        path.extend(shown_name(store, up));
        at = up;
    }
    path.reverse();
    (shown_name(store, obj), path)
}
