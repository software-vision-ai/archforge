//! Packages through the engine: nested as deep as needed, names told apart among
//! siblings, classes and packages moved, a nested package removed handing its contents and
//! diagrams up to its parent (owner decision, for every notation), a top-level one taking them
//! along, and a class of another package drawn as an outsider; each one undo step; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Store, Uuid, Value};
use archforge_uml::{ALONE, registry};
use serde_json::{Value as Json, json};
type S = Session<Store, ProjectView>;
fn session() -> S {
    let mut n: u64 = 0;
    let ids = Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    });
    let universe = archforge_diagram::universe_with(vec![archforge_core::metamodel().unwrap(), archforge_uml::metamodel().unwrap()]).unwrap();
    let slabs = archforge_diagram::with_slabs(
        CompositeSlabs::new()
            .with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))
            .unwrap()
            .with(archforge_uml::INDEX, Box::new(archforge_uml::UmlSlabs::default()))
            .unwrap(),
    )
    .unwrap();
    let mut st = Store::new(universe, Box::new(slabs), ids);
    st.create(archforge_core::MODEL).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}
fn round_trip(s: &mut S, cmd: &str, p: Json) {
    let before = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    let after = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.undo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), before, "undo of {cmd}");
    s.redo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), after, "redo of {cmd}");
}
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

fn snapshot(s: &S) -> String {
    format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap())
}

fn refused(s: &mut S, cmd: &str, p: Json, words: &str) {
    let before = snapshot(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} is refused"));
    assert!(e.to_string().contains(words), "{cmd} {p}: {e}");
    assert_eq!(snapshot(s), before, "{cmd} {p} changed nothing");
}

fn obj(s: &S, id: &str) -> archforge_mof::Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn alive(s: &S, id: &str) -> bool {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| s.doc().is_alive(o))
}

/// The names of the classes a package holds.
fn class_names(s: &S, pkg: &str) -> Vec<String> {
    archforge_uml::commands::classes(s.doc(), obj(s, pkg)).into_iter().map(|c| archforge_core::name_of(s.doc(), c)).collect()
}

fn parent_of(s: &S, id: &str) -> Option<archforge_mof::Obj> {
    archforge_uml::commands::package_of(s.doc(), obj(s, id))
}

#[test]
fn packages_nest_their_names_are_told_apart_among_siblings_and_the_list_names_each_parent() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    round_trip(&mut s, "uml.package.add", json!({ "name": "orders", "parent": sales }));
    let orders = s.execute("uml.package.list", &json!({})).unwrap().value["packages"][1]["id"].as_str().unwrap().to_string();
    let deep = run(&mut s, "uml.package.add", json!({ "name": "lines", "parent": orders }));
    // The same name in another parent is fine; twice in one parent, whatever the case, is not.
    run(&mut s, "uml.package.add", json!({ "name": "orders" }));
    refused(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }), "named Orders already");
    refused(&mut s, "uml.package.add", json!({ "name": "x", "parent": "not-an-id" }), "parent");
    let listed = s.execute("uml.package.list", &json!({})).unwrap().value["packages"].as_array().unwrap().clone();
    let names: Vec<(&str, Option<&str>)> = listed.iter().map(|p| (p["name"].as_str().unwrap(), p["parent"].as_str())).collect();
    assert_eq!(names, [("sales", None), ("orders", Some(sales.as_str())), ("lines", Some(orders.as_str())), ("orders", None)]);
    assert_eq!(parent_of(&s, &deep), Some(obj(&s, &orders)));
}

#[test]
fn a_class_and_a_package_move_a_cycle_or_a_clash_is_refused_and_the_diagrams_keep_showing_them() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "sales" }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    round_trip(&mut s, "uml.class.move", json!({ "class": order, "package": billing }));
    assert_eq!(class_names(&s, &billing), ["Invoice", "Order"]);
    assert!(class_names(&s, &sales).is_empty());
    assert!(alive(&s, &node), "the diagram still shows it, now as an outsider");
    let card = archforge_uml::figures::card(s.doc(), obj(&s, &order), obj(&s, &d)).unwrap();
    assert!(card.elsewhere);
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Said("Class · {package}", vec![("package", "billing".into())]));
    // A class named like one already there is refused.
    let other = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "order" }));
    refused(&mut s, "uml.class.move", json!({ "class": other, "package": billing }), "named order already");
    // A package goes into another, or back to the top; into itself or what it holds is refused.
    round_trip(&mut s, "uml.package.move", json!({ "package": billing, "to": inner }));
    assert_eq!(parent_of(&s, &billing), Some(obj(&s, &inner)));
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": billing }), "into itself");
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": sales }), "into itself");
    round_trip(&mut s, "uml.package.move", json!({ "package": billing, "to": "" }));
    assert_eq!(parent_of(&s, &billing), None);
    run(&mut s, "uml.package.add", json!({ "name": "inner" }));
    refused(&mut s, "uml.package.move", json!({ "package": inner, "to": "" }), "named inner already");
}

#[test]
fn a_nested_package_removed_hands_its_contents_and_diagrams_up_and_a_top_level_one_takes_them_along() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "orders", "parent": sales }));
    let deep = run(&mut s, "uml.package.add", json!({ "name": "lines", "parent": orders }));
    let order = run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    let customer = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Customer" }));
    let a = run(&mut s, "uml.association.add", json!({ "from": customer, "to": order }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": orders, "name": "orders" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    // orders goes: Order, the package lines and the diagram move up into sales.
    round_trip(&mut s, "element.remove", json!({ "ids": [orders] }));
    assert!(!alive(&s, &orders));
    assert_eq!(class_names(&s, &sales), ["Customer", "Order"]);
    assert_eq!(parent_of(&s, &deep), Some(obj(&s, &sales)));
    assert!(alive(&s, &d) && alive(&s, &a), "the diagram and the association stay");
    let of = s.doc().get(obj(&s, &d), archforge_diagram::diagram::ELEMENT).unwrap().as_ref();
    assert_eq!(of, Some(obj(&s, &sales)), "the diagram is of the parent now");
    // A clash in the parent is refused, with nothing moved.
    let dup = run(&mut s, "uml.package.add", json!({ "name": "dup", "parent": sales }));
    run(&mut s, "uml.class.add", json!({ "package": dup, "name": "customer" }));
    refused(&mut s, "element.remove", json!({ "ids": [dup] }), "named customer already");
    // A top-level package takes everything along, the relationships of its classes too.
    let other = run(&mut s, "uml.package.add", json!({ "name": "other" }));
    let outside = run(&mut s, "uml.class.add", json!({ "package": other, "name": "Outside" }));
    let dep = run(&mut s, "uml.dependency.add", json!({ "client": outside, "supplier": customer }));
    round_trip(&mut s, "element.remove", json!({ "ids": [sales] }));
    for gone in [&sales, &customer, &order, &deep, &a, &dep, &d] {
        assert!(!alive(&s, gone), "{gone} went with sales");
    }
    assert!(alive(&s, &outside));
}

#[test]
fn the_sweep_finds_no_command_that_breaks_the_store_with_packages_nested() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": inner, "name": "Order" }));
    let calls = sweep_with(&mut s, &[sales.as_str(), inner.as_str(), order.as_str(), "not-an-id"], |st| {
        format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())
    })
    .unwrap();
    assert!(calls > 100, "{calls}");
    let _ = Value::Null;
}
