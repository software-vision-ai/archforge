//! The project composed from its notations: every notation's metamodel, slabs and tables in one
//! store, a model of two notations rebuilt from its rows, and a second notation's diagrams,
//! removals and descriptions routed to it through the one project view.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use archforge_diagram::{Notation, OpenProblem, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{CommandError, Options};
use archforge_mof::rows::{self, Cell};
use archforge_mof::schema::{Column, Table};
use archforge_mof::{ClassId, Obj, Store, Uuid, Value};
use archforge_notations::{
    OpenError, PARTS, Session, UnknownDiagram, check_kinds, check_kinds_in_rows, new_project_session, registry, session_from_rows,
    session_from_rows_repaired, storage, store, universe,
};
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

/// AR-556: a fresh project's mixed structure diagram, a package, a class and an interface on it
/// and the class's dependency on the package drawn, saved as rows and opened again through
/// `session_from_rows`, is the same store row for row: the diagram still a structure diagram of
/// Sales, each box on it showing its element by the same id where it was placed, the line on it
/// joining the same boxes for the same dependency and drawn as it was. The real save, to a
/// project's database and back, is the app's
/// `a_project_whose_class_skill_overrides_a_project_skill_saves_and_opens_again`.
#[test]
fn a_mixed_structure_diagram_saved_as_rows_opens_again_as_the_same_store() {
    let mut s = new_project_session("Commerce", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = id(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let order = id(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let payable = id(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let d = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let shown = [&orders, &order, &payable];
    let boxes: Vec<String> =
        shown.iter().zip(0..).map(|(e, i)| id(&mut s, "diagram.place", json!({ "diagram": d, "element": e, "x": 300 * i, "y": 0 }))).collect();
    let dep = id(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": orders }));
    let connector = id(&mut s, "diagram.connect", json!({ "diagram": d, "element": dep, "source": boxes[1], "target": boxes[0] }));
    let (tables, junctions) = storage();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let back = session_from_rows(&saved, ids(1 << 40)).unwrap();
    assert_eq!(rows::snapshot(back.doc(), &tables, &junctions).unwrap(), saved, "the same store, row for row");
    let st = back.doc();
    let at = |id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    let referred = |o: Obj, prop| uuid(st, st.get(o, prop).unwrap().as_ref().unwrap());
    assert_eq!(st.get(at(&d), archforge_diagram::diagram::KIND).unwrap().as_str(), Some(archforge_uml::STRUCTURE_DIAGRAM));
    assert_eq!(referred(at(&d), archforge_diagram::diagram::ELEMENT), sales);
    for ((b, e), x) in boxes.iter().zip(shown).zip([0.0, 300.0, 600.0]) {
        assert_eq!(&referred(at(b), archforge_diagram::node::ELEMENT), e, "the box shows the same element");
        assert_eq!(archforge_diagram::diagram_of(st, at(b)), Some(at(&d)), "on Sales's diagram still");
        let place = [archforge_diagram::node::X, archforge_diagram::node::Y].map(|p| st.get(at(b), p).unwrap());
        assert_eq!(place, [Value::Real(x), Value::Real(0.0)], "where it was placed");
    }
    let line = |st: &Store| archforge_uml::UmlNotation.line(st, st.by_uuid(Uuid::parse(&dep).unwrap()).unwrap());
    let edge = at(&connector);
    assert_eq!(archforge_diagram::diagram_of(st, edge), Some(at(&d)), "the line is on the diagram still");
    let ends = [archforge_diagram::edge::ELEMENT, archforge_diagram::edge::SOURCE, archforge_diagram::edge::TARGET].map(|p| referred(edge, p));
    assert_eq!(ends, [dep.clone(), boxes[1].clone(), boxes[0].clone()], "drawing the dependency from the class's box to the package's");
    assert!(line(st).is_some_and(|l| l.dashed), "drawn dashed");
    assert_eq!(line(st), line(s.doc()), "the line drawn as it was");
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
    /// Every sketch node's size, its own so a test can tell it from the default.
    fn default_size(&self, _: ClassId) -> (f64, f64) {
        (200.0, 60.0)
    }
    /// Removes each object with its views; it hands nothing up and counts no binding or
    /// relationship apart, only the objects that went.
    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<archforge_diagram::Removed, CommandError> {
        let mut gone = 0;
        for o in objs {
            gone += archforge_diagram::remove_views_of(store, *o).map_err(|e| CommandError::new(e.to_string()))?;
            gone += store.destroy(*o).map_err(|e| CommandError::new(e.to_string()))?;
        }
        Ok(archforge_diagram::Removed { gone, ..Default::default() })
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
    let er = id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema }));
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

/// Both notations hold a description to one limit, and each reads back through the project's view
/// what describing set, ER's as its own column and UML's as a comment, undone and redone alike.
#[test]
fn descriptions_read_back_through_each_notation_under_one_limit() {
    let limit = archforge_uml::descriptions::MAX_DESCRIPTION;
    assert_eq!(limit, archforge_er::rules::MAX_TEXT, "one limit across the notations");
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let table = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "customer" }));
    let pkg = id(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let class = id(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    for (element, text) in [(&table, "Someone who buys."), (&class, "One purchase.")] {
        let o = s.doc().by_uuid(Uuid::parse(element).unwrap()).unwrap();
        let n = s.view().of_element(s.doc(), o).unwrap();
        id(&mut s, "element.describe", json!({ "element": element, "text": text }));
        assert_eq!(n.description(s.doc(), o), text, "{}", n.name());
        s.undo().unwrap();
        assert_eq!(n.description(s.doc(), o), "", "{}", n.name());
        s.redo().unwrap();
        assert_eq!(n.description(s.doc(), o), text, "{}", n.name());
        // The limit is the same in both: at it, accepted; past it, refused.
        id(&mut s, "element.describe", json!({ "element": element, "text": "x".repeat(limit) }));
        assert!(s.execute("element.describe", &json!({ "element": element, "text": "x".repeat(limit + 1) })).is_err(), "{}", n.name());
    }
}

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
    // The refusal carries its reason typed beside the unchanged English (AR-439).
    let Err(archforge_engine::EngineError::Failed { error, .. }) = s.execute("element.remove", &json!({ "ids": [table, pkg] })) else {
        panic!("the removal is refused")
    };
    assert_eq!((error.reason, error.key.as_deref()), (Some(archforge_engine::Reason::NotRemovable { notation: "Stubborn" }), Some("ids")));
    assert_eq!(error.message, "Stubborn elements cannot be removed yet");
    assert_eq!(rows::snapshot(s.doc(), &tables, &junctions).unwrap(), before);
}

/// One removal of both notations' elements adds up what each took (AR-675): every object that went,
/// and the relationships each took along with its own, ER's table its relationship and UML's
/// class its association; the relationships between what stays stay and are not counted.
#[test]
fn a_removal_across_notations_adds_up_what_each_took() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let [customer, order, item] = ["customer", "order", "item"].map(|name| id(&mut s, "er.entity.add", json!({ "schema": schema, "name": name })));
    for (from, to) in [(&customer, &order), (&order, &item)] {
        id(&mut s, "er.relationship.add", json!({ "from": from, "to": to, "from_end": "exactly-one", "to_end": "zero-or-more" }));
    }
    let pkg = id(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let [buyer, sale, line] = ["Buyer", "Sale", "Line"].map(|name| id(&mut s, "uml.class.add", json!({ "package": pkg, "name": name })));
    for (from, to) in [(&buyer, &sale), (&sale, &line)] {
        id(&mut s, "uml.association.add", json!({ "from": from, "to": to }));
    }
    let live = s.doc().alive().count();
    let out = s.execute("element.remove", &json!({ "ids": [customer, buyer] })).unwrap().value;
    assert_eq!(out["removed"], json!(live - s.doc().alive().count()), "{out}");
    assert_eq!((out["relationships"].as_u64(), out["bindings"].as_u64(), out["moved_elements"].as_u64()), (Some(2), Some(0), Some(0)), "{out}");
}

/// The shared commands refuse with their reason typed beside the unchanged English: the lists of
/// `view.select` and `element.remove` with an item that is no string, no id, of no element or gone,
/// or given twice, and an empty list to remove; `element.describe`'s id none, malformed or of no
/// element; an element of no notation removed or described; and a notation's elements it does not
/// describe (AR-517).
#[test]
fn every_shared_command_refusal_carries_its_reason_typed() {
    use archforge_diagram::project::{ELEMENT, NOTATION_ELEMENT};
    use archforge_engine::{EngineError, Reason};
    let mut st = store(ids(0)).unwrap();
    let model = st.create(archforge_core::MODEL).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(pkg)).unwrap();
    let pkg = uuid(&st, pkg);
    let mut s = Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(STUBBORN));
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let table = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "customer" }));
    let gone = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "gone" }));
    s.execute("element.remove", &json!({ "ids": [gone] })).unwrap();
    let map = id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "Map" }));
    let missing = ids(1 << 50)().to_string();
    let long = "x".repeat(50);
    let mut check = |cmd: &str, key: &str, value: Json, reason: Reason, english: String| {
        let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &json!({ key: value })) else { panic!("{cmd} {value} is refused") };
        assert_eq!((error.reason, error.key.as_deref()), (Some(reason), Some(key)), "{cmd} {value}: {}", error.message);
        assert_eq!(error.message, english, "{cmd} {value}");
    };
    let no_element = |id: &str| Reason::NoElement { id: id.chars().take(40).collect() };
    for cmd in ["view.select", "element.remove"] {
        check(cmd, "ids", json!([7]), Reason::IdsExpected { kind: ELEMENT }, "every id must be a string".into());
        check(cmd, "ids", json!([""]), Reason::IdEmpty { kind: ELEMENT }, "\"\" is not an id".into());
        check(cmd, "ids", json!([&table, &long]), no_element(&long), format!("{:?} is not an id", "x".repeat(40)));
        check(cmd, "ids", json!([&missing]), no_element(&missing), format!("there is no element {missing}"));
        check(cmd, "ids", json!([&gone]), no_element(&gone), format!("there is no element {gone}"));
        let twice = Reason::NamedTwice { kind: ELEMENT, name: table.clone() };
        check(cmd, "ids", json!([&table, &schema, &table]), twice, format!("{table} is listed twice"));
    }
    check("element.remove", "ids", json!([]), Reason::TooFew { kind: ELEMENT, min: 1 }, "nothing to remove".into());
    let not_of = Reason::NotA { kind: NOTATION_ELEMENT, name: "Map".into() };
    check("element.remove", "ids", json!([&table, &map]), not_of.clone(), format!("{map} is not an element of a notation"));
    check("element.describe", "element", json!(""), Reason::IdEmpty { kind: ELEMENT }, "\"\" is not an id".into());
    check("element.describe", "element", json!("nope"), no_element("nope"), "\"nope\" is not an id".into());
    check("element.describe", "element", json!(&missing), no_element(&missing), format!("there is no element {missing}"));
    check("element.describe", "element", json!(&map), not_of, format!("{map} is not an element of a notation"));
    let stubborn = Reason::NotDescribable { notation: "Stubborn" };
    check("element.describe", "element", json!(&pkg), stubborn, "Stubborn elements cannot be described yet".into());
}

