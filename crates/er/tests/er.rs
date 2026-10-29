//! The ER notation through the engine, on the MOF core's store with the shared core: every
//! command's round trip, refusals that change nothing, cascades with the shared core's comments and
//! links around a schema, the sweep, and seeded random sequences.

use archforge_core::{COMMENT, CommentObj, FOLDER, FolderObj, LinkKind, MODEL, ModelObj, comment, model};
use archforge_engine::testing::sweep_with;
use archforge_engine::{Document, EngineError, Options};
use archforge_er::rules::problems;
use archforge_er::*;
use archforge_mof::{ClassId, Obj, PropId, Store, Uuid, Value};
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

/// A store over the shared core and ER that already holds the project's Model.
fn rooted_store() -> (Store, Obj) {
    let mut st = store(ids()).unwrap();
    let m = st.create(MODEL).unwrap();
    (st, m)
}

fn session() -> Session {
    let (st, _) = rooted_store();
    Session::new(registry().unwrap(), st, 1, Options::default())
}

fn run(s: &mut Session, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// A schema with two entities, a keyed attribute, another attribute and a relationship.
struct Sample {
    schema: String,
    customer: String,
    order: String,
    key: String,
    total: String,
    places: String,
}

fn sample(s: &mut Session) -> Sample {
    let schema = run(s, "er.schema.add", json!({ "name": "shop" }));
    let customer = run(s, "er.entity.add", json!({ "schema": schema, "name": "Customer" }));
    let order = run(s, "er.entity.add", json!({ "schema": schema, "name": "Order" }));
    let key = run(s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT", "primary_key": true }));
    let total = run(s, "er.attribute.add", json!({ "entity": order, "name": "total", "type": "DECIMAL(10,2)" }));
    let places = run(
        s,
        "er.relationship.add",
        json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places" }),
    );
    Sample { schema, customer, order, key, total, places }
}

type Picture = Vec<(Obj, ClassId, Option<(Obj, PropId)>, Vec<(PropId, Vec<Value>)>)>;

/// Everything observable: every live object with its class, container and stored values.
fn picture(st: &Store) -> Picture {
    let u = st.universe();
    st.alive()
        .map(|o| {
            let class = st.class_of(o).unwrap();
            let values = u
                .info(class)
                .unwrap()
                .stored
                .iter()
                .map(|p| (*p, if u.prop(*p).unwrap().is_many() { st.list(o, *p).unwrap() } else { vec![st.get(o, *p).unwrap()] }))
                .collect();
            (o, class, st.container(o).unwrap(), values)
        })
        .collect()
}

/// Runs a command, undoes and redoes it, and checks the store's picture at each step.
fn round_trip(s: &mut Session, cmd: &str, p: &Json) {
    let before = picture(s.doc());
    s.execute(cmd, p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    let after = picture(s.doc());
    if after == before {
        return;
    }
    s.undo().unwrap_or_else(|e| panic!("undo of {cmd}: {e}"));
    assert_eq!(picture(s.doc()), before, "undo of {cmd}");
    s.redo().unwrap_or_else(|e| panic!("redo of {cmd}: {e}"));
    assert_eq!(picture(s.doc()), after, "redo of {cmd}");
}

#[test]
fn the_registry_and_the_metamodel_are_what_the_spec_says() {
    let reg = registry().unwrap();
    assert!(reg.len() > 20);
    let m = metamodel().unwrap();
    assert_eq!(m.classes.len(), 10);
    assert_eq!(
        m.data_types.len(),
        10,
        "the five primitives and the enumerations Cardinality, EntityKind, ReferentialAction, MatchType and Deferrability"
    );
    assert_eq!(Cardinality::from_index(1), Some(Cardinality::ExactlyOne));
    assert_eq!(commands::cardinality_name(Cardinality::ZeroOrMore), "zero-or-more");
    assert!(TABLES.iter().any(|t| t.name == "er_attribute") && JUNCTIONS.iter().any(|j| j.name == "er_entity_attribute" && j.ordered));
}

#[test]
fn every_command_applies_undoes_and_redoes_to_the_identical_store() {
    let mut s = session();
    let sm = sample(&mut s);
    let calls: Vec<(&str, Json)> = vec![
        ("er.schema.add", json!({ "name": "other" })),
        ("er.schema.rename", json!({ "schema": sm.schema, "name": "sales" })),
        ("er.entity.add", json!({ "schema": sm.schema, "name": "Product" })),
        ("er.entity.rename", json!({ "entity": sm.order, "name": "Purchase" })),
        ("er.attribute.add", json!({ "entity": sm.customer, "name": "email", "at": 0 })),
        ("er.attribute.rename", json!({ "attribute": sm.key, "name": "customer_id" })),
        ("er.attribute.set-type", json!({ "attribute": sm.key, "type": "UUID" })),
        ("er.attribute.set-flags", json!({ "attribute": sm.key, "primary_key": false, "nullable": "auto", "unique": true })),
        ("er.relationship.set-ends", json!({ "relationship": sm.places, "from_end": "zero-or-one", "to_end": "one-or-more" })),
        ("er.relationship.rename", json!({ "relationship": sm.places, "name": "" })),
        ("er.relationship.remove", json!({ "relationship": sm.places })),
        ("er.attribute.remove", json!({ "attribute": sm.total })),
        ("er.entity.remove", json!({ "entity": sm.order })),
        ("er.schema.remove", json!({ "schema": sm.schema })),
    ];
    for (cmd, p) in calls {
        round_trip(&mut s, cmd, &p);
        assert_eq!(problems(s.doc()), Vec::<String>::new(), "{cmd}");
    }
    let schema = run(&mut s, "er.schema.add", json!({ "name": "last" }));
    let e = run(&mut s, "er.entity.add", json!({ "schema": schema, "name": "E" }));
    let a = run(&mut s, "er.attribute.add", json!({ "entity": e, "name": "x" }));
    run(&mut s, "er.attribute.add", json!({ "entity": e, "name": "y" }));
    round_trip(&mut s, "er.attribute.move", &json!({ "attribute": a, "to": 1 }));
    assert_eq!(s.execute("er.entity.get", &json!({ "entity": e })).unwrap().value["attributes"][1]["name"], "x");
}

#[test]
fn refusals_name_their_key_and_change_nothing() {
    let mut s = session();
    let sm = sample(&mut s);
    let before = (picture(s.doc()), s.view().clone(), s.counter());
    let refused: Vec<(&str, Json)> = vec![
        ("er.schema.add", json!({ "name": "" })),
        ("er.entity.add", json!({ "schema": sm.schema, "name": "customer" })),
        ("er.entity.add", json!({ "schema": sm.schema, "name": "" })),
        ("er.entity.add", json!({ "schema": "not-an-id", "name": "X" })),
        ("er.entity.add", json!({ "schema": sm.customer, "name": "X" })),
        ("er.entity.rename", json!({ "entity": sm.key, "name": "X" })),
        ("er.attribute.add", json!({ "entity": sm.customer, "name": "ID" })),
        ("er.attribute.add", json!({ "entity": sm.customer, "name": "x", "primary_key": true, "nullable": "yes" })),
        ("er.attribute.add", json!({ "entity": sm.customer, "name": "x", "at": 5 })),
        ("er.attribute.add", json!({ "entity": sm.customer, "name": "x", "type": "a\tb" })),
        ("er.attribute.move", json!({ "attribute": sm.key, "to": 3 })),
        ("er.attribute.set-flags", json!({ "attribute": sm.key, "primary_key": true, "nullable": "yes", "unique": false })),
        (
            "er.relationship.add",
            json!({ "from": sm.customer, "to": "00000000-0000-7000-8000-000000000000", "from_end": "exactly-one", "to_end": "exactly-one" }),
        ),
        ("er.relationship.add", json!({ "from": sm.customer, "to": sm.order, "from_end": "many", "to_end": "exactly-one" })),
        ("er.remove", json!({ "ids": [] })),
        ("er.remove", json!({ "ids": [sm.customer, sm.customer] })),
        ("er.remove", json!({ "ids": ["not-an-id"] })),
        ("view.select", json!({ "ids": [sm.customer, sm.customer] })),
    ];
    for (cmd, p) in &refused {
        let e = s.execute(cmd, p).expect_err(&format!("{cmd} {p} was accepted"));
        if let EngineError::Failed { error, .. } = &e {
            assert!(error.key.is_some() || !error.message.is_empty(), "{cmd}: {e}");
        }
        assert_eq!((picture(s.doc()), s.view().clone(), s.counter()), before, "{cmd} {p} changed something");
    }
    let _ = sm.total;
}

#[test]
fn removing_an_entity_takes_its_attributes_relationships_and_links_in_one_step() {
    // Built on the store directly, so the shared core's folder, comment and link can sit around it.
    let (mut st, m) = rooted_store();
    let folder = st.create(FOLDER).unwrap();
    FolderObj::set_name(&mut st, folder, Some("Data")).unwrap();
    ModelObj::push_member(&mut st, m, Value::Ref(folder)).unwrap();
    let sch = st.create(SCHEMA).unwrap();
    SchemaObj::set_name(&mut st, sch, Some("shop")).unwrap();
    FolderObj::push_member(&mut st, folder, Value::Ref(sch)).unwrap();
    let customer = st.create(ENTITY).unwrap();
    EntityObj::set_name(&mut st, customer, Some("Customer")).unwrap();
    SchemaObj::push_entity(&mut st, sch, Value::Ref(customer)).unwrap();
    let order = st.create(ENTITY).unwrap();
    EntityObj::set_name(&mut st, order, Some("Order")).unwrap();
    SchemaObj::push_entity(&mut st, sch, Value::Ref(order)).unwrap();
    let key = st.create(ATTRIBUTE).unwrap();
    AttributeObj::set_name(&mut st, key, Some("id")).unwrap();
    AttributeObj::set_nullable(&mut st, key, false).unwrap();
    EntityObj::push_attribute(&mut st, customer, Value::Ref(key)).unwrap();
    let pk = st.create(PRIMARY_KEY).unwrap();
    PrimaryKeyObj::push_column(&mut st, pk, Value::Ref(key)).unwrap();
    EntityObj::set_primary_key(&mut st, customer, Some(pk)).unwrap();
    let places = st.create(RELATIONSHIP).unwrap();
    RelationshipObj::set_from(&mut st, places, Some(customer)).unwrap();
    RelationshipObj::set_to(&mut st, places, Some(order)).unwrap();
    SchemaObj::push_relationship(&mut st, sch, Value::Ref(places)).unwrap();
    let c = st.create(COMMENT).unwrap();
    CommentObj::set_body(&mut st, c, Some("the buyer")).unwrap();
    CommentObj::push_annotated_element(&mut st, c, Value::Ref(customer)).unwrap();
    ModelObj::push_comment(&mut st, m, Value::Ref(c)).unwrap();
    let link = archforge_core::link(&mut st, m, LinkKind::Persistence, order, customer, None).unwrap();
    assert_eq!(st.container(sch).unwrap(), Some((folder, archforge_core::folder::MEMBER)), "a folder holds the schema");
    assert_eq!(problems(&st), Vec::<String>::new());
    let before = picture(&st);
    st.begin();
    let gone = commands::remove_entity(&mut st, customer).unwrap();
    let delta = st.end().unwrap();
    assert_eq!(gone, 5, "the entity, its attribute and primary key, the relationship and the link");
    assert!(!st.is_alive(customer) && !st.is_alive(key) && !st.is_alive(pk) && !st.is_alive(places) && !st.is_alive(link));
    assert_eq!(st.list(c, comment::ANNOTATED_ELEMENT).unwrap(), vec![], "the comment let go");
    assert_eq!(st.list(m, model::LINK).unwrap(), vec![]);
    assert_eq!(st.list(sch, schema::ENTITY).unwrap(), vec![Value::Ref(order)]);
    assert_eq!(problems(&st), Vec::<String>::new());
    st.apply(&Store::invert(&delta)).unwrap();
    assert_eq!(picture(&st), before);
    // Through the engine: a mixed delete with an attribute and its entity both listed is one step.
    let mut s = session();
    let sm = sample(&mut s);
    let before = picture(s.doc());
    let out = s.execute("er.remove", &json!({ "ids": [sm.key, sm.customer, sm.places] })).unwrap();
    assert_eq!(out.value["removed"], 4, "the relationship, the key column with the primary key it was in, the entity");
    assert_eq!(s.doc().list(obj(s.doc(), &sm.schema), schema::ENTITY).unwrap(), vec![Value::Ref(obj(s.doc(), &sm.order))]);
    assert!(!s.view().selection.iter().any(|o| !s.doc().is_alive(*o)));
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

fn get(s: &mut Session, cmd: &str, p: Json) -> Json {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}")).value
}

fn all_rules_hold(st: &Store) {
    assert_eq!(problems(st), Vec::<String>::new());
    assert_eq!(archforge_core::problems(st), Vec::new());
    assert_eq!(archforge_diagram::rules::problems(st), Vec::<String>::new());
}

#[test]
fn foreign_keys_round_trip_keep_their_rules_and_go_with_what_they_depend_on() {
    let mut s = session();
    let sm = sample(&mut s);
    let fk_col = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "customer_id", "type": "BIGINT", "nullable": "no" }));
    let add = json!({ "entity": sm.order, "references": sm.customer, "columns": [fk_col], "name": "fk_order_customer", "relationship": sm.places });
    round_trip(&mut s, "er.foreign-key.add", &add);
    let order = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let fk = order["foreign_keys"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(order["foreign_keys"][0]["references"], sm.customer);
    assert_eq!(order["foreign_keys"][0]["relationship"], sm.places);
    assert_eq!(order["foreign_keys"][0]["on_delete"], "no-action");
    assert_eq!(order["attributes"][1]["foreign_key"], true, "the column knows it is in a key");
    all_rules_hold(s.doc());
    round_trip(&mut s, "er.foreign-key.set-actions", &json!({ "foreign_key": fk, "on_delete": "cascade", "on_update": "restrict" }));
    round_trip(&mut s, "er.foreign-key.rename", &json!({ "foreign_key": fk, "name": "fk_customer" }));
    let before = picture(s.doc());
    for (cmd, p) in [
        ("er.foreign-key.set-actions", json!({ "foreign_key": fk, "on_delete": "set-null", "on_update": "no-action" })),
        ("er.attribute.set-type", json!({ "attribute": fk_col, "type": "TEXT" })),
        ("er.attribute.set-type", json!({ "attribute": sm.key, "type": "TEXT" })),
        ("er.attribute.set-flags", json!({ "attribute": sm.key, "primary_key": false, "nullable": "auto", "unique": false })),
        ("er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "columns": [sm.total] })),
        ("er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "columns": [] })),
        ("er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "columns": [sm.key] })),
        ("er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "columns": [fk_col], "name": "FK_CUSTOMER" })),
        ("er.entity.set-kind", json!({ "entity": sm.order, "kind": "associative" })),
    ] {
        let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was accepted"));
        assert!(!e.to_string().is_empty());
        assert_eq!(picture(s.doc()), before, "{cmd} {p} changed something");
    }
    // Removing the referenced key column takes the foreign key with it, in one step.
    let out = get(&mut s, "er.attribute.remove", json!({ "attribute": sm.key }));
    assert_eq!(out["removed"], 3, "the attribute, the primary key it was in, and the foreign key onto that key");
    assert!(get(&mut s, "er.entity.get", json!({ "entity": sm.order }))["foreign_keys"].as_array().unwrap().is_empty());
    all_rules_hold(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    round_trip(&mut s, "er.foreign-key.remove", &json!({ "foreign_key": fk }));
}

#[test]
fn a_relationship_can_bring_its_foreign_key() {
    let mut s = session();
    let sm = sample(&mut s);
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "sku", "type": "TEXT", "primary_key": true }));
    let add = json!({ "from": sm.customer, "to": product, "from_end": "zero-or-one", "to_end": "zero-or-more", "name": "favours", "create_foreign_key": true });
    round_trip(&mut s, "er.relationship.add", &add);
    let p = get(&mut s, "er.entity.get", json!({ "entity": product }));
    let col = &p["attributes"][1];
    assert_eq!(
        (col["name"].as_str(), col["type"].as_str(), col["nullable"].as_bool(), col["primary_key"].as_bool()),
        (Some("customer_id"), Some("BIGINT"), Some(true), Some(false))
    );
    assert_eq!(p["foreign_keys"][0]["references"], sm.customer);
    all_rules_hold(s.doc());
    // Identifying: the column joins the child's key and is not nullable.
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    let refused = s.execute("er.relationship.add", &json!({ "from": sm.order, "to": line, "create_foreign_key": true, "identifying": true }));
    assert!(refused.is_err(), "Order has no primary key yet, so there is nothing to refer to");
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let r = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "one-or-more", "create_foreign_key": true, "identifying": true }),
    );
    let l = get(&mut s, "er.entity.get", json!({ "entity": line }));
    assert_eq!((l["attributes"][0]["name"].as_str(), l["attributes"][0]["primary_key"].as_bool()), (Some("order_order_id"), Some(true)));
    let schema = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert!(schema["relationships"].as_array().unwrap().iter().any(|x| x["id"] == r && x["identifying"] == true));
    all_rules_hold(s.doc());
    for p in [
        json!({ "from": sm.customer, "to": product, "from_end": "zero-or-more", "to_end": "zero-or-more", "create_foreign_key": true }),
        json!({ "from": sm.customer, "to": product, "identifying": true }),
        json!({ "from": sm.customer, "to": sm.customer, "create_foreign_key": true }),
    ] {
        assert!(s.execute("er.relationship.add", &p).is_err(), "{p}");
    }
    round_trip(&mut s, "er.relationship.set-identifying", &json!({ "relationship": r, "identifying": false }));
    assert!(
        s.execute("er.relationship.set-identifying", &json!({ "relationship": sm.places, "identifying": true })).is_err(),
        "places has no foreign key"
    );
}

