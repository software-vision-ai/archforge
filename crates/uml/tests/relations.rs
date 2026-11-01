//! Relating classes through the engine (archforge-3c5.1.4): associations of each kind, ends set,
//! generalizations and dependencies drawn, wrong ends refused with nothing changed, relationships
//! reconnected with their drawn lines following, and removed with the classes they touch; each one
//! undo step; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Store, Uuid, Value};
use archforge_uml::{ALONE, registry};
use serde_json::{Value as Json, json};
type S = Session<Store, ProjectView>;
fn session() -> S {
    let mut n: u64 = 0;
    let ids = Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    });
    let universe = archforge_diagram::universe_with(vec![archforge_core::metamodel().unwrap(), archforge_uml::metamodel().unwrap()]).unwrap();
    let slabs = archforge_diagram::with_slabs(
        CompositeSlabs::new()
            .with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))
            .unwrap()
            .with(archforge_uml::INDEX, Box::new(archforge_uml::UmlSlabs::default()))
            .unwrap(),
    )
    .unwrap();
    let mut st = Store::new(universe, Box::new(slabs), ids);
    st.create(archforge_core::MODEL).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}
fn round_trip(s: &mut S, cmd: &str, p: Json) {
    let before = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    let after = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.undo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), before, "undo of {cmd}");
    s.redo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), after, "redo of {cmd}");
}
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

fn snapshot(s: &S) -> String {
    format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap())
}

/// A package with classes `names`, on a class diagram that shows the first `shown` of them.
fn classes(s: &mut S, names: &[&str], shown: usize) -> (String, String, Vec<String>, Vec<String>) {
    let pkg = run(s, "uml.package.add", json!({ "name": "sales" }));
    let d = run(s, "diagram.add", json!({ "kind": "uml.class", "element": pkg, "name": "sales" }));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let c = run(s, "uml.class.add", json!({ "package": pkg, "name": n }));
        if i < shown {
            nodes.push(run(s, "diagram.place", json!({ "diagram": d, "element": c, "x": 300 * i, "y": 0 })));
        }
        ids.push(c);
    }
    (pkg, d, ids, nodes)
}

fn relationships(s: &mut S, class: &str) -> Vec<Json> {
    s.execute("uml.relationship.list", &json!({ "class": class })).unwrap().value["relationships"].as_array().unwrap().clone()
}

fn obj(s: &S, id: &str) -> archforge_mof::Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn marks(s: &S, id: &str) -> [archforge_diagram::figure::EndMark; 2] {
    let line = archforge_uml::figures::line(s.doc(), obj(s, id)).unwrap();
    [line.ends[0].mark, line.ends[1].mark]
}

fn refused(s: &mut S, cmd: &str, p: Json, key: &str) {
    let before = snapshot(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} is refused"));
    assert!(e.to_string().contains(key), "{cmd} {p}: {e}");
    assert_eq!(snapshot(s), before, "{cmd} {p} changed nothing");
}

#[test]
fn each_kind_of_association_draws_as_a_plain_line_its_kind_marks_and_no_class_gains_an_attribute() {
    use archforge_diagram::figure::EndMark as M;
    let mut s = session();
    let (_, _, c, _) = classes(&mut s, &["Order", "Line"], 0);
    let mut kinds = Vec::new();
    for kind in ["association", "directed", "aggregation", "composition"] {
        round_trip(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1], "kind": kind }));
        kinds.push(relationships(&mut s, &c[0]).last().unwrap()["id"].as_str().unwrap().to_string());
    }
    assert_eq!(marks(&s, &kinds[0]), [M::None, M::None], "navigable both ways: no arrows");
    assert_eq!(marks(&s, &kinds[1]), [M::None, M::OpenArrow], "directed to the second class");
    assert_eq!(marks(&s, &kinds[2]), [M::HollowDiamond, M::None], "the whole is the first class");
    assert_eq!(marks(&s, &kinds[3]), [M::FilledDiamond, M::None]);
    let plain = &relationships(&mut s, &c[0])[0];
    assert_eq!(plain["kind"], "association");
    for end in plain["ends"].as_array().unwrap() {
        assert_eq!((end["role"].as_str(), end["multiplicity"].as_str(), end["navigable"].as_bool()), (Some(""), Some(""), Some(true)));
    }
    // The ends are the association's: the classes' attributes stay as they were.
    for class in &c {
        assert!(s.execute("uml.class.get", &json!({ "class": class })).unwrap().value["attributes"].as_array().unwrap().is_empty());
    }
    // A class related to itself is fine; a kind that is none is refused.
    run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[0] }));
    refused(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1], "kind": "friendship" }), "kind");
}

