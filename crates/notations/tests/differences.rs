//! Two states of a project classified and named, built with the real commands.

use archforge_mof::rows::{self, Cell, Rows};
use archforge_mof::{ClassId, DataKind, PropId, Store, TypeRef, Uuid, Value};
use archforge_notations::differences::{
    Area, CardField, CardList, ChangeKind, ClassifyError, DiagramSide, DiagramSides, Difference, ElementCard, ElementSides, ListKind, Named,
    classified, diagram_sides, element_sides,
};
use archforge_notations::{Session, new_project_session, storage, universe};
use serde_json::{Value as Json, json};

/// Ids that never repeat within a test.
fn ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = 0u64;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// Runs a command and gives the id it made.
fn run(s: &mut Session, command: &str, params: Json) -> String {
    let value = s.execute(command, &params).unwrap().value;
    value["id"].as_str().unwrap_or_default().to_string()
}

/// The rows of the session now.
fn state(s: &Session) -> Rows {
    let (tables, junctions) = storage();
    rows::snapshot(s.doc(), &tables, &junctions).unwrap()
}

/// A project named Commerce holding a schema shop with a table customer and its column id.
fn project() -> (Session, String, String, String) {
    let mut s = new_project_session("Commerce", ids()).unwrap();
    let shop = run(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let customer = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "customer" }));
    let id = run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT", "primary_key": true }));
    (s, shop, customer, id)
}

/// What differs, with no repair.
fn differences(before: &Rows, after: &Rows) -> Vec<Difference> {
    classified(before, after, |_| {}).unwrap()
}

/// The one difference about the item `id`.
fn about<'a>(found: &'a [Difference], id: &str) -> &'a Difference {
    let of: Vec<&Difference> = found.iter().filter(|d| d.id.to_string() == id).collect();
    assert_eq!(of.len(), 1, "{id} in {found:?}");
    of[0]
}

/// The kind, name and path of a difference, to compare at once.
fn told(d: &Difference) -> (ChangeKind, Option<&str>, Vec<&str>) {
    (d.kind, d.name.as_deref(), d.path.iter().map(String::as_str).collect())
}

/// A removed item is named from the side where it still exists, its removed parents included.
#[test]
fn a_removed_item_keeps_its_name_and_the_path_of_removed_parents() {
    let (mut s, shop, customer, id) = project();
    let before = state(&s);
    run(&mut s, "element.remove", json!({ "ids": [shop] }));
    let found = differences(&before, &state(&s));
    assert_eq!(told(about(&found, &shop)), (ChangeKind::Removed, Some("shop"), vec!["Commerce"]));
    assert_eq!(told(about(&found, &customer)), (ChangeKind::Removed, Some("customer"), vec!["Commerce", "shop"]));
    assert_eq!(told(about(&found, &id)), (ChangeKind::Removed, Some("id"), vec!["Commerce", "shop", "customer"]));
}

/// With its owner renamed in the same change, a removed item keeps the old path and an added one has the new.
#[test]
fn an_owner_renamed_in_the_same_change_shows_on_each_side_as_it_was() {
    let (mut s, shop, customer, _) = project();
    let before = state(&s);
    run(&mut s, "er.schema.rename", json!({ "schema": shop, "name": "billing" }));
    run(&mut s, "element.remove", json!({ "ids": [customer] }));
    let invoice = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "invoice" }));
    let found = differences(&before, &state(&s));
    assert_eq!(told(about(&found, &customer)), (ChangeKind::Removed, Some("customer"), vec!["Commerce", "shop"]));
    assert_eq!(told(about(&found, &shop)), (ChangeKind::Changed, Some("billing"), vec!["Commerce"]));
    assert_eq!(told(about(&found, &invoice)), (ChangeKind::Added, Some("invoice"), vec!["Commerce", "billing"]));
}

/// The table decides the area: an element named like a diagram is model, a diagram named like a table is not.
#[test]
fn the_area_comes_from_the_table_and_never_from_a_name() {
    let (mut s, shop, customer, _) = project();
    let before = state(&s);
    let lookalike = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "diagram_node" }));
    let diagram = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop, "name": "customer" }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 0, "y": 0 }));
    let found = differences(&before, &state(&s));
    assert_eq!(about(&found, &lookalike).area, Area::Model);
    assert_eq!(about(&found, &diagram).area, Area::Diagram);
    assert_eq!(told(about(&found, &node)).1, Some("customer"), "a box is named by what it shows");
    assert_eq!(about(&found, &node).area, Area::Diagram);
}