#[test]
fn a_many_to_many_relationship_resolves_into_a_mapping_table_on_every_diagram() {
    let mut s = session();
    let sm = sample(&mut s);
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "sku", "type": "TEXT", "primary_key": true }));
    let m2n = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.order, "to": product, "from_end": "zero-or-more", "to_end": "one-or-more", "name": "contains" }),
    );
    let schema = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert_eq!(schema["notices"][0]["relationship"], m2n, "many-to-many is noticed, not refused");
    let d = run(&mut s, "diagram.add", json!({ "kind": "er", "element": sm.schema }));
    let a = run(&mut s, "diagram.place", json!({ "diagram": d, "element": sm.order, "x": 0, "y": 0 }));
    let b = run(&mut s, "diagram.place", json!({ "diagram": d, "element": product, "x": 400, "y": 0 }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": m2n, "source": a, "target": b }));
    let before = picture(s.doc());
    let m = run(&mut s, "er.relationship.resolve", json!({ "relationship": m2n }));
    let t = get(&mut s, "er.entity.get", json!({ "entity": m }));
    assert_eq!(t["name"], "order_product");
    assert_eq!(t["kind"], "associative");
    let names: Vec<&str> = t["attributes"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["order_order_id", "product_sku"]);
    assert!(t["attributes"].as_array().unwrap().iter().all(|x| x["primary_key"] == true && x["nullable"] == false && x["foreign_key"] == true));
    assert_eq!(t["foreign_keys"].as_array().unwrap().len(), 2);
    let schema = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert!(schema["notices"].as_array().unwrap().is_empty());
    assert!(!schema["relationships"].as_array().unwrap().iter().any(|x| x["id"] == m2n), "the many-to-many is gone");
    assert_eq!(
        schema["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["to"] == m && x["identifying"] == true && x["to_end"] == "zero-or-more")
            .count(),
        2
    );
    let diagram = get(&mut s, "diagram.get", json!({ "diagram": d }));
    let node = diagram["nodes"].as_array().unwrap().iter().find(|n| n["element"] == m).expect("the mapping table is placed");
    assert_eq!((node["x"].as_f64(), node["width"].as_f64()), (Some(200.0 + 80.0 - 80.0), Some(160.0)), "midway between the two entities");
    assert_eq!(diagram["edges"].as_array().unwrap().len(), 2);
    assert!(diagram["edges"].as_array().unwrap().iter().all(|e| e["target"] == node["id"]));
    let mut sources: Vec<&str> = diagram["edges"].as_array().unwrap().iter().map(|e| e["source"].as_str().unwrap()).collect();
    sources.sort_unstable();
    let mut ends = [a.as_str(), b.as_str()];
    ends.sort_unstable();
    assert_eq!(sources, ends, "one line from each end");
    all_rules_hold(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one step back");
    s.redo().unwrap();
    // Losing one of its keys makes the mapping table a regular entity again.
    let fk = t["foreign_keys"][0]["id"].as_str().unwrap().to_string();
    run(&mut s, "er.foreign-key.remove", json!({ "foreign_key": fk }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": m }))["kind"], "regular");
    all_rules_hold(s.doc());
    assert!(s.execute("er.entity.set-kind", &json!({ "entity": m, "kind": "associative" })).is_err());
    assert!(s.execute("er.relationship.resolve", &json!({ "relationship": sm.places })).is_err(), "places is one to many");
}

/// Runs a command that must be refused: it changes nothing, and the refusal names its parameter.
fn refused_on(s: &mut Session, cmd: &str, p: Json) -> String {
    let before = picture(s.doc());
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was accepted"));
    assert_eq!(picture(s.doc()), before, "{cmd} {p} changed something");
    match e {
        EngineError::Failed { error, .. } => error.key.unwrap_or_default(),
        other => panic!("{cmd} {p}: {other}"),
    }
}

fn relationship(s: &mut Session, schema: &str, r: &str) -> Json {
    get(s, "er.schema.get", json!({ "schema": schema }))["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == r)
        .cloned()
        .unwrap_or(Json::Null)
}

#[test]
fn a_new_key_column_never_breaks_a_foreign_key_that_targets_the_key() {
    let mut s = session();
    let sm = sample(&mut s);
    let col = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "customer_id", "type": "BIGINT" }));
    run(&mut s, "er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "columns": [col] }));
    // Customer's key would become (id, region), which the foreign key no longer covers.
    let key = refused_on(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "region", "type": "TEXT", "primary_key": true }));
    assert_eq!(key, "primary_key");
    run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "region", "type": "TEXT" }));
    // An identifying relationship adds key columns to its child; a key that targets the child refuses it.
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    run(&mut s, "er.attribute.add", json!({ "entity": line, "name": "line_no", "type": "INT", "primary_key": true }));
    let shipment = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Shipment" }));
    let line_ref = run(&mut s, "er.attribute.add", json!({ "entity": shipment, "name": "line_ref", "type": "INT" }));
    run(&mut s, "er.foreign-key.add", json!({ "entity": shipment, "references": line, "columns": [line_ref] }));
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let add =
        json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true });
    assert_eq!(refused_on(&mut s, "er.relationship.add", add), "identifying");
    all_rules_hold(s.doc());
}

