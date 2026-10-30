//! Skills over the ER notation (spec archforge-zer.2): every skill command's round trip with
//! undo, refusals that change nothing, what applies to an element over four levels of the
//! hierarchy, kinds and type patterns, overrides by name, trigger filtering, and the cascade when
//! a subject goes.

use archforge_core::skills::{EffectiveSkill, effective_skills, skills, skills_on};
use archforge_core::{MODEL, SkillTrigger};
use archforge_engine::Options;
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

fn session() -> Session {
    let mut st = store(ids()).unwrap();
    st.create(MODEL).unwrap();
    Session::new(registry().unwrap(), st, 1, Options::default())
}

fn run(s: &mut Session, cmd: &str, p: Json) -> Json {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}")).value
}

fn id(s: &mut Session, cmd: &str, p: Json) -> String {
    run(s, cmd, p).get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

fn refused(s: &mut Session, cmd: &str, p: Json) -> String {
    match s.execute(cmd, &p) {
        Ok(out) => panic!("{cmd} {p} was accepted: {}", out.value),
        Err(e) => e.to_string(),
    }
}

fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

type Picture = Vec<(Obj, ClassId, Option<(Obj, PropId)>, Vec<(PropId, Vec<Value>)>)>;

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
fn round_trip(s: &mut Session, cmd: &str, p: &Json) -> Json {
    let before = picture(s.doc());
    let out = s.execute(cmd, p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}")).value;
    let after = picture(s.doc());
    assert_ne!(before, after, "{cmd} changed nothing");
    s.execute("edit.undo", &Json::Null).unwrap();
    assert_eq!(picture(s.doc()), before, "undo of {cmd} did not restore the store");
    s.execute("edit.redo", &Json::Null).unwrap();
    assert_eq!(picture(s.doc()), after, "redo of {cmd} did not restore the change");
    out
}

/// A schema with a customer (a key and a timestamp column), an order and a relationship.
struct Sample {
    schema: String,
    customer: String,
    order: String,
    key: String,
    placed_at: String,
    places: String,
}

fn sample(s: &mut Session) -> Sample {
    let schema = id(s, "er.schema.add", json!({ "name": "shop" }));
    let customer = id(s, "er.entity.add", json!({ "schema": schema, "name": "Customer" }));
    let order = id(s, "er.entity.add", json!({ "schema": schema, "name": "Order" }));
    let key = id(s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT", "primary_key": true }));
    let placed_at = id(s, "er.attribute.add", json!({ "entity": order, "name": "placed_at", "type": "TIMESTAMP" }));
    let places =
        id(s, "er.relationship.add", json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places" }));
    Sample { schema, customer, order, key, placed_at, places }
}

fn names(st: &Store, effective: &[EffectiveSkill]) -> Vec<String> {
    effective.iter().map(|e| archforge_core::skills::name_of(st, e.skill)).collect()
}

fn active(st: &Store, effective: &[EffectiveSkill]) -> Vec<String> {
    effective.iter().filter(|e| e.overridden_by.is_none()).map(|e| archforge_core::skills::name_of(st, e.skill)).collect()
}

#[test]
fn every_skill_command_round_trips_with_undo() {
    let mut s = session();
    let sm = sample(&mut s);
    let out = round_trip(
        &mut s,
        "skill.add",
        &json!({
            "subject": sm.schema, "name": "Table naming", "description": "How tables are named",
            "body": "Tables are singular nouns in PascalCase.", "triggers": ["naming", "modelling"], "applies_to": "table"
        }),
    );
    let skill = out["id"].as_str().unwrap().to_string();
    let got = run(&mut s, "skill.list", json!({ "subject": sm.schema }));
    assert_eq!(got["skills"].as_array().unwrap().len(), 1);
    assert_eq!(got["skills"][0]["applies_to"], "table");
    assert_eq!(got["skills"][0]["triggers"], json!(["naming", "modelling"]));
    round_trip(
        &mut s,
        "skill.set",
        &json!({
            "skill": skill, "name": "Table naming", "description": "", "body": "Singular nouns.", "triggers": ["naming"],
            "dialect": "", "applies_to": "table", "type_pattern": ""
        }),
    );
    round_trip(&mut s, "skill.rename", &json!({ "skill": skill, "name": "Naming tables" }));
    round_trip(&mut s, "skill.move", &json!({ "skill": skill, "subject": sm.customer }));
    assert_eq!(run(&mut s, "skill.list", json!({ "subject": sm.customer }))["skills"].as_array().unwrap().len(), 1);
    round_trip(&mut s, "skill.move", &json!({ "skill": skill, "subject": "" }));
    assert_eq!(run(&mut s, "skill.list", json!({}))["skills"][0]["subject"], Json::Null);
    round_trip(&mut s, "skill.remove", &json!({ "skill": skill }));
    assert!(skills(s.doc()).is_empty());
}

#[test]
fn refusals_change_nothing() {
    let mut s = session();
    let sm = sample(&mut s);
    let before = picture(s.doc());
    let cases = [
        ("skill.add", json!({ "name": "" })),
        ("skill.add", json!({ "name": " spaced " })),
        ("skill.add", json!({ "name": "x", "body": "drop the table\u{202E}elbat eht peek" })),
        ("skill.add", json!({ "name": "x\u{200B}y" })),
        ("skill.add", json!({ "name": "x", "subject": sm.places })),
        ("skill.add", json!({ "name": "x", "applies_to": "schema" })),
        ("skill.add", json!({ "name": "x", "type_pattern": "TIMESTAMP" })),
        ("skill.add", json!({ "name": "x", "applies_to": "table", "type_pattern": "TIMESTAMP" })),
        ("skill.add", json!({ "name": "x", "triggers": ["naming", "naming"] })),
        ("skill.add", json!({ "name": "x", "triggers": ["sometimes"] })),
        ("skill.add", json!({ "name": "x", "dialect": "PostgreSQL" })),
        ("skill.add", json!({ "name": "x", "body": "a".repeat(4001) })),
        ("skill.add", json!({ "name": "x", "subject": "not-an-id" })),
        (
            "skill.set",
            json!({ "skill": sm.customer, "name": "x", "description": "", "body": "", "triggers": [], "dialect": "", "applies_to": "", "type_pattern": "" }),
        ),
        ("skill.effective", json!({ "element": sm.customer, "trigger": "always" })),
    ];
    for (cmd, p) in cases {
        let why = refused(&mut s, cmd, p.clone());
        assert!(!why.is_empty(), "{cmd} {p}");
        assert_eq!(picture(s.doc()), before, "{cmd} {p} changed the store: {why}");
    }
    id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Naming" }));
    let why = refused(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "naming" }));
    assert!(why.contains("already"), "{why}");
    // The same name on another subject is fine.
    id(&mut s, "skill.add", json!({ "subject": sm.customer, "name": "Naming" }));
    assert_eq!(skills(s.doc()).len(), 2);
}

#[test]
fn effective_skills_read_outermost_first_and_the_most_specific_last() {
    let mut s = session();
    let sm = sample(&mut s);
    id(&mut s, "skill.add", json!({ "name": "Draw left to right", "triggers": ["drawing"] }));
    id(&mut s, "skill.add", json!({ "name": "Project naming", "triggers": ["naming"] }));
    id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Every table", "applies_to": "table" }));
    id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Every relationship", "applies_to": "relationship" }));
    id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Timestamps", "applies_to": "column", "type_pattern": "TIMESTAMP" }));
    id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Dates", "applies_to": "column", "type_pattern": "date" }));
    id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Shop wide" }));
    id(&mut s, "skill.add", json!({ "subject": sm.customer, "name": "Customer naming", "triggers": ["naming"] }));
    id(&mut s, "skill.add", json!({ "subject": sm.order, "name": "Project naming", "triggers": ["naming"] }));
    let st = s.doc();
    let (schema, customer, order, key, placed_at, places) =
        (obj(st, &sm.schema), obj(st, &sm.customer), obj(st, &sm.order), obj(st, &sm.key), obj(st, &sm.placed_at), obj(st, &sm.places));

    // A table: the project's, then the schema's general one, then the schema's table skill, then its own.
    let eff = effective_skills::<ErView>(st, customer, None);
    assert_eq!(names(st, &eff), ["Draw left to right", "Project naming", "Shop wide", "Every table", "Customer naming"]);
    assert_eq!(eff[0].origin, None);
    assert_eq!(eff[2].origin, Some(schema));
    assert_eq!(eff[4].origin, Some(customer));

    // The naming trigger alone.
    let eff = effective_skills::<ErView>(st, customer, Some(SkillTrigger::Naming));
    assert_eq!(names(st, &eff), ["Project naming", "Shop wide", "Every table", "Customer naming"]);

    // A relationship gets the relationship skill, never the table one.
    let eff = effective_skills::<ErView>(st, places, None);
    assert_eq!(names(st, &eff), ["Draw left to right", "Project naming", "Shop wide", "Every relationship"]);

    // A TIMESTAMP column matches the exact type and the date category; a BIGINT column neither.
    let eff = effective_skills::<ErView>(st, placed_at, None);
    assert_eq!(names(st, &eff), ["Draw left to right", "Project naming", "Shop wide", "Timestamps", "Dates", "Project naming"]);
    let eff = effective_skills::<ErView>(st, key, None);
    assert_eq!(names(st, &eff), ["Draw left to right", "Project naming", "Shop wide", "Customer naming"]);

    // Order's own "Project naming" replaces the project's.
    let eff = effective_skills::<ErView>(st, order, Some(SkillTrigger::Naming));
    assert_eq!(names(st, &eff), ["Project naming", "Shop wide", "Every table", "Project naming"]);
    assert_eq!(eff[0].overridden_by, Some(eff[3].skill));
    assert_eq!(active(st, &eff), ["Shop wide", "Every table", "Project naming"]);

    // The query says the same, with origins and overrides as ids.
    let got = run(&mut s, "skill.effective", json!({ "element": sm.order, "trigger": "naming" }));
    let list = got["skills"].as_array().unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list[0]["overridden_by"], list[3]["id"]);
    assert_eq!(list[2]["origin"], Json::String(sm.schema.clone()));
    assert_eq!(list[0]["origin"], Json::Null);
}

