//! ER diagrams through the ER session: the diagram commands with the ER policy, refusals that
//! change nothing, the cascades from the model into the diagrams, the open diagram in the view, the
//! grid, the sweep, and seeded random sequences over model and diagram commands together.

use archforge_core::{MODEL, model};
use archforge_diagram::{DIAGRAM, DiagramView, EDGE, Grid, Hit, LABEL, NODE, Rect};
use archforge_engine::testing::sweep_with;
use archforge_engine::{EngineError, Options};
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
    Session::with_view(registry().unwrap(), st, 1, Options::default(), archforge_diagram::ProjectView::new(ALONE))
}

fn run(s: &mut Session, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn alive(st: &Store, id: &str) -> bool {
    st.by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
}

/// A shop schema with Customer and Order, the relationship `places`, and an ER diagram of the
/// schema showing both entities, the relationship with its name label, and a note.
struct Sample {
    schema: String,
    customer: String,
    order: String,
    places: String,
    diagram: String,
    customer_node: String,
    order_node: String,
    edge: String,
    label: String,
    note: String,
}

fn sample(s: &mut Session) -> Sample {
    let schema = run(s, "er.schema.add", json!({ "name": "shop" }));
    let customer = run(s, "er.entity.add", json!({ "schema": schema, "name": "Customer" }));
    let order = run(s, "er.entity.add", json!({ "schema": schema, "name": "Order" }));
    run(s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT", "primary_key": true }));
    let places = run(
        s,
        "er.relationship.add",
        json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "name": "places" }),
    );
    let diagram = run(s, "diagram.add", json!({ "kind": "er", "element": schema, "name": "shop" }));
    let customer_node = run(s, "diagram.place", json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100 }));
    let order_node = run(s, "diagram.place", json!({ "diagram": diagram, "element": order, "x": 500, "y": 100 }));
    let edge = run(s, "diagram.connect", json!({ "diagram": diagram, "element": places, "source": customer_node, "target": order_node }));
    let label = run(s, "diagram.label", json!({ "edge": edge, "kind": "name" }));
    let note = run(s, "diagram.note", json!({ "diagram": diagram, "text": "who buys", "x": 100, "y": 400 }));
    Sample { schema, customer, order, places, diagram, customer_node, order_node, edge, label, note }
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

fn no_problems(st: &Store) {
    assert_eq!(rules::problems(st), Vec::<String>::new());
    assert_eq!(archforge_diagram::rules::problems(st), Vec::<String>::new());
    assert_eq!(archforge_core::problems(st), Vec::new());
}

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
fn the_er_policy_is_what_the_spec_says() {
    let v = archforge_diagram::ProjectView::new(ALONE);
    assert_eq!(v.kinds(), ["er"]);
    assert!(v.is_of("er", SCHEMA) && !v.is_of("er", ENTITY));
    assert!(v.shows("er", ENTITY) && !v.shows("er", ATTRIBUTE) && !v.shows("er", SCHEMA));
    assert!(v.connects("er", RELATIONSHIP) && !v.connects("er", ENTITY));
    assert_eq!(v.text_slots(RELATIONSHIP), ["name", "from_end", "to_end", "from_role", "to_role"]);
    assert!(v.text_slots(ENTITY).is_empty());
    assert_eq!(v.default_size(ENTITY), ENTITY_SIZE);
    let reg = registry().unwrap();
    assert!(reg.ids().any(|i| i == "diagram.place") && reg.ids().any(|i| i == "er.entity.add"));
    let m = metamodel().unwrap();
    assert_eq!(m.classes.len(), 19, "the diagram metaclasses are the diagram metamodel's, not ER's");
}

