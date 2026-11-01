//! A project's model as rows of the generated tables and back (archforge-kjz.3): a model with every
//! element kind, its diagram and a skill comes back from its rows identical, object for object and
//! value for value; damaged rows are refused and leave the store as it was.

use std::collections::BTreeMap;

use archforge_engine::Options;
use archforge_er::*;
use archforge_mof::rows::{self, Cell, Rows};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{Store, Uuid, Value};
use serde_json::{Value as Json, json};

fn ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 0;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// Ids from another range, for the store a project is read into.
fn other_ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 1 << 40;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

fn tables() -> Vec<Table> {
    archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_er::TABLES).copied().collect()
}

fn junctions() -> Vec<Junction> {
    archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_er::JUNCTIONS).copied().collect()
}

fn run(s: &mut Session, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

/// Every object by its id: its class's name, its container's id and property, and every stored
/// value with references as ids. Two stores with the same picture hold the same model.
type Picture = BTreeMap<Uuid, (String, Option<(Uuid, String)>, Vec<(String, Vec<String>)>)>;

fn picture(st: &Store) -> Picture {
    let u = st.universe();
    let show = |v: &Value| match v {
        Value::Ref(o) => format!("ref {}", st.uuid(*o).unwrap()),
        other => format!("{other:?}"),
    };
    st.alive()
        .map(|o| {
            let class = st.class_of(o).unwrap();
            let values = u
                .info(class)
                .unwrap()
                .stored
                .iter()
                .map(|p| {
                    let prop = u.prop(*p).unwrap();
                    let vs = if prop.is_many() { st.list(o, *p).unwrap() } else { vec![st.get(o, *p).unwrap()] };
                    (prop.name.to_string(), vs.iter().map(show).collect())
                })
                .collect();
            let container = st.container(o).unwrap().map(|(c, p)| (st.uuid(c).unwrap(), u.prop(p).unwrap().name.to_string()));
            (st.uuid(o).unwrap(), (u.class(class).unwrap().name.to_string(), container, values))
        })
        .collect()
}

/// A project with a schema of every element kind, a second schema reached across, a diagram of
/// the first, a skill and a description.
fn rich_session() -> Session {
    let mut s = session_alone("Commerce platform", ids()).unwrap();
    let shop = run(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let customer = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Customer" }));
    let key = run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT", "primary_key": true }));
    run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "email", "type": "VARCHAR", "length": 255, "unique": true }));
    let order = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Order" }));
    run(&mut s, "er.attribute.add", json!({ "entity": order, "name": "total", "type": "DECIMAL", "precision": 10, "scale": 2, "default": "0" }));
    let places = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places", "create_foreign_key": true }),
    );
    run(&mut s, "er.check.add", json!({ "entity": order, "expression": "total >= 0", "name": "ck_total" }));
    run(&mut s, "er.index.add", json!({ "entity": customer, "columns": [{ "expression": "lower(email)" }], "name": "ix_email" }));
    let money = run(
        &mut s,
        "er.domain.add",
        json!({ "schema": shop, "name": "money", "base_type": "NUMERIC", "precision": 12, "scale": 2, "checks": ["value >= 0"] }),
    );
    run(&mut s, "er.attribute.add", json!({ "entity": order, "name": "tax", "domain": money }));
    let state = run(&mut s, "er.enum.add", json!({ "schema": shop, "name": "state", "literals": ["open", "paid"] }));
    run(&mut s, "er.attribute.add", json!({ "entity": order, "name": "state", "enum": state, "default": "'open'" }));
    let vip = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Vip" }));
    run(&mut s, "er.generalization.add", json!({ "supertype": customer, "subtype": vip }));
    run(&mut s, "er.sequence.add", json!({ "schema": shop, "name": "order_no", "start": 1000, "minimum": "1000", "owned_by": key }));
    run(&mut s, "er.view.add", json!({ "schema": shop, "name": "open_orders", "query": "select * from orders", "materialized": true }));
    run(&mut s, "er.describe", json!({ "element": customer, "text": "Someone who buys." }));
    let crm = run(&mut s, "er.schema.add", json!({ "name": "crm" }));
    let contact = run(&mut s, "er.entity.add", json!({ "schema": crm, "name": "Contact" }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": customer, "to": contact, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    let diagram = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop, "name": "Shop" }));
    let c = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100, "width": 240, "height": 120 }));
    let o = run(&mut s, "diagram.place", json!({ "diagram": diagram, "element": order, "x": 500, "y": 100, "width": 240, "height": 140 }));
    run(&mut s, "diagram.connect", json!({ "diagram": diagram, "element": places, "source": c, "target": o }));
    run(&mut s, "diagram.note", json!({ "diagram": diagram, "text": "Orders are never deleted.", "x": 100, "y": 400 }));
    run(&mut s, "skill.add", json!({ "subject": shop, "name": "Naming", "applies_to": "table", "triggers": ["naming"], "body": "Singular nouns." }));
    s
}

