//! Two states of a project classified and named, built with the real commands.

use archforge_mof::rows::{self, Cell, Rows};
use archforge_mof::{Store, Uuid, Value};
use archforge_notations::differences::{Area, ChangeKind, ClassifyError, Difference, classified};
use archforge_notations::{Session, new_project_session, storage};
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