#[test]
fn every_diagram_command_round_trips_on_the_er_session() {
    let mut s = session();
    let sm = sample(&mut s);
    assert_eq!(s.view().open_diagram, Some(obj(s.doc(), &sm.diagram)), "adding a diagram opens it");
    let st = s.doc();
    let m = st.alive().find(|o| st.class_of(*o).ok() == Some(MODEL)).unwrap();
    assert!(st.list(m, model::MEMBER).unwrap().contains(&Value::Ref(obj(st, &sm.diagram))), "a diagram is a member of the model");
    let product = run(&mut s, "er.entity.add", json!({ "schema": sm.schema, "name": "Product" }));
    let contains =
        run(&mut s, "er.relationship.add", json!({ "from": sm.order, "to": product, "from_end": "zero-or-more", "to_end": "one-or-more" }));
    let product_node =
        run(&mut s, "diagram.place", json!({ "diagram": sm.diagram, "element": product, "x": 900, "y": 100, "width": 200, "height": 140 }));
    let calls: Vec<(&str, Json)> = vec![
        ("diagram.rename", json!({ "diagram": sm.diagram, "name": "the shop" })),
        ("diagram.move", json!({ "nodes": [sm.customer_node, sm.order_node], "dx": 20, "dy": 30 })),
        ("diagram.resize", json!({ "node": sm.order_node, "width": 240, "height": 160 })),
        ("diagram.route", json!({ "edge": sm.edge, "points": [{ "x": 300, "y": 300 }] })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "from_end", "at": "source", "dx": -10, "dy": -10 })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "to_end", "at": "target" })),
        ("diagram.connect-all", json!({ "diagram": sm.diagram })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "source": sm.note, "target": sm.customer_node })),
        ("diagram.set-text", json!({ "node": sm.note, "text": "who buys what" })),
        ("diagram.style", json!({ "ids": [sm.customer_node, sm.edge], "name": "key", "fill": "#eef5ff", "stroke": "#224488", "line": 1.5 })),
        ("diagram.style", json!({ "ids": [sm.edge] })),
        ("diagram.unlabel", json!({ "label": sm.label })),
        ("diagram.disconnect", json!({ "edge": sm.edge })),
        ("diagram.unplace", json!({ "nodes": [product_node] })),
        ("diagram.remove", json!({ "diagram": sm.diagram })),
    ];
    for (cmd, p) in calls {
        round_trip(&mut s, cmd, &p);
        no_problems(s.doc());
    }
    assert_eq!(s.view().open_diagram, None, "the open diagram went with the diagram");
    assert!(alive(s.doc(), &contains) && alive(s.doc(), &product), "the model is untouched");
    s.undo().unwrap();
    assert_eq!(s.view().open_diagram, None, "undo does not reopen; the view is not the document's");
    run(&mut s, "view.open-diagram", json!({ "diagram": sm.diagram }));
    assert_eq!(s.view().open_diagram, Some(obj(s.doc(), &sm.diagram)));
    let got = s.execute("diagram.get", &json!({ "diagram": sm.diagram })).unwrap().value;
    assert_eq!(got["kind"], "er");
    assert_eq!(got["element"], sm.schema);
    assert_eq!(got["edges"].as_array().unwrap().len(), 1, "the anchor; `contains` from connect-all went with Product's node");
}