#[test]
fn every_session_a_project_makes_holds_its_notations() {
    let s = new_project_session("Shop", ids(0)).unwrap();
    assert!(std::ptr::eq(s.view().notations(), archforge_notations::NOTATIONS));
    let (tables, junctions) = storage();
    let back = session_from_rows(&rows::snapshot(s.doc(), &tables, &junctions).unwrap(), ids(1 << 40)).unwrap();
    assert!(std::ptr::eq(back.view().notations(), archforge_notations::NOTATIONS));
    // A view without notations knows no diagram kind, and says so, the reason typed (AR-512).
    let mut bare = Session::new(registry().unwrap(), store(ids(1 << 41)).unwrap(), 1, Options::default());
    let Err(archforge_engine::EngineError::Failed { error, .. }) = bare.execute("diagram.add", &json!({ "kind": archforge_er::DIAGRAM_KIND })) else {
        panic!("a view without notations adds no diagram")
    };
    assert!(error.message.contains("holds no notation"), "{}", error.message);
    let none =
        archforge_engine::NotOneOf { what: archforge_engine::WordPlace::DiagramKind, word: archforge_er::DIAGRAM_KIND.into(), known: Vec::new() };
    assert_eq!((error.reason, error.key.as_deref()), (Some(archforge_engine::Reason::NotOneOf(Box::new(none))), Some("kind")));
}

#[test]
fn a_model_past_one_transactions_steps_opens_again_from_its_rows() {
    // 400 packages of 1,000 classes each: over a million steps to rebuild, more than one store
    // transaction takes, built straight in the store as a big project's rows would hold them.
    let mut st = store(ids(0)).unwrap();
    let model = st.create(archforge_core::MODEL).unwrap();
    for p in 0..400 {
        let pkg = st.create(archforge_uml::PACKAGE).unwrap();
        st.set(pkg, archforge_uml::named_element::NAME, Value::str(&format!("p{p}"))).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(pkg)).unwrap();
        for c in 0..1_000 {
            let class = st.create(archforge_uml::CLASS).unwrap();
            st.set(class, archforge_uml::named_element::NAME, Value::str(&format!("C{c}"))).unwrap();
            st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(class)).unwrap();
        }
    }
    let (tables, junctions) = storage();
    let saved = rows::snapshot(&st, &tables, &junctions).unwrap();
    // One transaction refuses it; opening builds it without one.
    let mut again = store(ids(1)).unwrap();
    assert!(matches!(rows::restore(&mut again, &saved, &tables, &junctions), Err(archforge_mof::Invalid::TooManyOps)));
    let session = session_from_rows(&saved, ids(2)).unwrap();
    assert_eq!(session.doc().live(), 1 + 400 + 400_000);
    assert!(!session.is_dirty());
    // A store that already holds something is refused rather than mixed.
    let mut busy = store(ids(3)).unwrap();
    busy.create(archforge_core::MODEL).unwrap();
    assert!(rows::restore_into_empty(busy, &saved, &tables, &junctions).is_err());
}

