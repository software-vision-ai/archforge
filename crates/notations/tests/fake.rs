//! The fake notation's rows survive a save and a reopening: a model of every kind its commands make,
//! written to its tables and read back into a store of its own, gives the same rows and the same
//! elements.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use archforge_mof::{Store, Uuid, Value, rows};
use archforge_notations::fake::NOTATIONS;
use serde_json::{Value as Json, json};

/// Fresh ids from `from` on.
fn ids(from: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = from;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// The id `cmd` answers with.
fn run(s: &mut archforge_notations::Session, cmd: &str, p: Json) -> String {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd}: {e}")).value["id"].as_str().unwrap_or_default().to_string()
}

/// Every element of `st`, by its id, with its class and its name.
fn elements(st: &Store) -> Vec<(Uuid, String, String)> {
    let mut out: Vec<_> = st
        .alive()
        .map(|o| (st.uuid(o).unwrap(), st.universe().class(st.class_of(o).unwrap()).unwrap().name.to_string(), archforge_core::name_of(st, o)))
        .collect();
    out.sort();
    out
}

/// A package of classes with attributes, an operation, a generalization, an association with a
/// role and a description, and a sketch diagram showing the classes and their lines, written to the
/// fake notation's tables and read back, gives the same rows, the same elements and the same lists.
#[test]
fn the_fake_notations_rows_survive_a_save_and_a_reopening() {
    let mut s = NOTATIONS.new_project_session("Shop", ids(0)).unwrap();
    let shop = run(&mut s, "sketch.package.add", json!({ "name": "shop" }));
    let customer = run(&mut s, "sketch.class.add", json!({ "package": shop, "name": "Customer" }));
    let order = run(&mut s, "sketch.class.add", json!({ "package": shop, "name": "Order" }));
    let party = run(&mut s, "sketch.class.add", json!({ "package": shop, "name": "Party" }));
    run(&mut s, "sketch.attribute.add", json!({ "class": customer, "name": "id" }));
    run(&mut s, "sketch.attribute.add", json!({ "class": customer, "name": "email" }));
    run(&mut s, "sketch.operation.add", json!({ "class": order, "name": "total" }));
    run(&mut s, "sketch.inherit.add", json!({ "specific": customer, "general": party }));
    let places = run(&mut s, "sketch.association.add", json!({ "from": customer, "to": order, "name": "places", "from_role": "buyer" }));
    run(&mut s, "element.describe", json!({ "element": customer, "text": "People who buy." }));
    let d = run(&mut s, "diagram.add", json!({ "kind": archforge_notations::fake::KIND, "element": shop, "name": "Shop" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": customer, "x": 0, "y": 0 }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 400, "y": 0 }));
    run(&mut s, "diagram.connect-all", json!({ "diagram": d }));
    let (tables, junctions) = NOTATIONS.storage();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = NOTATIONS.store(ids(1 << 40)).unwrap();
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the same rows");
    assert_eq!(elements(&back), elements(s.doc()), "the same elements, classes and names");
    let list = |st: &Store, id: &str, prop| {
        let o = st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
        st.list(o, prop).unwrap().iter().filter_map(Value::as_ref).map(|o| st.uuid(o).unwrap()).collect::<Vec<_>>()
    };
    for (id, prop) in [
        (&customer, archforge_mof::toy::CLASS_OWNED_ATTRIBUTE),
        (&order, archforge_mof::toy::CLASS_OWNED_OPERATION),
        (&customer, archforge_mof::toy::CLASS_GENERALIZATION),
        (&places, archforge_mof::toy::ASSOCIATION_OWNED_END),
        (&shop, archforge_mof::toy::PACKAGE_PACKAGED_ELEMENT),
    ] {
        assert_eq!(list(&back, id, prop), list(s.doc(), id, prop), "{prop:?} in its order");
    }
    let reopened = NOTATIONS.session_from_rows(&saved, ids(1 << 41)).unwrap();
    let c = reopened.doc().by_uuid(Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(reopened.view().of_element(reopened.doc(), c).map(|n| n.description(reopened.doc(), c)).as_deref(), Some("People who buy."));
    assert!(!reopened.is_dirty(), "a model reopened is saved");
}