#[test]
fn foreign_key_refusals_name_the_parameter_at_fault() {
    let mut s = session();
    let sm = sample(&mut s);
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    let sku = run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "sku", "type": "TEXT", "primary_key": true }));
    let region = run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "region", "type": "TEXT", "primary_key": true }));
    let a = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "product_sku", "type": "text", "nullable": "yes" }));
    let b = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "product_region", "type": "TEXT", "nullable": "yes" }));
    let other = run(&mut s, "er.schema.add", json!({ "name": "billing" }));
    let invoice = run(&mut s, "er.entity.add", json!({ "schema": other, "name": "Invoice" }));
    run(&mut s, "er.attribute.add", json!({ "entity": invoice, "name": "number", "type": "TEXT", "primary_key": true }));
    let fk = |columns: Json| json!({ "entity": sm.order, "references": product, "columns": columns });
    for (p, key) in [
        (fk(json!([a])), "columns"),
        (fk(json!([a, a])), "columns"),
        (fk(json!([sku, b])), "columns"),
        (fk(json!(vec![a.clone(); 33])), "columns"),
        (json!({ "entity": sm.order, "references": invoice, "columns": [a] }), "key"),
        (json!({ "entity": sm.order, "key": sm.order, "columns": [a] }), "key"),
        (json!({ "entity": sm.order, "references": product, "columns": [a, b], "relationship": sm.places }), "relationship"),
    ] {
        assert_eq!(refused_on(&mut s, "er.foreign-key.add", p.clone()), key, "{p}");
    }
    // Two columns over a two-column key; types compare ignoring case; set null on nullable columns.
    let two = json!({ "entity": sm.order, "references": product, "columns": [a, b], "on_delete": "set-null", "match": "full", "deferrable": "initially-deferred" });
    round_trip(&mut s, "er.foreign-key.add", &two);
    all_rules_hold(s.doc());
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    assert_eq!((o["foreign_keys"][0]["match"].as_str(), o["foreign_keys"][0]["deferrable"].as_str()), (Some("full"), Some("initially-deferred")));
    assert_eq!(o["foreign_keys"][0]["targets"], json!([sku, region]), "matched to the key's columns by position");
    let total_fk = json!({ "entity": sm.order, "references": product, "columns": [sm.total, b] });
    assert_eq!(refused_on(&mut s, "er.foreign-key.add", total_fk), "columns", "DECIMAL is not TEXT");
}