#[test]
fn a_class_skill_named_as_a_project_skill_opens_again_and_one_subject_never_holds_two() {
    // Every skill sits in the model's one list, so the siblings' name check refused a
    // class's skill that overrides the project's of the same name, and the project would not open.
    let mut s = new_project_session("Review", ids(0)).unwrap();
    let project = id(&mut s, "skill.add", json!({ "name": "Review export conventions", "body": "Everywhere." }));
    let package = id(&mut s, "uml.package.add", json!({ "name": "Review domain" }));
    let invoice = id(&mut s, "uml.class.add", json!({ "package": package, "name": "Invoice" }));
    let own = id(&mut s, "skill.add", json!({ "subject": invoice, "name": "Review export conventions", "body": "Invoices only." }));
    let (tables, junctions) = storage();
    let written = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let back = session_from_rows(&written, ids(1 << 40)).expect("the override opens again");
    assert_eq!(rows::snapshot(back.doc(), &tables, &junctions).unwrap(), written);
    let st = back.doc();
    let obj = |id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    let effective = archforge_core::skills::effective_skills(back.view(), st, obj(&invoice), None);
    let found: Vec<(Obj, Option<Obj>, Option<Obj>)> = effective.iter().map(|e| (e.skill, e.origin, e.overridden_by)).collect();
    assert_eq!(found, [(obj(&project), None, Some(obj(&own))), (obj(&own), Some(obj(&invoice)), None)], "the class's own overrides the project's");
    // Two of one name on one subject still do not open, whichever way the rows came to hold them.
    let mut st = store(ids(1 << 41)).unwrap();
    rows::restore(&mut st, &written, &tables, &junctions).unwrap();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let twin = st.create(archforge_core::SKILL).unwrap();
    st.set(twin, archforge_core::named_element::NAME, Value::str("review EXPORT conventions")).unwrap();
    st.set(twin, archforge_core::skill::SUBJECT, Value::Ref(st.by_uuid(Uuid::parse(&invoice).unwrap()).unwrap())).unwrap();
    st.push(model, archforge_core::model::SKILL, Value::Ref(twin)).unwrap();
    let e = session_from_rows(&rows::snapshot(&st, &tables, &junctions).unwrap(), ids(1 << 42)).err().expect("refused");
    assert!(e.to_string().contains("DuplicateName"), "{e}");
}

/// A skill aims only at the kinds of its subject's notation and moves only within that notation.
#[test]
fn a_skill_aims_only_at_kinds_of_its_subjects_notation_and_moves_only_within_it() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let package = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let class = id(&mut s, "uml.class.add", json!({ "package": package, "name": "Order" }));
    let refused = |s: &mut Session, cmd: &str, p: Json| s.execute(cmd, &p).map(|o| o.value.to_string()).unwrap_err().to_string();
    let e = refused(&mut s, "skill.add", json!({ "subject": package, "name": "Tables", "applies_to": "table" }));
    assert!(
        e.contains("class, interface, enumeration, data type, primitive type, association class, attribute, operation, association")
            && !e.contains("column"),
        "{e}"
    );
    let e = refused(&mut s, "skill.add", json!({ "subject": schema, "name": "Classes", "applies_to": "class" }));
    assert!(e.contains("table, relationship, column") && !e.contains("operation"), "{e}");
    // The model's skill may aim at any notation's kind.
    id(&mut s, "skill.add", json!({ "subject": "", "name": "Classes", "applies_to": "class" }));
    let columns = id(&mut s, "skill.add", json!({ "subject": schema, "name": "Columns", "applies_to": "column" }));
    let e = refused(&mut s, "skill.move", json!({ "skill": columns, "subject": class }));
    assert!(e.contains("every column cannot sit on Class"), "{e}");
    id(&mut s, "skill.move", json!({ "skill": columns, "subject": "" }));
}

/// The names of the skills that apply to `element`, each with whether a later one replaces it.
fn applying(s: &Session, element: &str) -> Vec<(String, bool)> {
    let st = s.doc();
    let o = st.by_uuid(Uuid::parse(element).unwrap()).unwrap();
    archforge_core::skills::effective_skills(s.view(), st, o, None)
        .into_iter()
        .map(|e| (archforge_core::skills::name_of(st, e.skill), e.overridden_by.is_some()))
        .collect()
}

fn skill_count(s: &Session) -> usize {
    archforge_core::skills::skills(s.doc()).len()
}

#[test]
fn a_diagram_carries_skills_and_inherits_the_projects_and_its_elements() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = id(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let diagram = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": orders, "name": "Orders" }));
    id(&mut s, "skill.add", json!({ "subject": "", "name": "House style" }));
    id(&mut s, "skill.add", json!({ "subject": sales, "name": "Layout" }));
    id(&mut s, "skill.add", json!({ "subject": sales, "name": "Classes", "applies_to": "class" }));
    id(&mut s, "skill.add", json!({ "subject": orders, "name": "Naming" }));
    id(&mut s, "skill.add", json!({ "subject": diagram, "name": "Layout", "triggers": ["drawing"] }));
    assert_eq!(
        applying(&s, &diagram),
        [("House style".into(), false), ("Layout".into(), true), ("Naming".into(), false), ("Layout".into(), false)],
        "the project's, the shown package's and its parent's, then its own; a skill for every class aims not at it"
    );
    // A diagram's skill aims at no kind, and a kind's skill does not move onto it.
    let e = s.execute("skill.add", &json!({ "subject": diagram, "name": "Classes", "applies_to": "class" })).unwrap_err().to_string();
    assert!(e.contains("aims at no kind"), "{e}");
    let classes = archforge_core::skills::skills(s.doc()).into_iter().find(|k| archforge_core::skills::name_of(s.doc(), *k) == "Classes").unwrap();
    let classes = uuid(s.doc(), classes);
    let e = s.execute("skill.move", &json!({ "skill": classes, "subject": diagram })).unwrap_err().to_string();
    assert!(e.contains("cannot sit on Diagram"), "{e}");
}

/// In a project of every notation only UML binds a first diagram as a default: a schema's first
/// ER diagram binds nothing, a package's first class diagram is bound (AR-579).
#[test]
fn a_schemas_first_er_diagram_binds_nothing_and_a_packages_first_class_diagram_is_bound() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "shop" }));
    assert_eq!(archforge_diagram::defaults::bindings(s.doc()), [], "the schema's diagram is no default");
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let d = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let st = s.doc();
    let [binding] = archforge_diagram::defaults::bindings(st)[..] else { panic!("the package's diagram is bound") };
    let named = |prop| st.get(binding, prop).unwrap().as_ref().map(|o| uuid(st, o));
    use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
    assert_eq!((named(OWNER_ELEMENT), named(DEFAULT_DIAGRAM)), (Some(sales), Some(d)));
}

