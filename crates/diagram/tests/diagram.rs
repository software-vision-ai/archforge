//! The diagram metamodel through the engine over the MOF core's toy metamodel, with a test notation
//! whose diagrams show classes as nodes and generalizations as edges: every command's round trip,
//! refusals that change nothing, the cascades, the grid, the rules over hand-made data, the sweep,
//! and seeded random sequences.

use std::sync::Arc;

use archforge_diagram::rules::problems;
use archforge_diagram::*;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Document, EngineError, Options, Registry, Session};
use archforge_mof::toy::{self, CLASS, GENERALIZATION, PACKAGE, ToySlabs};
use archforge_mof::{ClassId, CompositeSlabs, Obj, PropId, Store, Uuid, Value};
use serde_json::{Value as Json, json};

/// The test notation's view: the selection, the open diagram, and a policy of classes and
/// generalizations.
#[derive(Debug, Clone, PartialEq, Default)]
struct TestView {
    selection: Vec<Obj>,
    open: Option<Obj>,
}

impl archforge_engine::View<Store> for TestView {
    fn retain_valid(&mut self, doc: &Store) {
        self.selection.retain(|o| doc.is_alive(*o));
        if self.open.is_some_and(|d| !doc.is_alive(d)) {
            self.open = None;
        }
    }
}

impl DiagramView for TestView {
    fn open_diagram(&self) -> Option<Obj> {
        self.open
    }
    fn set_open_diagram(&mut self, diagram: Option<Obj>) {
        self.open = diagram;
    }
    fn selection_mut(&mut self) -> &mut Vec<Obj> {
        &mut self.selection
    }
    fn kinds() -> &'static [&'static str] {
        &["toy.class"]
    }
    fn home(_: &Store) -> Result<Option<(Obj, PropId)>, String> {
        Ok(None)
    }
    fn is_of(_: &str, class: ClassId) -> bool {
        class == PACKAGE
    }
    fn shows(_: &str, class: ClassId) -> bool {
        class == CLASS
    }
    fn connects(_: &str, class: ClassId) -> bool {
        class == GENERALIZATION
    }
    fn ends(store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        if store.class_of(element).ok()? != GENERALIZATION {
            return None;
        }
        let specific = store.container(element).ok()??.0;
        let general = store.get(element, toy::GENERALIZATION_GENERAL).ok()?.as_ref()?;
        Some((specific, general))
    }
    fn text_slots(class: ClassId) -> &'static [&'static str] {
        if class == GENERALIZATION { &["general"] } else { &[] }
    }
    fn default_size(_: ClassId) -> (f64, f64) {
        (120.0, 80.0)
    }
}

type S = Session<Store, TestView>;

fn store() -> Store {
    let universe = universe_with(vec![toy::metamodel().unwrap()]).unwrap();
    let slabs = with_slabs(CompositeSlabs::new().with(toy::INDEX, Box::new(ToySlabs::default())).unwrap()).unwrap();
    Store::new(universe, Box::new(slabs), toy::counter_ids())
}

fn registry() -> Arc<Registry<Store, TestView>> {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend(commands::specs::<TestView>());
    Registry::new(specs).unwrap()
}

/// A package with three classes, `B` and `C` generalizing `A`.
struct Model {
    package: Obj,
    a: Obj,
    b: Obj,
    c: Obj,
    b_gen: Obj,
    c_gen: Obj,
}