/// A child that came is listed once, not also as a change to its owner; a real order change is the owner's.
#[test]
fn an_owner_is_changed_only_by_what_its_list_really_did() {
    let (mut s, _, customer, _) = project();
    let before = state(&s);
    run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "TEXT" }));
    let after = state(&s);
    let found = differences(&before, &after);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(told(&found[0]), (ChangeKind::Added, Some("email"), vec!["Commerce", "shop", "customer"]));
    let mut swapped = after.clone();
    let owner = Uuid::parse(&customer).unwrap();
    let list = swapped.junctions.iter_mut().find(|j| j.iter().filter(|r| r.0 == owner && matches!(r.2, Cell::Id(_))).count() == 2).unwrap();
    let positions: Vec<u32> = list.iter().filter(|r| r.0 == owner).map(|r| r.1).rev().collect();
    list.iter_mut().filter(|r| r.0 == owner).zip(positions).for_each(|(r, at)| r.1 = at);
    let reordered = differences(&after, &swapped);
    assert_eq!(told(about(&reordered, &customer)).0, ChangeKind::Changed);
    // A list that came with its owner leaves the owner added, not changed.
    let (mut s, shop, ..) = project();
    let before = state(&s);
    let states = run(&mut s, "er.enum.add", json!({ "schema": shop, "name": "state", "literals": ["open", "paid"] }));
    assert_eq!(about(&differences(&before, &state(&s)), &states).kind, ChangeKind::Added);
}

/// What an opening repairs shows no change, and a stored layout the repair touches stays as it was.
#[test]
fn the_same_repair_on_both_sides_shows_no_false_layout_change() {
    let (mut s, shop, customer, _) = project();
    let diagram = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 0, "y": 0, "width": 100, "height": 40 }));
    let before = state(&s);
    let stored = before.clone();
    // The after side is as an opening left it: the box fitted to its columns.
    run(&mut s, "diagram.resize", json!({ "node": node, "width": 100, "height": 200 }));
    let after = state(&s);
    let fit = |st: &mut Store| {
        let nodes: Vec<_> = st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE)).collect();
        for n in nodes {
            st.set(n, archforge_diagram::node::HEIGHT, Value::Real(200.0)).unwrap();
        }
    };
    let without = differences(&before, &after);
    assert_eq!((without.len(), about(&without, &node).kind, about(&without, &node).area), (1, ChangeKind::Changed, Area::Diagram));
    assert_eq!(classified(&before, &after, fit).unwrap(), []);
    assert_eq!(classified(&after, &before, fit).unwrap(), [], "the repair is on the after side too");
    assert_eq!(before, stored, "nothing was written to the rows given");
}

/// Rows that cannot be restored are refused, naming the side.
#[test]
fn a_side_that_cannot_be_read_is_named() {
    let good = state(&project().0);
    let refused: Result<Vec<Difference>, Box<dyn std::error::Error>> = (|| Ok(classified(&Rows::default(), &good, |_| {})?))();
    assert_eq!(refused.unwrap_err().to_string(), "the before side cannot be read: the rows do not match the project's tables");
    assert!(matches!(classified(&good, &Rows::default(), |_| {}), Err(ClassifyError::Unreadable { side: "after", .. })));
}

/// The two sides of the element `id`, with no repair.
fn sides(before: &Rows, after: &Rows, id: &str) -> ElementSides {
    element_sides(before, after, Uuid::parse(id).unwrap(), |_| {}).unwrap().unwrap()
}

/// The field of `prop` on the two cards.
fn field(seen: &ElementSides, prop: PropId) -> &CardField {
    seen.fields.iter().find(|f| f.prop == prop).unwrap()
}

/// A card to expect.
fn card(class: ClassId, name: &str) -> Option<ElementCard> {
    Some(ElementCard { class, name: Some(name.to_string()) })
}