#[test]
fn a_diagrams_skills_go_with_it_whether_it_or_its_element_goes_and_come_back_with_undo() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    // Sales's first diagram is its default, which is not removed alone (AR-578): the second goes.
    id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales overview" }));
    let class_diagram = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let er_diagram = id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "shop" }));
    id(&mut s, "skill.add", json!({ "subject": class_diagram, "name": "Layout" }));
    id(&mut s, "skill.add", json!({ "subject": er_diagram, "name": "Layout" }));
    assert_eq!(skill_count(&s), 2);
    // The diagram removed alone.
    s.execute("diagram.remove", &json!({ "diagram": class_diagram })).unwrap();
    assert_eq!(skill_count(&s), 1, "its skill went with it, none turned model-wide");
    s.undo().unwrap();
    assert_eq!(skill_count(&s), 2);
    // With the element it shows: a schema's diagram goes with it; a package sits in the model's
    // root package, so its diagram moves up into the root, beside the root's default (AR-673), and
    // keeps its skill.
    let root = uuid(s.doc(), archforge_uml::commands::root_package(s.doc()).unwrap());
    id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Shop" }));
    for (element, left) in [(&sales, 2), (&schema, 1)] {
        s.execute("element.remove", &json!({ "ids": [element] })).unwrap();
        assert_eq!(skill_count(&s), left, "{element}");
        assert!(archforge_core::skills::skills(s.doc()).into_iter().all(|k| archforge_core::skills::subject_of(s.doc(), k).is_some()));
        s.undo().unwrap();
        assert_eq!(skill_count(&s), 2);
    }
}

#[test]
fn a_nested_packages_diagram_keeps_its_skill_when_the_package_goes_and_inherits_from_the_parent() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = id(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let diagram = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": orders, "name": "Orders" }));
    id(&mut s, "skill.add", json!({ "subject": sales, "name": "Sales style" }));
    id(&mut s, "skill.add", json!({ "subject": orders, "name": "Orders style" }));
    id(&mut s, "skill.add", json!({ "subject": diagram, "name": "Layout" }));
    // Sales's default takes the diagram in (AR-673).
    id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    s.execute("element.remove", &json!({ "ids": [orders] })).unwrap();
    assert_eq!(applying(&s, &diagram), [("Sales style".into(), false), ("Layout".into(), false)], "moved up to Sales with its skill");
}

#[test]
fn a_diagram_crafted_to_show_itself_inherits_from_what_holds_it() {
    let mut st = store(ids(0)).unwrap();
    let model = st.create(archforge_core::MODEL).unwrap();
    let d = st.create(archforge_diagram::DIAGRAM).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(d)).unwrap();
    st.set(d, archforge_diagram::diagram::ELEMENT, Value::Ref(d)).unwrap();
    let project = st.create(archforge_core::SKILL).unwrap();
    st.push(model, archforge_core::model::SKILL, Value::Ref(project)).unwrap();
    let levels = archforge_core::skills::levels(&archforge_notations::view(), &st, d);
    assert_eq!(levels, [model, d], "the model above it, not the diagram again");
    let applying = archforge_core::skills::effective_skills(&archforge_notations::view(), &st, d, None);
    assert_eq!(applying.iter().map(|e| e.skill).collect::<Vec<_>>(), [project]);
}

/// A fresh project with an ER diagram, "Orders", and a UML one; the ER diagram's id.
fn two_diagrams() -> (Session, String) {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let shop = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let orders = id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": shop, "name": "Orders" }));
    let sales = id(&mut s, "uml.package.add", json!({ "name": "sales" }));
    id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Main" }));
    (s, orders)
}

/// The kind cell of `diagram`'s row in a project's rows.
fn kind_cell<'a>(rows: &'a mut rows::Rows, diagram: &str) -> &'a mut Cell {
    let (tables, _) = storage();
    let at = tables.iter().position(|t| t.name == "diagram_diagram").unwrap();
    let kind = tables[at].columns.iter().position(|c| c.name == "kind").unwrap();
    let row = rows.tables[at].iter_mut().find(|r| r[0] == Cell::Id(Uuid::parse(diagram).unwrap())).unwrap();
    &mut row[kind]
}

/// AR-547: a diagram of a kind no notation knows refuses its model, typed and naming it, before
/// any store is built; a fresh project's rows still open.
#[test]
fn a_diagram_of_a_kind_no_notation_knows_refuses_its_model_before_anything_is_built() {
    let (s, orders) = two_diagrams();
    let (tables, junctions) = storage();
    let fresh = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    // A fresh project's rows pass the check and open.
    assert_eq!(check_kinds_in_rows(&fresh, &tables), Ok(()));
    assert_eq!(check_kinds(s.doc()), Ok(()));
    assert!(session_from_rows(&fresh, ids(1 << 40)).is_ok());
    // The ER diagram's kind as an earlier version could have stored it.
    let mut legacy = fresh.clone();
    *kind_cell(&mut legacy, &orders) = Cell::Text("legacy.kind".into());
    let refused = check_kinds_in_rows(&legacy, &tables).unwrap_err();
    let named = |d: &UnknownDiagram| (d.id.to_string(), d.name.clone(), d.kind.clone());
    assert_eq!(refused.diagrams.iter().map(named).collect::<Vec<_>>(), [(orders.clone(), "Orders".to_string(), "legacy.kind".to_string())]);
    assert_eq!(refused.accepted, [archforge_er::DIAGRAM_KIND, archforge_uml::STRUCTURE_DIAGRAM]);
    // Refused whole: the repair never sees a store, so none is built, let alone handed back.
    let mut built = false;
    let why = session_from_rows_repaired(&legacy, ids(1 << 40), |_| built = true).err().unwrap();
    assert!(!built, "nothing is built from refused rows");
    assert_eq!(why, OpenError::UnknownKinds(refused), "callers match the refusal");
    let why = why.to_string();
    assert!(why.contains("\"Orders\"") && why.contains("\"legacy.kind\"") && why.contains("the kinds known are er, uml.structure"), "{why}");
    // A diagram of no kind is of none a notation knows.
    *kind_cell(&mut legacy, &orders) = Cell::Null;
    assert_eq!(check_kinds_in_rows(&legacy, &tables).unwrap_err().diagrams[0].kind, "");
    // A repair that would change a kind is refused too, and no session comes back.
    let why = session_from_rows_repaired(&fresh, ids(1 << 40), |st| {
        let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
        st.set(d, archforge_diagram::diagram::KIND, Value::str("legacy.kind")).unwrap();
    })
    .err()
    .unwrap();
    assert!(matches!(&why, OpenError::UnknownKinds(e) if e.diagrams[0].kind == "legacy.kind"), "{why}");
}