fn model(st: &mut Store) -> Model {
    let package = st.create(PACKAGE).unwrap();
    st.set(package, toy::NAMED_ELEMENT_NAME, Value::str("p")).unwrap();
    let class = |st: &mut Store, name: &str| {
        let c = st.create(CLASS).unwrap();
        st.set(c, toy::NAMED_ELEMENT_NAME, Value::str(name)).unwrap();
        st.push(package, toy::PACKAGE_PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        c
    };
    let (a, b, c) = (class(st, "A"), class(st, "B"), class(st, "C"));
    let generalize = |st: &mut Store, specific: Obj, general: Obj| {
        let g = st.create(GENERALIZATION).unwrap();
        st.set(g, toy::GENERALIZATION_GENERAL, Value::Ref(general)).unwrap();
        st.push(specific, toy::CLASS_GENERALIZATION, Value::Ref(g)).unwrap();
        g
    };
    let (b_gen, c_gen) = (generalize(st, b, a), generalize(st, c, a));
    Model { package, a, b, c, b_gen, c_gen }
}

fn session() -> (S, Model) {
    let mut st = store();
    let m = model(&mut st);
    (Session::new(registry(), st, 1, Options::default()), m)
}

fn id(st: &Store, o: Obj) -> String {
    st.uuid(o).unwrap().to_string()
}

fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

/// A diagram of the package with nodes for A and B, the edge for B's generalization, a label on
/// it, a note anchored to A and a style on A.
struct Sample {
    diagram: String,
    a: String,
    b: String,
    edge: String,
    label: String,
    note: String,
    anchor: String,
}

fn sample(s: &mut S, m: &Model) -> Sample {
    let st = s.doc();
    let (package, a, b, b_gen) = (id(st, m.package), id(st, m.a), id(st, m.b), id(st, m.b_gen));
    let diagram = run(s, "diagram.add", json!({ "kind": "toy.class", "element": package, "name": "overview" }));
    let a = run(s, "diagram.place", json!({ "diagram": diagram, "element": a, "x": 100, "y": 100 }));
    let b = run(s, "diagram.place", json!({ "diagram": diagram, "element": b, "x": 400, "y": 100, "width": 200, "height": 120 }));
    let edge = run(s, "diagram.connect", json!({ "diagram": diagram, "element": b_gen, "source": b, "target": a }));
    let label = run(s, "diagram.label", json!({ "edge": edge, "kind": "general", "at": "target", "dx": 4, "dy": -8 }));
    let note = run(s, "diagram.note", json!({ "diagram": diagram, "text": "the root", "x": 100, "y": 300 }));
    let anchor = run(s, "diagram.connect", json!({ "diagram": diagram, "source": note, "target": a }));
    run(s, "diagram.style", json!({ "ids": [a], "fill": "#ffeecc", "line": 2 }));
    Sample { diagram, a, b, edge, label, note, anchor }
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

fn round_trip(s: &mut S, cmd: &str, p: &Json) {
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
    let reg = registry();
    assert!(reg.ids().filter(|i| i.starts_with("diagram.")).count() == 18 && reg.ids().any(|i| i == "view.open-diagram"));
    let m = metamodel().unwrap();
    assert_eq!(m.classes.len(), 7);
    assert_eq!(m.data_types.len(), 6, "the five primitives and LabelPlace");
    assert_eq!(LabelPlace::from_index(1), Some(LabelPlace::Middle));
    assert_eq!(commands::place_name(LabelPlace::Target), "target");
    assert!(TABLES.iter().any(|t| t.name == "diagram_node") && JUNCTIONS.iter().any(|j| j.name == "diagram_diagram_node" && j.ordered));
    assert_eq!(INDEX, 5);
}

#[test]
fn every_command_applies_undoes_and_redoes_to_the_identical_store() {
    let (mut s, m) = session();
    let sm = sample(&mut s, &m);
    let (c, c_gen, package) = (id(s.doc(), m.c), id(s.doc(), m.c_gen), id(s.doc(), m.package));
    let c_node = run(&mut s, "diagram.place", json!({ "diagram": sm.diagram, "element": c, "x": 700, "y": 100 }));
    let port = run(&mut s, "diagram.note", json!({ "diagram": sm.diagram, "text": "in", "x": 710, "y": 110, "parent": c_node }));
    let _ = (port, package);
    let calls: Vec<(&str, Json)> = vec![
        ("diagram.rename", json!({ "diagram": sm.diagram, "name": "classes" })),
        ("diagram.rename", json!({ "diagram": sm.diagram, "name": "" })),
        ("diagram.set-text", json!({ "node": sm.note, "text": "the root\nof all" })),
        ("diagram.move", json!({ "nodes": [sm.a, sm.b], "dx": 10, "dy": -5 })),
        ("diagram.move", json!({ "nodes": [c_node], "dx": 0, "dy": 0 })),
        ("diagram.resize", json!({ "node": sm.a, "width": 300, "height": 50 })),
        ("diagram.route", json!({ "edge": sm.edge, "points": [{ "x": 250, "y": 50 }, { "x": 300, "y": 40 }] })),
        ("diagram.route", json!({ "edge": sm.edge, "points": [] })),
        ("diagram.connect-all", json!({ "diagram": sm.diagram })),
        ("diagram.style", json!({ "ids": [sm.a, sm.edge], "name": "accent", "stroke": "#112233", "text": "#000000" })),
        ("diagram.style", json!({ "ids": [sm.a, sm.edge] })),
        ("diagram.unlabel", json!({ "label": sm.label })),
        ("diagram.disconnect", json!({ "edge": sm.anchor })),
        ("diagram.unplace", json!({ "nodes": [sm.b] })),
        ("diagram.unplace", json!({ "nodes": [c_node] })),
        ("diagram.remove", json!({ "diagram": sm.diagram })),
    ];
    for (cmd, p) in calls {
        round_trip(&mut s, cmd, &p);
        assert_eq!(problems(s.doc()), Vec::<String>::new(), "{cmd}");
    }
    let _ = c_gen;
    // What is left: nothing of the diagram, the model untouched.
    assert!(only_toy(s.doc()));
    assert!(s.view().open.is_none(), "the open diagram went with the diagram");
}

#[test]
fn connect_all_adds_the_missing_edges_once() {
    let (mut s, m) = session();
    let sm = sample(&mut s, &m);
    let (c_id, package, a_id) = (id(s.doc(), m.c), id(s.doc(), m.package), id(s.doc(), m.a));
    let c = run(&mut s, "diagram.place", json!({ "diagram": sm.diagram, "element": c_id, "x": 700, "y": 100 }));
    let out = s.execute("diagram.connect-all", &json!({ "diagram": sm.diagram })).unwrap();
    assert_eq!(out.value["added"], 1, "C's generalization; B's is shown already");
    let out = s.execute("diagram.connect-all", &json!({ "diagram": sm.diagram })).unwrap();
    assert_eq!(out.value["added"], 0);
    let got = s.execute("diagram.get", &json!({ "diagram": sm.diagram })).unwrap().value;
    assert_eq!(got["edges"].as_array().unwrap().len(), 3, "B's, the anchor, C's");
    assert_eq!(got["nodes"].as_array().unwrap().len(), 4);
    assert_eq!(got["edges"][0]["labels"][0]["at"], "target");
    assert_eq!(got["nodes"][0]["style"]["fill"], "#ffeecc");
    assert_eq!(got["nodes"][0]["style"]["line"], 2.0);
    assert!(got["nodes"][2]["text"] == "the root" && got["nodes"][2]["kind"] == "note");
    let list = s.execute("diagram.list", &json!({ "element": package })).unwrap().value;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(s.execute("diagram.list", &json!({ "element": a_id })).unwrap().value.as_array().unwrap().len(), 0);
    let _ = c;
}

#[test]
fn refusals_name_their_key_and_change_nothing() {
    let (mut s, m) = session();
    let sm = sample(&mut s, &m);
    let (package, a, c, c_gen, b_gen) = (id(s.doc(), m.package), id(s.doc(), m.a), id(s.doc(), m.c), id(s.doc(), m.c_gen), id(s.doc(), m.b_gen));
    let other = run(&mut s, "diagram.add", json!({ "kind": "toy.class" }));
    let c_in_other = run(&mut s, "diagram.place", json!({ "diagram": other, "element": c, "x": 0, "y": 0 }));
    // Sixteen notes under B reach the deepest level; one more is refused.
    let mut parent = sm.b.clone();
    for i in 0..16 {
        parent = run(&mut s, "diagram.note", json!({ "diagram": sm.diagram, "text": format!("level {i}"), "x": 400, "y": 100, "parent": parent }));
    }
    let before = (picture(s.doc()), s.view().clone(), s.counter());
    let refused: Vec<(&str, Json)> = vec![
        ("diagram.add", json!({ "kind": "uml.class" })),
        ("diagram.add", json!({ "kind": "Toy Class" })),
        ("diagram.add", json!({ "kind": "toy.class", "element": a })),
        ("diagram.add", json!({ "kind": "toy.class", "name": " spaced" })),
        ("diagram.rename", json!({ "diagram": sm.a, "name": "x" })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": package, "x": 0, "y": 0 })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": a, "x": 0, "y": 0 })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": c, "x": 2000000, "y": 0 })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": c, "x": 0, "y": 0, "width": 5 })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": c, "x": 0, "y": 0, "parent": c_in_other })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": c, "x": 0, "y": 0, "parent": parent })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": c, "x": 0, "y": 0, "kind": "Port" })),
        ("diagram.place", json!({ "diagram": sm.diagram, "element": "not-an-id", "x": 0, "y": 0 })),
        ("diagram.note", json!({ "diagram": sm.diagram, "text": "", "x": 0, "y": 0 })),
        ("diagram.note", json!({ "diagram": sm.diagram, "text": "a\u{7}b", "x": 0, "y": 0 })),
        ("diagram.set-text", json!({ "node": sm.a, "text": "x" })),
        ("diagram.move", json!({ "nodes": [], "dx": 1, "dy": 1 })),
        ("diagram.move", json!({ "nodes": [sm.a, c_in_other], "dx": 1, "dy": 1 })),
        ("diagram.move", json!({ "nodes": [sm.a], "dx": 999_999, "dy": 0 })),
        ("diagram.move", json!({ "nodes": [sm.a], "dx": 0, "dy": -1_999_999 })),
        ("diagram.resize", json!({ "node": sm.a, "width": 10, "height": 50 })),
        ("diagram.move", json!({ "nodes": [sm.a, sm.a], "dx": 1, "dy": 1 })),
        ("diagram.resize", json!({ "node": sm.a, "width": 20000, "height": 50 })),
        ("diagram.resize", json!({ "node": sm.edge, "width": 200, "height": 50 })),
        ("diagram.unplace", json!({ "nodes": [] })),
        ("diagram.unplace", json!({ "nodes": [sm.edge] })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "source": sm.a, "target": sm.b })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": c_gen, "source": sm.a, "target": sm.b })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": c_gen, "source": c_in_other, "target": sm.a })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": package, "source": sm.b, "target": sm.a })),
        ("diagram.connect", json!({ "diagram": sm.diagram, "element": b_gen, "source": sm.b, "target": sm.a })),
        ("diagram.route", json!({ "edge": sm.edge, "points": [{ "x": 1 }] })),
        ("diagram.route", json!({ "edge": sm.edge, "points": [{ "x": 1, "y": 2000000 }] })),
        ("diagram.route", json!({ "edge": sm.edge, "points": (0..300).map(|i| json!({ "x": i, "y": i })).collect::<Vec<_>>() })),
        ("diagram.label", json!({ "edge": sm.anchor, "kind": "general" })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "name" })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "general" })),
        ("diagram.label", json!({ "edge": sm.edge, "kind": "general", "at": "end" })),
        ("diagram.unlabel", json!({ "label": sm.edge })),
        ("diagram.style", json!({ "ids": [sm.a], "fill": "red" })),
        ("diagram.style", json!({ "ids": [sm.a], "fill": "#12345" })),
        ("diagram.style", json!({ "ids": [sm.a], "name": "Accent Style" })),
        ("diagram.style", json!({ "ids": [sm.a], "line": 500 })),
        ("diagram.style", json!({ "ids": [sm.diagram] })),
        ("diagram.style", json!({ "ids": [] })),
        ("view.open-diagram", json!({ "diagram": sm.a })),
        ("diagram.get", json!({ "diagram": "00000000-0000-7000-8000-000000000000" })),
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
    let _ = sm.label;
}

