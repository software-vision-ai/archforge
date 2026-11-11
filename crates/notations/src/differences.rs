//! What differs between two states of a project, classified and named for a person to follow: in
//! the model or a diagram, added, changed or removed, with the names and path of the item. Each
//! side is restored into a store of its own and repaired as an opening repairs it, so a box an
//! opening fits shows no false change. One selected element's two sides are read the same way, as
//! typed cards with their fields aligned and its members and owner listed, for a
//! view to word later; one diagram's two sides are kept as stores of their own, for a view to draw.
//! Nothing here writes to the project.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::Arc;

use archforge_diagram::{Grid, LABEL, POINT, STYLE, diagram, diagram_of, edge, node, node_tree};
use archforge_mof::compare::{CompareError, MAX_COMPARED, RowChange, compare};
use archforge_mof::rows::{Cell, MAX_NESTING, Rows, restore_into_empty, snapshot};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{ClassId, Invalid, Obj, PropId, Property, Store, TypeRef, Uuid, Value};

use crate::{labels, storage, store};

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
    /// The item's name, or what it joins or states when it has none, or for a box or line in a
    /// diagram that of the element it shows; none when it has nothing to go by. A removed item's comes from the before side, any other's from the after side.
    pub name: Option<String>,
    /// The names of what contains the item, outermost first, from the same side as its name.
    pub path: Vec<String>,
}

/// What an element is and is called on one side of a comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementCard {
    pub class: ClassId,
    /// Its name on this side, what it joins or states when it has none, or for a box or line that
    /// of the element it shows; none when it has nothing to go by.
    pub name: Option<String>,
    /// Whether `name` is made from what it joins or states rather than being a name of its own.
    pub made: bool,
}

impl ElementCard {
    /// Whether the element was renamed between `other` and this card. Words made from what it joins
    /// or states follow those, so they differ without any renaming.
    pub fn renamed_from(&self, other: &ElementCard) -> bool {
        !(self.made && other.made) && self.name != other.name
    }
}

/// One property of a compared element, in the same place on both cards.
#[derive(Debug, Clone, PartialEq)]
pub struct CardField {
    pub prop: PropId,
    /// The value before: none when that side has no such property, `Some(Value::Null)` when it has
    /// one left empty.
    pub before: Option<Value>,
    /// The value after, read as `before` is.
    pub after: Option<Value>,
    /// Whether the element is on both sides and this field differs between them, a property only
    /// one side's class has included. An added or removed element has nothing to differ from.
    pub changed: bool,
}

/// One element's two sides, found by its id, so a rename or move stays one element. A side is none
/// when the element is not there: added elements have no `before`, removed ones no `after`.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementSides {
    pub id: Uuid,
    pub before: Option<ElementCard>,
    pub after: Option<ElementCard>,
    /// The stored, single-valued, data-typed properties of its class in the metamodel's order, id
    /// and name left out; when the classes differ, the after side's own ones follow.
    pub fields: Vec<CardField>,
    /// Each side's members and owner, shared so that a copy of the sides stays cheap however long a list is.
    pub lists: Arc<CardLists>,
}

/// One list of a side, in the order a view shows them (frames V1.1 and V3.3 of page 09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListKind {
    Attributes,
    /// A table's columns, which are an ER notation's attributes.
    Columns,
    Operations,
    Literals,
    Members,
    Ownership,
}

/// What a row names, typed so a view words it in the language shown.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Named {
    Said(String),
    /// Words made from what a nameless element joins or states, which follow those: a rename of
    /// what it joins leaves its own row as it was.
    Made(String),
    Unnamed,
    /// There is no owner to name.
    Nothing,
    /// A reference set on this side whose element this side has no way of showing.
    Unavailable,
}

/// One row of a list on one side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    /// The member or owner the row stands for; the element itself when no owner can be named.
    pub id: Uuid,
    pub class: ClassId,
    /// Its notation text, or its name.
    pub name: Named,
    /// How it differs from the other side's row, when the element is on both.
    pub change: Option<ChangeKind>,
}

/// The rows of one kind on one side, in model order, kept whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardList {
    pub kind: ListKind,
    pub rows: Vec<ListRow>,
}

/// The lists of each side. A side the element is not on has none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardLists {
    pub before: Vec<CardList>,
    pub after: Vec<CardList>,
    /// How many members differ, counted once when read, so a summary doesn't walk the lists each frame.
    pub member_changes: usize,
}

