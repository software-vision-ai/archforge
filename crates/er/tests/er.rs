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
    let total = run(s, "er.attribute.add", json!({ "entity": order, "name": "total", "type": "DECIMAL", "precision": 10, "scale": 2 }));
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
    assert_eq!(m.classes.len(), 19);
    assert_eq!(
        m.data_types.len(),
        14,
        "the five primitives and the enumerations Cardinality, EntityKind, ReferentialAction, MatchType, Deferrability, Generation, SubtypeMapping, SortDirection and NullsOrder"
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
    // Customer's key becomes (id, region): the foreign key grows a matching column that follows region.
    let region = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "region", "type": "TEXT", "primary_key": true }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    assert_eq!(o["foreign_keys"][0]["columns"].as_array().map(Vec::len), Some(2));
    let grown = o["attributes"].as_array().unwrap().iter().find(|a| a["name"] == "customer_region").expect("the grown column");
    assert_eq!((grown["type"].as_str(), grown["nullable"].as_bool(), grown["linked_to"].as_str()), (Some("TEXT"), Some(true), Some(region.as_str())));
    // An identifying relationship adds key columns to its child; a key that targets the child grows too.
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    run(&mut s, "er.attribute.add", json!({ "entity": line, "name": "line_no", "type": "INT", "primary_key": true }));
    let shipment = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Shipment" }));
    let line_ref = run(&mut s, "er.attribute.add", json!({ "entity": shipment, "name": "line_ref", "type": "INT" }));
    run(&mut s, "er.foreign-key.add", json!({ "entity": shipment, "references": line, "columns": [line_ref] }));
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let add =
        json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true });
    run(&mut s, "er.relationship.add", add);
    let sh = get(&mut s, "er.entity.get", json!({ "entity": shipment }));
    assert_eq!(sh["foreign_keys"][0]["columns"].as_array().map(Vec::len), Some(2), "Shipment's key onto Line grew with Line's key");
    let _ = line_ref;
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
        // A key in another schema is fine (slice S7); its types still pair up.
        (json!({ "entity": sm.order, "references": invoice, "columns": [sm.total] }), "columns"),
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
    // A key that a foreign key points at may lose a column: the foreign key loses its match and a
    // hand-made column stays; a new name is fine; a column added back grows a follower.
    let set = |columns: Json, name: &str| json!({ "unique": u, "columns": columns, "name": name, "nulls_distinct": true });
    run(&mut s, "er.unique.set", set(json!([region]), "uq_region_number"));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    assert_eq!(o["foreign_keys"][0]["columns"], json!([r_region]));
    assert!(o["attributes"].as_array().unwrap().iter().any(|a| a["id"] == r_number), "the hand-made column stays");
    round_trip(&mut s, "er.unique.set", &set(json!([region, number]), "uq_customer_region_number"));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    assert_eq!(o["foreign_keys"][0]["columns"].as_array().map(Vec::len), Some(2));
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
    // Customer's key grows: Note's set-null foreign key gets a nullable follower; shrinking it back
    // takes the follower away again.
    run(&mut s, "er.primary-key.set", json!({ "entity": sm.customer, "columns": [sm.key, region], "name": "" }));
    let n = get(&mut s, "er.entity.get", json!({ "entity": note }));
    assert_eq!(n["foreign_keys"][0]["columns"].as_array().map(Vec::len), Some(2));
    assert_eq!(n["attributes"].as_array().map(Vec::len), Some(3));
    run(&mut s, "er.primary-key.set", json!({ "entity": sm.customer, "columns": [sm.key], "name": "" }));
    let n = get(&mut s, "er.entity.get", json!({ "entity": note }));
    assert_eq!((n["foreign_keys"][0]["columns"].as_array().map(Vec::len), n["attributes"].as_array().map(Vec::len)), (Some(1), Some(2)));
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
fn roles_name_the_key_columns_and_tell_relationships_apart() {
    let mut s = session();
    let sm = sample(&mut s);
    let address = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Address" }));
    run(&mut s, "er.attribute.add", json!({ "entity": address, "name": "address_id", "type": "BIGINT", "primary_key": true }));
    // Two relationships between Order and Address: the roles name the columns.
    let to_address = |role: &str| json!({ "from": address, "to": sm.order, "from_end": "zero-or-one", "to_end": "zero-or-more", "create_foreign_key": true, "from_role": role });
    round_trip(&mut s, "er.relationship.add", &to_address("billing"));
    let billing = relationship_ids(&mut s, &sm.schema).last().unwrap().clone();
    run(&mut s, "er.relationship.add", to_address("shipping"));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let names: Vec<&str> = o["attributes"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["total", "billing_address_id", "shipping_address_id"]);
    let r = relationship(&mut s, &sm.schema, &billing);
    assert_eq!((r["from_role"].as_str(), r["to_role"].as_str()), (Some("billing"), None));
    assert!(get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["notices"].as_array().unwrap().is_empty(), "the roles tell them apart");
    all_rules_hold(s.doc());
    // Two without roles are noticed, not refused; giving one a role ends the notice.
    let plain = json!({ "from": sm.customer, "to": address, "from_end": "exactly-one", "to_end": "zero-or-more" });
    let first = run(&mut s, "er.relationship.add", plain.clone());
    run(&mut s, "er.relationship.add", plain);
    let notices = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["notices"].clone();
    assert_eq!(notices.as_array().map(Vec::len), Some(2));
    assert_eq!(notices[0]["notice"], json!(rules::SAME_ENDS));
    round_trip(&mut s, "er.relationship.set-roles", &json!({ "relationship": first, "from_role": "", "to_role": "home" }));
    assert!(get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["notices"].as_array().unwrap().is_empty());
    for (cmd, p, key) in [
        ("er.relationship.add", json!({ "from": sm.customer, "to": address, "from_role": " padded " }), "from_role"),
        ("er.relationship.set-roles", json!({ "relationship": first, "from_role": "", "to_role": "a\tb" }), "to_role"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    assert!(s.execute("er.relationship.set-roles", &json!({ "relationship": first, "from_role": "x" })).is_err(), "both roles are required");
    all_rules_hold(s.doc());
}

#[test]
fn an_entity_relates_to_itself_once_the_end_it_points_at_has_a_role() {
    let mut s = session();
    let schema = run(&mut s, "er.schema.add", json!({ "name": "plant" }));
    let part = run(&mut s, "er.entity.add", json!({ "schema": schema, "name": "Part" }));
    run(&mut s, "er.attribute.add", json!({ "entity": part, "name": "part_id", "type": "BIGINT", "primary_key": true }));
    // A hierarchy in the table itself: parent_part_id points at the parent.
    let tree = json!({ "from": part, "to": part, "from_end": "zero-or-one", "to_end": "zero-or-more", "create_foreign_key": true });
    assert_eq!(refused_on(&mut s, "er.relationship.add", tree.clone()), "create_foreign_key");
    let mut with_roles = tree;
    with_roles["from_role"] = json!("parent");
    with_roles["to_role"] = json!("child");
    round_trip(&mut s, "er.relationship.add", &with_roles);
    let p = get(&mut s, "er.entity.get", json!({ "entity": part }));
    assert_eq!(p["attributes"][1]["name"], "parent_part_id");
    assert_eq!((p["attributes"][1]["nullable"].as_bool(), p["foreign_keys"][0]["references"].as_str()), (Some(true), Some(part.as_str())));
    all_rules_hold(s.doc());
    // A hierarchy in a mapping table: the roles name its columns and go to its relationships.
    let m2n = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": part, "to": part, "from_end": "zero-or-more", "to_end": "zero-or-more", "from_role": "parent", "to_role": "child" }),
    );
    let m = run(&mut s, "er.relationship.resolve", json!({ "relationship": m2n, "name": "part_structure" }));
    let t = get(&mut s, "er.entity.get", json!({ "entity": m }));
    let names: Vec<&str> = t["attributes"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["parent_part_id", "child_part_id"]);
    assert_eq!(t["kind"], "associative");
    let schema_json = get(&mut s, "er.schema.get", json!({ "schema": schema }));
    let mut roles: Vec<&str> =
        schema_json["relationships"].as_array().unwrap().iter().filter(|r| r["to"] == m).filter_map(|r| r["from_role"].as_str()).collect();
    roles.sort_unstable();
    assert_eq!(roles, ["child", "parent"]);
    assert!(schema_json["notices"].as_array().unwrap().is_empty());
    all_rules_hold(s.doc());
}

#[test]
fn roles_follow_the_end_a_key_points_to_in_every_shape() {
    let mut s = session();
    let schema = run(&mut s, "er.schema.add", json!({ "name": "plant" }));
    let part = run(&mut s, "er.entity.add", json!({ "schema": schema, "name": "Part" }));
    run(&mut s, "er.attribute.add", json!({ "entity": part, "name": "part_id", "type": "BIGINT", "primary_key": true }));
    let code = run(&mut s, "er.attribute.add", json!({ "entity": part, "name": "code", "type": "TEXT", "nullable": "no" }));
    let code_key = run(&mut s, "er.unique.add", json!({ "entity": part, "columns": [code] }));
    let column = |s: &mut Session, id: &str| {
        let p = get(s, "er.entity.get", json!({ "entity": part }));
        let a = p["attributes"].as_array().unwrap().iter().find(|a| a["id"] == id).unwrap().clone();
        (a["name"].as_str().unwrap().to_string(), a["nullable"].as_bool().unwrap())
    };
    // The many end first: the child is `from`, so the role at `to` names the column.
    let add = json!({ "from": part, "to": part, "from_end": "zero-or-more", "to_end": "zero-or-one", "from_role": "child", "to_role": "parent", "create_foreign_key": true });
    run(&mut s, "er.relationship.add", add);
    let fk = get(&mut s, "er.entity.get", json!({ "entity": part }))["foreign_keys"][0].clone();
    let fk_id = fk["id"].as_str().unwrap().to_string();
    assert_eq!(column(&mut s, fk["columns"][0].as_str().unwrap()), ("parent_part_id".to_string(), true));
    // Pointed at another key without columns: the same end's role names the new column.
    round_trip(&mut s, "er.foreign-key.set-key", &json!({ "foreign_key": fk_id, "key": code_key }));
    let fk = get(&mut s, "er.entity.get", json!({ "entity": part }))["foreign_keys"][0].clone();
    assert_eq!(column(&mut s, fk["columns"][0].as_str().unwrap()), ("parent_code".to_string(), true));
    all_rules_hold(s.doc());
    // A one-to-one names its column by the role and keeps it unique.
    let unit = run(&mut s, "er.entity.add", json!({ "schema": schema, "name": "Unit" }));
    run(&mut s, "er.attribute.add", json!({ "entity": unit, "name": "unit_id", "type": "INT", "primary_key": true }));
    let one = json!({ "from": unit, "to": part, "from_end": "exactly-one", "to_end": "zero-or-one", "from_role": "stock_unit", "create_foreign_key": true });
    run(&mut s, "er.relationship.add", one);
    let p = get(&mut s, "er.entity.get", json!({ "entity": part }));
    assert_eq!(p["attributes"].as_array().unwrap().last().unwrap()["name"], "stock_unit_unit_id");
    assert_eq!(p["attributes"].as_array().unwrap().last().unwrap()["unique"], true);
    // Refusals: an entity identifying itself, roles without a letter or a digit.
    let ident = json!({ "from": part, "to": part, "from_end": "exactly-one", "to_end": "zero-or-more", "from_role": "whole", "create_foreign_key": true, "identifying": true });
    assert_eq!(refused_on(&mut s, "er.relationship.add", ident), "identifying");
    assert_eq!(refused_on(&mut s, "er.relationship.add", json!({ "from": part, "to": unit, "to_role": "--" })), "to_role");
    all_rules_hold(s.doc());
}

#[test]
fn the_same_ends_notice_ignores_direction_and_case() {
    let mut s = session();
    let sm = sample(&mut s);
    let notices = |s: &mut Session| get(s, "er.schema.get", json!({ "schema": sm.schema }))["notices"].as_array().map(Vec::len).unwrap_or(0);
    // places is Customer to Order; one the other way round with no roles joins the same ends.
    let back = run(&mut s, "er.relationship.add", json!({ "from": sm.order, "to": sm.customer }));
    assert_eq!(notices(&mut s), 2);
    run(&mut s, "er.relationship.set-roles", json!({ "relationship": sm.places, "from_role": "Buyer", "to_role": "" }));
    assert_eq!(notices(&mut s), 0);
    run(&mut s, "er.relationship.set-roles", json!({ "relationship": back, "from_role": "", "to_role": "buyer" }));
    assert_eq!(notices(&mut s), 2, "roles that differ only in case do not tell them apart");
    // Two relationships of an entity with itself, with the roles swapped, are the same ends.
    let one = run(&mut s, "er.relationship.add", json!({ "from": sm.customer, "to": sm.customer, "from_role": "referrer", "to_role": "referred" }));
    let other = run(&mut s, "er.relationship.add", json!({ "from": sm.customer, "to": sm.customer, "from_role": "referred", "to_role": "referrer" }));
    let n = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["notices"].clone();
    for r in [&one, &other] {
        assert!(n.as_array().unwrap().iter().any(|x| x["relationship"] == **r), "{r}");
    }
}

fn relationship_ids(s: &mut Session, schema: &str) -> Vec<String> {
    get(s, "er.schema.get", json!({ "schema": schema }))["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
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
    let skill = run(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Table naming", "applies_to": "table", "triggers": ["naming"] }));
    let dom = run(&mut s, "er.domain.add", json!({ "schema": sm.schema, "name": "money", "base_type": "NUMERIC", "precision": 12, "scale": 2 }));
    let en = run(&mut s, "er.enum.add", json!({ "schema": sm.schema, "name": "state", "literals": ["open", "closed"] }));
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "amount", "domain": dom }));
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "state", "enum": en, "default": "'open'" }));
    let sub = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "VipCustomer" }));
    let generalization = run(&mut s, "er.generalization.add", json!({ "supertype": sm.customer, "subtype": sub }));
    let seq =
        run(&mut s, "er.sequence.add", json!({ "schema": sm.schema, "name": "order_no", "minimum": "1", "maximum": "1000", "owned_by": sm.key }));
    let view = run(&mut s, "er.view.add", json!({ "schema": sm.schema, "name": "big_orders", "query": "select * from orders" }));
    let crm = run(&mut s, "er.schema.add", json!({ "name": "crm" }));
    let contact = run(&mut s, "er.entity.add", json!({ "schema": crm, "name": "Contact" }));
    let across = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.customer, "to": contact, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    let ids = [
        &sm.schema,
        &sm.customer,
        &sm.key,
        &sm.places,
        &pk,
        &u,
        &fk,
        &skill,
        &dom,
        &en,
        &sub,
        &generalization,
        &seq,
        &view,
        &crm,
        &contact,
        &across,
        "not-an-id",
        "00000000-0000-7000-8000-000000000000",
    ];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 1000, "{calls}");
    assert_eq!(problems(s.doc()), Vec::<String>::new());
    assert!(archforge_core::skills::problems(s.doc()).is_empty());
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
    match rng.below(24) {
        22 => (
            "er.sequence.add",
            json!({ "schema": rng.pick(&schemas), "name": name, "increment": if n.is_multiple_of(2) { 1 } else { -1 }, "owned_by": rng.pick(&attributes) }),
        ),
        23 => ("er.view.add", json!({ "schema": rng.pick(&schemas), "name": name, "query": "select 1", "materialized": n.is_multiple_of(2) })),
        16 => ("er.primary-key.set", json!({ "entity": rng.pick(&entities), "columns": pair(rng), "name": "" })),
        17 => ("er.unique.add", json!({ "entity": rng.pick(&entities), "columns": pair(rng), "nulls_distinct": n.is_multiple_of(2) })),
        18 => ("er.foreign-key.set-key", json!({ "foreign_key": rng.pick(&keys), "key": rng.pick(&all_keys) })),
        19 => ("er.unique.remove", json!({ "unique": rng.pick(&uniques) })),
        20 => ("er.primary-key.remove", json!({ "entity": rng.pick(&entities) })),
        21 => (
            "er.relationship.set-roles",
            json!({ "relationship": rng.pick(&relationships), "from_role": if n.is_multiple_of(2) { "parent" } else { "" }, "to_role": "child" }),
        ),
        12 => (
            "er.relationship.add",
            json!({ "from": rng.pick(&entities), "to": rng.pick(&entities), "from_end": end(rng), "to_end": end(rng), "create_foreign_key": true, "identifying": n.is_multiple_of(2), "from_role": if n.is_multiple_of(3) { "parent" } else { "" }, "to_role": if n.is_multiple_of(5) { "child" } else { "" } }),
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

#[test]
fn a_foreign_key_without_columns_gets_new_ones_named_after_the_parent() {
    let mut s = session();
    let sm = sample(&mut s);
    let before = picture(s.doc());
    let fk = run(&mut s, "er.foreign-key.add", json!({ "entity": sm.order, "references": sm.customer, "relationship": sm.places }));
    let st = s.doc();
    let fk_obj = obj(st, &fk);
    let columns: Vec<Obj> = st.list(fk_obj, foreign_key::COLUMN).unwrap().iter().filter_map(Value::as_ref).collect();
    assert_eq!(columns.len(), 1, "one column per column of Customer's primary key");
    assert_eq!(rules::name_of(st, columns[0]), "customer_id", "named after the parent and its key column");
    assert_eq!(rules::type_of(st, columns[0]), "BIGINT", "typed as the key column");
    assert_eq!(rules::entity_of(st, columns[0]), Some(obj(st, &sm.order)), "added to the child");
    assert_eq!(
        st.get(fk_obj, foreign_key::REFERENCES).unwrap().as_ref(),
        rules::primary_key_of(st, obj(st, &sm.customer)),
        "points at the primary key"
    );
    all_rules_hold(st);
    s.execute("edit.undo", &Json::Null).unwrap();
    assert_eq!(picture(s.doc()), before, "one undo takes the key and its column");
}

#[test]
fn a_column_takes_its_size_default_generation_collation_and_description_with_the_rules() {
    let mut s = session();
    let sm = sample(&mut s);
    // A varchar takes a length; a numeric a precision and a scale; an integer neither.
    let code = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "code", "type": "VARCHAR", "length": 32 }));
    let price = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "price", "type": "NUMERIC", "precision": 10, "scale": 2 }));
    assert_eq!(rules::type_display(s.doc(), obj(s.doc(), &code)), "VARCHAR(32)");
    assert_eq!(rules::type_display(s.doc(), obj(s.doc(), &price)), "NUMERIC(10, 2)");
    assert_eq!(rules::type_display(s.doc(), obj(s.doc(), &sm.key)), "BIGINT");
    let skill = run(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Table naming", "applies_to": "table", "triggers": ["naming"] }));
    let before = picture(s.doc());
    for (cmd, p) in [
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "type": "INTEGER", "length": 10 })),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "type": "NUMERIC", "precision": 2, "scale": 5 })),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "type": "VARCHAR", "scale": 5 })),
        ("er.attribute.set-type", json!({ "attribute": code, "type": "TEXT", "length": 32 })),
        ("er.attribute.set-generation", json!({ "attribute": code, "generation": "identity-always" })),
        ("er.attribute.set-generation", json!({ "attribute": code, "generation": "computed-stored" })),
        ("er.attribute.set-generation", json!({ "attribute": code, "generation": "sometimes" })),
        ("er.describe", json!({ "element": sm.order, "text": " padded " })),
        // A size belongs in its own parameter, never in the type's name.
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "type": "VARCHAR(255)", "length": 255 })),
        ("er.attribute.set-type", json!({ "attribute": code, "type": "VARCHAR(255)" })),
        // A new column takes its default and generation in the same call, under the same rules.
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "type": "TEXT", "generation": "identity-always" })),
        (
            "er.attribute.add",
            json!({ "entity": sm.order, "name": "x", "type": "INTEGER", "generation": "computed-stored", "expression": "1", "default": "0" }),
        ),
        // Only elements of the ER model take a description.
        ("er.describe", json!({ "element": skill, "text": "A skill is no element" })),
    ] {
        let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was accepted"));
        assert!(!e.to_string().is_empty());
        assert_eq!(picture(s.doc()), before, "{cmd} {p} changed something");
    }
    // A column lands with its default, generation and collation in one step.
    let total = run(
        &mut s,
        "er.attribute.add",
        json!({ "entity": sm.order, "name": "doubled", "type": "NUMERIC", "precision": 12, "scale": 2, "generation": "computed-virtual", "expression": "price * 2", "collation": "" }),
    );
    assert_eq!(rules::generation_of(s.doc(), obj(s.doc(), &total)), Generation::ComputedVirtual);
    assert_eq!(rules::text_of(s.doc(), obj(s.doc(), &total), attribute::EXPRESSION), "price * 2");
    run(&mut s, "er.attribute.remove", json!({ "attribute": total }));
    // An identity column stays not null, whichever command would make it nullable.
    let qty = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "qty", "type": "INTEGER", "generation": "identity-by-default" }));
    let held = picture(s.doc());
    let e = s
        .execute("er.attribute.set-flags", &json!({ "attribute": qty, "primary_key": false, "nullable": "yes", "unique": false }))
        .expect_err("an identity column was made nullable");
    assert!(e.to_string().contains("identity"), "{e}");
    assert_eq!(picture(s.doc()), held);
    assert!(!rules::is_nullable(s.doc(), obj(s.doc(), &qty)), "auto keeps an identity column not null");
    run(&mut s, "er.attribute.remove", json!({ "attribute": qty }));
    // Default, a computed column, an identity column, a collation and descriptions round-trip.
    round_trip(&mut s, "er.attribute.set-default", &json!({ "attribute": code, "default": "'none'" }));
    round_trip(
        &mut s,
        "er.attribute.set-generation",
        &json!({ "attribute": price, "generation": "computed-stored", "expression": "quantity * unit_price" }),
    );
    round_trip(&mut s, "er.attribute.set-generation", &json!({ "attribute": sm.key, "generation": "identity-always" }));
    round_trip(&mut s, "er.attribute.set-collation", &json!({ "attribute": code, "collation": "en_US" }));
    round_trip(&mut s, "er.describe", &json!({ "element": sm.order, "text": "One purchase by a customer." }));
    run(&mut s, "er.attribute.set-generation", json!({ "attribute": price, "generation": "computed-stored", "expression": "quantity * unit_price" }));
    let e = s.execute("er.attribute.set-default", &json!({ "attribute": price, "default": "0" })).expect_err("a computed column takes no default");
    assert!(e.to_string().contains("default"));
    run(&mut s, "er.describe", json!({ "element": sm.order, "text": "One purchase by a customer." }));
    assert_eq!(rules::text_of(s.doc(), obj(s.doc(), &sm.order), element::DESCRIPTION), "One purchase by a customer.");
    all_rules_hold(s.doc());
}