#[test]
fn removing_an_element_takes_its_views_and_undo_brings_them_back() {
    let mut st = store();
    let m = model(&mut st);
    // A diagram of the package with A, B (with a child showing C) and both edges, built directly.
    let d = st.create(DIAGRAM).unwrap();
    DiagramObj::set_kind(&mut st, d, Some("toy.class")).unwrap();
    DiagramObj::set_element(&mut st, d, Some(m.package)).unwrap();
    let node = |st: &mut Store, element: Obj, x: f64| {
        let n = st.create(NODE).unwrap();
        NodeObj::set_element(st, n, Some(element)).unwrap();
        NodeObj::set_x(st, n, x).unwrap();
        NodeObj::set_width(st, n, 100.0).unwrap();
        NodeObj::set_height(st, n, 50.0).unwrap();
        n
    };
    let (a, b, c) = (node(&mut st, m.a, 0.0), node(&mut st, m.b, 300.0), node(&mut st, m.c, 320.0));
    DiagramObj::push_node(&mut st, d, Value::Ref(a)).unwrap();
    DiagramObj::push_node(&mut st, d, Value::Ref(b)).unwrap();
    NodeObj::push_child(&mut st, b, Value::Ref(c)).unwrap();
    let edge = |st: &mut Store, element: Obj, source: Obj, target: Obj| {
        let e = st.create(EDGE).unwrap();
        EdgeObj::set_element(st, e, Some(element)).unwrap();
        EdgeObj::set_source(st, e, Some(source)).unwrap();
        EdgeObj::set_target(st, e, Some(target)).unwrap();
        DiagramObj::push_edge(st, d, Value::Ref(e)).unwrap();
        e
    };
    let (eb, ec) = (edge(&mut st, m.b_gen, b, a), edge(&mut st, m.c_gen, c, a));
    assert_eq!(problems(&st), Vec::<String>::new());
    assert_eq!(edges_on(&st, b), vec![eb, ec], "the edge on the child counts for the parent");
    assert_eq!(views_of(&st, m.package), (vec![d], vec![], vec![]));

    // Removing the views of B takes its node with the child and both edges; the diagram and A stay.
    let before = picture(&st);
    st.begin();
    let gone = remove_views_of(&mut st, m.b).unwrap();
    let delta = st.end().unwrap();
    assert_eq!(gone, 4, "B's node, C's node under it, the two edges");
    assert!(st.is_alive(d) && st.is_alive(a) && !st.is_alive(b) && !st.is_alive(c) && !st.is_alive(eb) && !st.is_alive(ec));
    assert_eq!(problems(&st), Vec::<String>::new());
    st.apply(&Store::invert(&delta)).unwrap();
    assert_eq!(picture(&st), before);

    // Removing the views of the package takes the diagram with everything in it.
    st.begin();
    assert_eq!(remove_views_of(&mut st, m.package).unwrap(), 6);
    let delta = st.end().unwrap();
    assert!(only_toy(&st));
    st.apply(&Store::invert(&delta)).unwrap();
    assert_eq!(picture(&st), before);

    // A node alone cannot go while an edge needs it; with its edges it can.
    assert!(st.destroy(a).is_err());
    assert_eq!(destroy_node_with_edges(&mut st, a).unwrap(), 3);
    assert_eq!(problems(&st), Vec::<String>::new());
}