/// The gate's table of diagram kinds (`xtask/diagram-kinds.tsv`) lists exactly the kinds the
/// notations register: `cargo xtask kinds` refuses each listed word written out anywhere but its
/// constant, so a kind registered without its row would slip past that check (AR-770).
#[test]
fn the_gates_table_of_kinds_lists_every_registered_kind() {
    let table = include_str!("../../../xtask/diagram-kinds.tsv");
    let mut listed: Vec<&str> = table.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).filter_map(|l| l.split('\t').next()).collect();
    let mut registered = archforge_notations::kinds();
    listed.sort_unstable();
    registered.sort_unstable();
    assert_eq!(registered, listed, "every registered kind needs its row in xtask/diagram-kinds.tsv, and every row a registered kind");
}

/// AR-551: the project knows the ER kind and the one UML kind, `uml.structure`, which a new UML
/// diagram is stored as; the earlier `uml.class` is no alias of it: adding a diagram of it is
/// refused, typed and naming the kinds known, with no undo entry, and rows holding one are refused.
#[test]
fn the_kinds_known_are_er_and_uml_structure_and_the_earlier_uml_class_is_refused() {
    assert_eq!(archforge_diagram::DiagramView::kinds(&archforge_notations::view()), [archforge_er::DIAGRAM_KIND, archforge_uml::STRUCTURE_DIAGRAM]);
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let main = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Main" }));
    let (tables, junctions) = storage();
    let state = |s: &Session| (rows::snapshot(s.doc(), &tables, &junctions).unwrap(), s.undo_label().map(String::from), s.revision(), s.counter());
    let before = state(&s);
    let Err(archforge_engine::EngineError::Failed { error, .. }) =
        s.execute("diagram.add", &json!({ "kind": "uml.class", "element": sales, "name": "Old" }))
    else {
        panic!("a diagram of the earlier kind is refused")
    };
    assert!(error.message.contains("\"uml.class\" is not one of er, uml.structure"), "{}", error.message);
    let known = vec![archforge_er::DIAGRAM_KIND.to_string(), archforge_uml::STRUCTURE_DIAGRAM.into()];
    let unknown = archforge_engine::NotOneOf { what: archforge_engine::WordPlace::DiagramKind, word: "uml.class".into(), known };
    assert_eq!((error.reason, error.key.as_deref()), (Some(archforge_engine::Reason::NotOneOf(Box::new(unknown))), Some("kind")));
    assert_eq!(state(&s), before, "nothing changed and no undo entry was made");
    // The diagram is stored as `uml.structure`, and its rows open again.
    let mut fresh = before.0;
    assert_eq!(*kind_cell(&mut fresh, &main), Cell::Text(archforge_uml::STRUCTURE_DIAGRAM.into()));
    assert!(session_from_rows(&fresh, ids(1 << 40)).is_ok());
    // Rows holding a diagram of the earlier kind are refused whole, never read as a structure diagram.
    *kind_cell(&mut fresh, &main) = Cell::Text("uml.class".into());
    let refused = check_kinds_in_rows(&fresh, &tables).unwrap_err();
    let named = |d: &UnknownDiagram| (d.id.to_string(), d.name.clone(), d.kind.clone());
    assert_eq!(refused.diagrams.iter().map(named).collect::<Vec<_>>(), [(main, "Main".to_string(), "uml.class".to_string())]);
    assert_eq!(refused.accepted, [archforge_er::DIAGRAM_KIND, archforge_uml::STRUCTURE_DIAGRAM]);
    let why = session_from_rows_repaired(&fresh, ids(1 << 40), |_| ()).err().unwrap();
    assert_eq!(why, OpenError::UnknownKinds(refused));
}

/// A text column of a table of an earlier shape.
const fn column(name: &'static str) -> Column {
    Column { name, prop: None, sql: "MEDIUMTEXT NULL" }
}

/// A diagram table of `columns` alone, as an earlier shape could have stored it.
fn diagrams_of(columns: &'static [Column]) -> [Table; 1] {
    [Table { name: "diagram_diagram", class: archforge_diagram::DIAGRAM, columns }]
}

/// The rows check finds the diagram table and its columns by name, so a host can run it on the
/// rows of an earlier shape; every unknown diagram is named, one of no kind among them.
#[test]
fn the_kind_check_reads_a_diagram_table_of_any_shape_by_its_column_names() {
    // An earlier shape: the diagram table alone, its kind before its id, and no name column.
    const EARLIER: &[Column] = &[column("kind"), column("id")];
    const KINDLESS: &[Column] = &[column("id")];
    let earlier = diagrams_of(EARLIER);
    let [known, legacy, kindless] = [1, 2, 3].map(|n| Uuid::v7(1_761_670_800_000, [n; 10]));
    let row = |kind: Cell, id| vec![kind, Cell::Id(id)];
    let diagrams = vec![
        row(Cell::Text(archforge_uml::STRUCTURE_DIAGRAM.into()), known),
        row(Cell::Text("legacy.kind".into()), legacy),
        row(Cell::Null, kindless),
    ];
    let rows = rows::Rows { tables: vec![diagrams], junctions: Vec::new() };
    let refused = check_kinds_in_rows(&rows, &earlier).unwrap_err();
    let unknown = |id, kind: &str| UnknownDiagram { id, name: String::new(), kind: kind.into() };
    assert_eq!(refused.diagrams, [unknown(legacy, "legacy.kind"), unknown(kindless, "")], "every one, in the order read");
    let said = format!(
        "the diagram ({legacy}) is of the kind \"legacy.kind\", which no notation knows, and so are 1 more; the kinds known are er, uml.structure"
    );
    assert_eq!(refused.to_string(), said);
    // A shape without the diagram table holds no diagram.
    assert_eq!(check_kinds_in_rows(&rows, &[]), Ok(()));
    // One whose diagram table has no kind column holds diagrams of no kind.
    let rows = rows::Rows { tables: vec![vec![vec![Cell::Id(known)]]], junctions: Vec::new() };
    let refused = check_kinds_in_rows(&rows, &diagrams_of(KINDLESS)).unwrap_err();
    assert_eq!((refused.diagrams.len(), refused.diagrams[0].kind.as_str()), (1, ""));
}