#[test]
fn removals_leave_no_identifying_relationship_without_its_key() {
    let mut s = session();
    let sm = sample(&mut s);
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    let add =
        json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true });
    let r = run(&mut s, "er.relationship.add", add);
    let fk = get(&mut s, "er.entity.get", json!({ "entity": line }))["foreign_keys"][0]["id"].as_str().unwrap().to_string();
    run(&mut s, "er.foreign-key.remove", json!({ "foreign_key": fk }));
    assert_eq!(relationship(&mut s, &sm.schema, &r)["identifying"], false);
    all_rules_hold(s.doc());
    s.undo().unwrap();
    assert_eq!(relationship(&mut s, &sm.schema, &r)["identifying"], true);
    // Removing the relationship keeps the key and clears its link to the relationship.
    run(&mut s, "er.remove", json!({ "ids": [r] }));
    let l = get(&mut s, "er.entity.get", json!({ "entity": line }));
    assert_eq!(l["foreign_keys"][0]["relationship"], Json::Null);
    all_rules_hold(s.doc());
    s.undo().unwrap();
    // A removal that names a key and what the key depends on takes both in one call.
    let out = get(&mut s, "er.remove", json!({ "ids": [sm.order, fk] }));
    assert!(out["removed"].as_u64().unwrap() >= 4);
    assert!(get(&mut s, "er.entity.get", json!({ "entity": line }))["foreign_keys"].as_array().unwrap().is_empty());
    all_rules_hold(s.doc());
}