#[test]
fn the_grid_finds_the_topmost_node_the_nearest_edge_and_what_a_viewport_shows() {
    let (mut s, m) = session();
    let sm = sample(&mut s, &m);
    let (c_id, package) = (id(s.doc(), m.c), id(s.doc(), m.package));
    // C overlaps A and comes later, so it is on top; a port sits inside C.
    let c = run(&mut s, "diagram.place", json!({ "diagram": sm.diagram, "element": c_id, "x": 150, "y": 150, "width": 100, "height": 100 }));
    let port =
        run(&mut s, "diagram.note", json!({ "diagram": sm.diagram, "text": "in", "x": 240, "y": 200, "width": 20, "height": 20, "parent": c }));
    let _ = package;
    run(&mut s, "diagram.route", json!({ "edge": sm.edge, "points": [{ "x": 300, "y": 600 }] }));
    let st = s.doc();
    let d = obj(st, &sm.diagram);
    let grid = Grid::build(st, d);
    assert_eq!(grid.len(), (5, 2));
    assert_eq!(grid.hit(110.0, 110.0), Some(Hit::Node(obj(st, &sm.a))), "A alone");
    assert_eq!(grid.hit(200.0, 160.0), Some(Hit::Node(obj(st, &c))), "C over A");
    assert_eq!(grid.hit(250.0, 210.0), Some(Hit::Node(obj(st, &port))), "the port over C");
    assert_eq!(grid.hit(500.0, 150.0), Some(Hit::Node(obj(st, &sm.b))));
    // B's edge goes from B's centre (500, 160) through (300, 600) to A's centre (160, 140).
    assert_eq!(grid.hit(400.0, 380.0), Some(Hit::Edge(obj(st, &sm.edge))));
    assert_eq!(grid.hit(403.0, 380.0), Some(Hit::Edge(obj(st, &sm.edge))), "within the tolerance");
    assert_eq!(grid.hit(430.0, 380.0), None, "beyond it");
    assert_eq!(grid.hit(5000.0, 5000.0), None);
    assert_eq!(grid.hit(f64::NAN, 0.0), None);
    let seen = grid.visible(Rect::new(0.0, 0.0, 300.0, 290.0));
    assert_eq!(seen.nodes, vec![obj(st, &sm.a), obj(st, &c), obj(st, &port)], "in drawing order; B and the note are outside");
    assert_eq!(seen.edges.len(), 2, "both edges reach into the viewport");
    let seen = grid.visible(Rect::new(0.0, 0.0, 300.0, 300.0));
    assert_eq!(seen.nodes.len(), 4, "the note touches the viewport's edge");
    // The anchor runs from the note's centre (180, 340) to A's centre (160, 140).
    let seen = grid.visible(Rect::new(150.0, 300.0, 50.0, 100.0));
    assert_eq!(seen.nodes, vec![obj(st, &sm.note)]);
    assert!(seen.edges.contains(&obj(st, &sm.anchor)), "culling is by the edges' bounds, so the anchor is in; B's edge may be too");
    assert!(grid.visible(Rect::new(0.0, 300.0, 50.0, 100.0)).edges.is_empty(), "nothing reaches the far left");
    assert_eq!(grid.visible(Rect::new(f64::NAN, 0.0, 10.0, 10.0)), grid::Visible::default());
    assert_eq!(grid.visible(Rect::new(0.0, 0.0, -10.0, 10.0)), grid::Visible::default());
    // A viewport far wider than the cell budget still sees everything in it.
    let wide = grid.visible(Rect::new(-500_000.0, -500_000.0, 1_000_000.0, 1_000_000.0));
    assert_eq!(wide.nodes.len(), 5);
    assert_eq!(wide.edges.len(), 2);
    assert!(grid.visible(Rect::new(-9000.0, -9000.0, 10.0, 10.0)).nodes.is_empty());
    assert!(Grid::build(st, obj(st, &sm.a)).is_empty(), "a node is not a diagram");
    let out = s.execute("diagram.style", &json!({ "ids": [sm.a, sm.b] })).unwrap().value;
    assert_eq!((out["styled"].as_u64(), out["cleared"].as_u64()), (Some(0), Some(1)), "A's style went; B had none");
}