/// The rows of a project whose packages' bindings are each unsound in their own way: Sales has
/// two, Billing's names an ER diagram, Stock's names Billing's diagram, and Orders has a diagram
/// but none.
fn unsound_bindings() -> rows::Rows {
    use archforge_diagram::default_diagram_binding::DEFAULT_DIAGRAM;
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let shop = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let er = id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": shop, "name": "shop" }));
    let mut diagrams = Vec::new();
    for name in ["Sales", "Billing", "Stock", "Orders"] {
        let p = id(&mut s, "uml.package.add", json!({ "name": name }));
        diagrams.push(id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": p, "name": name })));
    }
    let (tables, junctions) = storage();
    let mut st = store(ids(1 << 40)).unwrap();
    rows::restore(&mut st, &rows::snapshot(s.doc(), &tables, &junctions).unwrap(), &tables, &junctions).unwrap();
    let obj = |st: &Store, id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    let binding_to = |st: &Store, d: &str| archforge_diagram::defaults::bindings_to(st, obj(st, d))[0];
    let (sales, billing, stock, orders) =
        (binding_to(&st, &diagrams[0]), binding_to(&st, &diagrams[1]), binding_to(&st, &diagrams[2]), binding_to(&st, &diagrams[3]));
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let twin = st.create(archforge_diagram::DEFAULT_DIAGRAM_BINDING).unwrap();
    for prop in [archforge_diagram::default_diagram_binding::OWNER_ELEMENT, DEFAULT_DIAGRAM] {
        let end = st.get(sales, prop).unwrap();
        st.set(twin, prop, end).unwrap();
    }
    st.push(model, archforge_core::model::MEMBER, Value::Ref(twin)).unwrap();
    st.set(billing, DEFAULT_DIAGRAM, Value::Ref(obj(&st, &er))).unwrap();
    st.set(stock, DEFAULT_DIAGRAM, Value::Ref(obj(&st, &diagrams[1]))).unwrap();
    st.destroy(orders).unwrap();
    rows::snapshot(&st, &tables, &junctions).unwrap()
}

/// Each problem reported as a project opens as the notation reporting it, the name of the element
/// it concerns and its code.
fn diagnosed(st: &Store, all: &[OpenProblem]) -> Vec<(&'static str, String, &'static str)> {
    let owner = |p: &OpenProblem| p.owner.map(|o| archforge_core::name_of(st, o)).unwrap_or_default();
    all.iter().map(|p| (p.notation, owner(p), p.code)).collect()
}

/// AR-586: a project whose bindings are unsound opens as it was stored, its rows unchanged and
/// nothing to save, and says what is wrong with each; a repair that would change a binding is
/// refused.
#[test]
fn unsound_bindings_open_unchanged_and_are_reported() {
    let written = unsound_bindings();
    let (tables, junctions) = storage();
    let opened = archforge_notations::session_from_rows_diagnosed(&written, ids(1 << 41), |_| {}).unwrap();
    let st = opened.session.doc();
    assert_eq!(rows::snapshot(st, &tables, &junctions).unwrap(), written, "the rows are as they were read");
    assert!(!opened.session.is_dirty(), "nothing to save");
    let named = |pairs: [(&str, &'static str); 4]| pairs.map(|(p, k)| ("UML", p.to_string(), k)).to_vec();
    let expected = named([
        ("Sales", "default_diagram_duplicate"),
        ("Billing", "default_diagram_wrong_kind"),
        ("Stock", "default_diagram_wrong_owner"),
        ("Orders", "default_diagram_missing"),
    ]);
    assert_eq!(diagnosed(st, &opened.problems), expected);
    assert_eq!(opened.problems, archforge_notations::open_problems(st), "the problems of the session's own store");
    // UML's own typed problems, asked of UML: the same owners in the same order, in the same words.
    let typed: Vec<_> = archforge_uml::defaults::problems(st).into_iter().map(|d| (d.owner, d.problem.to_string())).collect();
    assert_eq!(opened.problems.iter().map(|p| (p.owner, p.message.clone())).collect::<Vec<_>>(), typed);
    // A repair that touches a binding is refused, and so is one that adds one.
    let retarget = |st: &mut Store| {
        let b = archforge_diagram::defaults::bindings(st)[0];
        st.set(b, archforge_diagram::default_diagram_binding::DEFAULT_DIAGRAM, Value::Null).unwrap();
    };
    let add = |st: &mut Store| {
        st.create(archforge_diagram::DEFAULT_DIAGRAM_BINDING).unwrap();
    };
    for refused in [session_from_rows_repaired(&written, ids(1 << 41), retarget).err(), session_from_rows_repaired(&written, ids(1 << 41), add).err()]
    {
        assert!(matches!(&refused, Some(OpenError::Unreadable(why)) if why.contains("binding")), "{refused:?}");
    }
}

/// AR-586: a repair is judged by ids, whatever store it leaves: the rows rebuilt as they were open,
/// rebuilt with the binding under another id, its handles all the same, they are refused. What is
/// reported is the store the session gets: a diagram the repair gives another owner is reported so.
#[test]
fn a_repair_is_judged_by_ids_and_the_problems_reported_follow_it() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let billing = id(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    let d = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let (tables, junctions) = storage();
    let written = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let rebuilt = |rows: &rows::Rows| rows::restore_into_empty(store(ids(1 << 42)).unwrap(), rows, &tables, &junctions).unwrap();
    assert!(session_from_rows_repaired(&written, ids(1 << 41), |st| *st = rebuilt(&written)).is_ok(), "the same rows rebuilt open");
    let binding = s.doc().uuid(archforge_diagram::defaults::bindings(s.doc())[0]).unwrap();
    let (old, new) = (Cell::Id(binding), Cell::Id(Uuid::v7(1_761_670_800_000, [8; 10])));
    let mut renamed = written.clone();
    for cell in renamed.tables.iter_mut().flatten().flatten().chain(renamed.junctions.iter_mut().flatten().map(|(_, _, c)| c)) {
        if *cell == old {
            *cell = new.clone();
        }
    }
    let refused = session_from_rows_repaired(&written, ids(1 << 41), |st| *st = rebuilt(&renamed)).err();
    assert!(matches!(&refused, Some(OpenError::Unreadable(why)) if why.contains("binding")), "{refused:?}");
    // Sales's diagram given to Billing: Sales's binding names another's diagram, and Billing has one but no binding.
    let opened = archforge_notations::session_from_rows_diagnosed(&written, ids(1 << 41), |st| {
        let obj = |st: &Store, id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
        let (d, billing) = (obj(st, &d), obj(st, &billing));
        st.set(d, archforge_diagram::diagram::ELEMENT, Value::Ref(billing)).unwrap();
    })
    .unwrap();
    let expected = [("Sales", "default_diagram_wrong_owner"), ("Billing", "default_diagram_missing")].map(|(p, k)| ("UML", p.to_string(), k));
    assert_eq!(diagnosed(opened.session.doc(), &opened.problems), expected);
}

/// A project of both notations with one of each figure the canvas asks its notation about
/// (AR-784): UML's nested package, classes, a qualified binary association, an n-ary association
/// with its diamond, an association class with its card and a generalization set with its label,
/// all on one class diagram; ER's two tables and a relationship on a schema's diagram.
struct Figures {
    s: Session,
    package: Obj,
    package_node: Obj,
    class: Obj,
    class_node: Obj,
    association: Obj,
    end: Obj,
    qualifier: Obj,
    nary: Obj,
    leg: Obj,
    association_class: Obj,
    generalization: Obj,
    set: Obj,
    label_node: Obj,
    schema: Obj,
    entity: Obj,
    entity_node: Obj,
    relationship: Obj,
}

/// [`Figures`], each made through its notation's commands.
fn figures() -> Figures {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let shop = id(&mut s, "uml.package.add", json!({ "name": "shop" }));
    let inner = id(&mut s, "uml.package.add", json!({ "name": "inner", "parent": shop }));
    let class = |s: &mut Session, name: &str| id(s, "uml.class.add", json!({ "package": shop, "name": name }));
    let (order, line, product, card, bank) =
        (class(&mut s, "Order"), class(&mut s, "Line"), class(&mut s, "Product"), class(&mut s, "Card"), class(&mut s, "Bank"));
    let association = id(&mut s, "uml.association.add", json!({ "from": order, "to": line }));
    let qualifier = id(&mut s, "uml.qualifier.add", json!({ "association": association, "end": 0, "name": "lineNo" }));
    let nary = id(&mut s, "uml.association.add-nary", json!({ "classes": [order, line, product] }));
    let association_class = id(&mut s, "uml.association-class.add", json!({ "from": order, "to": product }));
    let generalization = id(&mut s, "uml.generalization.add", json!({ "specific": card, "general": order }));
    let other = id(&mut s, "uml.generalization.add", json!({ "specific": bank, "general": order }));
    let set = id(&mut s, "uml.generalization-set.add", json!({ "generalizations": [generalization, other], "name": "method" }));
    let d = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": shop, "name": "shop" }));
    let place = |s: &mut Session, element: &str, x: f64| id(s, "diagram.place", json!({ "diagram": d, "element": element, "x": x, "y": 0.0 }));
    let package_node = place(&mut s, &inner, 0.0);
    let class_node = place(&mut s, &order, 300.0);
    for (i, c) in [&line, &product, &card, &bank].into_iter().enumerate() {
        place(&mut s, c, 600.0 + 300.0 * i as f64);
    }
    place(&mut s, &nary, 900.0);
    place(&mut s, &association_class, 1200.0);
    id(&mut s, "uml.generalization-set.place", json!({ "diagram": d, "set": set, "x": 300.0, "y": 400.0 }));
    let schema = id(&mut s, "er.schema.add", json!({ "name": "sales" }));
    let customer = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "customer" }));
    let invoice = id(&mut s, "er.entity.add", json!({ "schema": schema, "name": "invoice" }));
    let relationship =
        id(&mut s, "er.relationship.add", json!({ "from": customer, "to": invoice, "from_end": "exactly-one", "to_end": "zero-or-more" }));
    let er = id(&mut s, "diagram.add", json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema }));
    let entity_node = id(&mut s, "diagram.place", json!({ "diagram": er, "element": customer, "x": 0.0, "y": 0.0 }));
    let st = s.doc();
    let o = |id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    let (association, nary, set) = (o(&association), o(&nary), o(&set));
    let end = archforge_uml::figures::member_ends(st, association).unwrap()[0];
    let leg = archforge_uml::nary::ends(st, nary)[0];
    let label_node = archforge_uml::generalization_sets::label_node(st, o(&d), set).unwrap();
    Figures {
        package: o(&inner),
        package_node: o(&package_node),
        class: o(&order),
        class_node: o(&class_node),
        association,
        end,
        qualifier: o(&qualifier),
        nary,
        leg,
        association_class: o(&association_class),
        generalization: o(&generalization),
        set,
        label_node,
        schema: o(&schema),
        entity: o(&customer),
        entity_node: o(&entity_node),
        relationship: o(&relationship),
        s,
    }
}

