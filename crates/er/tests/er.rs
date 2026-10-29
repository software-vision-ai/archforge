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
    assert_eq!(m.classes.len(), 6);
    assert_eq!(m.data_types.len(), 6, "the five primitives and Cardinality");
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
    AttributeObj::set_primary_key(&mut st, key, true).unwrap();
    AttributeObj::set_nullable(&mut st, key, false).unwrap();
    EntityObj::push_attribute(&mut st, customer, Value::Ref(key)).unwrap();
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
    assert_eq!(gone, 4, "the entity, its attribute, the relationship and the link");
    assert!(!st.is_alive(customer) && !st.is_alive(key) && !st.is_alive(places) && !st.is_alive(link));
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
    assert_eq!(out.value["removed"], 3);
    assert_eq!(s.doc().list(obj(s.doc(), &sm.schema), schema::ENTITY).unwrap(), vec![Value::Ref(obj(s.doc(), &sm.order))]);
    assert!(!s.view().selection.iter().any(|o| !s.doc().is_alive(*o)));
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

#[test]
fn the_sweep_finds_nothing() {
    let mut s = session();
    let sm = sample(&mut s);
    let calls =
        sweep_with(&mut s, &[&sm.schema, &sm.customer, &sm.key, &sm.places, "not-an-id", "00000000-0000-7000-8000-000000000000"], picture).unwrap();
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
    let n = rng.next() % 1000;
    let name = format!("n{}", rng.next() % 30);
    let ends = ["zero-or-one", "exactly-one", "zero-or-more", "one-or-more"];
    let end = |r: &mut Rng| ends[(r.next() % 4) as usize];
    match rng.below(12) {
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