#[test]
fn the_rules_report_what_the_commands_never_make() {
    let mut st = store();
    let m = model(&mut st);
    let d = st.create(DIAGRAM).unwrap();
    DiagramObj::set_kind(&mut st, d, Some("Bad Kind")).unwrap();
    let n = st.create(NODE).unwrap();
    NodeObj::set_element(&mut st, n, Some(m.a)).unwrap();
    NodeObj::set_text(&mut st, n, Some("own text")).unwrap();
    NodeObj::set_width(&mut st, n, 50_000.0).unwrap();
    NodeObj::set_height(&mut st, n, 5.0).unwrap();
    NodeObj::set_x(&mut st, n, -2_000_000.0).unwrap();
    DiagramObj::push_node(&mut st, d, Value::Ref(n)).unwrap();
    let other = st.create(DIAGRAM).unwrap();
    DiagramObj::set_kind(&mut st, other, Some("toy.class")).unwrap();
    let far = st.create(NODE).unwrap();
    NodeObj::set_width(&mut st, far, 50.0).unwrap();
    NodeObj::set_height(&mut st, far, 50.0).unwrap();
    DiagramObj::push_node(&mut st, other, Value::Ref(far)).unwrap();
    let e = st.create(EDGE).unwrap();
    EdgeObj::set_source(&mut st, e, Some(n)).unwrap();
    EdgeObj::set_target(&mut st, e, Some(far)).unwrap();
    DiagramObj::push_edge(&mut st, d, Value::Ref(e)).unwrap();
    let l = st.create(LABEL).unwrap();
    LabelObj::set_kind(&mut st, l, Some("")).unwrap_or_default();
    EdgeObj::push_label(&mut st, e, Value::Ref(l)).unwrap();
    let s = st.create(STYLE).unwrap();
    StyleObj::set_fill(&mut st, s, Some("red")).unwrap();
    StyleObj::set_line(&mut st, s, 400.0).unwrap();
    NodeObj::set_style(&mut st, n, Some(s)).unwrap();
    let loose = st.create(NODE).unwrap();
    NodeObj::set_width(&mut st, loose, 50.0).unwrap();
    NodeObj::set_height(&mut st, loose, 50.0).unwrap();
    let found = problems(&st).join("\n");
    for expected in [
        "kind of",
        "shows an element and carries text",
        "width of",
        "height of",
        "x of",
        "is in another diagram",
        "fill of",
        "line width of",
        "belongs to no diagram",
    ] {
        assert!(found.contains(expected), "{expected:?} missing in:\n{found}");
    }
    assert!(st.validate_all().is_empty() || !found.is_empty());
    assert_eq!(diagram_of(&st, e), Some(d));
    assert_eq!(diagram_of(&st, loose), None);
    assert_eq!(rules::kind_problem("er"), None);
    assert!(rules::kind_problem(&"a".repeat(65)).is_some());
    assert_eq!(rules::colour_problem("#A1b2C3"), None);
    assert!(rules::colour_problem("#a1b2c3d").is_some() && rules::colour_problem("a1b2c3").is_some());
    assert!(rules::coord_problem(f64::INFINITY).is_some() && rules::size_problem(f64::NAN).is_some());
}