/// The answer of each of `elements` as its own notation gives it.
fn told<T>(f: &Figures, elements: &[Obj], own: impl Fn(&dyn Notation, &Store, Obj) -> T) -> Vec<T> {
    let (st, view) = (f.s.doc(), f.s.view());
    elements.iter().map(|e| own(view.of_element(st, *e).unwrap(), st, *e)).collect()
}

/// [`told`], asked through the project's view too, which must give the same answers.
fn answers<T: PartialEq + std::fmt::Debug>(
    f: &Figures,
    elements: &[Obj],
    own: impl Fn(&dyn Notation, &Store, Obj) -> T,
    viewed: impl Fn(&ProjectView, &Store, Obj) -> T,
) -> Vec<T> {
    let told = told(f, elements, own);
    let (st, view) = (f.s.doc(), f.s.view());
    assert_eq!(elements.iter().map(|e| viewed(view, st, *e)).collect::<Vec<_>>(), told, "the view asks each element's notation");
    told
}

/// A click on an n-ary association's leg selects its association, the diamond and every leg
/// being the one element (AR-128); anything else of UML or ER selects itself, ER answering by
/// default; something no notation holds selects itself.
#[test]
fn a_click_selects_what_its_notation_says_the_figure_stands_for() {
    let f = figures();
    let uml = answers(&f, &[f.leg, f.nary, f.end, f.association, f.class], |n, st, e| n.selects_as(st, e), |v, st, e| v.selects_as(st, e));
    assert_eq!(uml, [f.nary, f.nary, f.end, f.association, f.class]);
    let er = answers(&f, &[f.entity, f.relationship, f.schema], |n, st, e| n.selects_as(st, e), |v, st, e| v.selects_as(st, e));
    assert_eq!(er, [f.entity, f.relationship, f.schema]);
    assert_eq!(f.s.view().selects_as(f.s.doc(), f.class_node), f.class_node, "a node belongs to no notation");
}

/// A selected qualifier draws its association's line selected with it (AR-127); nothing else of
/// UML, and nothing of ER, by default, draws another figure selected.
#[test]
fn a_selected_qualifier_draws_its_associations_line_selected_and_nothing_else_does() {
    let f = figures();
    let uml = answers(
        &f,
        &[f.qualifier, f.end, f.association, f.class, f.leg],
        |n, st, e| n.line_selected_with(st, e),
        |v, st, e| v.line_selected_with(st, e),
    );
    assert_eq!(uml, [Some(f.association), None, None, None, None]);
    let er = answers(&f, &[f.entity, f.relationship], |n, st, e| n.line_selected_with(st, e), |v, st, e| v.line_selected_with(st, e));
    assert_eq!(er, [None, None]);
    assert_eq!(f.s.view().line_selected_with(f.s.doc(), f.class_node), None, "a node belongs to no notation");
}

/// A line keeps its end handles unless a node shows its element, by default, so for ER's
/// relationship and table and for what no notation holds; UML's association class keeps them
/// beside its card, and an n-ary association's leg has none, moving only its class end by its own
/// handle (AR-128).
#[test]
fn a_line_shows_end_handles_unless_its_notation_says_it_has_none() {
    let f = figures();
    let uml = answers(
        &f,
        &[f.association, f.association_class, f.leg, f.nary, f.class],
        |n, st, e| n.line_handles(st, e),
        |v, st, e| v.line_handles(st, e),
    );
    assert_eq!(uml, [true, true, false, false, false]);
    let er = answers(&f, &[f.relationship, f.entity], |n, st, e| n.line_handles(st, e), |v, st, e| v.line_handles(st, e));
    assert_eq!(er, [true, false]);
    // What no notation holds takes the default rule: a node, which no node shows, keeps them.
    assert!(f.s.view().line_handles(f.s.doc(), f.class_node), "a node belongs to no notation");
}