#[test]
fn generated_names_step_aside_and_one_to_one_keys_are_unique_together() {
    let mut s = session();
    let sm = sample(&mut s);
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "sku", "type": "TEXT", "primary_key": true }));
    run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "customer_id", "type": "BIGINT" }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.customer, "to": product, "from_end": "exactly-one", "to_end": "zero-or-one", "create_foreign_key": true }),
    );
    let p = get(&mut s, "er.entity.get", json!({ "entity": product }));
    assert_eq!((p["attributes"][2]["name"].as_str(), p["attributes"][2]["unique"].as_bool()), (Some("customer_id_2"), Some(true)));
    // A one-to-one over a two-column key makes the pair unique together, neither column alone.
    let region = json!({ "entity": sm.customer, "name": "region", "type": "TEXT", "primary_key": true });
    let fk = p["foreign_keys"][0]["id"].as_str().unwrap().to_string();
    run(&mut s, "er.foreign-key.remove", json!({ "foreign_key": fk }));
    run(&mut s, "er.attribute.add", region);
    let profile = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Profile" }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.customer, "to": profile, "from_end": "exactly-one", "to_end": "zero-or-one", "create_foreign_key": true }),
    );
    let pr = get(&mut s, "er.entity.get", json!({ "entity": profile }));
    assert!(pr["attributes"].as_array().unwrap().iter().all(|x| x["unique"] == false));
    assert_eq!(pr["unique_constraints"][0]["columns"].as_array().map(Vec::len), Some(2));
    all_rules_hold(s.doc());
    // Resolve: the default name steps aside, a given name that is taken is refused.
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "order_product" }));
    let m2n = json!({ "from": sm.order, "to": product, "from_end": "zero-or-more", "to_end": "zero-or-more" });
    let r = run(&mut s, "er.relationship.add", m2n.clone());
    assert_eq!(refused_on(&mut s, "er.relationship.resolve", json!({ "relationship": r, "name": "Customer" })), "name");
    let m = run(&mut s, "er.relationship.resolve", json!({ "relationship": r }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": m }))["name"], "order_product_2");
    let r = run(&mut s, "er.relationship.add", m2n);
    let m = run(&mut s, "er.relationship.resolve", json!({ "relationship": r, "name": "order_lines" }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": m }))["name"], "order_lines");
    // Removing an end of a mapping table makes it a regular entity.
    run(&mut s, "er.entity.remove", json!({ "entity": product }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": m }))["kind"], "regular");
    all_rules_hold(s.doc());
}

#[test]
fn a_many_to_many_relationship_of_an_entity_with_itself_resolves_beside_it() {
    let mut s = session();
    let schema = run(&mut s, "er.schema.add", json!({ "name": "plant" }));
    let part = run(&mut s, "er.entity.add", json!({ "schema": schema, "name": "Part" }));
    run(&mut s, "er.attribute.add", json!({ "entity": part, "name": "id", "type": "BIGINT", "primary_key": true }));
    let r = run(&mut s, "er.relationship.add", json!({ "from": part, "to": part, "from_end": "zero-or-more", "to_end": "zero-or-more" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "er", "element": schema }));
    let n = run(&mut s, "diagram.place", json!({ "diagram": d, "element": part, "x": 0, "y": 0 }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": r, "source": n, "target": n }));
    let m = run(&mut s, "er.relationship.resolve", json!({ "relationship": r }));
    let t = get(&mut s, "er.entity.get", json!({ "entity": m }));
    assert_eq!(t["name"], "part_part");
    let names: Vec<&str> = t["attributes"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["part_id", "part_id_2"]);
    let diagram = get(&mut s, "diagram.get", json!({ "diagram": d }));
    let node = diagram["nodes"].as_array().unwrap().iter().find(|x| x["element"] == m).unwrap();
    assert!(node["x"].as_f64().unwrap() > 0.0, "beside the part, not on top of it");
    assert!(diagram["edges"].as_array().unwrap().iter().all(|e| e["source"] == n && e["target"] == node["id"]));
    all_rules_hold(s.doc());
}

#[test]
fn keys_are_named_constraints_with_their_own_column_order() {
    let mut s = session();
    let sm = sample(&mut s);
    let region = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "region", "type": "TEXT", "nullable": "yes" }));
    let email = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "email", "type": "TEXT" }));
    let number = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "number", "type": "INT" }));
    // The key's order is its own, its columns become not nullable, and it takes a name.
    round_trip(&mut s, "er.primary-key.set", &json!({ "entity": sm.customer, "columns": [region, sm.key], "name": "pk_customer" }));
    assert!(
        s.execute("er.primary-key.set", &json!({ "entity": sm.customer, "columns": [sm.key] })).is_err(),
        "the name is required, so it is never reset"
    );
    let c = get(&mut s, "er.entity.get", json!({ "entity": sm.customer }));
    assert_eq!(c["primary_key"]["columns"], json!([region, sm.key]));
    assert_eq!(c["primary_key"]["name"], "pk_customer");
    assert_eq!(c["attributes"][1]["nullable"], false, "a key column is not nullable");
    assert_eq!((c["attributes"][1]["primary_key"].as_bool(), c["attributes"][2]["primary_key"].as_bool()), (Some(true), Some(false)));
    // A composite unique constraint, with nulls not distinct.
    let add = json!({ "entity": sm.customer, "columns": [region, number], "name": "uq_customer_number", "nulls_distinct": false });
    round_trip(&mut s, "er.unique.add", &add);
    let c = get(&mut s, "er.entity.get", json!({ "entity": sm.customer }));
    let u = c["unique_constraints"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        (c["unique_constraints"][0]["columns"].clone(), c["unique_constraints"][0]["nulls_distinct"].clone()),
        (json!([region, number]), json!(false))
    );
    all_rules_hold(s.doc());
    for (cmd, p, key) in [
        ("er.primary-key.set", json!({ "entity": sm.customer, "columns": [], "name": "" }), "columns"),
        ("er.primary-key.set", json!({ "entity": sm.customer, "columns": [sm.total], "name": "" }), "columns"),
        ("er.primary-key.set", json!({ "entity": sm.customer, "columns": [sm.key], "name": "UQ_CUSTOMER_NUMBER" }), "name"),
        ("er.unique.add", json!({ "entity": sm.customer, "columns": [email], "name": "pk_customer" }), "name"),
        ("er.unique.add", json!({ "entity": sm.customer, "columns": [email, email] }), "columns"),
        ("er.unique.add", json!({ "entity": sm.customer, "columns": [] }), "columns"),
        ("er.unique.set", json!({ "unique": sm.key, "columns": [email], "name": "", "nulls_distinct": true }), "unique"),
        ("er.unique.set", json!({ "unique": u, "columns": [region, number], "name": "PK_CUSTOMER", "nulls_distinct": false }), "name"),
        ("er.primary-key.remove", json!({ "entity": sm.order }), "entity"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    round_trip(&mut s, "er.unique.set", &json!({ "unique": u, "columns": [number, region], "name": "", "nulls_distinct": false }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["unique_constraints"][0]["nulls_distinct"], false);
    // The Unique checkbox makes and removes a one-column constraint.
    round_trip(&mut s, "er.attribute.set-flags", &json!({ "attribute": email, "primary_key": false, "nullable": "yes", "unique": true }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["attributes"][2]["unique"], true);
    round_trip(&mut s, "er.attribute.set-flags", &json!({ "attribute": email, "primary_key": false, "nullable": "yes", "unique": false }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["unique_constraints"].as_array().map(Vec::len), Some(1));
    // Removing a column takes every key it is in, whole, as SQL does.
    let out = get(&mut s, "er.attribute.remove", json!({ "attribute": number }));
    assert_eq!(out["removed"], 2, "the column and the unique constraint over it");
    all_rules_hold(s.doc());
    s.undo().unwrap();
    round_trip(&mut s, "er.unique.remove", &json!({ "unique": u }));
    round_trip(&mut s, "er.primary-key.remove", &json!({ "entity": sm.customer }));
}

#[test]
fn a_foreign_key_points_at_any_key_and_the_key_stays_while_it_does() {
    let mut s = session();
    let sm = sample(&mut s);
    let region = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "region", "type": "TEXT" }));
    let number = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "number", "type": "INT" }));
    let email = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "email", "type": "TEXT", "unique": true }));
    let u = run(&mut s, "er.unique.add", json!({ "entity": sm.customer, "columns": [region, number], "name": "uq_region_number" }));
    let r_region = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "customer_region", "type": "text" }));
    let r_number = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "customer_number", "type": "INT" }));
    // Onto a composite alternate key.
    let add = json!({ "entity": sm.order, "key": u, "columns": [r_region, r_number], "name": "fk_order_customer_number" });
    round_trip(&mut s, "er.foreign-key.add", &add);
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let fk = o["foreign_keys"][0]["id"].as_str().unwrap().to_string();
    assert_eq!((o["foreign_keys"][0]["key"].as_str(), o["foreign_keys"][0]["references"].as_str()), (Some(u.as_str()), Some(sm.customer.as_str())));
    all_rules_hold(s.doc());
    // A key that a foreign key points at keeps its columns; a new name is fine.
    let set = |columns: Json, name: &str| json!({ "unique": u, "columns": columns, "name": name, "nulls_distinct": true });
    assert_eq!(refused_on(&mut s, "er.unique.set", set(json!([region]), "uq_region_number")), "columns");
    round_trip(&mut s, "er.unique.set", &set(json!([region, number]), "uq_customer_region_number"));
    assert_eq!(
        refused_on(&mut s, "er.foreign-key.add", json!({ "entity": sm.order, "key": u, "columns": [r_region] })),
        "columns",
        "one column for a two-column key"
    );
    // Point it at the one-column unique key instead: with given columns, or with new ones.
    let email_key = get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["unique_constraints"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(refused_on(&mut s, "er.foreign-key.set-key", json!({ "foreign_key": fk, "key": email_key, "columns": [r_number] })), "columns");
    round_trip(&mut s, "er.foreign-key.set-key", &json!({ "foreign_key": fk, "key": email_key }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let new_col = o["foreign_keys"][0]["columns"][0].as_str().unwrap().to_string();
    assert_eq!(o["attributes"].as_array().unwrap().iter().find(|a| a["id"] == new_col).unwrap()["name"], "customer_email");
    all_rules_hold(s.doc());
    round_trip(&mut s, "er.foreign-key.set-options", &json!({ "foreign_key": fk, "match": "partial", "deferrable": "initially-immediate" }));
    for (cmd, p) in [
        ("er.foreign-key.set-options", json!({ "foreign_key": fk, "match": "partial" })),
        ("er.foreign-key.set-options", json!({ "foreign_key": fk, "match": "loose", "deferrable": "not-deferrable" })),
        ("er.foreign-key.add", json!({ "entity": sm.order, "key": email_key, "columns": [new_col], "deferrable": "later" })),
    ] {
        assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}: a missing or unknown option is refused before the handler");
    }
    for (cmd, p, key) in [
        ("er.foreign-key.set-key", json!({ "foreign_key": fk, "key": sm.key }), "key"),
        ("er.foreign-key.set-key", json!({ "foreign_key": fk, "key": email_key }), "key"),
        ("er.foreign-key.add", json!({ "entity": sm.order, "key": email_key, "references": sm.order, "columns": [new_col] }), "references"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    // Unticking Unique on a column a foreign key points at is refused, and removing the key takes the
    // foreign key with it.
    assert_eq!(
        refused_on(&mut s, "er.attribute.set-flags", json!({ "attribute": email, "primary_key": false, "nullable": "yes", "unique": false })),
        "unique"
    );
    let out = get(&mut s, "er.unique.remove", json!({ "unique": email_key }));
    assert_eq!(out["removed"], 2, "the unique constraint and the foreign key onto it");
    all_rules_hold(s.doc());
}

#[test]
fn a_relationship_points_at_the_key_it_is_given() {
    let mut s = session();
    let sm = sample(&mut s);
    run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "email", "type": "TEXT", "unique": true }));
    let email_key = get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["unique_constraints"][0]["id"].as_str().unwrap().to_string();
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let order_pk = get(&mut s, "er.entity.get", json!({ "entity": sm.order }))["primary_key"]["id"].as_str().unwrap().to_string();
    let add = json!({ "from": sm.customer, "to": sm.order, "from_end": "zero-or-one", "to_end": "zero-or-more", "create_foreign_key": true, "points_to": email_key });
    round_trip(&mut s, "er.relationship.add", &add);
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let fk = o["foreign_keys"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(fk["key"], json!(email_key));
    let col = fk["columns"][0].as_str().unwrap().to_string();
    let attr = o["attributes"].as_array().unwrap().iter().find(|a| a["id"] == col).unwrap().clone();
    assert_eq!((attr["name"].as_str(), attr["type"].as_str(), attr["nullable"].as_bool()), (Some("customer_email"), Some("TEXT"), Some(true)));
    all_rules_hold(s.doc());
    for (p, key) in [
        (json!({ "from": sm.customer, "to": sm.order, "create_foreign_key": true, "points_to": order_pk }), "points_to"),
        (json!({ "from": sm.customer, "to": sm.order, "points_to": email_key }), "points_to"),
        (json!({ "from": sm.customer, "to": sm.order, "create_foreign_key": true, "points_to": sm.key }), "points_to"),
    ] {
        assert_eq!(refused_on(&mut s, "er.relationship.add", p.clone()), key, "{p}");
    }
}

#[test]
fn removing_a_primary_key_takes_its_foreign_keys_and_what_they_made() {
    let mut s = session();
    let sm = sample(&mut s);
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "sku", "type": "TEXT", "primary_key": true }));
    let r = run(&mut s, "er.relationship.add", json!({ "from": sm.order, "to": product, "from_end": "zero-or-more", "to_end": "zero-or-more" }));
    let m = run(&mut s, "er.relationship.resolve", json!({ "relationship": r }));
    let before = picture(s.doc());
    // The mapping table's own primary key goes: both its relationships stop identifying, it is a
    // regular entity again, its foreign keys stay.
    let out = get(&mut s, "er.primary-key.remove", json!({ "entity": m }));
    assert_eq!(out["removed"], 1);
    let t = get(&mut s, "er.entity.get", json!({ "entity": m }));
    assert_eq!((t["kind"].as_str(), t["foreign_keys"].as_array().map(Vec::len)), (Some("regular"), Some(2)));
    let schema = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert!(schema["relationships"].as_array().unwrap().iter().filter(|x| x["to"] == m).all(|x| x["identifying"] == false));
    all_rules_hold(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    // Product's primary key goes: the foreign key onto it goes too.
    let out = get(&mut s, "er.primary-key.remove", json!({ "entity": product }));
    assert_eq!(out["removed"], 2, "the primary key and the foreign key onto it");
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": m }))["kind"], "regular");
    all_rules_hold(s.doc());
}