/// The name of an enumeration value of `prop`, through the metamodel the view will word it from.
fn literal(prop: PropId, value: &Option<Value>) -> String {
    let all = universe().unwrap();
    let (Some(TypeRef::Data(data)), Some(Value::Enum(at))) = (all.prop(prop).map(|p| p.ty), value) else { panic!("{value:?} is no literal") };
    let DataKind::Enumeration(literals) = &all.data_type(data).unwrap().kind else { panic!("{data:?} is no enumeration") };
    literals[*at as usize].to_string()
}

/// A project with a UML package shop holding a class Order and its private attribute total.
fn uml_project() -> (Session, String, String, String) {
    let mut s = new_project_session("Commerce", ids()).unwrap();
    let shop = run(&mut s, "uml.package.add", json!({ "name": "shop" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": shop, "name": "Order" }));
    let total = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total", "visibility": "private" }));
    (s, shop, order, total)
}

/// An ER sequence edited: its typed values on both sides, and marks on only what differs.
#[test]
fn an_er_element_marks_only_the_fields_that_differ() {
    use archforge_er::sequence::{CYCLE, INCREMENT, MAXIMUM, MINIMUM, START};
    let (mut s, shop, ..) = project();
    let orders = run(&mut s, "er.sequence.add", json!({ "schema": shop, "name": "orders", "start": 5, "increment": 7 }));
    let states = run(&mut s, "er.enum.add", json!({ "schema": shop, "name": "state", "literals": ["open", "paid"] }));
    let before = state(&s);
    run(&mut s, "er.sequence.set", json!({ "sequence": orders, "name": "orders", "start": 20, "increment": 7, "cycle": true, "minimum": "3" }));
    let seen = sides(&before, &state(&s), &orders);
    assert_eq!((seen.before.clone(), seen.after.clone()), (card(archforge_er::SEQUENCE, "orders"), card(archforge_er::SEQUENCE, "orders")));
    assert_eq!(
        seen.fields.iter().map(|f| f.prop).collect::<Vec<_>>(),
        [archforge_er::element::DESCRIPTION, START, INCREMENT, MINIMUM, MAXIMUM, CYCLE],
        "the metamodel's order, no name, no reference"
    );
    assert!(sides(&before, &before, &states).fields.iter().all(|f| f.prop != archforge_er::enum_type::LITERAL), "a list is left to the members");
    let told = |prop| (field(&seen, prop).before.clone(), field(&seen, prop).after.clone(), field(&seen, prop).changed);
    assert_eq!(told(START), (Some(Value::Int(5)), Some(Value::Int(20)), true));
    assert_eq!(told(INCREMENT), (Some(Value::Int(7)), Some(Value::Int(7)), false));
    assert_eq!(told(MINIMUM), (Some(Value::Null), Some(Value::str("3")), true));
    assert_eq!(told(MAXIMUM), (Some(Value::Null), Some(Value::Null), false));
    assert_eq!(told(CYCLE), (Some(Value::Bool(false)), Some(Value::Bool(true)), true));
}

/// UML through the same reader: an enumeration stays a typed literal, and only data properties reach a card.
#[test]
fn a_uml_element_is_read_from_its_own_metamodel() {
    use archforge_uml::{class, named_element};
    let (mut s, _, order, total) = uml_project();
    let before = state(&s);
    run(&mut s, "uml.attribute.set", json!({ "attribute": total, "visibility": "public" }));
    run(&mut s, "uml.class.set", json!({ "class": order, "abstract": true }));
    let after = state(&s);
    let attribute = sides(&before, &after, &total);
    let shown = field(&attribute, named_element::VISIBILITY);
    assert_eq!((literal(shown.prop, &shown.before), literal(shown.prop, &shown.after), shown.changed), ("private".into(), "public".into(), true));
    assert_eq!(attribute.fields.iter().filter(|f| f.changed).count(), 1, "{attribute:?}");
    let owner = sides(&before, &after, &order);
    assert_eq!(field(&owner, class::IS_ABSTRACT).after, Some(Value::Bool(true)));
    let all = universe().unwrap();
    for f in owner.fields.iter().chain(&attribute.fields) {
        let p = all.prop(f.prop).unwrap();
        assert!(matches!(p.ty, TypeRef::Data(_)) && !p.is_many() && !p.is_name && p.is_stored(), "{}", all.prop_name(f.prop));
    }
}

/// An added or removed element has an explicit absent side, told apart from a value left empty.
#[test]
fn an_added_or_removed_element_has_an_absent_side_and_no_marks() {
    let (mut s, _, customer, id) = project();
    let before = state(&s);
    let email = run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "TEXT" }));
    run(&mut s, "element.remove", json!({ "ids": [id] }));
    let after = state(&s);
    let added = sides(&before, &after, &email);
    assert_eq!((added.before.clone(), added.after.clone()), (None, card(archforge_er::ATTRIBUTE, "email")));
    assert!(!added.fields.is_empty() && added.fields.iter().all(|f| f.before.is_none() && f.after.is_some() && !f.changed), "{added:?}");
    assert_eq!(field(&added, archforge_er::attribute::DEFAULT).after, Some(Value::Null), "an empty value is a value");
    let removed = sides(&before, &after, &id);
    assert_eq!((removed.before.clone(), removed.after.clone()), (card(archforge_er::ATTRIBUTE, "id"), None));
    assert!(!removed.fields.is_empty() && removed.fields.iter().all(|f| f.before.is_some() && f.after.is_none() && !f.changed), "{removed:?}");
    let (mut s, _, order, _) = uml_project();
    let before = state(&s);
    run(&mut s, "element.remove", json!({ "ids": [order] }));
    let gone = sides(&before, &state(&s), &order);
    assert_eq!((gone.before, gone.after), (card(archforge_uml::CLASS, "Order"), None));
}