#[test]
fn refusals_follow_the_er_policy_and_change_nothing() {
    let mut s = session();
    let sm = sample(&mut s);
    let key = s.execute("er.entity.get", &json!({ "entity": sm.customer })).unwrap().value["attributes"][0]["id"].as_str().unwrap().to_string();
    let other = run(&mut s, "er.schema.add", json!({ "name": "other" }));
    let elsewhere = run(&mut s, "er.entity.add", json!({ "schema": other, "name": "Elsewhere" }));
    let before = (picture(s.doc()), s.view().clone(), s.counter());
    let refused: Vec<(&str, Json)> = vec![
        ("diagram.add", json!({ "kind": "uml.class", "element": sm.schema })),
        ("diagram.add", json!({ "kind": "er", "element": sm.customer })),
        ("diagram.add", json!({ "kind": "er" })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": key, "x": 0, "y": 0 })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": sm.schema, "x": 0, "y": 0 })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": sm.customer, "x": 0, "y": 0 })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": sm.places, "source": sm.order_node, "target": sm.customer_node })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": sm.places, "source": sm.customer_node, "target": sm.order_node })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": sm.customer, "source": sm.customer_node, "target": sm.order_node })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "source": sm.customer_node, "target": sm.order_node })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "cardinality" })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "name" })),
        ("diagram.style", json!({ "ids": [sm.customer_node], "fill": "blue" })),
        ("er.entity.rename", json!({ "entity": sm.customer_node, "name": "X" })),
        ("element.remove", json!({ "ids": [sm.customer_node] })),
    ];
    for (cmd, p) in &refused {
        let e = s.execute(cmd, p).expect_err(&format!("{cmd} {p} was accepted"));
        // A handler's refusal carries the key; the engine's own refusal names the parameter.
        match &e {
            EngineError::Failed { error, .. } => assert!(error.key.is_some(), "{cmd}: {e}"),
            other => assert!(other.to_string().contains("parameter"), "{cmd}: {other}"),
        }
        assert_eq!((picture(s.doc()), s.view().clone(), s.counter()), before, "{cmd} {p} changed something");
    }
    // A schema's diagram may show a table of another schema (slice S7).
    let node = run(&mut s, "diagram.place", json!({ "diagram": sm.diagram, "element": elsewhere, "x": 0, "y": 0 }));
    assert!(!node.is_empty());
}

