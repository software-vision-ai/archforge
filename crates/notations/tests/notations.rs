//! The project composed from its notations: every notation's metamodel, slabs and tables in one
//! store, a model of two notations rebuilt from its rows, and a second notation's diagrams,
//! removals and descriptions routed to it through the one project view.

use archforge_diagram::{Notation, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{CommandError, Options};
use archforge_mof::rows;
use archforge_mof::{ClassId, Obj, Store, Uuid, Value};
use archforge_notations::{PARTS, Session, new_project_session, registry, session_from_rows, storage, store, universe};
use serde_json::{Value as Json, json};

fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

#[test]
fn every_notation_joins_the_universe_and_the_tables_once() {
    let u = universe().unwrap();
    for n in PARTS {
        let m = (n.metamodel)().unwrap();
        assert!(u.metamodels().any(|held| held.uri == m.uri), "{} is in the universe", n.name);
    }
    let (tables, junctions) = storage();
    assert_eq!(tables.len(), PARTS.iter().map(|n| n.tables.len()).sum::<usize>());
    assert_eq!(junctions.len(), PARTS.iter().map(|n| n.junctions.len()).sum::<usize>());
    let mut names = std::collections::BTreeSet::new();
    assert!(tables.iter().map(|t| t.name).chain(junctions.iter().map(|j| j.name)).all(|n| names.insert(n)), "every table once");
}

#[test]
fn a_model_of_er_and_uml_comes_back_from_its_rows() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let shop = s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap();
    assert!(shop.value["id"].is_string());
    let (tables, junctions) = storage();
    let mut written = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    // A UML class in a package, a member of the same Model, beside the ER schema.
    let mut st = store(ids(1 << 40)).unwrap();
    rows::restore(&mut st, &written, &tables, &junctions).unwrap();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_uml::PackageObj::set_name(&mut st, pkg, Some("sales")).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(pkg)).unwrap();
    let class = st.create(archforge_uml::CLASS).unwrap();
    archforge_uml::ClassObj::set_name(&mut st, class, Some("Customer")).unwrap();
    st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(class)).unwrap();
    written = rows::snapshot(&st, &tables, &junctions).unwrap();
    let back = session_from_rows(&written, ids(1 << 41)).unwrap();
    assert_eq!(rows::snapshot(back.doc(), &tables, &junctions).unwrap(), written);
    assert_eq!(
        archforge_core::name_of(back.doc(), back.doc().alive().find(|o| back.doc().class_of(*o).ok() == Some(archforge_uml::CLASS)).unwrap()),
        "Customer"
    );
}

/// A second notation, for the test alone: diagrams of a UML package that show its classes and
/// their generalizations; removing a class takes it and its views, and describing one names it.
struct Sketch;

const KIND: &str = "test.sketch";

impl Notation for Sketch {
    fn name(&self) -> &'static str {
        "Sketch"
    }
    fn metamodel(&self) -> u16 {
        archforge_uml::INDEX
    }
    fn kinds(&self) -> &'static [&'static str] {
        &[KIND]
    }
    fn is_of(&self, _: &str, class: ClassId) -> bool {
        class == archforge_uml::PACKAGE
    }
    fn shows(&self, _: &str, class: ClassId) -> bool {
        class == archforge_uml::CLASS
    }
    fn connects(&self, _: &str, class: ClassId) -> bool {
        class == archforge_uml::GENERALIZATION
    }
    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        let specific = store.container(element).ok()??.0;
        let general = store.get(element, archforge_uml::generalization::GENERAL).ok()?.as_ref()?;
        Some((specific, general))
    }
    fn default_size(&self, _: ClassId) -> (f64, f64) {
        (200.0, 60.0)
    }
    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<usize, CommandError> {
        let mut gone = 0;
        for o in objs {
            gone += archforge_diagram::remove_views_of(store, *o).map_err(|e| CommandError::new(e.to_string()))?;
            gone += store.destroy(*o).map_err(|e| CommandError::new(e.to_string()))?;
        }
        Ok(gone)
    }
    fn describe(&self, store: &mut Store, obj: Obj, text: &str) -> Result<(), CommandError> {
        store.set(obj, archforge_uml::named_element::NAME, Value::str(text)).map_err(|e| CommandError::new(e.to_string()))
    }
}

/// A project of ER and the sketch notation.
static BOTH: &[&dyn Notation] = &[&archforge_er::ErNotation, &Sketch];