/// A rename or a move is one element on both sides with no field marked; so is nothing at all.
#[test]
fn a_renamed_moved_or_unchanged_element_stays_one_element() {
    let (mut s, _, customer, _) = project();
    let before = state(&s);
    run(&mut s, "er.entity.rename", json!({ "entity": customer, "name": "client" }));
    let renamed = sides(&before, &state(&s), &customer);
    assert_eq!(
        (renamed.id.to_string(), renamed.before.clone(), renamed.after.clone()),
        (customer.clone(), card(archforge_er::ENTITY, "customer"), card(archforge_er::ENTITY, "client"))
    );
    assert!(!renamed.fields.is_empty() && renamed.fields.iter().all(|f| !f.changed && f.before == f.after), "{renamed:?}");
    let unchanged = sides(&before, &before, &customer);
    assert_eq!(unchanged.before, unchanged.after);
    assert!(!unchanged.fields.is_empty() && unchanged.fields.iter().all(|f| !f.changed && f.before.is_some() && f.before == f.after));
    let (mut s, _, order, _) = uml_project();
    let warehouse = run(&mut s, "uml.package.add", json!({ "name": "warehouse" }));
    let before = state(&s);
    run(&mut s, "uml.class.move", json!({ "class": order, "package": warehouse }));
    let moved = sides(&before, &state(&s), &order);
    assert_eq!((moved.before, moved.after.clone()), (moved.after, card(archforge_uml::CLASS, "Order")));
    assert!(moved.fields.iter().all(|f| !f.changed));
}

/// Each list on a side as its kind and rows, a row as its name and mark.
fn lists(of: &[CardList]) -> Vec<(ListKind, Vec<(Named, Option<ChangeKind>)>)> {
    of.iter().map(|l| (l.kind, l.rows.iter().map(|r| (r.name.clone(), r.change)).collect())).collect()
}

/// The name a row of `text` holds.
fn said(text: &str) -> Named {
    Named::Said(text.to_string())
}

/// A class's members group by kind; an added, a removed and a changed one are marked on the sides that have them.
#[test]
fn members_are_grouped_and_marked_on_their_own_side() {
    let (mut s, shop, order, total) = uml_project();
    let cut = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "cut" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": shop, "name": "Status" }));
    run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "open" }));
    let before = state(&s);
    run(&mut s, "uml.attribute.set", json!({ "attribute": total, "visibility": "public" }));
    run(&mut s, "uml.operation.add", json!({ "class": order, "name": "pay" }));
    run(&mut s, "element.remove", json!({ "ids": [cut] }));
    let seen = sides(&before, &state(&s), &order);
    let (was, is) = (lists(&seen.lists.before), lists(&seen.lists.after));
    assert_eq!(was.iter().map(|l| l.0).collect::<Vec<_>>(), [ListKind::Attributes, ListKind::Ownership], "{was:?}");
    assert_eq!(is.iter().map(|l| l.0).collect::<Vec<_>>(), [ListKind::Attributes, ListKind::Operations, ListKind::Ownership], "{is:?}");
    assert_eq!(was[0].1[..2], [(said("- total"), Some(ChangeKind::Changed)), (said("+ cut"), Some(ChangeKind::Removed))], "{was:?}");
    assert_eq!(is[0].1[0], (said("+ total"), Some(ChangeKind::Changed)), "{is:?}");
    assert_eq!(is[1].1, [(said("+ pay()"), Some(ChangeKind::Added))], "added, on the after side only");
    assert_eq!(seen.lists.member_changes, 3, "the changed one once, the owner not at all");
    assert_eq!(lists(&sides(&before, &before, &status).lists.after)[0], (ListKind::Literals, vec![(said("open"), None)]));
    assert_eq!(was.last().unwrap().1, [(said("shop"), None)], "the same owner is plain");
}