#[test]
fn key_edits_that_would_break_a_foreign_key_are_refused_and_one_to_one_stays_unique() {
    let mut s = session();
    let sm = sample(&mut s);
    // An identifying foreign key's columns cannot leave the primary key.
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    let line_no = run(&mut s, "er.attribute.add", json!({ "entity": line, "name": "line_no", "type": "INT", "primary_key": true }));
    let add =
        json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true });
    run(&mut s, "er.relationship.add", add);
    assert_eq!(refused_on(&mut s, "er.primary-key.set", json!({ "entity": line, "columns": [line_no], "name": "" })), "columns");
    // A key a foreign key points at keeps its columns; a set-null column cannot join a key.
    let note = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Note" }));
    let note_id = run(&mut s, "er.attribute.add", json!({ "entity": note, "name": "note_id", "type": "INT", "primary_key": true }));
    let about = run(&mut s, "er.attribute.add", json!({ "entity": note, "name": "customer_id", "type": "BIGINT", "nullable": "yes" }));
    run(&mut s, "er.foreign-key.add", json!({ "entity": note, "references": sm.customer, "columns": [about], "on_delete": "set-null" }));
    let region = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "region", "type": "TEXT" }));
    assert_eq!(refused_on(&mut s, "er.primary-key.set", json!({ "entity": sm.customer, "columns": [sm.key, region], "name": "" })), "columns");
    assert_eq!(refused_on(&mut s, "er.primary-key.set", json!({ "entity": note, "columns": [note_id, about], "name": "" })), "columns");
    // A column of a composite unique constraint goes with the constraint and the foreign key onto it.
    let number = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "number", "type": "INT" }));
    let u = run(&mut s, "er.unique.add", json!({ "entity": sm.customer, "columns": [region, number] }));
    let (r2, n2) = (
        run(&mut s, "er.attribute.add", json!({ "entity": note, "name": "region", "type": "TEXT" })),
        run(&mut s, "er.attribute.add", json!({ "entity": note, "name": "number", "type": "INT" })),
    );
    run(&mut s, "er.foreign-key.add", json!({ "entity": note, "key": u, "columns": [r2, n2] }));
    let out = get(&mut s, "er.attribute.remove", json!({ "attribute": number }));
    assert_eq!(out["removed"], 3, "the column, the unique constraint over it and the foreign key onto that");
    all_rules_hold(s.doc());
    // An identifying one-to-one keeps its foreign key unique when the child's key is wider.
    let profile = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Profile" }));
    run(&mut s, "er.attribute.add", json!({ "entity": profile, "name": "profile_id", "type": "INT", "primary_key": true }));
    let add = json!({ "from": sm.customer, "to": profile, "from_end": "exactly-one", "to_end": "zero-or-one", "create_foreign_key": true, "identifying": true });
    run(&mut s, "er.relationship.add", add);
    let p = get(&mut s, "er.entity.get", json!({ "entity": profile }));
    assert_eq!(p["primary_key"]["columns"].as_array().map(Vec::len), Some(2));
    assert_eq!(p["unique_constraints"][0]["columns"], p["foreign_keys"][0]["columns"], "the one-to-one's columns are unique together");
    all_rules_hold(s.doc());
}