#[test]
fn checks_and_indexes_are_named_constraints_of_a_table_and_go_with_their_columns() {
    let mut s = session();
    let sm = sample(&mut s);
    let status = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "status", "type": "TEXT" }));
    let created = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "created_at", "type": "TIMESTAMP" }));
    // A check holds an expression; an index its columns in order, each with a direction and a
    // nulls order, or an expression instead of a column.
    round_trip(&mut s, "er.check.add", &json!({ "entity": sm.order, "expression": "total >= 0", "name": "ck_order_total_positive" }));
    let check = run(&mut s, "er.check.add", json!({ "entity": sm.order, "expression": "total >= 0" }));
    let index = json!({
        "entity": sm.order,
        "columns": [status, { "attribute": created, "direction": "descending", "nulls": "last" }, { "expression": "lower(status)" }],
        "name": "ix_order_status_created",
        "unique": false,
        "where": "status <> 'cancelled'",
        "method": "btree",
    });
    round_trip(&mut s, "er.index.add", &index);
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let ix = o["indexes"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(o["checks"].as_array().map(Vec::len), Some(2), "the redone named check and the unnamed one");
    assert_eq!(o["checks"][1]["expression"], "total >= 0");
    assert_eq!(o["indexes"][0]["columns"][1], json!({ "attribute": created, "expression": null, "direction": "descending", "nulls": "last" }));
    assert_eq!(o["indexes"][0]["columns"][2]["expression"], "lower(status)");
    assert_eq!((o["indexes"][0]["where"].as_str(), o["indexes"][0]["method"].as_str()), (Some("status <> 'cancelled'"), Some("btree")));
    // Unnamed ones show a generated name; names are unique with the keys of the schema.
    assert_eq!(rules::display_name(s.doc(), obj(s.doc(), &check)), "ck_order_2");
    run(&mut s, "er.index.add", json!({ "entity": sm.order, "columns": [status] }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let ix2 = o["indexes"][1]["id"].as_str().unwrap().to_string();
    assert_eq!(rules::display_name(s.doc(), obj(s.doc(), &ix2)), "ix_order_status");
    all_rules_hold(s.doc());
    for (cmd, p, key) in [
        ("er.check.add", json!({ "entity": sm.order, "expression": "   " }), "expression"),
        ("er.check.add", json!({ "entity": sm.order, "expression": "total > 0", "name": "IX_ORDER_STATUS_CREATED" }), "name"),
        ("er.check.set", json!({ "check": ix, "expression": "total > 0" }), "check"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [sm.key] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [status, status] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [{ "attribute": status, "expression": "lower(status)" }] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [{}] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [{ "attribute": status, "direction": "sideways" }] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [{ "attribute": status, "directon": "descending" }] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [{ "attribute": 7 }] }), "columns"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [status], "name": "ck_order_total_positive" }), "name"),
        ("er.index.add", json!({ "entity": sm.order, "columns": [status], "method": " " }), "method"),
        ("er.index.set", json!({ "index": check, "columns": [status] }), "index"),
        ("er.unique.add", json!({ "entity": sm.order, "columns": [status], "name": "ix_order_status_created" }), "name"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    // Set replaces the columns and the fields; remove takes an index or a check whole.
    round_trip(
        &mut s,
        "er.index.set",
        &json!({ "index": ix, "columns": [{ "attribute": created, "direction": "descending" }], "name": "", "unique": true }),
    );
    round_trip(&mut s, "er.check.set", &json!({ "check": check, "expression": "total >= 1", "name": "ck_order_total" }));
    round_trip(&mut s, "er.check.remove", &json!({ "check": check }));
    round_trip(&mut s, "er.index.remove", &json!({ "index": ix2 }));
    // A column that goes takes its index column; an index left without columns goes too.
    run(&mut s, "er.index.set", json!({ "index": ix, "columns": [status, created], "name": "ix_order_status_created" }));
    round_trip(&mut s, "er.attribute.remove", &json!({ "attribute": created }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    assert_eq!(o["indexes"][0]["columns"].as_array().map(Vec::len), Some(1), "{}", o["indexes"]);
    run(&mut s, "er.attribute.remove", json!({ "attribute": status }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    assert_eq!(o["indexes"].as_array().map(Vec::len), Some(0), "{}", o["indexes"]);
    all_rules_hold(s.doc());
}

#[test]
fn relationships_and_keys_agree_through_nullability_growing_keys_links_and_collapse() {
    let mut s = session();
    let sm = sample(&mut s);
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    let pid = run(&mut s, "er.attribute.add", json!({ "entity": product, "name": "product_id", "type": "BIGINT", "primary_key": true }));
    let rel = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": product, "to": sm.order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    let column = |s: &mut Session, entity: &str, name: &str| -> Json {
        get(s, "er.entity.get", json!({ "entity": entity }))["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == name)
            .cloned()
            .unwrap_or(Json::Null)
    };
    let follower = column(&mut s, &sm.order, "product_product_id");
    assert_eq!((follower["nullable"].as_bool(), follower["linked_to"].as_str()), (Some(false), Some(pid.as_str())));
    let fid = follower["id"].as_str().unwrap().to_string();
    // (a) The parent end drives nullability, and nullability drives the parent end.
    round_trip(&mut s, "er.relationship.set-ends", &json!({ "relationship": rel, "from_end": "zero-or-one", "to_end": "zero-or-more" }));
    assert_eq!(column(&mut s, &sm.order, "product_product_id")["nullable"], true);
    run(&mut s, "er.relationship.set-ends", json!({ "relationship": rel, "from_end": "exactly-one", "to_end": "zero-or-more" }));
    assert_eq!(column(&mut s, &sm.order, "product_product_id")["nullable"], false);
    run(&mut s, "er.relationship.set-ends", json!({ "relationship": rel, "from_end": "exactly-one", "to_end": "one-or-more" }));
    assert!(rules::notices(s.doc()).iter().any(|(r, n)| *r == obj(s.doc(), &rel) && *n == rules::ONE_OR_MORE), "{:?}", rules::notices(s.doc()));
    run(&mut s, "er.attribute.set-flags", json!({ "attribute": fid, "primary_key": false, "nullable": "yes", "unique": false }));
    let r = relationship(&mut s, &sm.schema, &rel);
    assert_eq!(r["from_end"], "zero-or-one", "a nullable key column makes the parent end optional");
    assert_eq!(column(&mut s, &sm.order, "product_product_id")["linked_to"], Json::Null, "a direct edit clears the link");
    all_rules_hold(s.doc());
    // (c) A follower follows name and type; its own edit frees it; unlink frees it too.
    let tag = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Tag" }));
    let tid = run(&mut s, "er.attribute.add", json!({ "entity": tag, "name": "tag_id", "type": "INT", "primary_key": true }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": tag, "to": sm.order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    assert_eq!(column(&mut s, &sm.order, "tag_tag_id")["linked_to"], tid);
    round_trip(&mut s, "er.attribute.rename", &json!({ "attribute": tid, "name": "id" }));
    assert!(column(&mut s, &sm.order, "tag_id")["linked_to"] == tid, "the follower took the new name");
    round_trip(&mut s, "er.attribute.set-type", &json!({ "attribute": tid, "type": "VARCHAR", "length": 12 }));
    let follower = column(&mut s, &sm.order, "tag_id")["id"].as_str().unwrap().to_string();
    assert_eq!(rules::type_display(s.doc(), obj(s.doc(), &follower)), "VARCHAR(12)");
    round_trip(&mut s, "er.attribute.unlink", &json!({ "attribute": follower }));
    assert_eq!(refused_on(&mut s, "er.attribute.unlink", json!({ "attribute": follower })), "attribute");
    run(&mut s, "er.attribute.rename", json!({ "attribute": tid, "name": "tag_id" }));
    assert_eq!(column(&mut s, &sm.order, "tag_id")["id"], follower, "a freed column keeps its name");
    // (b) A key that grows grows the foreign keys onto it; one that shrinks takes the followers.
    run(&mut s, "er.attribute.add", json!({ "entity": tag, "name": "scope", "type": "TEXT", "primary_key": true }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let fk_tag = o["foreign_keys"].as_array().unwrap().iter().find(|k| k["references"] == tag).unwrap().clone();
    assert_eq!(fk_tag["columns"].as_array().map(Vec::len), Some(2));
    assert!(column(&mut s, &sm.order, "tag_scope")["linked_to"].as_str().is_some());
    // An index on the follower goes with it; the shrink leaves every rule holding.
    let scope_follower = column(&mut s, &sm.order, "tag_scope")["id"].as_str().unwrap().to_string();
    run(&mut s, "er.index.add", json!({ "entity": sm.order, "columns": [scope_follower] }));
    round_trip(&mut s, "er.primary-key.set", &json!({ "entity": tag, "columns": [tid], "name": "" }));
    run(&mut s, "er.primary-key.set", json!({ "entity": tag, "columns": [tid], "name": "" }));
    assert_eq!(column(&mut s, &sm.order, "tag_scope"), Json::Null, "the follower went with the key column");
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": sm.order }))["indexes"].as_array().map(Vec::len), Some(0), "its index went too");
    assert_eq!(column(&mut s, &sm.order, "tag_id")["id"], follower, "the freed column stays");
    all_rules_hold(s.doc());
    // A chain of identifying keys follows a rename all the way down, and never closes into a cycle.
    let oid = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "order_id", "type": "BIGINT", "primary_key": true }));
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    let identifying = |from: &str, to: &str| json!({ "from": from, "to": to, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true });
    run(&mut s, "er.relationship.add", identifying(&sm.order, &line));
    let shipment = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Shipment" }));
    run(&mut s, "er.relationship.add", identifying(&line, &shipment));
    assert!(column(&mut s, &shipment, "line_order_order_id")["linked_to"].as_str().is_some());
    run(&mut s, "er.attribute.rename", json!({ "attribute": oid, "name": "id" }));
    assert!(column(&mut s, &line, "order_id")["linked_to"] == oid, "Line's follower took the new name");
    assert!(column(&mut s, &shipment, "line_order_id")["linked_to"].as_str().is_some(), "Shipment's follower followed on");
    assert_eq!(refused_on(&mut s, "er.relationship.add", identifying(&shipment, &sm.order)), "identifying");
    let back = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": shipment, "to": sm.order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    assert_eq!(refused_on(&mut s, "er.relationship.set-identifying", json!({ "relationship": back, "identifying": true })), "identifying");
    run(&mut s, "er.relationship.remove", json!({ "relationship": back }));
    run(&mut s, "er.attribute.rename", json!({ "attribute": oid, "name": "order_id" }));
    all_rules_hold(s.doc());
    // (d) A pure mapping table collapses into the many-to-many line it stood for, on the diagram too.
    let m2m = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.order, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more", "from_role": "tagged" }),
    );
    let d = run(&mut s, "diagram.add", json!({ "kind": "er", "element": sm.schema }));
    let n_order = run(&mut s, "diagram.place", json!({ "diagram": d, "element": sm.order, "x": 0.0, "y": 0.0 }));
    let n_tag = run(&mut s, "diagram.place", json!({ "diagram": d, "element": tag, "x": 400.0, "y": 0.0 }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": m2m, "source": n_order, "target": n_tag }));
    let mapping = run(&mut s, "er.relationship.resolve", json!({ "relationship": m2m }));
    assert!(rules::collapse_problem(s.doc(), obj(s.doc(), &mapping)).is_none());
    run(&mut s, "er.check.add", json!({ "entity": mapping, "expression": "1 = 1" }));
    assert_eq!(refused_on(&mut s, "er.relationship.collapse", json!({ "entity": mapping })), "entity");
    let check = get(&mut s, "er.entity.get", json!({ "entity": mapping }))["checks"][0]["id"].as_str().unwrap().to_string();
    run(&mut s, "er.check.remove", json!({ "check": check }));
    let before = picture(s.doc());
    let back = run(&mut s, "er.relationship.collapse", json!({ "entity": mapping }));
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "undo brings the mapping table back");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    let r = relationship(&mut s, &sm.schema, &back);
    assert_eq!((r["from_end"].as_str(), r["to_end"].as_str(), r["from_role"].as_str()), (Some("zero-or-more"), Some("zero-or-more"), Some("tagged")));
    assert!(s.doc().by_uuid(Uuid::parse(&mapping).unwrap()).is_none_or(|m| !s.doc().is_alive(m)), "the mapping table went");
    let (_, edges, nodes) = archforge_diagram::views_of(s.doc(), obj(s.doc(), &back));
    assert_eq!((edges.len(), nodes.len()), (1, 0), "the line is back on the diagram");
    assert_eq!(refused_on(&mut s, "er.relationship.collapse", json!({ "entity": sm.order })), "entity");
    all_rules_hold(s.doc());
}

#[test]
fn a_cycle_of_identifying_keys_in_a_loaded_document_cannot_grow_and_a_self_key_onto_a_unique_may_identify() {
    // Past the commands' refusals (a document from a file could carry it): A identifies B and B
    // identifies A. Growing either key runs around the circle until the budget refuses it.
    let (mut st, _) = rooted_store();
    let m = st.alive().find(|o| st.class_of(*o).ok() == Some(MODEL)).unwrap();
    let s = st.create(SCHEMA).unwrap();
    st.set(s, named_element::NAME, Value::str("loop")).unwrap();
    st.push(m, model::MEMBER, Value::Ref(s)).unwrap();
    let entity = |st: &mut Store, name: &str| {
        let e = st.create(ENTITY).unwrap();
        st.set(e, named_element::NAME, Value::str(name)).unwrap();
        st.push(s, schema::ENTITY, Value::Ref(e)).unwrap();
        let a = st.create(ATTRIBUTE).unwrap();
        st.set(a, named_element::NAME, Value::str("id")).unwrap();
        st.set(a, attribute::TYPE, Value::str("INT")).unwrap();
        st.set(a, attribute::NULLABLE, Value::Bool(false)).unwrap();
        st.push(e, entity::ATTRIBUTE, Value::Ref(a)).unwrap();
        let pk = st.create(PRIMARY_KEY).unwrap();
        st.set(e, entity::PRIMARY_KEY, Value::Ref(pk)).unwrap();
        st.push(pk, key::COLUMN, Value::Ref(a)).unwrap();
        (e, pk)
    };
    let (a, a_pk) = entity(&mut st, "A");
    let (b, b_pk) = entity(&mut st, "B");
    let identify = |st: &mut Store, parent: Obj, parent_pk: Obj, child: Obj, child_pk: Obj| {
        let r = st.create(RELATIONSHIP).unwrap();
        st.set(r, relationship::FROM, Value::Ref(parent)).unwrap();
        st.set(r, relationship::TO, Value::Ref(child)).unwrap();
        st.set(r, relationship::FROM_END, Value::Enum(Cardinality::ExactlyOne.index())).unwrap();
        st.set(r, relationship::TO_END, Value::Enum(Cardinality::ZeroOrMore.index())).unwrap();
        st.set(r, relationship::IDENTIFYING, Value::Bool(true)).unwrap();
        st.push(s, schema::RELATIONSHIP, Value::Ref(r)).unwrap();
        let c = st.create(ATTRIBUTE).unwrap();
        st.set(c, named_element::NAME, Value::str("parent_id")).unwrap();
        st.set(c, attribute::TYPE, Value::str("INT")).unwrap();
        st.set(c, attribute::NULLABLE, Value::Bool(false)).unwrap();
        st.push(child, entity::ATTRIBUTE, Value::Ref(c)).unwrap();
        st.push(child_pk, key::COLUMN, Value::Ref(c)).unwrap();
        let fk = st.create(FOREIGN_KEY).unwrap();
        st.set(fk, foreign_key::REFERENCES, Value::Ref(parent_pk)).unwrap();
        st.push(fk, foreign_key::COLUMN, Value::Ref(c)).unwrap();
        st.set(fk, foreign_key::RELATIONSHIP, Value::Ref(r)).unwrap();
        st.push(child, entity::FOREIGN_KEY, Value::Ref(fk)).unwrap();
    };
    identify(&mut st, a, a_pk, b, b_pk);
    identify(&mut st, b, b_pk, a, a_pk);
    let a_id = st.uuid(a).unwrap().to_string();
    let mut s = Session::new(registry().unwrap(), st, 1, Options::default());
    let before = picture(s.doc());
    let e = s
        .execute("er.attribute.add", &json!({ "entity": a_id, "name": "more", "type": "INT", "primary_key": true }))
        .expect_err("the loop grows forever");
    assert!(e.to_string().contains("deeper than") && e.to_string().contains("or form a cycle"), "{e}");
    assert_eq!(picture(s.doc()), before, "nothing of the attempt stays");
    // A self-reference onto a unique key may identify: it cannot recurse.
    let mut s = session();
    let sm = sample(&mut s);
    let code = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "code", "type": "INT", "unique": true }));
    let uq = get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["unique_constraints"][0]["id"].as_str().unwrap().to_string();
    let _ = code;
    let own = json!({ "from": sm.customer, "to": sm.customer, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true, "from_role": "parent", "points_to": uq });
    run(&mut s, "er.relationship.add", own);
    all_rules_hold(s.doc());
}

#[test]
fn domains_and_enum_types_type_columns_under_the_schemas_rules() {
    let mut s = session();
    let sm = sample(&mut s);
    // A domain and an enum type, each named once in the schema's namespace with the tables.
    let domain = json!({
        "schema": sm.schema, "name": "email_address", "base_type": "VARCHAR", "length": 255, "not_null": true,
        "checks": ["position('@' in value) > 1", { "expression": "length(value) > 3", "name": "ck_email_len" }],
    });
    round_trip(&mut s, "er.domain.add", &domain);
    let sg = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    let dom = sg["domains"][0]["id"].as_str().unwrap().to_string();
    assert_eq!((sg["domains"][0]["checks"].as_array().map(Vec::len), sg["domains"][0]["length"].as_i64()), (Some(2), Some(255)));
    let en = run(&mut s, "er.enum.add", json!({ "schema": sm.schema, "name": "order_status", "literals": ["pending", "paid", "shipped"] }));
    let sg = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert_eq!(sg["enums"][0]["literals"], json!(["pending", "paid", "shipped"]));
    // Columns take them: rows show the type's name, the rules see the base type and its size.
    let email = run(&mut s, "er.attribute.add", json!({ "entity": sm.customer, "name": "email", "domain": dom }));
    let status = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "status", "enum": en, "default": "'paid'" }));
    assert_eq!(rules::type_display(s.doc(), obj(s.doc(), &email)), "email_address");
    assert_eq!(rules::type_of(s.doc(), obj(s.doc(), &email)), "VARCHAR");
    assert_eq!(rules::size_of(s.doc(), obj(s.doc(), &email)), (Some(255), None, None));
    assert_eq!(rules::type_display(s.doc(), obj(s.doc(), &status)), "order_status");
    all_rules_hold(s.doc());
    for (cmd, p, key) in [
        ("er.domain.add", json!({ "schema": sm.schema, "name": "Customer", "base_type": "TEXT" }), "name"),
        ("er.enum.add", json!({ "schema": sm.schema, "name": "email_address", "literals": ["a"] }), "name"),
        ("er.entity.add", json!({ "schema": sm.schema, "name": "order_status" }), "name"),
        ("er.domain.add", json!({ "schema": sm.schema, "name": "d", "base_type": "VARCHAR(10)" }), "base_type"),
        ("er.domain.add", json!({ "schema": sm.schema, "name": "d", "base_type": "INTEGER", "length": 4 }), "length"),
        ("er.domain.add", json!({ "schema": sm.schema, "name": "d", "base_type": "TEXT", "checks": [" "] }), "checks"),
        ("er.domain.add", json!({ "schema": sm.schema, "name": "d", "base_type": "TEXT", "checks": [{ "expresion": "1 = 1" }] }), "checks"),
        ("er.enum.add", json!({ "schema": sm.schema, "name": "e", "literals": [] }), "literals"),
        ("er.enum.add", json!({ "schema": sm.schema, "name": "e", "literals": ["a", "A"] }), "literals"),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "domain": dom, "enum": en }), "enum"),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "type": "TEXT", "domain": dom }), "type"),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "domain": dom, "length": 10 }), "length"),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "enum": en, "default": "'cancelled'" }), "default"),
        ("er.attribute.set-default", json!({ "attribute": email, "default": "'a@b'" }), "default"),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "domain": dom, "default": "''" }), "default"),
        ("er.attribute.add", json!({ "entity": sm.order, "name": "x", "domain": dom, "collation": "C" }), "collation"),
        ("er.attribute.set-default", json!({ "attribute": status, "default": "'cancelled'" }), "default"),
        ("er.attribute.set-collation", json!({ "attribute": email, "collation": "C" }), "collation"),
        ("er.domain.remove", json!({ "domain": dom }), "domain"),
        ("er.enum.remove", json!({ "enum": en }), "enum"),
        ("er.enum.set", json!({ "enum": en, "name": "order_status", "literals": ["pending"] }), "literals"),
        ("er.domain.set", json!({ "domain": dom, "name": "email_address", "base_type": "INTEGER", "length": 255 }), "length"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    // Clearing a domain column's own default is fine; moving a column with a default onto a domain
    // drops the default, since the domain's counts.
    run(&mut s, "er.attribute.set-default", json!({ "attribute": email, "default": "" }));
    let dated = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "dated", "type": "TEXT", "default": "'x'", "collation": "C" }));
    run(&mut s, "er.attribute.set-type", json!({ "attribute": dated, "domain": dom }));
    let o = get(&mut s, "er.entity.get", json!({ "entity": sm.order }));
    let dated_json = o["attributes"].as_array().unwrap().iter().find(|a| a["id"] == dated).unwrap().clone();
    assert_eq!(dated_json["domain"], dom);
    assert_eq!(rules::text_of(s.doc(), obj(s.doc(), &dated), attribute::DEFAULT), "");
    assert_eq!(rules::text_of(s.doc(), obj(s.doc(), &dated), attribute::COLLATION), "");
    run(&mut s, "er.attribute.remove", json!({ "attribute": dated }));
    // A type name takes the domain's place and back; the enum's literals change under its default.
    round_trip(&mut s, "er.attribute.set-type", &json!({ "attribute": email, "type": "TEXT" }));
    run(&mut s, "er.attribute.set-type", json!({ "attribute": email, "type": "TEXT" }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": sm.customer }))["attributes"][1]["domain"], Json::Null);
    round_trip(&mut s, "er.attribute.set-type", &json!({ "attribute": email, "domain": dom }));
    round_trip(&mut s, "er.enum.set", &json!({ "enum": en, "name": "order_state", "literals": ["pending", "paid"] }));
    round_trip(&mut s, "er.domain.set", &json!({ "domain": dom, "name": "email_address", "base_type": "VARCHAR", "length": 320, "default": "''" }));
    all_rules_hold(s.doc());
    // Unused types go; a used one stays until its columns are retyped.
    run(&mut s, "er.attribute.set-type", json!({ "attribute": email, "type": "TEXT" }));
    round_trip(&mut s, "er.domain.remove", &json!({ "domain": dom }));
    run(&mut s, "er.attribute.remove", json!({ "attribute": status }));
    round_trip(&mut s, "er.enum.remove", &json!({ "enum": en }));
    all_rules_hold(s.doc());
}

#[test]
fn subtypes_share_their_supertypes_key_and_keep_it_in_step() {
    let mut s = session();
    let sm = sample(&mut s);
    let party = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Party" }));
    let pid = run(&mut s, "er.attribute.add", json!({ "entity": party, "name": "party_id", "type": "BIGINT", "primary_key": true }));
    let kind = run(&mut s, "er.attribute.add", json!({ "entity": party, "name": "party_type", "type": "TEXT" }));
    let person = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Person" }));
    let company = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Company" }));
    // Table per type: each subtype gets the supertype's key as its own, linked and identifying.
    round_trip(&mut s, "er.generalization.add", &json!({ "supertype": party, "subtype": person }));
    let g_company = run(&mut s, "er.generalization.add", json!({ "supertype": party, "subtype": company }));
    let g_person = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["generalizations"][0]["id"].as_str().unwrap().to_string();
    let p = get(&mut s, "er.entity.get", json!({ "entity": person }));
    assert_eq!(p["attributes"][0]["name"], "party_id");
    assert_eq!(p["attributes"][0]["linked_to"], pid);
    assert_eq!(p["primary_key"]["columns"], json!([p["attributes"][0]["id"]]));
    let sg = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    let gens = sg["generalizations"].as_array().unwrap();
    assert_eq!(gens.len(), 2);
    let key_rel = gens[0]["key_relationship"].as_str().unwrap().to_string();
    let r = relationship(&mut s, &sm.schema, &key_rel);
    assert_eq!((r["from_end"].as_str(), r["to_end"].as_str(), r["identifying"].as_bool()), (Some("exactly-one"), Some("zero-or-one"), Some(true)));
    all_rules_hold(s.doc());
    // The key follows the supertype: a rename reaches the subtypes' key columns.
    run(&mut s, "er.attribute.rename", json!({ "attribute": pid, "name": "id" }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": company }))["attributes"][0]["name"], "id");
    run(&mut s, "er.attribute.rename", json!({ "attribute": pid, "name": "party_id" }));
    // The set facts live on the supertype; a mapping change removes and brings back the keys.
    round_trip(
        &mut s,
        "er.entity.set-subtyping",
        &json!({ "entity": party, "complete": true, "disjoint": true, "discriminator": kind, "mapping": "single-table" }),
    );
    run(
        &mut s,
        "er.entity.set-subtyping",
        json!({ "entity": party, "complete": true, "disjoint": true, "discriminator": kind, "mapping": "single-table" }),
    );
    assert_eq!(
        get(&mut s, "er.entity.get", json!({ "entity": person }))["attributes"].as_array().map(Vec::len),
        Some(0),
        "single table: no key of its own"
    );
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": party }))["subtyping"]["mapping"], "single-table");
    all_rules_hold(s.doc());
    run(
        &mut s,
        "er.entity.set-subtyping",
        json!({ "entity": party, "complete": true, "disjoint": true, "discriminator": kind, "mapping": "table-per-type" }),
    );
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": person }))["attributes"][0]["name"], "party_id");
    all_rules_hold(s.doc());
    // A supertype without a key gives its subtypes theirs when it gets one.
    let asset = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Asset" }));
    let car = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Car" }));
    run(&mut s, "er.generalization.add", json!({ "supertype": asset, "subtype": car }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": car }))["attributes"].as_array().map(Vec::len), Some(0));
    run(&mut s, "er.attribute.add", json!({ "entity": asset, "name": "asset_id", "type": "UUID", "primary_key": true }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": car }))["attributes"][0]["name"], "asset_id");
    all_rules_hold(s.doc());
    // Refusals: itself, a second supertype, a circle, another schema, a foreign discriminator,
    // a mapping word, and the subtype key on its own.
    let other = run(&mut s, "er.schema.add", json!({ "name": "other" }));
    let elsewhere = run(&mut s, "er.entity.add", json!({ "schema": other, "name": "Elsewhere" }));
    for (cmd, p, key) in [
        ("er.generalization.add", json!({ "supertype": party, "subtype": party }), "subtype"),
        ("er.generalization.add", json!({ "supertype": asset, "subtype": person }), "subtype"),
        ("er.generalization.add", json!({ "supertype": person, "subtype": party }), "supertype"),
        ("er.generalization.add", json!({ "supertype": party, "subtype": elsewhere }), "subtype"),
        ("er.entity.set-subtyping", json!({ "entity": party, "discriminator": sm.key, "mapping": "table-per-type" }), "discriminator"),
        ("er.relationship.remove", json!({ "relationship": key_rel }), "relationship"),
        ("er.remove", json!({ "ids": [key_rel] }), "ids"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    // A generalization draws on a diagram from the subtype to the supertype.
    let d = run(&mut s, "diagram.add", json!({ "kind": "er", "element": sm.schema }));
    let n_party = run(&mut s, "diagram.place", json!({ "diagram": d, "element": party, "x": 200, "y": 0 }));
    let n_person = run(&mut s, "diagram.place", json!({ "diagram": d, "element": person, "x": 0, "y": 300 }));
    round_trip(&mut s, "diagram.connect", &json!({ "diagram": d, "element": g_person, "source": n_person, "target": n_party }));
    // Removing a generalization takes its key; removing the supertype takes its generalizations.
    round_trip(&mut s, "er.generalization.remove", &json!({ "generalization": g_company }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": company }))["attributes"].as_array().map(Vec::len), Some(0));
    round_trip(&mut s, "er.entity.remove", &json!({ "entity": party }));
    let sg = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert_eq!(sg["generalizations"].as_array().map(Vec::len), Some(1), "only Asset and Car stay");
    all_rules_hold(s.doc());
}

#[test]
fn subtype_keys_hold_through_chains_later_keys_removals_and_circles() {
    let mut s = session();
    let sm = sample(&mut s);
    let table = |s: &mut Session, name: &str| run(s, "er.entity.add", json!({ "schema": sm.schema, "name": name }));
    // A key set later with er.primary-key.set reaches the subtypes.
    let party = table(&mut s, "Party");
    let pid = run(&mut s, "er.attribute.add", json!({ "entity": party, "name": "party_id", "type": "BIGINT" }));
    let person = table(&mut s, "Person");
    let employee = table(&mut s, "Employee");
    run(&mut s, "er.generalization.add", json!({ "supertype": party, "subtype": person }));
    run(&mut s, "er.generalization.add", json!({ "supertype": person, "subtype": employee }));
    round_trip(&mut s, "er.primary-key.set", &json!({ "entity": party, "columns": [pid], "name": "" }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": employee }))["attributes"][0]["name"], "party_id", "three levels down");
    all_rules_hold(s.doc());
    // A piece of a subtype key does not go on its own; a call that takes its table along may.
    let follower = get(&mut s, "er.entity.get", json!({ "entity": person }))["attributes"][0]["id"].as_str().unwrap().to_string();
    for (cmd, p, key) in [
        ("er.attribute.remove", json!({ "attribute": follower }), "attribute"),
        ("er.primary-key.remove", json!({ "entity": person }), "entity"),
        ("er.remove", json!({ "ids": [follower, sm.order] }), "ids"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    // Removing the upper link of a three-level chain takes the keys below it cleanly.
    let g_upper = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["generalizations"][0]["id"].as_str().unwrap().to_string();
    round_trip(&mut s, "er.generalization.remove", &json!({ "generalization": g_upper }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": employee }))["attributes"].as_array().map(Vec::len), Some(0));
    all_rules_hold(s.doc());
    s.undo().unwrap();
    // Removing the supertype's key column takes the subtype keys with it and leaves no broken link.
    round_trip(&mut s, "er.attribute.remove", &json!({ "attribute": pid }));
    all_rules_hold(s.doc());
    s.undo().unwrap();
    round_trip(&mut s, "er.primary-key.remove", &json!({ "entity": party }));
    all_rules_hold(s.doc());
    s.undo().unwrap();
    round_trip(&mut s, "er.entity.remove", &json!({ "entity": party }));
    all_rules_hold(s.doc());
    s.undo().unwrap();
    // A table identified by another cannot become that table's supertype.
    let a = table(&mut s, "A");
    run(&mut s, "er.attribute.add", json!({ "entity": a, "name": "a_id", "type": "INT", "primary_key": true }));
    let b = table(&mut s, "B");
    run(&mut s, "er.attribute.add", json!({ "entity": b, "name": "b_id", "type": "INT", "primary_key": true }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": b, "to": a, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true, "identifying": true }),
    );
    assert_eq!(refused_on(&mut s, "er.generalization.add", json!({ "supertype": a, "subtype": b })), "subtype");
    all_rules_hold(s.doc());
    // A chain deeper than the budget is refused, without a deep recursion.
    let mut chain = vec![table(&mut s, "L0")];
    for i in 1..=31 {
        let next = table(&mut s, &format!("L{i}"));
        run(&mut s, "er.generalization.add", json!({ "supertype": chain[i - 1], "subtype": next }));
        chain.push(next);
    }
    let last = table(&mut s, "L32");
    assert_eq!(refused_on(&mut s, "er.generalization.add", json!({ "supertype": chain[31], "subtype": last })), "subtype");
    // Thirty-one levels take a key added at the top in one step, all the way down, and a second
    // key column after it.
    run(&mut s, "er.attribute.add", json!({ "entity": chain[0], "name": "id", "type": "INT", "primary_key": true }));
    run(&mut s, "er.attribute.add", json!({ "entity": chain[0], "name": "region", "type": "TEXT", "primary_key": true }));
    assert_eq!(get(&mut s, "er.entity.get", json!({ "entity": chain[31] }))["attributes"].as_array().map(Vec::len), Some(2));
    all_rules_hold(s.doc());
    // A wider subtype's own key column cannot go: its key goes whole, and the subtype key with it.
    let wide = table(&mut s, "Wide");
    run(&mut s, "er.attribute.add", json!({ "entity": wide, "name": "wide_id", "type": "INT", "primary_key": true }));
    run(&mut s, "er.generalization.add", json!({ "supertype": a, "subtype": wide }));
    let own = get(&mut s, "er.entity.get", json!({ "entity": wide }))["attributes"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(refused_on(&mut s, "er.attribute.remove", json!({ "attribute": own })), "attribute");
    all_rules_hold(s.doc());
}

#[test]
fn removing_the_top_of_a_very_deep_loaded_chain_is_refused_not_a_crash() {
    // Two hundred keyed subtype levels built straight into a store, past the commands' limit, as a
    // loaded document could carry them.
    let (mut st, _) = rooted_store();
    let m = st.alive().find(|o| st.class_of(*o).ok() == Some(MODEL)).unwrap();
    let s = st.create(SCHEMA).unwrap();
    st.set(s, named_element::NAME, Value::str("deep")).unwrap();
    st.push(m, model::MEMBER, Value::Ref(s)).unwrap();
    let mut prev: Option<(Obj, Obj, Obj)> = None;
    let mut top = None;
    for i in 0..200 {
        let e = st.create(ENTITY).unwrap();
        st.set(e, named_element::NAME, Value::str(&format!("T{i}"))).unwrap();
        st.push(s, schema::ENTITY, Value::Ref(e)).unwrap();
        let a = st.create(ATTRIBUTE).unwrap();
        st.set(a, named_element::NAME, Value::str("id")).unwrap();
        st.set(a, attribute::TYPE, Value::str("INT")).unwrap();
        st.set(a, attribute::NULLABLE, Value::Bool(false)).unwrap();
        st.push(e, entity::ATTRIBUTE, Value::Ref(a)).unwrap();
        let pk = st.create(PRIMARY_KEY).unwrap();
        st.set(e, entity::PRIMARY_KEY, Value::Ref(pk)).unwrap();
        st.push(pk, key::COLUMN, Value::Ref(a)).unwrap();
        if let Some((pe, pa, ppk)) = prev {
            st.set(a, attribute::LINKED_TO, Value::Ref(pa)).unwrap();
            let r = st.create(RELATIONSHIP).unwrap();
            st.set(r, relationship::FROM, Value::Ref(pe)).unwrap();
            st.set(r, relationship::TO, Value::Ref(e)).unwrap();
            st.set(r, relationship::FROM_END, Value::Enum(Cardinality::ExactlyOne.index())).unwrap();
            st.set(r, relationship::TO_END, Value::Enum(Cardinality::ZeroOrOne.index())).unwrap();
            st.set(r, relationship::IDENTIFYING, Value::Bool(true)).unwrap();
            st.push(s, schema::RELATIONSHIP, Value::Ref(r)).unwrap();
            let fk = st.create(FOREIGN_KEY).unwrap();
            st.set(fk, foreign_key::REFERENCES, Value::Ref(ppk)).unwrap();
            st.push(fk, foreign_key::COLUMN, Value::Ref(a)).unwrap();
            st.set(fk, foreign_key::RELATIONSHIP, Value::Ref(r)).unwrap();
            st.push(e, entity::FOREIGN_KEY, Value::Ref(fk)).unwrap();
            let g = st.create(GENERALIZATION).unwrap();
            st.set(g, generalization::SUPERTYPE, Value::Ref(pe)).unwrap();
            st.set(g, generalization::SUBTYPE, Value::Ref(e)).unwrap();
            st.set(g, generalization::KEY_RELATIONSHIP, Value::Ref(r)).unwrap();
            st.push(s, schema::GENERALIZATION, Value::Ref(g)).unwrap();
        } else {
            top = Some(st.uuid(e).unwrap().to_string());
        }
        prev = Some((e, a, pk));
    }
    let mut session = Session::new(registry().unwrap(), st, 1, Options::default());
    let before = picture(session.doc());
    let e = session.execute("er.primary-key.remove", &json!({ "entity": top.unwrap() })).expect_err("too deep to cascade");
    assert!(e.to_string().contains("deeper than") && e.to_string().contains("entity"), "{e}");
    assert_eq!(picture(session.doc()), before, "nothing of the attempt stays");
}

#[test]
fn sequences_and_views_belong_to_a_schema_under_its_rules() {
    let mut s = session();
    let sm = sample(&mut s);
    let other = run(&mut s, "er.schema.add", json!({ "name": "crm" }));
    let elsewhere = run(&mut s, "er.entity.add", json!({ "schema": other, "name": "Contact" }));
    let foreign_col = run(&mut s, "er.attribute.add", json!({ "entity": elsewhere, "name": "id", "type": "BIGINT" }));
    // A sequence owned by a column of its schema; its bounds are whole numbers kept as text.
    let seq = json!({
        "schema": sm.schema, "name": "order_no", "start": 1000, "increment": 1, "minimum": "1000", "maximum": "+99999999", "owned_by": sm.key,
    });
    round_trip(&mut s, "er.sequence.add", &seq);
    let view = json!({ "schema": sm.schema, "name": "big_orders", "query": "select *\nfrom \"Order\"\nwhere total > 100", "materialized": true });
    round_trip(&mut s, "er.view.add", &view);
    let sg = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    let q = sg["sequences"][0]["id"].as_str().unwrap().to_string();
    let v = sg["views"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(
        (&sg["sequences"][0]["start"], &sg["sequences"][0]["minimum"], &sg["sequences"][0]["maximum"], &sg["sequences"][0]["owned_by"]),
        (&json!(1000), &json!(1000), &json!(99_999_999), &json!(sm.key))
    );
    assert_eq!((&sg["views"][0]["materialized"], sg["views"][0]["query"].as_str().map(|q| q.lines().count())), (&json!(true), Some(3)));
    all_rules_hold(s.doc());
    // Tables, types, sequences and views share one namespace; the numbers keep their rules.
    for (cmd, p, key) in [
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "customer" }), "name"),
        ("er.view.add", json!({ "schema": sm.schema, "name": "ORDER_NO", "query": "select 1" }), "name"),
        ("er.entity.add", json!({ "schema": sm.schema, "name": "big_orders" }), "name"),
        ("er.domain.add", json!({ "schema": sm.schema, "name": "order_no", "base_type": "TEXT" }), "name"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "increment": 0 }), "increment"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "minimum": "one" }), "minimum"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "maximum": "99999999999999999999" }), "maximum"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "minimum": "10", "maximum": "10" }), "maximum"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "start": 5, "minimum": "10" }), "start"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "start": 50, "maximum": "10" }), "start"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "owned_by": foreign_col }), "owned_by"),
        ("er.sequence.add", json!({ "schema": sm.schema, "name": "s", "owned_by": sm.customer }), "owned_by"),
        ("er.sequence.set", json!({ "sequence": v, "name": "s" }), "sequence"),
        ("er.view.add", json!({ "schema": sm.schema, "name": "w", "query": "" }), "query"),
        ("er.view.add", json!({ "schema": sm.schema, "name": "w", "query": " select 1" }), "query"),
        ("er.view.set", json!({ "view": v, "name": "Customer", "query": "select 1" }), "name"),
        ("er.view.remove", json!({ "view": q }), "view"),
    ] {
        assert_eq!(refused_on(&mut s, cmd, p.clone()), key, "{cmd} {p}");
    }
    // A sequence counts down and cycles; a view changes its query and drops its rows.
    round_trip(
        &mut s,
        "er.sequence.set",
        &json!({ "sequence": q, "name": "countdown", "start": 10, "increment": -1, "minimum": "-5", "maximum": "10", "cycle": true }),
    );
    round_trip(&mut s, "er.view.set", &json!({ "view": v, "name": "all_orders", "query": "select * from \"Order\"" }));
    let sg = get(&mut s, "er.schema.get", json!({ "schema": sm.schema }));
    assert_eq!((&sg["sequences"][0]["owned_by"], &sg["sequences"][0]["minimum"]), (&Json::Null, &json!(-5)));
    assert_eq!(sg["views"][0]["materialized"], json!(false));
    // The column that owns a sequence goes; the sequence stays and forgets it, and undo brings it back.
    let no = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "no", "type": "BIGINT" }));
    run(&mut s, "er.sequence.set", json!({ "sequence": q, "name": "countdown", "owned_by": no }));
    round_trip(&mut s, "er.attribute.remove", &json!({ "attribute": no }));
    assert_eq!(get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["sequences"][0]["owned_by"], Json::Null);
    s.undo().unwrap();
    assert_eq!(get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["sequences"][0]["owned_by"], json!(no));
    run(&mut s, "er.entity.remove", json!({ "entity": sm.order }));
    assert_eq!(get(&mut s, "er.schema.get", json!({ "schema": sm.schema }))["sequences"][0]["owned_by"], Json::Null);
    all_rules_hold(s.doc());
    // They go one at a time or together with the Delete key's command.
    round_trip(&mut s, "er.remove", &json!({ "ids": [q, v] }));
    s.undo().unwrap();
    round_trip(&mut s, "er.sequence.remove", &json!({ "sequence": q }));
    round_trip(&mut s, "er.view.remove", &json!({ "view": v }));
    all_rules_hold(s.doc());
}