/// A move to another owner changes its one row on both sides, even one of the same name; an element
/// with no owner says so, and a column writes its type. An element on one side has no marks.
#[test]
fn the_owner_is_one_row_and_a_side_alone_has_no_marks() {
    let (mut s, _, order, _) = uml_project();
    let outer = run(&mut s, "uml.package.add", json!({ "name": "outer" }));
    let twin = run(&mut s, "uml.package.add", json!({ "name": "shop", "parent": outer }));
    let before = state(&s);
    run(&mut s, "uml.class.move", json!({ "class": order, "package": twin }));
    let at_twin = state(&s);
    let moved = sides(&before, &at_twin, &order);
    let owner = |of: &[CardList]| lists(of).last().unwrap().1.clone();
    assert_eq!(moved.lists.member_changes, 0, "an owner is not a member");
    assert_eq!(
        (owner(&moved.lists.before), owner(&moved.lists.after)),
        (vec![(said("shop"), Some(ChangeKind::Changed))], vec![(said("shop"), Some(ChangeKind::Changed))])
    );
    run(&mut s, "uml.package.rename", json!({ "package": twin, "name": "store" }));
    let renamed = sides(&at_twin, &state(&s), &order);
    assert_eq!(
        (owner(&renamed.lists.before), owner(&renamed.lists.after)),
        (vec![(said("shop"), None)], vec![(said("store"), None)]),
        "the same owner"
    );
    let mut at = sides(&before, &before, &order).lists.after.last().unwrap().rows[0].clone();
    for _ in 0..8 {
        if at.name != Named::Nothing {
            at = sides(&before, &before, &at.id.to_string()).lists.after.last().unwrap().rows[0].clone();
        }
    }
    assert_eq!(at.name, Named::Nothing, "the root says it has no owner");

    let (mut s, _, customer, _) = project();
    let before = state(&s);
    let email = run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "TEXT" }));
    let after = state(&s);
    let columns = lists(&sides(&before, &after, &customer).lists.after);
    assert_eq!(columns[0].1, [(said("id : BIGINT"), None), (said("email : TEXT"), Some(ChangeKind::Added))], "{columns:?}");
    assert!(lists(&sides(&before, &after, &email).lists.after).iter().all(|l| l.1.iter().all(|r| r.1.is_none())), "only on the after side");
}

/// A box a repair fits shows no false change on either side, and is named by what it shows.
#[test]
fn a_card_shows_each_side_as_an_opening_repairs_it() {
    use archforge_diagram::node::HEIGHT;
    let (mut s, shop, customer, _) = project();
    let diagram = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 0, "y": 0, "width": 100, "height": 40 }));
    let before = state(&s);
    run(&mut s, "diagram.resize", json!({ "node": node, "width": 100, "height": 200 }));
    let after = state(&s);
    let raw = sides(&before, &after, &node);
    assert_eq!((field(&raw, HEIGHT).before.clone(), field(&raw, HEIGHT).changed), (Some(Value::Real(40.0)), true));
    assert_eq!(raw.after.and_then(|c| c.name).as_deref(), Some("customer"));
    let fit = |st: &mut Store| {
        let nodes: Vec<_> = st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE)).collect();
        for n in nodes {
            st.set(n, HEIGHT, Value::Real(200.0)).unwrap();
        }
    };
    let id = Uuid::parse(&node).unwrap();
    for (one, other) in [(&before, &after), (&after, &before)] {
        let seen = element_sides(one, other, id, fit).unwrap().unwrap();
        assert_eq!((field(&seen, HEIGHT).before.clone(), field(&seen, HEIGHT).changed), (Some(Value::Real(200.0)), false));
    }
}