/// An element is drawn as a card when one of its notation's kinds shows its class as nodes, by
/// default and so for ER's table but not its relationship; UML's binary association is a line,
/// though an n-ary one's diamond is a card (AR-128).
#[test]
fn an_element_is_drawn_as_a_card_as_its_notation_says() {
    let f = figures();
    let elements = [f.class, f.association_class, f.nary, f.association, f.generalization, f.package];
    let uml = answers(&f, &elements, |n, st, e| n.drawn_as_card(st, e), |v, st, e| v.drawn_as_card(st, e));
    assert_eq!(uml, [true, true, true, false, false, true]);
    let er = answers(&f, &[f.entity, f.relationship, f.schema], |n, st, e| n.drawn_as_card(st, e), |v, st, e| v.drawn_as_card(st, e));
    assert_eq!(er, [true, false, false]);
    assert!(!f.s.view().drawn_as_card(f.s.doc(), f.class_node), "a node belongs to no notation");
}

/// A package's card is drawn, and hit, under the other figures (AR-570), its node so through the
/// view; nothing else of UML, and nothing of ER by default.
#[test]
fn a_packages_card_is_drawn_behind_and_nothing_else_is() {
    let f = figures();
    let uml = told(&f, &[f.package, f.class, f.association, f.set], |n, st, e| n.drawn_behind(st, e));
    assert_eq!(uml, [true, false, false, false]);
    let er = told(&f, &[f.schema, f.entity, f.relationship], |n, st, e| n.drawn_behind(st, e));
    assert_eq!(er, [false, false, false]);
    let (st, view) = (f.s.doc(), f.s.view());
    let nodes = [f.package_node, f.class_node, f.label_node, f.entity_node].map(|n| view.node_drawn_behind(st, n));
    assert_eq!(nodes, [true, false, false, false]);
    assert!(!view.node_drawn_behind(st, f.package), "an element is no node");
}

/// A generalization set's node holds its label, drawn with its lines (AR-129), its node so through
/// the view; nothing else of UML, and nothing of ER by default.
#[test]
fn a_generalization_sets_node_holds_a_label_and_no_other_does() {
    let f = figures();
    let uml = told(&f, &[f.set, f.generalization, f.class, f.package], |n, st, e| n.is_label(st, e));
    assert_eq!(uml, [true, false, false, false]);
    let er = told(&f, &[f.entity, f.relationship, f.schema], |n, st, e| n.is_label(st, e));
    assert_eq!(er, [false, false, false]);
    let (st, view) = (f.s.doc(), f.s.view());
    let nodes = [f.label_node, f.class_node, f.package_node, f.entity_node].map(|n| view.node_is_label(st, n));
    assert_eq!(nodes, [true, false, false, false]);
    assert!(!view.node_is_label(st, f.set), "an element is no node");
}

/// AR-586: a binding whose diagram or owner the project does not hold, or which names none, is
/// refused as the project opens, typed and naming it, before anything is built; one that names
/// what the project holds opens.
#[test]
fn a_binding_that_names_nothing_refuses_its_model_before_anything_is_built() {
    use archforge_notations::{BindingEnd, BrokenBinding, check_bindings_in_rows};
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let (tables, junctions) = storage();
    let fresh = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    assert_eq!(check_bindings_in_rows(&fresh, &tables), Ok(()));
    let at = tables.iter().position(|t| t.name == "diagram_default_diagram_binding").unwrap();
    let column = |name| tables[at].columns.iter().position(|c| c.name == name).unwrap();
    let Cell::Id(binding) = fresh.tables[at][0][column("id")] else { panic!("the binding has an id") };
    let gone = Uuid::v7(1_761_670_800_000, [7; 10]);
    for (name, cell, end, names) in [
        ("default_diagram", Cell::Id(gone), BindingEnd::Diagram, Some(gone)),
        ("default_diagram", Cell::Null, BindingEnd::Diagram, None),
        ("owner_element", Cell::Id(gone), BindingEnd::Owner, Some(gone)),
    ] {
        let mut broken = fresh.clone();
        broken.tables[at][0][column(name)] = cell;
        let mut built = false;
        let why = session_from_rows_repaired(&broken, ids(1 << 40), |_| built = true).err().unwrap();
        assert!(!built, "nothing is built from refused rows");
        assert_eq!(why, OpenError::BrokenBindings(vec![BrokenBinding { binding, end, names }]), "{name}");
    }
    let mut both = fresh.clone();
    for name in ["owner_element", "default_diagram"] {
        both.tables[at][0][column(name)] = Cell::Id(gone);
    }
    let said = format!("the default-diagram binding {binding} names the owner {gone}, which the project does not hold, and so do 1 more");
    assert_eq!(session_from_rows(&both, ids(1 << 40)).err().unwrap().to_string(), said);
}

/// A commit check for the test alone: it refuses a change that leaves a diagram named Refused.
fn no_diagram_named_refused(st: &Store, _: &archforge_mof::Delta) -> Result<(), CommandError> {
    let named = |o: Obj| st.get(o, archforge_diagram::diagram::NAME).ok().and_then(|v| v.as_str().map(|n| n == "Refused"));
    let refused = st.alive().any(|o| st.class_of(o).ok() == Some(archforge_diagram::DIAGRAM) && named(o) == Some(true));
    if refused { Err(CommandError::new("no diagram is named Refused")) } else { Ok(()) }
}

/// A default removed with a new empty replacement whose commit is refused is taken back whole: no
/// diagram made is left, the binding names the old default, which stays, and no undo step is kept.
/// The same session keeps one under another name, so the check alone refused it (AR-641).
#[test]
fn a_new_replacement_whose_commit_is_refused_is_taken_back_whole() {
    let mut st = store(ids(0)).unwrap();
    st.create(archforge_core::MODEL).unwrap();
    let checked = archforge_notations::registry_checked(no_diagram_named_refused).unwrap();
    let mut s = Session::with_view(checked, st, 1, Options::default(), archforge_notations::view());
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let d = id(&mut s, "diagram.add", json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let (tables, junctions) = storage();
    let state =
        |s: &Session| (rows::snapshot(s.doc(), &tables, &junctions).unwrap(), s.token(), s.undo_label().map(str::to_string), s.undo_revision());
    let remove = |s: &Session, name: &str| {
        let at = s.token();
        json!({ "diagram": d, "create_replacement": { "name": name }, "expected_revision": { "history": at.history, "revision": at.revision } })
    };
    let before = state(&s);
    let Err(archforge_engine::EngineError::Failed { error, .. }) = s.execute("diagram.remove", &remove(&s, "Refused")) else {
        panic!("the commit check refuses the new replacement")
    };
    assert_eq!(error.message, "no diagram is named Refused");
    assert!(state(&s) == before, "no diagram made, the binding as it was, no undo step");
    let kept = s.execute("diagram.remove", &remove(&s, "Sales map")).unwrap().value;
    assert_eq!((&kept["diagram"], &kept["created"]), (&json!(d), &json!(true)));
}