#[test]
fn a_model_with_every_element_kind_comes_back_from_its_rows_identical() {
    let s = rich_session();
    let (tables, junctions) = (tables(), junctions());
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    assert!(saved.len() > 60, "{} rows", saved.len());
    let mut back = store(other_ids()).unwrap();
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(picture(&back), picture(s.doc()));
    assert!(back.validate_all().is_empty(), "{:?}", back.validate_all());
    // The rows read back are the rows written.
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved);
    // A session over the store read back works as one made in place.
    let mut reopened = Session::new(registry().unwrap(), back, 1, Options::default());
    let shop = reopened.doc().alive().find(|o| rules::name_of(reopened.doc(), *o) == "shop").unwrap();
    let shop = reopened.doc().uuid(shop).unwrap().to_string();
    run(&mut reopened, "er.entity.add", json!({ "schema": shop, "name": "Product" }));
    assert!(rules::problems(reopened.doc()).is_empty(), "{:?}", rules::problems(reopened.doc()));
}

fn refused(saved: &Rows) -> String {
    let (tables, junctions) = (tables(), junctions());
    let mut back = store(other_ids()).unwrap();
    let before = picture(&back);
    let e = rows::restore(&mut back, saved, &tables, &junctions).expect_err("damaged rows were taken");
    assert_eq!(picture(&back), before, "a refused read changed the store");
    e.to_string()
}

#[test]
fn damaged_rows_are_refused_whole_and_change_nothing() {
    let s = rich_session();
    let saved = rows::snapshot(s.doc(), &tables(), &junctions()).unwrap();
    let er_entity = tables().iter().position(|t| t.name == "er_entity").unwrap();
    let er_attribute = tables().iter().position(|t| t.name == "er_attribute").unwrap();
    // An id there twice.
    let mut twice = saved.clone();
    let first = twice.tables[er_entity][0].clone();
    twice.tables[er_entity].push(first);
    assert!(refused(&twice).contains("twice"));
    // A reference to no object.
    let mut dangling = saved.clone();
    let row = &mut dangling.tables[er_attribute][0];
    let domain_col = archforge_er::TABLES.iter().find(|t| t.name == "er_attribute").unwrap().columns.iter().position(|c| c.name == "domain").unwrap();
    row[domain_col] = Cell::Id(Uuid::parse("00000000-0000-7000-8000-000000000001").unwrap());
    assert!(refused(&dangling).contains("not in the project"));
    // A value of the wrong kind, a short row, a list with a gap, and rows for other tables.
    let mut wrong = saved.clone();
    let name_col = archforge_er::TABLES.iter().find(|t| t.name == "er_entity").unwrap().columns.iter().position(|c| c.name == "name").unwrap();
    wrong.tables[er_entity][0][name_col] = Cell::Int(7);
    assert!(refused(&wrong).contains("wrong kind"));
    let mut short = saved.clone();
    short.tables[er_entity][0].pop();
    assert!(refused(&short).contains("columns"));
    let mut gap = saved.clone();
    let list = gap.junctions.iter_mut().find(|j| j.len() > 1).unwrap();
    list.retain(|(_, at, _)| *at != 0);
    assert!(refused(&gap).contains("position"));
    let mut other = saved.clone();
    other.tables.pop();
    assert!(refused(&other).contains("do not match"));
}

/// Nodes nested in a chain `n` deep under the diagram's first node, or in a loop of their own.
fn nested(n: u64, looped: bool) -> Rows {
    let s = rich_session();
    let mut saved = rows::snapshot(s.doc(), &tables(), &junctions()).unwrap();
    let nodes = tables().iter().position(|t| t.name == "diagram_node").unwrap();
    let children = junctions().iter().position(|j| j.name == "diagram_node_child").unwrap();
    let template = saved.tables[nodes][0].clone();
    let Cell::Id(top) = template[0] else { panic!("a node row starts with its id") };
    let id = |i: u64| {
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&(i + (1 << 50)).to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    };
    let mut parent = top;
    for i in 0..n {
        let mut row = template.clone();
        row[0] = Cell::Id(id(i));
        row[1] = Cell::Null;
        saved.tables[nodes].push(row);
        if i > 0 || !looped {
            saved.junctions[children].push((parent, 0, Cell::Id(id(i))));
        }
        parent = id(i);
    }
    if looped {
        saved.junctions[children].push((parent, 0, Cell::Id(id(0))));
    }
    saved
}