/// An id that moves to another class keeps one card pair: shared fields line up, the rest fall on one side.
#[test]
fn fields_line_up_by_property_when_the_class_differs() {
    use archforge_uml::{class, classifier, packageable_element};
    let (mut s, shop, order, _) = uml_project();
    let port = run(&mut s, "uml.interface.add", json!({ "package": shop, "name": "Port" }));
    let before = state(&s);
    run(&mut s, "element.remove", json!({ "ids": [order] }));
    let (mut after, from, to) = (state(&s), Uuid::parse(&port).unwrap(), Uuid::parse(&order).unwrap());
    after.tables.iter_mut().flatten().flatten().filter(|c| **c == Cell::Id(from)).for_each(|c| *c = Cell::Id(to));
    for (owner, _, value) in after.junctions.iter_mut().flatten() {
        *owner = if *owner == from { to } else { *owner };
        *value = if *value == Cell::Id(from) { Cell::Id(to) } else { value.clone() };
    }
    let seen = sides(&before, &after, &order);
    assert_eq!((seen.before.clone(), seen.after.clone()), (card(archforge_uml::CLASS, "Order"), card(archforge_uml::INTERFACE, "Port")));
    let shared = field(&seen, packageable_element::VISIBILITY);
    assert!(shared.before.is_some() && shared.before == shared.after && !shared.changed, "{shared:?}");
    let only_before = field(&seen, class::IS_ABSTRACT);
    assert!(only_before.before.is_some() && only_before.after.is_none() && only_before.changed, "{only_before:?}");
    let at = |prop| seen.fields.iter().position(|f| f.prop == prop).unwrap();
    let only_after = field(&seen, classifier::IS_ABSTRACT);
    assert!(only_after.before.is_none() && only_after.after.is_some() && only_after.changed, "{only_after:?}");
    assert!(at(class::IS_ABSTRACT) < at(classifier::IS_ABSTRACT), "the after side's own ones follow");
}

/// An id on neither side is told as such; a side that cannot be restored is refused and named.
#[test]
fn an_unknown_element_is_none_and_an_unreadable_side_is_named() {
    let (s, _, customer, _) = project();
    let good = state(&s);
    let id = Uuid::parse(&customer).unwrap();
    assert_eq!(element_sides(&good, &good, Uuid([7; 16]), |_| {}), Ok(None));
    assert!(matches!(element_sides(&Rows::default(), &good, id, |_| {}), Err(ClassifyError::Unreadable { side: "before", .. })));
    assert!(matches!(element_sides(&good, &Rows::default(), id, |_| {}), Err(ClassifyError::Unreadable { side: "after", .. })));
}

/// A project with a diagram of its schema, the tables customer and invoice placed on it.
struct Placed {
    s: Session,
    shop: String,
    customer: String,
    invoice: String,
    diagram: String,
    first: String,
    second: String,
}

/// The project every diagram comparison starts from.
fn placed() -> Placed {
    let (mut s, shop, customer, _) = project();
    let invoice = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "invoice" }));
    let diagram = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop, "name": "Billing" }));
    let first = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 0, "y": 0 }));
    let second = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": invoice, "x": 400, "y": 0 }));
    Placed { s, shop, customer, invoice, diagram, first, second }
}

/// The two sides of the diagram holding `id`, with no repair.
fn drawn(before: &Rows, after: &Rows, id: &str) -> DiagramSides {
    diagram_sides(before, after, Uuid::parse(id).unwrap(), |_| {}).unwrap().unwrap()
}

/// What `seen` marks the item `id` with.
fn mark(seen: &DiagramSides, id: &str) -> Option<ChangeKind> {
    seen.marks.get(&Uuid::parse(id).unwrap()).copied()
}