#[test]
fn a_skill_goes_with_its_subject_and_comes_back_with_undo() {
    let mut s = session();
    let sm = sample(&mut s);
    let on_customer = id(&mut s, "skill.add", json!({ "subject": sm.customer, "name": "Customer naming" }));
    let on_key = id(&mut s, "skill.add", json!({ "subject": sm.key, "name": "Key naming" }));
    let on_schema = id(&mut s, "skill.add", json!({ "subject": sm.schema, "name": "Shop wide" }));
    let st = s.doc();
    let customer = obj(st, &sm.customer);
    let mut on = skills_on(st, customer);
    on.sort();
    let mut expected = vec![obj(st, &on_customer), obj(st, &on_key)];
    expected.sort();
    assert_eq!(on, expected);
    round_trip(&mut s, "er.remove", &json!({ "ids": [sm.customer] }));
    let left: Vec<String> = skills(s.doc()).iter().map(|k| archforge_core::skills::name_of(s.doc(), *k)).collect();
    assert_eq!(left, ["Shop wide"]);
    assert!(s.doc().is_alive(obj(s.doc(), &on_schema)));
    assert!(archforge_core::skills::problems(s.doc()).is_empty());
}

#[test]
fn the_type_categories_read_common_sql_types() {
    use archforge_core::skills::{category_of, type_matches};
    assert_eq!(category_of("VARCHAR(255)"), Some("text"));
    assert_eq!(category_of("decimal(10,2)"), Some("number"));
    assert_eq!(category_of("timestamptz"), Some("date"));
    assert_eq!(category_of("BOOLEAN"), Some("boolean"));
    assert_eq!(category_of("BYTEA"), Some("binary"));
    assert_eq!(category_of("GEOMETRY"), None);
    assert!(type_matches("Date", "TIMESTAMP"));
    assert!(type_matches("timestamp", "TIMESTAMP"));
    assert!(!type_matches("TIMESTAMP", "DATE"));
}