#[test]
fn the_sweep_finds_nothing() {
    let (mut s, m) = session();
    let sm = sample(&mut s, &m);
    let c_gen = id(s.doc(), m.c_gen);
    let ids = [&*sm.diagram, &*sm.a, &*sm.edge, &*sm.label, &*sm.note, &*sm.anchor, &*c_gen, "not-an-id", "00000000-0000-7000-8000-000000000000"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 1000, "{calls}");
    assert_eq!(problems(s.doc()), Vec::<String>::new());
    let _ = sm.b;
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

fn ids_of(s: &S, class: ClassId) -> Vec<String> {
    s.doc().alive().filter(|o| s.doc().class_of(*o).ok() == Some(class)).map(|o| s.doc().uuid(o).unwrap().to_string()).collect()
}

/// Whether every live object is the toy's: nothing of a diagram is left.
fn only_toy(st: &Store) -> bool {
    st.alive().all(|o| ![DIAGRAM, NODE, EDGE, POINT, LABEL, STYLE].contains(&st.class_of(o).unwrap()))
}

/// A random call against the store as it is.
fn random_call(rng: &mut Rng, s: &S) -> (&'static str, Json) {
    let diagrams = ids_of(s, DIAGRAM);
    let nodes = ids_of(s, NODE);
    let edges = ids_of(s, EDGE);
    let labels = ids_of(s, LABEL);
    let classes = ids_of(s, CLASS);
    let gens = ids_of(s, GENERALIZATION);
    let places = ["source", "middle", "target"];
    match rng.below(16) {
        0 if diagrams.len() < 2 => ("diagram.add", json!({ "kind": "toy.class", "name": format!("d{}", rng.next() % 9) })),
        0..=2 => ("diagram.place", json!({ "diagram": rng.pick(&diagrams), "element": rng.pick(&classes), "x": rng.coord(), "y": rng.coord() })),
        3 => (
            "diagram.note",
            json!({ "diagram": rng.pick(&diagrams), "text": "n", "x": rng.coord(), "y": rng.coord(), "parent": rng.pick(&nodes).cloned().unwrap_or_default() }),
        ),
        4 => ("diagram.move", json!({ "nodes": [rng.pick(&nodes).cloned().unwrap_or_default()], "dx": rng.coord(), "dy": rng.coord() })),
        5 => ("diagram.resize", json!({ "node": rng.pick(&nodes), "width": 20 + rng.below(500), "height": 20 + rng.below(500) })),
        6 => (
            "diagram.connect",
            json!({ "diagram": rng.pick(&diagrams), "element": rng.pick(&gens), "source": rng.pick(&nodes), "target": rng.pick(&nodes) }),
        ),
        7 => ("diagram.connect", json!({ "diagram": rng.pick(&diagrams), "source": rng.pick(&nodes), "target": rng.pick(&nodes) })),
        8 => ("diagram.connect-all", json!({ "diagram": rng.pick(&diagrams) })),
        9 => ("diagram.route", json!({ "edge": rng.pick(&edges), "points": [{ "x": rng.coord(), "y": rng.coord() }] })),
        10 => ("diagram.label", json!({ "edge": rng.pick(&edges), "kind": "general", "at": places[rng.below(3)] })),
        11 => ("diagram.unlabel", json!({ "label": rng.pick(&labels) })),
        12 => ("diagram.style", json!({ "ids": [rng.pick(&nodes).cloned().unwrap_or_default()], "fill": "#abcdef", "line": rng.below(5) })),
        13 => ("diagram.disconnect", json!({ "edge": rng.pick(&edges) })),
        14 => ("diagram.unplace", json!({ "nodes": [rng.pick(&nodes).cloned().unwrap_or_default()] })),
        _ if rng.below(10) == 0 => ("diagram.remove", json!({ "diagram": rng.pick(&diagrams) })),
        _ => ("diagram.style", json!({ "ids": [rng.pick(&edges).cloned().unwrap_or_default()] })),
    }
}

#[test]
fn random_sequences_keep_every_rule_and_undo_back_to_the_start() {
    for seed in 1..=20u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let (mut s, _) = session();
        let start = picture(s.doc());
        for i in 0..120 {
            let (cmd, p) = random_call(&mut rng, &s);
            let _ = s.execute(cmd, &p);
            assert_eq!(problems(s.doc()), Vec::<String>::new(), "seed {seed} step {i} after {cmd} {p}");
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
