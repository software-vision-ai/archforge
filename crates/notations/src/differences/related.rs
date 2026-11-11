//! What a compared element is connected to: what it or anything inside it refers to, what refers
//! to them, and its owner. What it contains is left out, as its own card lists that.

use std::collections::BTreeSet;

use archforge_mof::{Invalid, Obj, Store, Uuid, Value};

use super::ClassifyError;

/// What the element `id` is connected to on each of `sides`, each once, in the order of the sides;
/// a side it is not on adds nothing. A reference that cannot be read refuses the side, naming it.
pub(super) fn connected_to(id: Uuid, sides: [(&'static str, &Store); 2]) -> Result<Vec<Uuid>, ClassifyError> {
    let (mut seen, mut all) = (BTreeSet::new(), Vec::new());
    for (side, store) in sides {
        let Some(obj) = store.by_uuid(id) else { continue };
        let found = on_side(store, obj).map_err(|e| ClassifyError::Unreadable { side, why: e.to_string() })?;
        all.extend(found.into_iter().filter(|other| seen.insert(*other)));
    }
    Ok(all)
}

/// The ids of what `root` is connected to in `store`, in the store's own order.
fn on_side(store: &Store, root: Obj) -> Result<Vec<Uuid>, Invalid> {
    let inside = within(store, root)?;
    let mut linked: BTreeSet<Obj> = store.container(root)?.map(|(owner, _)| owner).into_iter().collect();
    for obj in &inside {
        linked.extend(store.referrers(*obj).into_iter().map(|(from, _)| from));
        linked.extend(refers_to(store, *obj)?);
    }
    // What has no id any more can't be in a list of changes, so it needs no link.
    Ok(linked.difference(&inside).filter_map(|obj| store.uuid(*obj).ok()).collect())
}

/// `root` and all it contains, at any depth. The set also ends a loop in what was read.
fn within(store: &Store, root: Obj) -> Result<BTreeSet<Obj>, Invalid> {
    let (mut inside, mut todo) = (BTreeSet::from([root]), vec![root]);
    while let Some(obj) = todo.pop() {
        for child in store.owned_elements(obj)? {
            if inside.insert(child) {
                todo.push(child);
            }
        }
    }
    Ok(inside)
}

/// What the stored properties of `obj` point at.
fn refers_to(store: &Store, obj: Obj) -> Result<Vec<Obj>, Invalid> {
    let class = store.class_of(obj)?;
    let universe = store.universe();
    let info = universe.info(class).ok_or_else(|| Invalid::Internal(format!("there is no class {}", class.0)))?;
    let mut targets = Vec::new();
    for id in &info.stored {
        let Some(prop) = universe.prop(*id) else { continue };
        let values = if prop.is_many() { store.list(obj, *id)? } else { vec![store.get(obj, *id)?] };
        targets.extend(values.iter().filter_map(Value::as_ref));
    }
    Ok(targets)
}
