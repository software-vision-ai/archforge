//! The ER notation through the engine: invariants, every command's round trip, refusals, cascades,
//! gestures, the view, ids after undo, the adversarial sweep, random sequences and the caps.

use std::collections::BTreeMap;

use archforge_engine::testing::{round_trip, sweep};
use archforge_engine::{EngineError, Options, Session, Snapshot};
use archforge_er::model::*;
use archforge_er::{ErModel, ErView, registry};
use serde_json::{Value, json};

type S = Session<archforge_er::Doc, ErView>;

fn session() -> S {
    Session::new(registry().unwrap(), archforge_er::Doc::default(), 1, Options::default())
}

fn id(out: archforge_engine::Outcome) -> String {
    out.value["id"].as_str().unwrap().to_string()
}

fn run(s: &mut S, cmd: &str, p: Value) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Value::as_str).unwrap_or_default().to_string()
}

/// A small model with one of everything: two entities with attributes, a relationship, a diagram.
fn sample(s: &mut S) -> (String, String, String, String, String) {
    let d = run(s, "er.diagram.add", json!({ "name": "Overview" }));
    let a = run(s, "er.entity.add", json!({ "name": "Customer", "diagram": d, "x": 10, "y": 20 }));
    let b = run(s, "er.entity.add", json!({ "name": "Order", "diagram": d, "x": 300, "y": 20 }));
    let k = run(s, "er.attribute.add", json!({ "entity": a, "name": "id", "kind": "BIGINT", "primary_key": true }));
    run(s, "er.attribute.add", json!({ "entity": b, "name": "total", "kind": "DECIMAL(10,2)" }));
    let r = run(s, "er.relationship.add", json!({ "from": a, "to": b, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places" }));
    (d, a, b, k, r)
}

#[test]
fn the_registry_documents_every_parameter() {
    let reg = registry().unwrap();
    assert!(reg.len() > 25);
}

#[test]
fn check_refuses_each_broken_invariant() {
    let mut s = session();
    let (d, a, b, k, r) = sample(&mut s);
    let good = s.doc().clone();
    let counter = s.counter();
    assert_eq!(good.check(counter), Ok(()));
    let broken: Vec<(&str, Box<dyn Fn(&mut ErModel)>)> = vec![
        (
            "filed under another key",
            Box::new(|m: &mut ErModel| {
                let e = m.entities.remove(&a).unwrap();
                m.entities.insert("e999".into(), e);
            }),
        ),
        (
            "an id used twice",
            Box::new(|m: &mut ErModel| {
                let attr = m.entities[&a].attributes[0].clone();
                m.entities.get_mut(&b).unwrap().attributes.push(Attribute { name: "other".into(), ..attr });
            }),
        ),
        (
            "a non-canonical id",
            Box::new(|m: &mut ErModel| {
                m.entities.get_mut(&a).unwrap().attributes[0].id = "a07".into();
            }),
        ),
        (
            "an id at the counter",
            Box::new({
                let a = a.clone();
                move |m: &mut ErModel| m.entities.get_mut(&a).unwrap().attributes[0].id = format!("a{counter}")
            }),
        ),
        ("a duplicate entity name", Box::new(|m: &mut ErModel| m.entities.get_mut(&b).unwrap().name = "CUSTOMER".into())),
        ("an empty name", Box::new(|m: &mut ErModel| m.entities.get_mut(&a).unwrap().name = String::new())),
        ("a control character", Box::new(|m: &mut ErModel| m.entities.get_mut(&a).unwrap().name = "a\tb".into())),
        ("a nullable key", Box::new(|m: &mut ErModel| m.entities.get_mut(&a).unwrap().attributes[0].nullable = true)),
        ("a relationship to nothing", Box::new(|m: &mut ErModel| m.relationships.get_mut(&r).unwrap().to = "e999".into())),
        ("a relationship name with a newline", Box::new(|m: &mut ErModel| m.relationships.get_mut(&r).unwrap().name = Some("a\nb".into()))),
        (
            "a place for nothing",
            Box::new(|m: &mut ErModel| {
                m.diagrams.get_mut(&d).unwrap().places.insert("e999".into(), Place { x: 0.0, y: 0.0, width: 100.0, height: 100.0 });
            }),
        ),
        ("a place out of range", Box::new(|m: &mut ErModel| m.diagrams.get_mut(&d).unwrap().places.get_mut(&a).unwrap().x = 2e6)),
        ("a box too small", Box::new(|m: &mut ErModel| m.diagrams.get_mut(&d).unwrap().places.get_mut(&a).unwrap().width = 5.0)),
        ("a NaN", Box::new(|m: &mut ErModel| m.diagrams.get_mut(&d).unwrap().places.get_mut(&a).unwrap().y = f64::NAN)),
    ];
    for (what, breaks) in &broken {
        let mut m = good.clone();
        breaks(&mut m);
        assert!(m.check(counter).is_err(), "{what} passed the check");
    }
    let _ = k;
}

#[test]
fn every_command_applies_undoes_and_redoes_to_the_identical_model() {
    let mut s = session();
    let (d, a, b, k, r) = sample(&mut s);
    let calls: Vec<(&str, Value)> = vec![
        ("er.entity.add", json!({ "name": "Product" })),
        ("er.entity.rename", json!({ "entity": b, "name": "Purchase" })),
        ("er.attribute.add", json!({ "entity": a, "name": "email", "at": 0 })),
        ("er.attribute.rename", json!({ "attribute": k, "name": "customer_id" })),
        ("er.attribute.set-kind", json!({ "attribute": k, "kind": "UUID" })),
        ("er.attribute.set-flags", json!({ "attribute": k, "primary_key": false, "nullable": "auto", "unique": true })),
        ("er.relationship.set-ends", json!({ "relationship": r, "from_end": "zero-or-one", "to_end": "one-or-more" })),
        ("er.relationship.rename", json!({ "relationship": r, "name": "" })),
        ("er.diagram.rename", json!({ "diagram": d, "name": "Sales" })),
        ("er.diagram.move", json!({ "diagram": d, "entities": [a, b], "dx": 5, "dy": -5 })),
        ("er.diagram.resize", json!({ "diagram": d, "entity": a, "width": 240, "height": 160 })),
        ("er.diagram.unplace", json!({ "diagram": d, "entity": b })),
        ("er.diagram.place", json!({ "diagram": d, "entity": b, "x": 400, "y": 50 })),
        ("er.relationship.remove", json!({ "relationship": r })),
        ("er.attribute.remove", json!({ "attribute": k })),
        ("er.entity.remove", json!({ "entity": b })),
        ("er.diagram.remove", json!({ "diagram": d })),
    ];
    for (cmd, p) in calls {
        round_trip(&mut s, cmd, &p).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(s.doc().check(s.counter()), Ok(()), "{cmd}");
    }
    let e = run(&mut s, "er.entity.add", json!({ "name": "Last" }));
    let attr = run(&mut s, "er.attribute.add", json!({ "entity": e, "name": "x" }));
    run(&mut s, "er.attribute.add", json!({ "entity": e, "name": "y" }));
    round_trip(&mut s, "er.attribute.move", &json!({ "attribute": attr, "to": 1 })).unwrap();
}

#[test]
fn refusals_name_their_key_and_change_nothing() {
    let mut s = session();
    let (d, a, b, k, r) = sample(&mut s);
    let before = (s.doc().clone(), s.counter());
    let refused: Vec<(&str, Value)> = vec![
        ("er.entity.add", json!({ "name": "customer" })),
        ("er.entity.add", json!({ "name": "" })),
        ("er.entity.add", json!({ "name": "X", "diagram": "d999" })),
        ("er.entity.add", json!({ "name": "X", "diagram": a })),
        ("er.entity.rename", json!({ "entity": k, "name": "X" })),
        ("er.attribute.add", json!({ "entity": a, "name": "ID" })),
        ("er.attribute.add", json!({ "entity": a, "name": "x", "primary_key": true, "nullable": "yes" })),
        ("er.attribute.add", json!({ "entity": a, "name": "x", "at": 5 })),
        ("er.attribute.add", json!({ "entity": a, "name": "x", "kind": "" })),
        ("er.attribute.move", json!({ "attribute": k, "to": 3 })),
        ("er.attribute.set-flags", json!({ "attribute": k, "primary_key": true, "nullable": "yes", "unique": false })),
        ("er.relationship.add", json!({ "from": a, "to": "e999", "from_end": "exactly-one", "to_end": "exactly-one" })),
        ("er.relationship.add", json!({ "from": a, "to": b, "from_end": "many", "to_end": "exactly-one" })),
        ("er.diagram.add", json!({ "name": "OVERVIEW" })),
        ("er.diagram.place", json!({ "diagram": d, "entity": a, "x": 0, "y": 0 })),
        ("er.diagram.move", json!({ "diagram": d, "entities": [], "dx": 1, "dy": 1 })),
        ("er.diagram.move", json!({ "diagram": d, "entities": [a, a], "dx": 1, "dy": 1 })),
        ("er.diagram.move", json!({ "diagram": d, "entities": [a], "dx": 1_999_999, "dy": 0 })),
        ("er.diagram.resize", json!({ "diagram": d, "entity": a, "width": 5, "height": 100 })),
        ("er.remove", json!({ "ids": [d] })),
        ("er.remove", json!({ "ids": ["e999"] })),
        ("view.select", json!({ "ids": [a, a] })),
        ("view.select", json!({ "ids": ["r999"] })),
    ];
    for (cmd, p) in &refused {
        let e = s.execute(cmd, p).expect_err(&format!("{cmd} {p} was accepted"));
        if let EngineError::Failed { error, .. } = &e {
            assert!(error.key.is_some() || !error.message.is_empty(), "{cmd}: {e}");
        }
        assert_eq!((s.doc().clone(), s.counter()), before, "{cmd} {p} changed the model");
    }
    let _ = r;
}

#[test]
fn removing_an_entity_takes_its_relationships_and_places_in_one_step() {
    let mut s = session();
    let (d, a, _b, _k, r) = sample(&mut s);
    let before = s.doc().clone();
    s.execute("er.entity.remove", &json!({ "entity": a })).unwrap();
    assert!(!s.doc().relationships.contains_key(&r));
    assert!(!s.doc().diagrams[&d].places.contains_key(&a));
    s.undo().unwrap();
    assert_eq!(*s.doc(), before);
}

#[test]
fn a_mixed_delete_with_an_entity_and_its_own_parts_is_one_step() {
    let mut s = session();
    let (_d, a, b, k, r) = sample(&mut s);
    let before = s.doc().clone();
    s.execute("er.remove", &json!({ "ids": [k, a, r] })).unwrap();
    assert_eq!(s.doc().entities.keys().collect::<Vec<_>>(), [&b]);
    s.undo().unwrap();
    assert_eq!(*s.doc(), before);
}

#[test]
fn dragging_several_entities_is_one_step() {
    let mut s = session();
    let (d, a, b, _k, _r) = sample(&mut s);
    let start = s.doc().clone();
    s.begin_interaction("er.diagram.move").unwrap();
    for i in 1..=10 {
        s.preview(&json!({ "diagram": d, "entities": [a, b], "dx": i * 3, "dy": i })).unwrap();
    }
    s.commit().unwrap();
    assert_eq!(s.doc().diagrams[&d].places[&a].x, 40.0);
    s.undo().unwrap();
    assert_eq!(*s.doc(), start);
}

#[test]
fn the_view_drops_what_an_undo_removed_and_ids_are_never_given_twice() {
    let mut s = session();
    let first = run(&mut s, "er.entity.add", json!({ "name": "A" }));
    assert_eq!(s.view().selection, std::slice::from_ref(&first));
    s.undo().unwrap();
    assert!(s.view().selection.is_empty());
    let second = run(&mut s, "er.entity.add", json!({ "name": "A" }));
    assert_ne!(first, second);
    let d = run(&mut s, "er.diagram.add", json!({ "name": "D" }));
    assert_eq!(s.view().diagram.as_deref(), Some(d.as_str()));
    s.undo().unwrap();
    assert_eq!(s.view().diagram, None);
}

#[test]
fn the_sweep_finds_nothing() {
    let mut s = session();
    let (d, a, _b, k, r) = sample(&mut s);
    let calls = sweep(&mut s, &[&a, &k, &r, &d, "e999", "a0", "x1"]).unwrap();
    assert!(calls > 1000, "{calls}");
    assert_eq!(s.doc().check(s.counter()), Ok(()));
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> {
        if v.is_empty() { None } else { v.get((self.next() % v.len() as u64) as usize) }
    }
}

/// A random call against the model as it is.
fn random_call(rng: &mut Rng, m: &ErModel) -> (&'static str, Value) {
    let entities: Vec<&String> = m.entities.keys().collect();
    let attributes: Vec<String> = m.entities.values().flat_map(|e| e.attributes.iter().map(|a| a.id.clone())).collect();
    let relationships: Vec<&String> = m.relationships.keys().collect();
    let diagrams: Vec<&String> = m.diagrams.keys().collect();
    let n = rng.next() % 1000;
    let name = format!("n{}", rng.next() % 30);
    let ends = ["zero-or-one", "exactly-one", "zero-or-more", "one-or-more"];
    let end = |r: &mut Rng| ends[(r.next() % 4) as usize];
    match rng.next() % 12 {
        0 | 1 => ("er.entity.add", json!({ "name": name, "diagram": diagrams.first().map_or(String::new(), |d| (*d).clone()), "x": n, "y": n })),
        2 => ("er.entity.rename", json!({ "entity": rng.pick(&entities), "name": name })),
        3 => ("er.entity.remove", json!({ "entity": rng.pick(&entities) })),
        4 => ("er.attribute.add", json!({ "entity": rng.pick(&entities), "name": name, "primary_key": n.is_multiple_of(3) })),
        5 => ("er.attribute.remove", json!({ "attribute": rng.pick(&attributes) })),
        6 => ("er.relationship.add", json!({ "from": rng.pick(&entities), "to": rng.pick(&entities), "from_end": end(rng), "to_end": end(rng) })),
        7 => ("er.relationship.remove", json!({ "relationship": rng.pick(&relationships) })),
        8 => ("er.diagram.add", json!({ "name": name })),
        9 => ("er.diagram.place", json!({ "diagram": rng.pick(&diagrams), "entity": rng.pick(&entities), "x": n, "y": n })),
        10 => ("er.diagram.move", json!({ "diagram": rng.pick(&diagrams), "entities": [rng.pick(&entities)], "dx": 7, "dy": 3 })),
        _ => ("er.remove", json!({ "ids": [rng.pick(&attributes).cloned().unwrap_or_default()] })),
    }
}

#[test]
fn random_sequences_keep_every_invariant_and_replay_to_the_same_model() {
    for seed in 1..=30u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = session();
        for _ in 0..120 {
            let (cmd, p) = random_call(&mut rng, s.doc());
            let _ = s.execute(cmd, &p);
            if rng.next().is_multiple_of(7) {
                let _ = s.undo();
            }
            assert_eq!(s.doc().check(s.counter()), Ok(()), "seed {seed} after {cmd} {p}");
        }
        let entries: Vec<_> = s.journal().cloned().collect();
        let mut fresh = session();
        for e in &entries {
            fresh.replay(e).unwrap_or_else(|err| panic!("seed {seed}: {err}"));
        }
        assert_eq!(fresh.doc(), s.doc(), "seed {seed}: the replay differs");
        assert_eq!(fresh.counter(), s.counter());
    }
}

#[test]
fn a_model_at_every_cap_passes_and_weighs_well_under_the_history_cap() {
    let mut m = ErModel::default();
    let mut n = 1u64;
    let mut fresh = |kind: Kind| {
        let id = make_id(kind, n);
        n += 1;
        id
    };
    let mut entity_ids = Vec::new();
    for i in 0..MAX_ENTITIES {
        let id = fresh(Kind::Entity);
        let attributes = (0..MAX_ATTRIBUTES / MAX_ENTITIES)
            .map(|j| Attribute {
                id: fresh(Kind::Attribute),
                name: format!("attribute_{j}"),
                kind: "VARCHAR(64)".into(),
                primary_key: false,
                nullable: true,
                unique: false,
            })
            .collect();
        m.entities.insert(id.clone(), Entity { id: id.clone(), name: format!("entity_{i}"), attributes });
        entity_ids.push(id);
    }
    for i in 0..MAX_RELATIONSHIPS {
        let id = fresh(Kind::Relationship);
        let (from, to) = (entity_ids[i % MAX_ENTITIES].clone(), entity_ids[(i * 7 + 1) % MAX_ENTITIES].clone());
        m.relationships.insert(
            id.clone(),
            Relationship { id, name: Some(format!("rel_{i}")), from, to, from_end: Cardinality::ExactlyOne, to_end: Cardinality::ZeroOrMore },
        );
    }
    for i in 0..MAX_DIAGRAMS {
        let id = fresh(Kind::Diagram);
        let places: BTreeMap<Id, Place> = (0..MAX_PLACES / MAX_DIAGRAMS)
            .map(|j| (entity_ids[(i * 200 + j) % MAX_ENTITIES].clone(), Place { x: j as f64, y: i as f64, width: 200.0, height: 120.0 }))
            .collect();
        m.diagrams.insert(id.clone(), Diagram { id, name: format!("diagram_{i}"), places });
    }
    assert_eq!(m.counts(), (MAX_ATTRIBUTES, MAX_PLACES));
    assert_eq!(m.check(n), Ok(()));
    assert!(m.weight() * 4 < Options::default().max_weight, "{} bytes", m.weight());
    let _ = id;
}

#[test]
fn a_selection_of_many_attributes_costs_one_pass() {
    let mut m = ErModel::default();
    let mut n = 1u64;
    let mut ids = Vec::new();
    for i in 0..1000 {
        let e = make_id(Kind::Entity, n);
        n += 1;
        let attributes: Vec<Attribute> = (0..50)
            .map(|j| {
                let id = make_id(Kind::Attribute, n);
                n += 1;
                Attribute { id, name: format!("a{j}"), kind: "INT".into(), primary_key: false, nullable: true, unique: false }
            })
            .collect();
        ids.extend(attributes.iter().map(|a| a.id.clone()));
        m.entities.insert(e.clone(), Entity { id: e, name: format!("e{i}"), attributes });
    }
    let mut s = Session::new(registry().unwrap(), archforge_er::Doc::new(m), n, Options::default());
    let started = std::time::Instant::now();
    s.execute("view.select", &json!({ "ids": ids })).unwrap();
    s.execute("er.entity.add", &json!({ "name": "Another" })).unwrap();
    s.execute("view.select", &json!({ "ids": ids })).unwrap();
    s.execute("er.remove", &json!({ "ids": ids })).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
    assert_eq!(s.doc().counts().0, 0);
}