/// A box, or the diagram itself, finds the diagram; each side holds its own state, and a model
/// element or an unknown id holds no diagram.
#[test]
fn each_side_of_a_diagram_is_its_own_state() {
    /// Compiles only for a type a reading thread can hand to the interface thread.
    fn moves_between_threads<T: Send>() {}
    moves_between_threads::<DiagramSides>();
    let Placed { mut s, customer, diagram, first, .. } = placed();
    let before = state(&s);
    run(&mut s, "er.entity.rename", json!({ "entity": customer, "name": "client" }));
    let after = state(&s);
    let name_on = |side: &DiagramSide| archforge_core::name_of(&side.store, side.store.by_uuid(Uuid::parse(&customer).unwrap()).unwrap());
    for id in [&first, &diagram] {
        let seen = drawn(&before, &after, id);
        assert_eq!((seen.id.to_string(), seen.name.as_deref()), (diagram.clone(), Some("Billing")));
        assert_eq!(seen.path, ["Commerce"]);
        assert_eq!((name_on(seen.before.as_ref().unwrap()), name_on(seen.after.as_ref().unwrap())), ("customer".into(), "client".into()));
        assert_eq!(seen.after.as_ref().unwrap().grid.len().0, 2, "both boxes");
    }
    assert!(diagram_sides(&before, &after, Uuid::parse(&customer).unwrap(), |_| {}).unwrap().is_none(), "a model element");
    assert!(diagram_sides(&before, &after, Uuid([7; 16]), |_| {}).unwrap().is_none(), "an unknown id");
}

/// A diagram only one side has leaves the other absent, named from the side that has it.
#[test]
fn a_diagram_on_one_side_has_an_absent_side() {
    let Placed { mut s, diagram, .. } = placed();
    let with = state(&s);
    run(&mut s, "diagram.remove", json!({ "diagram": diagram }));
    let without = state(&s);
    let gone = drawn(&with, &without, &diagram);
    assert!(gone.before.is_some() && gone.after.is_none());
    assert_eq!(gone.name.as_deref(), Some("Billing"));
    let came = drawn(&without, &with, &diagram);
    assert!(came.before.is_none() && came.after.is_some());
    assert_eq!(came.name.as_deref(), Some("Billing"));
}

/// A box is marked with its own change, or Changed for what it draws; one nothing touched has no mark.
#[test]
fn boxes_are_marked_by_their_own_change_and_by_what_they_show() {
    let Placed { mut s, diagram, first, second, .. } = placed();
    let before = state(&s);
    run(&mut s, "diagram.move", json!({ "nodes": [first], "dx": 40, "dy": 40 }));
    let seen = drawn(&before, &state(&s), &diagram);
    assert_eq!((mark(&seen, &first), mark(&seen, &second)), (Some(ChangeKind::Changed), None), "moved, and left alone");

    let Placed { mut s, shop, customer, diagram, .. } = placed();
    let other = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop, "name": "Other" }));
    let elsewhere = run(&mut s, "diagram.place", json!({ "diagram": other, "element": customer, "x": 0, "y": 0 }));
    let before = state(&s);
    run(&mut s, "diagram.move", json!({ "nodes": [elsewhere], "dx": 40, "dy": 40 }));
    assert_eq!(drawn(&before, &state(&s), &diagram).marks, Default::default(), "a box on another diagram");

    let Placed { mut s, customer, diagram, first, second, .. } = placed();
    let before = state(&s);
    run(&mut s, "er.entity.rename", json!({ "entity": customer, "name": "client" }));
    let seen = drawn(&before, &state(&s), &diagram);
    assert_eq!((mark(&seen, &first), mark(&seen, &second)), (Some(ChangeKind::Changed), None), "what it shows renamed");

    let Placed { mut s, diagram, first, second, .. } = placed();
    let before = state(&s);
    run(&mut s, "diagram.unplace", json!({ "nodes": [first] }));
    let after = state(&s);
    let seen = drawn(&before, &after, &diagram);
    assert_eq!((mark(&seen, &first), mark(&seen, &second)), (Some(ChangeKind::Removed), None));
    assert_eq!(mark(&drawn(&after, &before, &diagram), &first), Some(ChangeKind::Added));
}

