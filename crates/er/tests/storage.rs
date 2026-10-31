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
    let mut s = new_project_session("Commerce platform", ids()).unwrap();
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