#[test]
fn keys_and_relationships_cross_schemas_and_go_with_either_schema() {
    let mut s = session();
    let sm = sample(&mut s);
    let crm = run(&mut s, "er.schema.add", json!({ "name": "crm" }));
    let contact = run(&mut s, "er.entity.add", json!({ "schema": crm, "name": "Contact" }));
    // A relationship from a table of shop to one of crm brings its key; it lives in the child's schema.
    let across = json!({ "from": sm.customer, "to": contact, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true });
    round_trip(&mut s, "er.relationship.add", &across);
    let r = get(&mut s, "er.schema.get", json!({ "schema": crm }))["relationships"][0]["id"].as_str().unwrap().to_string();
    assert!(!relationship(&mut s, &crm, &r).is_null(), "the relationship is crm's");
    assert!(relationship(&mut s, &sm.schema, &r).is_null(), "and not shop's");
    let c = get(&mut s, "er.entity.get", json!({ "entity": contact }));
    assert_eq!(c["attributes"][0]["name"], "customer_id");
    // A foreign key of its own points into the other schema too.
    let owner = run(&mut s, "er.attribute.add", json!({ "entity": contact, "name": "owner", "type": "BIGINT" }));
    round_trip(&mut s, "er.foreign-key.add", &json!({ "entity": contact, "references": sm.customer, "columns": [owner] }));
    all_rules_hold(s.doc());
    // A many-to-many relationship across schemas resolves into a mapping table of its schema.
    let tag = run(&mut s, "er.entity.add", json!({ "schema": crm, "name": "Tag" }));
    run(&mut s, "er.attribute.add", json!({ "entity": tag, "name": "id", "type": "BIGINT", "primary_key": true }));
    let tagged = run(&mut s, "er.relationship.add", json!({ "from": sm.customer, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more" }));
    round_trip(&mut s, "er.relationship.resolve", &json!({ "relationship": tagged }));
    let names: Vec<String> = get(&mut s, "er.schema.get", json!({ "schema": crm }))["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(names.iter().any(|n| n == "customer_tag"), "{names:?}");
    all_rules_hold(s.doc());
    // Generalizations stay inside one schema.
    assert_eq!(refused_on(&mut s, "er.generalization.add", json!({ "supertype": sm.customer, "subtype": contact })), "subtype");
    // Either schema goes with everything that reaches into it; undo brings it all back.
    round_trip(&mut s, "er.schema.remove", &json!({ "schema": crm }));
    all_rules_hold(s.doc());
    s.undo().unwrap();
    round_trip(&mut s, "er.schema.remove", &json!({ "schema": sm.schema }));
    all_rules_hold(s.doc());
    // Only Tag's own relationship to the mapping table stays; the table is a regular one again.
    let left = get(&mut s, "er.schema.get", json!({ "schema": crm }));
    assert_eq!(left["relationships"].as_array().map(Vec::len), Some(1));
    let mapping = left["entities"].as_array().unwrap().iter().find(|e| e["name"] == "customer_tag").cloned().unwrap();
    assert_eq!(mapping["kind"], "regular");
    assert!(get(&mut s, "er.entity.get", json!({ "entity": contact }))["foreign_keys"].as_array().unwrap().is_empty());
}

#[test]
fn view_queries_sequence_owners_and_types_of_another_schema_keep_their_limits() {
    let mut s = session();
    let sm = sample(&mut s);
    // A query as long as the store keeps is fine; one character more is refused under its name.
    let longest = "x".repeat(rules::MAX_QUERY);
    run(&mut s, "er.view.add", json!({ "schema": sm.schema, "name": "v", "query": longest }));
    let longer = "x".repeat(rules::MAX_QUERY + 1);
    assert_eq!(refused_on(&mut s, "er.view.add", json!({ "schema": sm.schema, "name": "w", "query": longer })), "query");
    assert_eq!(refused_on(&mut s, "er.view.add", json!({ "schema": sm.schema, "name": "w", "query": "select 1\u{202E}" })), "query");
    // A sequence is owned by a whole-number column only.
    let note = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "note", "type": "TEXT" }));
    assert_eq!(refused_on(&mut s, "er.sequence.add", json!({ "schema": sm.schema, "name": "q", "owned_by": note })), "owned_by");
    // An owner retyped later stays, and the sequence still saves with it unchanged.
    let no = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "no", "type": "BIGINT" }));
    let q = run(&mut s, "er.sequence.add", json!({ "schema": sm.schema, "name": "q", "owned_by": no }));
    run(&mut s, "er.attribute.set-type", json!({ "attribute": no, "type": "TEXT" }));
    run(&mut s, "er.sequence.set", json!({ "sequence": q, "name": "q_renamed", "owned_by": no }));
    assert_eq!(refused_on(&mut s, "er.sequence.set", json!({ "sequence": q, "name": "q_renamed", "owned_by": note })), "owned_by");
    run(&mut s, "er.sequence.remove", json!({ "sequence": q }));
    // A key column in another schema follows its target's domain; when the domain's schema goes,
    // the column keeps the base type and size.
    let ident = run(&mut s, "er.domain.add", json!({ "schema": sm.schema, "name": "ident", "base_type": "NUMERIC", "precision": 18, "scale": 0 }));
    run(&mut s, "er.attribute.set-type", json!({ "attribute": sm.key, "domain": ident }));
    let crm = run(&mut s, "er.schema.add", json!({ "name": "crm" }));
    let contact = run(&mut s, "er.entity.add", json!({ "schema": crm, "name": "Contact" }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.customer, "to": contact, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    let follower = get(&mut s, "er.entity.get", json!({ "entity": contact }))["attributes"][0].clone();
    assert_eq!(follower["domain"], json!(ident));
    let mine = run(&mut s, "er.attribute.add", json!({ "entity": contact, "name": "own_ident", "domain": ident }));
    all_rules_hold(s.doc());
    run(&mut s, "er.schema.remove", json!({ "schema": sm.schema }));
    let a = obj(s.doc(), &mine);
    assert_eq!((rules::type_of(s.doc(), a), rules::size_of(s.doc(), a)), ("NUMERIC".to_string(), (None, Some(18), None)));
    all_rules_hold(s.doc());
}

#[test]
fn a_schema_never_holds_two_relationships_of_one_name() {
    let mut s = session();
    let sm = sample(&mut s);
    let line = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Line" }));
    // A second "places" is refused, on add and on rename; unnamed ones may be many.
    let again = json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places" });
    assert_eq!(refused_on(&mut s, "er.relationship.add", again), "name");
    let holds = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.order, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "holds" }),
    );
    assert_eq!(refused_on(&mut s, "er.relationship.rename", json!({ "relationship": holds, "name": "places" })), "name");
    run(&mut s, "er.relationship.rename", json!({ "relationship": holds, "name": "holds" }));
    run(&mut s, "er.relationship.rename", json!({ "relationship": holds, "name": "" }));
    run(&mut s, "er.relationship.add", json!({ "from": sm.customer, "to": line, "from_end": "exactly-one", "to_end": "zero-or-more" }));
    // What the commands leave, a project can read back.
    assert_eq!(s.doc().validate_all(), vec![]);
}