#[test]
fn the_sweep_finds_nothing() {
    let mut s = session();
    let sm = sample(&mut s);
    let col = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "customer_id", "type": "BIGINT", "unique": true }));
    let fk = run(&mut s, "er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "columns": [col] }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let u = o["unique_constraints"][0]["id"].as_str().unwrap().to_string();
    let pk = get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["primary_key"]["id"].as_str().unwrap().to_string();
    let ids = [&sm.schema, &sm.customer, &sm.key, &sm.places, &pk, &u, &fk, "not-an-id", "00000000-0000-7000-8000-000000000000"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 1000, "{calls}");
    assert_eq!(problems(s.doc()), Vec::<String>::new());
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn pick<'a>(&mut self, v: &'a [String]) -> Option<&'a String> {
        if v.is_empty() { None } else { v.get(self.below(v.len())) }
    }
}

fn ids_of(s: &Session, class: ClassId) -> Vec<String> {
    s.doc().alive().filter(|o| s.doc().class_of(*o).ok() == Some(class)).map(|o| s.doc().uuid(o).unwrap().to_string()).collect()
}

/// A random call against the store as it is.
fn random_call(rng: &mut Rng, s: &Session) -> (&'static str, Json) {
    let schemas = ids_of(s, SCHEMA);
    let entities = ids_of(s, ENTITY);
    let attributes = ids_of(s, ATTRIBUTE);
    let relationships = ids_of(s, RELATIONSHIP);
    let keys = ids_of(s, FOREIGN_KEY);
    let uniques = ids_of(s, UNIQUE_CONSTRAINT);
    let all_keys: Vec<String> = ids_of(s, PRIMARY_KEY).into_iter().chain(uniques.iter().cloned()).collect();
    let n = rng.next() % 1000;
    let name = format!("n{}", rng.next() % 30);
    let ends = ["zero-or-one", "exactly-one", "zero-or-more", "one-or-more"];
    let end = |r: &mut Rng| ends[(r.next() % 4) as usize];
    let pair = |r: &mut Rng| json!([r.pick(&attributes), r.pick(&attributes)]);
    match rng.below(21) {
        16 => ("er.primary-key.set", json!({ "entity": rng.pick(&entities), "columns": pair(rng), "name": "" })),
        17 => ("er.unique.add", json!({ "entity": rng.pick(&entities), "columns": pair(rng), "nulls_distinct": n.is_multiple_of(2) })),
        18 => ("er.foreign-key.set-key", json!({ "foreign_key": rng.pick(&keys), "key": rng.pick(&all_keys) })),
        19 => ("er.unique.remove", json!({ "unique": rng.pick(&uniques) })),
        20 => ("er.primary-key.remove", json!({ "entity": rng.pick(&entities) })),
        12 => (
            "er.relationship.add",
            json!({ "from": rng.pick(&entities), "to": rng.pick(&entities), "from_end": end(rng), "to_end": end(rng), "create_foreign_key": true, "identifying": n.is_multiple_of(2) }),
        ),
        13 => ("er.relationship.resolve", json!({ "relationship": rng.pick(&relationships) })),
        14 => ("er.foreign-key.remove", json!({ "foreign_key": rng.pick(&keys) })),
        15 => ("er.attribute.set-type", json!({ "attribute": rng.pick(&attributes), "type": if n.is_multiple_of(2) { "INT" } else { "TEXT" } })),
        0 if schemas.len() < 2 => ("er.schema.add", json!({ "name": name })),
        0..=2 => ("er.entity.add", json!({ "schema": rng.pick(&schemas), "name": name })),
        3 => ("er.entity.rename", json!({ "entity": rng.pick(&entities), "name": name })),
        4 => ("er.entity.remove", json!({ "entity": rng.pick(&entities) })),
        5 | 6 => ("er.attribute.add", json!({ "entity": rng.pick(&entities), "name": name, "primary_key": n.is_multiple_of(3) })),
        7 => ("er.attribute.remove", json!({ "attribute": rng.pick(&attributes) })),
        8 => ("er.relationship.add", json!({ "from": rng.pick(&entities), "to": rng.pick(&entities), "from_end": end(rng), "to_end": end(rng) })),
        9 => ("er.relationship.remove", json!({ "relationship": rng.pick(&relationships) })),
        10 => ("er.attribute.set-flags", json!({ "attribute": rng.pick(&attributes), "primary_key": n.is_multiple_of(2), "nullable": "auto" })),
        _ => ("er.remove", json!({ "ids": [rng.pick(&attributes).cloned().unwrap_or_default()] })),
    }
}

#[test]
fn random_sequences_keep_every_rule_and_undo_back_to_the_start() {
    for seed in 1..=20u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = session();
        let start = picture(s.doc());
        for i in 0..120 {
            let (cmd, p) = random_call(&mut rng, &s);
            let _ = s.execute(cmd, &p);
            assert_eq!(problems(s.doc()), Vec::<String>::new(), "seed {seed} step {i} after {cmd} {p}");
            if rng.next().is_multiple_of(7) && s.can_undo() {
                s.undo().unwrap();
            }
        }
        while s.can_undo() {
            s.undo().unwrap();
        }
        assert_eq!(picture(s.doc()), start, "seed {seed}: undo back to the start");
    }
}