#[test]
fn an_association_end_takes_a_role_multiplicity_navigability_and_aggregation_and_wrong_ends_are_refused() {
    let mut s = session();
    let (_, _, c, _) = classes(&mut s, &["Order", "Line"], 0);
    let a = run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1] }));
    let end = |role: &str, m: &str, nav: bool, agg: &str| json!({ "role": role, "multiplicity": m, "navigable": nav, "aggregation": agg });
    let set = |i: u8, e: Json| {
        let mut p = e;
        p["association"] = json!(a);
        p["end"] = json!(i);
        p
    };
    round_trip(&mut s, "uml.association.set-end", set(1, end("lines", "1..*", true, "composite")));
    round_trip(&mut s, "uml.association.set-end", set(0, end("order", "1", false, "none")));
    let all = relationships(&mut s, &c[0]);
    let r = &all[0]["ends"];
    assert_eq!(r[1]["role"], "lines");
    assert_eq!(r[1]["multiplicity"], "1..*");
    assert_eq!(r[1]["aggregation"], "composite");
    assert_eq!(r[0]["navigable"], false);
    use archforge_diagram::figure::EndMark as M;
    assert_eq!(r[0]["role"], "order", "the ends keep their order");
    assert_eq!(marks(&s, &a), [M::FilledDiamond, M::OpenArrow], "the diamond at the whole, the arrow at the part it navigates to");
    // The same role on both ends, both ends aggregating, a whole that allows many: refused.
    refused(&mut s, "uml.association.set-end", set(0, end("Lines", "1", true, "none")), "role");
    refused(&mut s, "uml.association.set-end", set(0, end("order", "1", true, "shared")), "aggregation");
    refused(&mut s, "uml.association.set-end", set(0, end("order", "*", true, "none")), "multiplicity");
    refused(&mut s, "uml.association.set-end", set(0, end("order", "1..2", true, "none")), "multiplicity");
    refused(&mut s, "uml.association.set-end", set(1, end("lines", "two", true, "composite")), "multiplicity");
    refused(&mut s, "uml.association.set-end", set(2, end("x", "", true, "none")), "end");
    // The whole's end allowing many first, then composite on the part, is refused too.
    run(&mut s, "uml.association.set-end", set(1, end("lines", "1..*", true, "none")));
    run(&mut s, "uml.association.set-end", set(0, end("order", "*", true, "none")));
    refused(&mut s, "uml.association.set-end", set(1, end("lines", "1..*", true, "composite")), "aggregation");
    run(&mut s, "uml.association.set-end", set(1, end("lines", "1..*", true, "shared")));
    // Renamed, and renamed to nothing.
    round_trip(&mut s, "uml.relationship.rename", json!({ "relationship": a, "name": "contains" }));
    assert_eq!(relationships(&mut s, &c[0])[0]["name"], "contains");
    run(&mut s, "uml.relationship.rename", json!({ "relationship": a, "name": "" }));
}

#[test]
fn generalizations_and_dependencies_are_drawn_and_a_cycle_a_repeat_or_a_class_on_itself_is_refused() {
    let mut s = session();
    let (_, _, c, _) = classes(&mut s, &["Payment", "Card", "Gift", "Pricing"], 0);
    round_trip(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }));
    run(&mut s, "uml.generalization.add", json!({ "specific": c[2], "general": c[1] }));
    refused(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[1] }), "general");
    refused(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }), "general");
    refused(&mut s, "uml.generalization.add", json!({ "specific": c[0], "general": c[2] }), "general");
    let g = relationships(&mut s, &c[0]).into_iter().find(|r| r["kind"] == "generalization").unwrap();
    assert_eq!((g["specific"].as_str(), g["general"].as_str()), (Some(c[1].as_str()), Some(c[0].as_str())));
    round_trip(&mut s, "uml.dependency.add", json!({ "client": c[0], "supplier": c[3], "name": "uses" }));
    refused(&mut s, "uml.dependency.add", json!({ "client": c[3], "supplier": c[3] }), "supplier");
    let d = relationships(&mut s, &c[3]).into_iter().find(|r| r["kind"] == "dependency").unwrap();
    assert_eq!((d["client"].as_str(), d["name"].as_str()), (Some(c[0].as_str()), Some("uses")));
}

