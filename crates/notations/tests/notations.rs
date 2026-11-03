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
    assert!(e.contains("DuplicateName"), "{e}");
}

#[test]
fn a_skill_aims_only_at_kinds_of_its_subjects_notation_and_moves_only_within_it() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let package = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let class = id(&mut s, "uml.class.add", json!({ "package": package, "name": "Order" }));
    let refused = |s: &mut Session, cmd: &str, p: Json| s.execute(cmd, &p).map(|o| o.value.to_string()).unwrap_err().to_string();
    let e = refused(&mut s, "skill.add", json!({ "subject": package, "name": "Tables", "applies_to": "table" }));
    assert!(e.contains("class, attribute, operation, association") && !e.contains("column"), "{e}");
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
    let diagram = id(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": orders, "name": "Orders" }));
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

#[test]
fn a_diagrams_skills_go_with_it_whether_it_or_its_element_goes_and_come_back_with_undo() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = id(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let schema = id(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let class_diagram = id(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "Sales" }));
    let er_diagram = id(&mut s, "diagram.add", json!({ "kind": "er", "element": schema, "name": "shop" }));
    id(&mut s, "skill.add", json!({ "subject": class_diagram, "name": "Layout" }));
    id(&mut s, "skill.add", json!({ "subject": er_diagram, "name": "Layout" }));
    assert_eq!(skill_count(&s), 2);
    // The diagram removed alone.
    s.execute("diagram.remove", &json!({ "diagram": class_diagram })).unwrap();
    assert_eq!(skill_count(&s), 1, "its skill went with it, none turned model-wide");
    s.undo().unwrap();
    assert_eq!(skill_count(&s), 2);
    // With the element it shows, in either notation.
    for element in [&sales, &schema] {
        s.execute("element.remove", &json!({ "ids": [element] })).unwrap();
        assert_eq!(skill_count(&s), 1, "{element}");
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
    let diagram = id(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": orders, "name": "Orders" }));
    id(&mut s, "skill.add", json!({ "subject": sales, "name": "Sales style" }));
    id(&mut s, "skill.add", json!({ "subject": orders, "name": "Orders style" }));
    id(&mut s, "skill.add", json!({ "subject": diagram, "name": "Layout" }));
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