#[test]
fn removing_model_elements_removes_their_views_and_undo_brings_them_back() {
    let mut s = session();
    let sm = sample(&mut s);
    let before = picture(s.doc());
    // The relationship goes: its edge with the label goes, the nodes stay.
    let out = s.execute("er.relationship.remove", &json!({ "relationship": sm.places })).unwrap();
    assert_eq!(out.value["removed"], 3, "the edge, its label, the relationship");
    assert!(!alive(s.doc(), &sm.edge) && alive(s.doc(), &sm.customer_node));
    no_problems(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    // The entity goes: its node, the relationship with its edge and label, its attribute.
    let out = s.execute("er.entity.remove", &json!({ "entity": sm.customer })).unwrap();
    assert_eq!(out.value["removed"], 7, "the edge, the label, the relationship, the node, the attribute, the primary key, the entity");
    assert!(!alive(s.doc(), &sm.customer_node) && alive(s.doc(), &sm.order_node) && alive(s.doc(), &sm.diagram));
    no_problems(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    // The schema goes: the diagram of it goes with everything in it.
    assert_eq!(s.view().open_diagram, Some(obj(s.doc(), &sm.diagram)));
    s.execute("er.schema.remove", &json!({ "schema": sm.schema })).unwrap();
    assert!(s.doc().alive().all(|o| ![DIAGRAM, NODE, EDGE, LABEL].contains(&s.doc().class_of(o).unwrap())));
    assert_eq!(s.view().open_diagram, None);
    no_problems(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    // The mixed delete: a node is not an ER element, an entity and the relationship together are one step.
    let out = s.execute("element.remove", &json!({ "ids": [sm.order, sm.places] })).unwrap();
    assert_eq!(out.value["removed"], 5);
    no_problems(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

#[test]
fn without_a_model_root_no_diagram_can_be_added() {
    let mut st = store(ids()).unwrap();
    let schema = st.create(SCHEMA).unwrap();
    SchemaObj::set_name(&mut st, schema, Some("loose")).unwrap();
    let id = st.uuid(schema).unwrap().to_string();
    let mut s = Session::with_view(registry().unwrap(), st, 1, Options::default(), archforge_diagram::ProjectView::new(ALONE));
    let e = s.execute("diagram.add", &json!({ "kind": "er", "element": id })).unwrap_err();
    assert!(e.to_string().contains("no model root"), "{e}");
    assert_eq!(s.doc().live(), 1, "nothing was created");
}

#[test]
fn the_grid_works_on_an_er_diagram() {
    let mut s = session();
    let sm = sample(&mut s);
    let grid = Grid::build(s.doc(), obj(s.doc(), &sm.diagram));
    assert_eq!(grid.len(), (3, 1));
    assert_eq!(grid.hit(150.0, 150.0), Some(Hit::Node(obj(s.doc(), &sm.customer_node))));
    assert_eq!(grid.hit(380.0, 150.0), Some(Hit::Edge(obj(s.doc(), &sm.edge))), "between the entities' centres");
    assert_eq!(grid.hit(380.0, 300.0), None);
    let seen = grid.visible(Rect::new(0.0, 0.0, 300.0, 300.0));
    assert_eq!(seen.nodes, vec![obj(s.doc(), &sm.customer_node)]);
    assert_eq!(seen.edges, vec![obj(s.doc(), &sm.edge)]);
    let _ = sm.note;
}

#[test]
fn the_sweep_finds_nothing() {
    let mut s = session();
    let sm = sample(&mut s);
    let ids = [&*sm.schema, &*sm.customer, &*sm.places, &*sm.diagram, &*sm.customer_node, &*sm.edge, &*sm.label, &*sm.note, "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 1500, "{calls}");
    no_problems(s.doc());
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
    fn coord(&mut self) -> i64 {
        (self.next() % 2000) as i64 - 1000
    }
}

fn ids_of(s: &Session, class: ClassId) -> Vec<String> {
    s.doc().alive().filter(|o| s.doc().class_of(*o).ok() == Some(class)).map(|o| s.doc().uuid(o).unwrap().to_string()).collect()
}

/// A random model or diagram call against the store as it is.
fn random_call(rng: &mut Rng, s: &Session) -> (&'static str, Json) {
    let schemas = ids_of(s, SCHEMA);
    let entities = ids_of(s, ENTITY);
    let relationships = ids_of(s, RELATIONSHIP);
    let diagrams = ids_of(s, DIAGRAM);
    let nodes = ids_of(s, NODE);
    let edges = ids_of(s, EDGE);
    let name = format!("n{}", rng.next() % 30);
    let views: Vec<String> = nodes.iter().chain(&edges).cloned().collect();
    match rng.below(17) {
        0 if schemas.is_empty() => ("er.schema.add", json!({ "name": name })),
        0 | 1 => ("er.entity.add", json!({ "schema": rng.pick(&schemas), "name": name })),
        2 => (
            "er.relationship.add",
            json!({ "from": rng.pick(&entities), "to": rng.pick(&entities), "from_end": "exactly-one", "to_end": "zero-or-more" }),
        ),
        3 => ("er.entity.remove", json!({ "entity": rng.pick(&entities) })),
        4 => ("er.relationship.remove", json!({ "relationship": rng.pick(&relationships) })),
        5 if diagrams.len() < 2 => ("diagram.add", json!({ "kind": "er", "element": rng.pick(&schemas) })),
        5..=7 => ("diagram.place", json!({ "diagram": rng.pick(&diagrams), "element": rng.pick(&entities), "x": rng.coord(), "y": rng.coord() })),
        8 => (
            "diagram.connect",
            json!({ "diagram": rng.pick(&diagrams), "element": rng.pick(&relationships), "source": rng.pick(&nodes), "target": rng.pick(&nodes) }),
        ),
        9 => ("diagram.connect-all", json!({ "diagram": rng.pick(&diagrams) })),
        10 => ("diagram.move", json!({ "nodes": [rng.pick(&nodes).cloned().unwrap_or_default()], "dx": rng.coord(), "dy": rng.coord() })),
        11 => ("diagram.label", json!({ "edge": rng.pick(&edges), "kind": "name" })),
        12 => ("diagram.unplace", json!({ "nodes": [rng.pick(&nodes).cloned().unwrap_or_default()] })),
        13 => ("diagram.disconnect", json!({ "edge": rng.pick(&edges) })),
        14 if rng.below(8) == 0 => ("er.schema.remove", json!({ "schema": rng.pick(&schemas) })),
        14 => ("diagram.note", json!({ "diagram": rng.pick(&diagrams), "text": "n", "x": rng.coord(), "y": rng.coord() })),
        15 => ("diagram.link-note", json!({ "diagram": rng.pick(&diagrams), "note": rng.pick(&nodes), "target": rng.pick(&views) })),
        _ => ("element.remove", json!({ "ids": [rng.pick(&entities).cloned().unwrap_or_default()] })),
    }
}

#[test]
fn random_sequences_keep_every_rule_and_undo_back_to_the_start() {
    for seed in 1..=20u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = session();
        let start = picture(s.doc());
        for i in 0..150 {
            let (cmd, p) = random_call(&mut rng, &s);
            let _ = s.execute(cmd, &p);
            no_problems(s.doc());
            assert!(s.doc().validate_all().is_empty(), "seed {seed} step {i} after {cmd} {p}");
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
fn a_note_linked_to_a_relationships_line_goes_with_every_command_that_removes_the_line() {
    // Each command, from the sample with the note linked to the line; undo brings both back.
    let link =
        |s: &mut Session, sm: &Sample, edge: &str| run(s, "diagram.link-note", json!({ "diagram": sm.diagram, "note": sm.note, "target": edge }));
    for which in ["remove relationship", "remove entity", "disconnect"] {
        let mut s = session();
        let sm = sample(&mut s);
        let l = link(&mut s, &sm, &sm.edge);
        let (cmd, p) = match which {
            "remove relationship" => ("er.relationship.remove", json!({ "relationship": sm.places })),
            "remove entity" => ("er.entity.remove", json!({ "entity": sm.order })),
            _ => ("diagram.disconnect", json!({ "edge": sm.edge })),
        };
        round_trip(&mut s, cmd, &p);
        assert!(!alive(s.doc(), &l), "{which}: the link went with the line");
        no_problems(s.doc());
    }
    // Resolving a many-to-many into a mapping table, and collapsing it back.
    let mut s = session();
    let sm = sample(&mut s);
    run(&mut s, "er.attribute.add", json!({ "entity": sm.order, "name": "id", "type": "BIGINT", "primary_key": true }));
    let m2n = run(
        &mut s,
        "er.relationship.add",
        json!({ "from": sm.customer, "to": sm.order, "from_end": "zero-or-more", "to_end": "zero-or-more", "name": "watches" }),
    );
    let line = run(&mut s, "diagram.connect", json!({ "diagram": sm.diagram, "element": m2n, "source": sm.customer_node, "target": sm.order_node }));
    let l = link(&mut s, &sm, &line);
    let resolve = json!({ "relationship": m2n });
    round_trip(&mut s, "er.relationship.resolve", &resolve);
    assert!(!alive(s.doc(), &l), "the link went with the many-to-many's line");
    no_problems(s.doc());
    let mapping = {
        let st = s.doc();
        let t = st.alive().find(|o| st.class_of(*o).ok() == Some(ENTITY) && rules::name_of(st, *o) == "customer_order").expect("the mapping table");
        st.uuid(t).unwrap().to_string()
    };
    // A note linked to the mapping table's node goes when collapsing removes the table.
    let node = {
        let st = s.doc();
        let t = obj(st, &mapping);
        let n = archforge_diagram::views_of(st, t).2.first().copied().expect("the mapping table is placed");
        st.uuid(n).unwrap().to_string()
    };
    let l = link(&mut s, &sm, &node);
    round_trip(&mut s, "er.relationship.collapse", &json!({ "entity": mapping }));
    assert!(!alive(s.doc(), &l));
    no_problems(s.doc());
}