#[test]
fn a_reconnected_relationship_moves_its_line_on_a_diagram_that_shows_the_new_class_and_leaves_one_that_does_not() {
    let mut s = session();
    let (_, d, c, nodes) = classes(&mut s, &["Order", "Line", "Product", "Hidden"], 3);
    let a = run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1] }));
    let edge = run(&mut s, "diagram.connect", json!({ "diagram": d, "element": a, "source": nodes[0], "target": nodes[1] }));
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": a, "end": "target", "class": c[2] }));
    let target = s.doc().get(obj(&s, &edge), archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap();
    assert_eq!(target, obj(&s, &nodes[2]), "the line follows to Product's node");
    assert!(relationships(&mut s, &c[1]).is_empty(), "Line is no longer related");
    // To a class the diagram does not show: the line leaves this diagram, the association stays.
    run(&mut s, "uml.relationship.reconnect", json!({ "relationship": a, "end": "source", "class": c[3] }));
    assert!(s.doc().by_uuid(Uuid::parse(&edge).unwrap()).is_none_or(|e| !s.doc().is_alive(e)), "the line left the diagram");
    assert_eq!(relationships(&mut s, &c[3]).len(), 1);
    // A generalization's specific end moves to another class; one that would cycle is refused.
    let g = run(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }));
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "source", "class": c[2] }));
    let moved = relationships(&mut s, &c[2]).into_iter().find(|r| r["kind"] == "generalization").unwrap();
    assert_eq!(moved["specific"].as_str(), Some(c[2].as_str()));
    run(&mut s, "uml.generalization.add", json!({ "specific": c[0], "general": c[1] }));
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "target", "class": c[2] }), "class");
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "source", "class": c[1] }), "class");
    // A dependency's supplier moves; onto its own client it is refused.
    let dep = run(&mut s, "uml.dependency.add", json!({ "client": c[0], "supplier": c[1] }));
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": dep, "end": "target", "class": c[2] }));
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": dep, "end": "target", "class": c[0] }), "class");
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": dep, "end": "middle", "class": c[1] }), "end");
}

#[test]
fn removing_a_class_takes_its_relationships_and_their_lines_and_a_relationship_goes_alone() {
    let mut s = session();
    let (pkg, d, c, nodes) = classes(&mut s, &["Order", "Line", "Payment", "Card"], 4);
    let a = run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1], "kind": "composition" }));
    let g = run(&mut s, "uml.generalization.add", json!({ "specific": c[3], "general": c[2] }));
    let to_order = run(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }));
    let dep = run(&mut s, "uml.dependency.add", json!({ "client": c[2], "supplier": c[0] }));
    for (el, from, to) in [(&a, 0, 1), (&g, 3, 2), (&to_order, 1, 0), (&dep, 2, 0)] {
        run(&mut s, "diagram.connect", json!({ "diagram": d, "element": el, "source": nodes[from], "target": nodes[to] }));
    }
    let edges = |s: &S| s.doc().list(obj(s, &d), archforge_diagram::diagram::EDGE).unwrap().len();
    assert_eq!(edges(&s), 4);
    // A relationship removed alone takes its line; the classes stay.
    round_trip(&mut s, "element.remove", json!({ "ids": [g] }));
    assert_eq!(edges(&s), 3);
    // Order goes with the association, the generalization to it and the dependency on it.
    round_trip(&mut s, "element.remove", json!({ "ids": [c[0]] }));
    assert_eq!(edges(&s), 0);
    for r in [&a, &to_order, &dep] {
        assert!(s.doc().by_uuid(Uuid::parse(r).unwrap()).is_none_or(|o| !s.doc().is_alive(o)), "{r} went");
    }
    assert!(relationships(&mut s, &c[1]).is_empty() && relationships(&mut s, &c[2]).is_empty());
    s.undo().unwrap();
    assert_eq!(edges(&s), 3, "undo brings them back with their lines");
    // An association's end alone is refused: it goes with its association.
    let end = s.execute("uml.relationship.list", &json!({ "class": c[0] })).unwrap().value["relationships"][0]["ends"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    refused(&mut s, "element.remove", json!({ "ids": [end] }), "ids");
    let _ = pkg;
}

#[test]
fn the_sweep_finds_no_command_that_breaks_the_store_with_classes_related() {
    let mut s = session();
    let (pkg, d, c, _) = classes(&mut s, &["Order", "Line"], 2);
    let a = run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1] }));
    let calls = sweep_with(&mut s, &[pkg.as_str(), d.as_str(), c[0].as_str(), c[1].as_str(), a.as_str(), "not-an-id"], |st| {
        format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())
    })
    .unwrap();
    assert!(calls > 100, "{calls}");
    let _ = Value::Null;
}