#[test]
fn a_nesting_too_deep_or_in_a_loop_is_refused_before_anything_is_built() {
    let started = std::time::Instant::now();
    assert!(refused(&nested(100_000, false)).contains("nests deeper"));
    assert!(refused(&nested(40, true)).contains("nests deeper"));
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "the refusal took {:?}", started.elapsed());
    // A nesting within the limit is not refused for its depth.
    let mut back = store(other_ids()).unwrap();
    let within = rows::restore(&mut back, &nested(40, false), &tables(), &junctions());
    assert!(within.as_ref().err().is_none_or(|e| !e.to_string().contains("nests deeper")), "{within:?}");
}

/// `n` nodes all in the diagram's first node's list of children.
fn wide(n: u64) -> Rows {
    let mut saved = nested(0, false);
    let nodes = tables().iter().position(|t| t.name == "diagram_node").unwrap();
    let children = junctions().iter().position(|j| j.name == "diagram_node_child").unwrap();
    let template = saved.tables[nodes][0].clone();
    let Cell::Id(top) = template[0] else { panic!("a node row starts with its id") };
    let before = saved.junctions[children].iter().filter(|(o, _, _)| *o == top).count() as u32;
    for i in 0..n {
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&(i + (1 << 51)).to_be_bytes());
        let id = Uuid::v7(1_761_670_800_000, r);
        let mut row = template.clone();
        row[0] = Cell::Id(id);
        row[1] = Cell::Null;
        saved.tables[nodes].push(row);
        saved.junctions[children].push((top, before + i as u32, Cell::Id(id)));
    }
    saved
}

#[test]
fn a_long_list_is_built_in_linear_time_and_a_value_twice_in_it_is_refused() {
    // No stored property of these metamodels has a stored opposite or subsets another, so adding
    // to a list never searches another list: building is linear.
    let st = store(other_ids()).unwrap();
    for t in archforge_er::storage().0 {
        let info = st.universe().info(t.class).unwrap();
        assert!(info.subsetted.is_empty(), "{} has stored subsets", t.name);
        assert!(info.stored.iter().all(|p| st.universe().prop(*p).unwrap().opposite.is_none()), "{} has a stored opposite", t.name);
    }
    // A list as long as one reading can build (the store's step cap bounds a reading): searching
    // the list for each value would take billions of comparisons.
    let rows = wide(80_000);
    let started = std::time::Instant::now();
    let mut back = store(other_ids()).unwrap();
    rows::restore(&mut back, &rows, &tables(), &junctions()).unwrap();
    // Linear, it takes a fraction of a second; searching the list for each value takes seconds.
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "building took {:?}", started.elapsed());
    let mut twice = wide(3);
    let children = junctions().iter().position(|j| j.name == "diagram_node_child").unwrap();
    let (owner, at, cell) = twice.junctions[children].last().unwrap().clone();
    twice.junctions[children].push((owner, at + 1, cell));
    assert!(refused(&twice).contains("twice"));
}

#[test]
fn an_object_in_two_containers_is_refused_before_anything_is_built() {
    // A deep chain hidden behind a shallow listing of the same nodes: the store would keep the
    // deep one.
    let mut saved = nested(100_000, false);
    let children = junctions().iter().position(|j| j.name == "diagram_node_child").unwrap();
    let chain: Vec<_> = saved.junctions[children].iter().rev().take(100_000).map(|(_, _, c)| c.clone()).collect();
    let nodes = tables().iter().position(|t| t.name == "diagram_node").unwrap();
    let Cell::Id(top) = saved.tables[nodes][0][0] else { panic!("a node row starts with its id") };
    let before = saved.junctions[children].iter().filter(|(o, _, _)| *o == top).count() as u32;
    let mut shallow: Vec<_> = chain.into_iter().enumerate().map(|(k, c)| (top, before + k as u32, c)).collect();
    shallow.append(&mut saved.junctions[children]);
    saved.junctions[children] = shallow;
    let started = std::time::Instant::now();
    assert!(refused(&saved).contains("listed twice"));
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "the refusal took {:?}", started.elapsed());
}

#[test]
fn a_part_listed_twice_by_one_owner_is_refused() {
    // The Model lists its members and its comments in two lists; a part in both would be moved
    // from one to the other while the model is built, and come back changed.
    let mut saved = rows::snapshot(rich_session().doc(), &tables(), &junctions()).unwrap();
    let members = junctions().iter().position(|j| j.name == "core_model_member").unwrap();
    let comments = junctions().iter().position(|j| j.name == "core_model_owned_comment").unwrap();
    let (owner, _, part) = saved.junctions[members][0].clone();
    let at = saved.junctions[comments].iter().filter(|(o, _, _)| *o == owner).count() as u32;
    saved.junctions[comments].push((owner, at, part));
    assert!(refused(&saved).contains("listed twice"));
}