fn id(s: &mut Session, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

fn uuid(st: &Store, o: Obj) -> String {
    st.uuid(o).unwrap().to_string()
}

#[test]
fn a_second_notations_diagrams_removals_and_descriptions_route_to_it() {
    let mut st = store(ids(0)).unwrap();
    let model = st.create(archforge_core::MODEL).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(pkg)).unwrap();
    let class = |st: &mut Store, name: &str| {
        let c = st.create(archforge_uml::CLASS).unwrap();
        archforge_uml::ClassObj::set_name(st, c, Some(name)).unwrap();
        st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        c
    };
    let (party, customer) = (class(&mut st, "Party"), class(&mut st, "Customer"));
    let g = st.create(archforge_uml::GENERALIZATION).unwrap();
    st.set(g, archforge_uml::generalization::GENERAL, Value::Ref(party)).unwrap();
    st.push(customer, archforge_uml::classifier::GENERALIZATION, Value::Ref(g)).unwrap();
    let (pkg, party, customer, g) = (uuid(&st, pkg), uuid(&st, party), uuid(&st, customer), uuid(&st, g));
    let mut s = Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(BOTH));

    // The sketch's diagrams: of a package, showing classes and the generalization between them.
    let d = id(&mut s, "diagram.add", json!({ "kind": KIND, "element": pkg, "name": "Parties" }));
    let a = id(&mut s, "diagram.place", json!({ "diagram": d, "element": party, "x": 0, "y": 0 }));
    let b = id(&mut s, "diagram.place", json!({ "diagram": d, "element": customer, "x": 0, "y": 200 }));
    id(&mut s, "diagram.connect", json!({ "diagram": d, "element": g, "source": b, "target": a }));
    let node = s.execute("diagram.get", &json!({ "diagram": d })).unwrap().value;
    assert_eq!(node["nodes"][0]["width"], 200.0, "the sketch's own size");
    // ER's diagrams beside them, each kind kept to its own notation's elements.
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let table = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "customer" }));
    let er = id(&mut s, "diagram.add", json!({ "kind": "er", "element": schema }));
    id(&mut s, "diagram.place", json!({ "diagram": er, "element": table, "x": 0, "y": 0 }));
    assert!(s.execute("diagram.place", &json!({ "diagram": er, "element": party, "x": 0, "y": 0 })).is_err());
    assert!(s.execute("diagram.place", &json!({ "diagram": d, "element": table, "x": 300, "y": 0 })).is_err());
    assert!(s.execute("diagram.add", &json!({ "kind": KIND, "element": schema })).is_err());

    // Describing goes to the element's notation: the sketch names its class, ER describes its table.
    id(&mut s, "element.describe", json!({ "element": customer, "text": "Buyer" }));
    let st = s.doc();
    let named = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(archforge_core::name_of(st, named), "Buyer");
    id(&mut s, "element.describe", json!({ "element": table, "text": "Someone who buys." }));

    // One removal of both notations' elements: each removes its own, with the views of them.
    let out = s.execute("element.remove", &json!({ "ids": [customer, table] })).unwrap();
    assert!(out.value["removed"].as_u64().unwrap() >= 4, "{}", out.value);
    let left = s.execute("diagram.get", &json!({ "diagram": d })).unwrap().value;
    assert_eq!(left["nodes"].as_array().unwrap().len(), 1, "the customer's node went with it");
    assert!(s.execute("diagram.get", &json!({ "diagram": er })).unwrap().value["nodes"].as_array().unwrap().is_empty());
    s.undo().unwrap();
    assert_eq!(s.execute("diagram.get", &json!({ "diagram": d })).unwrap().value["nodes"].as_array().unwrap().len(), 2);

    // Every command, every key, wrong values: nothing panics and the model stays whole.
    let ids = [pkg.as_str(), party.as_str(), customer.as_str(), g.as_str(), d.as_str(), er.as_str(), schema.as_str(), table.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, |st| rows::snapshot(st, &storage().0, &storage().1).unwrap()).unwrap();
    assert!(calls > 100, "{calls}");
}

/// A notation that removes nothing, as a notation does by default.
struct Stubborn;

impl Notation for Stubborn {
    fn name(&self) -> &'static str {
        "Stubborn"
    }
    fn metamodel(&self) -> u16 {
        archforge_uml::INDEX
    }
}

static STUBBORN: &[&dyn Notation] = &[&archforge_er::ErNotation, &Stubborn];

#[test]
fn a_removal_one_notation_refuses_changes_nothing_in_any() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let table = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "customer" }));
    let (tables, junctions) = storage();
    let mut st = store(ids(1 << 40)).unwrap();
    rows::restore(&mut st, &rows::snapshot(s.doc(), &tables, &junctions).unwrap(), &tables, &junctions).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(pkg)).unwrap();
    let pkg = uuid(&st, pkg);
    let mut s = Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(STUBBORN));
    let before = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    // ER removes its table first, then the second notation refuses: the table comes back.
    let e = s.execute("element.remove", &json!({ "ids": [table, pkg] })).unwrap_err();
    assert!(e.to_string().contains("cannot be removed"), "{e}");
    assert_eq!(rows::snapshot(s.doc(), &tables, &junctions).unwrap(), before);
}

#[test]
fn every_session_a_project_makes_holds_its_notations() {
    let s = new_project_session("Shop", ids(0)).unwrap();
    assert!(std::ptr::eq(s.view().notations(), archforge_notations::NOTATIONS));
    let (tables, junctions) = storage();
    let back = session_from_rows(&rows::snapshot(s.doc(), &tables, &junctions).unwrap(), ids(1 << 40)).unwrap();
    assert!(std::ptr::eq(back.view().notations(), archforge_notations::NOTATIONS));
    // A view without notations knows no diagram kind, and says so.
    let mut bare = Session::new(registry().unwrap(), store(ids(1 << 41)).unwrap(), 1, Options::default());
    let e = bare.execute("diagram.add", &json!({ "kind": "er" })).unwrap_err();
    assert!(e.to_string().contains("holds no notation"), "{e}");
}