/// A member is on one side only, and marks the box that lists it whichever side that is. A table
/// beside it in the schema is drawn by no box, so it marks none.
#[test]
fn a_member_on_one_side_marks_the_box_that_lists_it() {
    let Placed { mut s, customer, diagram, first, second, .. } = placed();
    let before = state(&s);
    run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "TEXT" }));
    let after = state(&s);
    for (one, other) in [(&before, &after), (&after, &before)] {
        let seen = drawn(one, other, &diagram);
        assert_eq!((mark(&seen, &first), mark(&seen, &second)), (Some(ChangeKind::Changed), None));
    }
    let Placed { mut s, shop, diagram, .. } = placed();
    let before = state(&s);
    run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "unplaced" }));
    assert_eq!(drawn(&before, &state(&s), &diagram).marks, Default::default(), "a neighbour no box draws");
}

/// A parameter is written on its operation's line, so it marks the box; a generalization the class
/// owns isn't written on the box, so with no line for it nothing is marked.
#[test]
fn only_what_a_box_lists_marks_it() {
    let (mut s, shop, order, _) = uml_project();
    let item = run(&mut s, "uml.class.add", json!({ "package": shop, "name": "Item" }));
    let pay = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "pay", "parameters": [{ "name": "sum" }] }));
    let diagram = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": shop, "name": "Orders" }));
    let card = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": order, "x": 0, "y": 0 }));
    let before = state(&s);
    run(&mut s, "uml.generalization.add", json!({ "specific": order, "general": item }));
    let related = state(&s);
    let found = differences(&before, &related);
    assert!(found.iter().any(|d| d.class == archforge_uml::GENERALIZATION), "the class owns something new: {found:?}");
    assert_eq!(mark(&drawn(&before, &related, &diagram), &card), None);
    run(&mut s, "uml.operation.set", json!({ "operation": pay, "name": "pay", "parameters": [{ "name": "amount" }] }));
    assert_eq!(mark(&drawn(&related, &state(&s), &diagram), &card), Some(ChangeKind::Changed));
}

/// A line is marked like a box: its own arrival, and a bend in its route, which is a part of it.
#[test]
fn lines_are_marked_like_boxes() {
    let Placed { mut s, customer, invoice, diagram, first, second, .. } = placed();
    let relationship = run(&mut s, "er.relationship.add", json!({ "from": customer, "to": invoice }));
    let before = state(&s);
    let line = run(&mut s, "diagram.connect", json!({ "diagram": diagram, "element": relationship, "source": first, "target": second }));
    let connected = state(&s);
    assert_eq!(mark(&drawn(&before, &connected, &diagram), &line), Some(ChangeKind::Added));
    run(&mut s, "diagram.route", json!({ "edge": line, "points": [{ "x": 200, "y": 300 }] }));
    let seen = drawn(&connected, &state(&s), &diagram);
    assert_eq!((mark(&seen, &line), mark(&seen, &first)), (Some(ChangeKind::Changed), None), "the bend is the line's");
    run(&mut s, "diagram.style", json!({ "ids": [line], "fill": "#112233" }));
    let styled = state(&s);
    run(&mut s, "diagram.style", json!({ "ids": [line], "fill": "#445566" }));
    assert_eq!(mark(&drawn(&styled, &state(&s), &diagram), &line), Some(ChangeKind::Changed), "its look is a part of it");
    let restyled = state(&s);
    run(&mut s, "diagram.label", json!({ "edge": line, "kind": "name", "dx": 9 }));
    assert_eq!(mark(&drawn(&restyled, &state(&s), &diagram), &line), Some(ChangeKind::Changed), "so is its label");
}

/// What an opening repairs marks nothing, on either side.
#[test]
fn a_repair_marks_nothing() {
    let Placed { mut s, diagram, first, .. } = placed();
    let before = state(&s);
    run(&mut s, "diagram.resize", json!({ "node": first, "width": 100, "height": 200 }));
    let after = state(&s);
    let fit = |st: &mut Store| {
        let nodes: Vec<_> = st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE)).collect();
        for n in nodes {
            st.set(n, archforge_diagram::node::HEIGHT, Value::Real(200.0)).unwrap();
            st.set(n, archforge_diagram::node::WIDTH, Value::Real(100.0)).unwrap();
        }
    };
    assert_eq!(mark(&drawn(&before, &after, &diagram), &first), Some(ChangeKind::Changed), "without the repair it differs");
    let seen = diagram_sides(&before, &after, Uuid::parse(&diagram).unwrap(), fit).unwrap().unwrap();
    assert_eq!(seen.marks, Default::default());
}
