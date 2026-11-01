//! A class's members through the engine (archforge-3c5.1.3): attributes and operations added,
//! changed, moved, listed and removed, each one undo step; built-in types made in the Types package
//! the first time one is used and reused after; multiplicities read or refused; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Store, Uuid, Value};
use archforge_uml::members::parse_multiplicity;
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

fn get(s: &mut S, class: &str) -> Json {
    s.execute("uml.class.get", &json!({ "class": class })).unwrap().value
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

#[test]
fn multiplicities_read_as_typed_and_the_rest_are_refused() {
    assert_eq!(parse_multiplicity(""), Ok(None));
    assert_eq!(parse_multiplicity("1"), Ok(Some((1, Some(1)))));
    assert_eq!(parse_multiplicity("0..1"), Ok(Some((0, Some(1)))));
    assert_eq!(parse_multiplicity("*"), Ok(Some((0, None))));
    assert_eq!(parse_multiplicity(" 1..* "), Ok(Some((1, None))));
    assert_eq!(parse_multiplicity("2..5"), Ok(Some((2, Some(5)))));
    for bad in ["5..2", "*..1", "0", "0..0", "a", "1..", "-1", "1..2..3", "99999999999"] {
        assert!(parse_multiplicity(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn attributes_and_operations_are_added_changed_moved_and_removed_as_one_step_each() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let money = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Money" }));
    // A built-in type makes the Types package once, then is reused.
    let id = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "id", "type": "Integer", "visibility": "public" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "integer" }));
    let listed = s.execute("uml.package.list", &json!({})).unwrap().value;
    let names: Vec<&str> = listed["packages"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["sales", "Types"], "the Types package, made once");
    let types = s.execute("uml.type.list", &json!({})).unwrap().value;
    assert_eq!(types["types"].as_array().unwrap().iter().filter(|t| t["name"] == "Integer").count(), 1, "Integer made once");
    // A model class as a type, a multiplicity, at a place.
    round_trip(
        &mut s,
        "uml.attribute.add",
        json!({ "class": order, "name": "total", "type": money, "multiplicity": "0..1", "visibility": "private", "at": 0 }),
    );
    let c = get(&mut s, &order);
    let attrs: Vec<&str> = c["attributes"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(attrs, ["total", "id", "count"]);
    assert_eq!(c["attributes"][0]["type"]["name"], "Money");
    assert_eq!(c["attributes"][0]["multiplicity"], "0..1");
    assert_eq!(c["attributes"][0]["visibility"], "private");
    // Changed, moved, static.
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": id, "name": "number", "type": "String", "multiplicity": "1..*", "static": true }));
    round_trip(&mut s, "uml.attribute.move", json!({ "attribute": id, "at": 2 }));
    let c = get(&mut s, &order);
    assert_eq!(c["attributes"][2]["name"], "number");
    assert_eq!(c["attributes"][2]["static"], true);
    assert_eq!(c["attributes"][2]["multiplicity"], "1..*");
    // An operation with parameters and a return type, then abstract.
    round_trip(
        &mut s,
        "uml.operation.add",
        json!({ "class": order, "name": "total", "visibility": "public", "parameters": [{ "name": "currency", "type": "String" }, { "name": "rounding", "type": "Integer", "multiplicity": "0..1" }], "returns": money }),
    );
    let op = get(&mut s, &order)["operations"][0]["id"].as_str().unwrap().to_string();
    round_trip(&mut s, "uml.operation.set", json!({ "operation": op, "name": "total", "abstract": true, "parameters": [], "returns": "Real" }));
    let c = get(&mut s, &order);
    assert_eq!(c["operations"][0]["abstract"], true);
    assert_eq!(c["operations"][0]["parameters"].as_array().unwrap().len(), 0);
    assert_eq!(c["operations"][0]["returns"]["name"], "Real");
    round_trip(&mut s, "uml.class.set", json!({ "class": order, "abstract": true }));
    assert_eq!(get(&mut s, &order)["abstract"], true);
    // Removed through element.remove; a type that goes leaves what had it untyped.
    round_trip(&mut s, "element.remove", json!({ "ids": [op] }));
    round_trip(&mut s, "element.remove", json!({ "ids": [money] }));
    assert_eq!(get(&mut s, &order)["attributes"][0]["type"], Json::Null);
    // Refusals name their key and change nothing.
    for (cmd, p, key) in [
        ("uml.attribute.add", json!({ "class": order, "name": "count" }), "name"),
        ("uml.attribute.add", json!({ "class": order, "name": "x", "type": "Decimal" }), "type"),
        ("uml.attribute.add", json!({ "class": order, "name": "x", "type": pkg }), "type"),
        ("uml.attribute.add", json!({ "class": order, "name": "x", "multiplicity": "3..1" }), "multiplicity"),
        ("uml.attribute.add", json!({ "class": order, "name": "x", "visibility": "secret" }), "visibility"),
        ("uml.attribute.add", json!({ "class": pkg, "name": "x" }), "class"),
        ("uml.operation.add", json!({ "class": order, "name": "f", "parameters": [{ "name": "a" }, { "name": "A" }] }), "parameters"),
        ("uml.operation.add", json!({ "class": order, "name": "f", "parameters": [{ "name": "a", "multiplicity": "x" }] }), "parameters"),
        ("uml.operation.add", json!({ "class": order, "name": "" }), "name"),
    ] {
        let before = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
        let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p}"));
        assert!(format!("{e:?}").contains(key), "{cmd}: {e:?}");
        assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), before, "{cmd} changed nothing");
    }
    let calls = sweep_with(&mut s, &[pkg.as_str(), order.as_str(), id.as_str(), "not-an-id"], |st| {
        format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())
    })
    .unwrap();
    assert!(calls > 100, "{calls}");
    let _ = Value::Null;
}