#[test]
fn binary_and_varbinary_take_a_length() {
    let mut s = session();
    let sm = sample(&mut s);
    // A project's own ids: BINARY(16).
    let id = run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "uuid", "type": "BINARY", "length": 16 }));
    let a = s.doc().by_uuid(archforge_mof::Uuid::parse(&id).unwrap()).unwrap();
    assert_eq!(archforge_er::rules::type_display(s.doc(), a), "BINARY(16)");
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "blob", "type": "varbinary", "length": 65535 }));
    // BINARY holds at most 255 bytes; the size never goes into the name.
    assert_eq!(refused_on(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "big", "type": "BINARY", "length": 256 })), "length");
    assert_eq!(refused_on(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "named", "type": "BINARY(16)" })), "type");
    // A domain on BINARY(16), as the project model uses.
    run(&mut s, "er.domain.add", json!({ "schema": sm.schema, "name": "uuid16", "base_type": "BINARY", "length": 16 }));
}

#[test]
fn an_er_element_and_a_core_folder_are_named_by_the_shared_lookup() {
    let mut s = session();
    let sm = sample(&mut s);
    let st = s.doc();
    let obj = |id: &str| st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
    // The schema, a table, a column and a relationship, each by its own name property.
    for (id, name) in [(&sm.schema, "shop"), (&sm.customer, "Customer"), (&sm.key, "id"), (&sm.places, "places")] {
        assert_eq!(archforge_core::name_of(st, obj(id)), name);
        assert_eq!(archforge_er::rules::name_of(st, obj(id)), name, "ER's lookup is the shared one");
    }
    // The model root is the core's: named through the core's own name property.
    let (mut st, root) = rooted_store();
    assert_eq!(archforge_core::name_of(&st, root), "", "an unnamed root has no name");
    st.set(root, archforge_core::named_element::NAME, archforge_mof::Value::str("Shop")).unwrap();
    assert_eq!(archforge_core::name_of(&st, root), "Shop");
}