/// One side of a compared diagram: a store of its own, so names and places are what they were there.
pub struct DiagramSide {
    pub store: Store,
    pub diagram: Obj,
    pub grid: Grid,
}

/// One diagram's two sides, found by its id. A side is none when the diagram is not there.
pub struct DiagramSides {
    pub id: Uuid,
    /// Its name and the names of what contains it, from the after side when it is there.
    pub name: Option<String>,
    pub path: Vec<String>,
    pub before: Option<DiagramSide>,
    pub after: Option<DiagramSide>,
    /// What happened to each box and line of this diagram that differs, by its id: Added or Removed
    /// when it is on one side only, Changed when it or what it draws differs: its element, or a member listed on it.
    pub marks: BTreeMap<Uuid, ChangeKind>,
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

/// What happened to each item that differs between two states' rows, by id and table.
fn kinds_between(old: &Rows, new: &Rows, tables: &[Table], junctions: &[Junction]) -> Result<BTreeMap<(Uuid, ClassId), ChangeKind>, ClassifyError> {
    let found = compare(old, new, tables, junctions, MAX_COMPARED).map_err(ClassifyError::Compare)?;
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
    Ok(kinds)
}

/// Classifies and names what differs between `before` and `after`, sorted by id. `repair` is what
/// an opening does to a store, applied to a private copy of each side.
pub fn classified(before: &Rows, after: &Rows, repair: impl Fn(&mut Store)) -> Result<Vec<Difference>, ClassifyError> {
    let (tables, junctions) = storage();
    let (old, old_rows) = repaired("before", before, &tables, &junctions, &repair)?;
    let (new, new_rows) = repaired("after", after, &tables, &junctions, &repair)?;
    let kinds = kinds_between(&old_rows, &new_rows, &tables, &junctions)?;
    let named = |((id, class), kind): ((Uuid, ClassId), ChangeKind)| {
        let (name, path) = name_and_path(if kind == ChangeKind::Removed { &old } else { &new }, id);
        let area = if class.metamodel() == archforge_diagram::INDEX { Area::Diagram } else { Area::Model };
        Difference { id, class, area, kind, name, path }
    };
    Ok(kinds.into_iter().map(named).collect())
}

/// The two sides of the element `id` between `before` and `after`, each restored and repaired as
/// [`classified`] does it, so a card and the list read the same states. Built for one selected
/// element from rows the caller holds; `Ok(None)` when neither side has the element.
pub fn element_sides(before: &Rows, after: &Rows, id: Uuid, repair: impl Fn(&mut Store)) -> Result<Option<ElementSides>, ClassifyError> {
    let (tables, junctions) = storage();
    let (old, old_rows) = repaired("before", before, &tables, &junctions, &repair)?;
    let (new, new_rows) = repaired("after", after, &tables, &junctions, &repair)?;
    let (card_before, was) = read_side("before", &old, id)?.unzip();
    let (card_after, is) = read_side("after", &new, id)?.unzip();
    if card_before.is_none() && card_after.is_none() {
        return Ok(None);
    }
    let on_both = card_before.is_some() && card_after.is_some();
    let ((was, mut before_lists), (is, mut after_lists)) = (was.unwrap_or_default(), is.unwrap_or_default());
    if on_both {
        let mut changed: BTreeSet<Uuid> = kinds_between(&old_rows, &new_rows, &tables, &junctions)?.into_keys().map(|(id, _)| id).collect();
        // A part's row is its own, so a change there, such as a constraint's expression, is its holder's too.
        let held: Vec<Uuid> = changed.iter().filter_map(|id| new.container(new.by_uuid(*id)?).ok()?.and_then(|(up, _)| new.uuid(up).ok())).collect();
        changed.extend(held);
        marked(&mut before_lists, &after_lists, ChangeKind::Removed, &changed);
        marked(&mut after_lists, &before_lists, ChangeKind::Added, &changed);
    }
    let marks = |of: &[CardList], only: Option<ChangeKind>| {
        of.iter()
            .filter(|l| l.kind != ListKind::Ownership)
            .flat_map(|l| &l.rows)
            .filter(|r| r.change.is_some() && (only.is_none() || r.change == only))
            .count()
    };
    // A changed row is on both sides, so it counts once, on the before side.
    let member_changes = marks(&before_lists, None) + marks(&after_lists, Some(ChangeKind::Added));
    let lists = Arc::new(CardLists { before: before_lists, after: after_lists, member_changes });
    Ok(Some(ElementSides { id, before: card_before, after: card_after, fields: aligned(&was, &is, on_both), lists }))
}

/// The two sides of the diagram that holds the item `id`, a box or a line of it or the diagram
/// itself, each restored and repaired as [`classified`] does it. `Ok(None)` when neither side has one.
pub fn diagram_sides(before: &Rows, after: &Rows, id: Uuid, repair: impl Fn(&mut Store)) -> Result<Option<DiagramSides>, ClassifyError> {
    let (tables, junctions) = storage();
    let (old, old_rows) = repaired("before", before, &tables, &junctions, &repair)?;
    let (new, new_rows) = repaired("after", after, &tables, &junctions, &repair)?;
    let holder = |store: &Store| store.by_uuid(id).and_then(|o| diagram_of(store, o)).and_then(|d| store.uuid(d).ok());
    let Some(diagram) = holder(&new).or_else(|| holder(&old)) else { return Ok(None) };
    let changed: BTreeSet<Uuid> = kinds_between(&old_rows, &new_rows, &tables, &junctions)?.into_keys().map(|(id, _)| id).collect();
    let (name, path) = name_and_path(if new.by_uuid(diagram).is_some() { &new } else { &old }, diagram);
    let side = |store: Store| store.by_uuid(diagram).map(|d| DiagramSide { grid: Grid::build(&store, d), diagram: d, store });
    let (before, after) = (side(old), side(new));
    let marks = marks_of(&changed, [("before", before.as_ref()), ("after", after.as_ref())])?;
    Ok(Some(DiagramSides { id: diagram, name, path, before, after, marks }))
}

/// What each box and line of `sides` is marked with. One on a single side was added or removed
/// there, whether or not its own row differs, as a box can move between diagrams. One on both is
/// Changed when something it draws differs on either, as a member gone is only on the side that still has it.
fn marks_of(changed: &BTreeSet<Uuid>, sides: [(&'static str, Option<&DiagramSide>); 2]) -> Result<BTreeMap<Uuid, ChangeKind>, ClassifyError> {
    let items = |(name, side): (&'static str, Option<&DiagramSide>)| {
        side.map_or(Ok(BTreeMap::new()), |s| items_of(s, changed)).map_err(|e| ClassifyError::Unreadable { side: name, why: e.to_string() })
    };
    let [was, is] = sides.map(items);
    let (was, is) = (was?, is?);
    let mut marks: BTreeMap<Uuid, ChangeKind> = is.keys().filter(|id| !was.contains_key(id)).map(|id| (*id, ChangeKind::Added)).collect();
    for (id, differs) in &was {
        match is.get(id) {
            None => drop(marks.insert(*id, ChangeKind::Removed)),
            Some(now) if *differs || *now => drop(marks.insert(*id, ChangeKind::Changed)),
            Some(_) => {}
        }
    }
    Ok(marks)
}

/// Every box of `side`, nested ones too, and every line, with whether it draws something in `changed`.
fn items_of(side: &DiagramSide, changed: &BTreeSet<Uuid>) -> Result<BTreeMap<Uuid, bool>, Invalid> {
    let store = &side.store;
    let refs = |list| Ok::<_, Invalid>(store.list(side.diagram, list)?.iter().filter_map(Value::as_ref).collect::<Vec<Obj>>());
    let boxes = refs(diagram::NODE)?.into_iter().flat_map(|top| node_tree(store, top)).map(|n| (n, node::ELEMENT));
    let mut found = BTreeMap::new();
    for (item, shows) in boxes.chain(refs(diagram::EDGE)?.into_iter().map(|e| (e, edge::ELEMENT))) {
        let element = store.get(item, shows)?.as_ref().filter(|e| store.is_alive(*e));
        let differs =
            draws_a_change(store, item, changed, MAX_NESTING)? || element.map_or(Ok(false), |e| draws_a_change(store, e, changed, MAX_NESTING))?;
        found.insert(store.uuid(item)?, differs);
    }
    Ok(found)
}

/// Whether `obj` or a part drawn with it differs, within `depth` levels.
fn draws_a_change(store: &Store, obj: Obj, changed: &BTreeSet<Uuid>, depth: usize) -> Result<bool, Invalid> {
    if changed.contains(&store.uuid(obj)?) {
        return Ok(true);
    }
    if depth > 0 {
        for part in store.owned_elements(obj)? {
            let class = store.class_of(part)?;
            if ([POINT, LABEL, STYLE].contains(&class) || is_feature(class)) && draws_a_change(store, part, changed, depth - 1)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether `class` is listed on its owner's box.
fn is_feature(class: ClassId) -> bool {
    class == archforge_uml::PARAMETER || kind_of(class) != ListKind::Members
}

/// Whether `prop` belongs on a card: one stored data value, not the element's id or name. The
/// words a card shows are made for these same properties.
pub fn on_a_card(prop: &Property) -> bool {
    prop.is_stored() && !prop.is_many() && matches!(prop.ty, TypeRef::Data(_)) && !prop.is_id && !prop.is_name
}

/// The card, field values and lists of the element `id` in `store`; none when it is not there. A
/// value that cannot be read refuses the side, naming it.
fn read_side(side: &'static str, store: &Store, id: Uuid) -> Result<Option<(ElementCard, (Vec<(PropId, Value)>, Vec<CardList>))>, ClassifyError> {
    let unreadable = |e: Invalid| ClassifyError::Unreadable { side, why: e.to_string() };
    let Some(obj) = store.by_uuid(id) else { return Ok(None) };
    let class = store.class_of(obj).map_err(unreadable)?;
    let universe = store.universe();
    let info = universe.info(class).ok_or_else(|| unreadable(Invalid::Internal(format!("there is no class {}", class.0))))?;
    let mut values = Vec::new();
    for prop in info.stored.iter().filter(|p| universe.prop(**p).is_some_and(on_a_card)) {
        values.push((*prop, store.get(obj, *prop).map_err(unreadable)?));
    }
    let (name, made) = resolved_name(store, obj).map_or((None, false), |(name, made)| (Some(name), made));
    Ok(Some((ElementCard { class, name, made }, (values, lists_of(store, obj).map_err(unreadable)?))))
}

/// Marks each row of `side` against the rows of `other`: `gone` when `other` has no such row,
/// Changed when it says something else, or for words made from what it joins when it or a part of it
/// is in `changed`. A row is one member by its id, an ER enum's literal by its text; a
/// card's owner is one row whatever the owner's id, so a move reads as a changed owner and a renamed owner doesn't.
fn marked(side: &mut [CardList], other: &[CardList], gone: ChangeKind, changed: &BTreeSet<Uuid>) {
    let key = |kind: ListKind, row: &ListRow| {
        let text = (kind == ListKind::Literals && row.class == archforge_er::ENUM_TYPE).then(|| row.name.clone());
        (kind, if kind == ListKind::Ownership { None } else { Some(row.id) }, text)
    };
    let differ = |kind: ListKind, a: &ListRow, b: &ListRow| match (&a.name, &b.name) {
        _ if kind == ListKind::Ownership => a.id != b.id,
        (Named::Made(_), Named::Made(_)) => changed.contains(&a.id),
        _ => (a.id, a.class, &a.name) != (b.id, b.class, &b.name),
    };
    let held: HashMap<_, _> = other.iter().flat_map(|l| l.rows.iter().map(move |r| (key(l.kind, r), r))).collect();
    for list in side {
        for row in &mut list.rows {
            row.change = match held.get(&key(list.kind, row)) {
                None => Some(gone),
                Some(theirs) if differ(list.kind, theirs, row) => Some(ChangeKind::Changed),
                Some(_) => None,
            };
        }
    }
}

/// The lists of `obj`: its own members and its owner, read once, off the interface thread, so a
/// view only ever draws what is held. An association's end the element owns is a member like any other.
fn lists_of(store: &Store, obj: Obj) -> Result<Vec<CardList>, Invalid> {
    let mut lists: BTreeMap<ListKind, Vec<ListRow>> = BTreeMap::new();
    for member in store.owned_elements(obj)? {
        let class = store.class_of(member)?;
        lists.entry(kind_of(class)).or_default().push(ListRow { name: member_name(store, member, class), ..plain_row(store, member, class) });
    }
    if store.class_of(obj)? == archforge_er::ENUM_TYPE {
        let literal = |text: String| ListRow { name: Named::Said(text), ..plain_row(store, obj, archforge_er::ENUM_TYPE) };
        lists.entry(ListKind::Literals).or_default().extend(archforge_er::rules::literals_of(store, obj).into_iter().map(literal));
    }
    lists.entry(ListKind::Ownership).or_default().push(owner_row(store, obj));
    Ok(lists.into_iter().map(|(kind, rows)| CardList { kind, rows }).collect())
}

/// A row for `obj` naming it, unmarked.
fn plain_row(store: &Store, obj: Obj, class: ClassId) -> ListRow {
    ListRow { id: store.uuid(obj).unwrap_or(Uuid([0; 16])), class, name: named(store, obj), change: None }
}

/// `obj`'s name, the words made for it, or that it has none.
fn named(store: &Store, obj: Obj) -> Named {
    match labels::unnamed(store, obj).filter(|_| archforge_core::name_of(store, obj).is_empty()) {
        Some(words) => Named::Made(words),
        None => shown_name(store, obj).map_or(Named::Unnamed, Named::Said),
    }
}

/// The row of the owner of `obj`; an element with no owner says so, and one whose owner cannot be read says that.
fn owner_row(store: &Store, obj: Obj) -> ListRow {
    let class = store.class_of(obj).unwrap_or(archforge_core::ELEMENT);
    match store.container(obj) {
        Ok(Some((up, _))) if store.is_alive(up) => plain_row(store, up, store.class_of(up).unwrap_or(class)),
        Ok(None) => ListRow { name: Named::Nothing, ..plain_row(store, obj, class) },
        _ => ListRow { name: Named::Unavailable, ..plain_row(store, obj, class) },
    }
}

/// The list a member of `class` belongs to.
fn kind_of(class: ClassId) -> ListKind {
    match class {
        c if c == archforge_uml::PROPERTY => ListKind::Attributes,
        c if c == archforge_er::ATTRIBUTE => ListKind::Columns,
        c if c == archforge_uml::OPERATION => ListKind::Operations,
        c if c == archforge_uml::ENUMERATION_LITERAL => ListKind::Literals,
        _ => ListKind::Members,
    }
}

/// What a member's row writes: a feature as its element box lists it, a column with its type, anything else what [`named`] gives.
fn member_name(store: &Store, m: Obj, class: ClassId) -> Named {
    match class {
        c if c == archforge_uml::PROPERTY || c == archforge_uml::OPERATION => Named::Said(archforge_uml::figures::member_text(store, m)),
        c if c == archforge_er::ATTRIBUTE => {
            let (name, ty) = (archforge_core::name_of(store, m), archforge_er::rules::type_display(store, m));
            Named::Said(if ty.is_empty() { name } else { format!("{name} : {ty}") })
        }
        _ => named(store, m),
    }
}

/// The fields of both sides lined up by property, `was`'s order first; `on_both` says whether the
/// element is on both sides, so a difference can mean a change.
fn aligned(was: &[(PropId, Value)], is: &[(PropId, Value)], on_both: bool) -> Vec<CardField> {
    let value = |side: &[(PropId, Value)], prop: PropId| side.iter().find(|(p, _)| *p == prop).map(|(_, v)| v.clone());
    let only_after = is.iter().map(|(p, _)| *p).filter(|p| value(was, *p).is_none());
    let props = was.iter().map(|(p, _)| *p).chain(only_after);
    let field = |prop| {
        let (before, after) = (value(was, prop), value(is, prop));
        CardField { prop, changed: on_both && before != after, before, after }
    };
    props.map(field).collect()
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

/// The name of `obj` or the words made for it, or those of the element it shows; none when there are neither.
fn shown_name(store: &Store, obj: Obj) -> Option<String> {
    resolved_name(store, obj).map(|(name, _)| name)
}

/// What [`shown_name`] finds, with whether those are words made rather than a name of its own.
fn resolved_name(store: &Store, obj: Obj) -> Option<(String, bool)> {
    let called = |o: Obj| {
        let own = Some(archforge_core::name_of(store, o)).filter(|name| !name.is_empty()).map(|name| (name, false));
        own.or_else(|| labels::unnamed(store, o).map(|words| (words, true)))
    };
    called(obj).or_else(|| {
        called([archforge_diagram::node::ELEMENT, archforge_diagram::edge::ELEMENT].iter().find_map(|p| store.get(obj, *p).ok()?.as_ref())?)
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A member read that fails is an error, never an empty list that would read as no members.
    #[test]
    fn a_failed_member_read_is_not_an_empty_list() {
        let empty = store(Box::new(|| Uuid([1; 16]))).unwrap();
        assert!(matches!(lists_of(&empty, Obj(0)), Err(Invalid::NoSuchObject(_) | Invalid::Dead(_))));
    }
}
