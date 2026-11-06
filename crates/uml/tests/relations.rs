//! Relating classes through the engine: associations of each kind, ends set,
//! generalizations, dependencies and interface realizations drawn, wrong ends refused with nothing
//! changed, relationships reconnected with their drawn lines following, and removed with the
//! classes they touch; each one undo step; realizations saved as rows and read back; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Store, Uuid, Value};
use archforge_uml::{ALONE, registry};
use serde_json::{Value as Json, json};
type S = Session<Store, ProjectView>;

/// A store over the core, the diagrams and UML, its ids counted up from `start` so two stores never
/// share one.
fn store(start: u64) -> Store {
    let mut n: u64 = start;
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
    Store::new(universe, Box::new(slabs), ids)
}

/// A session over a store with the project's Model.
fn session() -> S {
    let mut st = store(0);
    st.create(archforge_core::MODEL).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}
/// Runs `cmd`, which must succeed, and gives the id it made or named.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}
/// Runs `cmd`, which must succeed as one undo step: undone the rows are as before, redone as after.
fn round_trip(s: &mut S, cmd: &str, p: Json) {
    let before = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    let after = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.undo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), before, "undo of {cmd}");
    s.redo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), after, "redo of {cmd}");
}
/// The tables and junctions of the core, the diagrams and UML, as a project stores them.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

/// The session's model as the rows it would save.
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

/// The relationships of the classifier `class`, as `uml.relationship.list` gives them.
fn relationships(s: &mut S, class: &str) -> Vec<Json> {
    s.execute("uml.relationship.list", &json!({ "class": class })).unwrap().value["relationships"].as_array().unwrap().clone()
}

/// The element with the id `id`.
fn obj(s: &S, id: &str) -> archforge_mof::Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// The marks at the start and the end of the line of the relationship `id`.
fn marks(s: &S, id: &str) -> [archforge_diagram::figure::EndMark; 2] {
    let line = archforge_uml::figures::line(s.doc(), obj(s, id)).unwrap();
    [line.ends[0].mark, line.ends[1].mark]
}

/// `cmd` is refused with a message naming `key`, and the model is as it was.
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
    assert_eq!(marks(&s, &kinds[0]), [M::None, M::None], "navigable to neither end: no arrows");
    assert_eq!(marks(&s, &kinds[1]), [M::None, M::OpenArrow], "directed to the second class");
    assert_eq!(marks(&s, &kinds[2]), [M::HollowDiamond, M::None], "the whole is the first class");
    assert_eq!(marks(&s, &kinds[3]), [M::FilledDiamond, M::None]);
    let plain = &relationships(&mut s, &c[0])[0];
    assert_eq!(plain["kind"], "association");
    for end in plain["ends"].as_array().unwrap() {
        assert_eq!((end["role"].as_str(), end["multiplicity"].as_str(), end["navigable"].as_bool()), (Some(""), Some(""), Some(false)));
    }
    // An arrow at every end it navigates to (owner decision), a diamond taking an arrow's place.
    let set = |a: &str, end: u8, nav: bool| json!({ "association": a, "end": end, "role": "", "multiplicity": "", "navigable": nav, "aggregation": if end == 1 && a == kinds[3] { "composite" } else { "none" } });
    for end in [0, 1] {
        run(&mut s, "uml.association.set-end", set(&kinds[0], end, true));
        // The composition's whole end, at Order, is never navigable; its part end is.
        run(&mut s, "uml.association.set-end", set(&kinds[3], end, end == 1));
    }
    assert_eq!(marks(&s, &kinds[0]), [M::OpenArrow, M::OpenArrow], "navigable both ways: an arrow at each end");
    assert_eq!(marks(&s, &kinds[3]), [M::FilledDiamond, M::OpenArrow], "the diamond at the whole, the arrow at the part");
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
    // The whole's end is never navigable (owner decision): asking for it is refused.
    refused(&mut s, "uml.association.set-end", set(0, end("order", "1", true, "none")), "navigable");
    // An end that starts to aggregate makes the other end, now the whole's, not navigable.
    let both = run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1] }));
    let set_both = |i: u8, e: Json| {
        let mut p = e;
        p["association"] = json!(both);
        p["end"] = json!(i);
        p
    };
    run(&mut s, "uml.association.set-end", set_both(0, end("", "1", true, "none")));
    run(&mut s, "uml.association.set-end", set_both(1, end("", "*", true, "none")));
    assert_eq!(marks(&s, &both), [M::OpenArrow, M::OpenArrow]);
    round_trip(&mut s, "uml.association.set-end", set_both(1, end("", "*", true, "shared")));
    assert_eq!(marks(&s, &both), [M::HollowDiamond, M::OpenArrow]);
    let row = relationships(&mut s, &c[0]).into_iter().find(|r| r["id"] == json!(both)).unwrap();
    assert_eq!((row["ends"][0]["navigable"].clone(), row["ends"][1]["navigable"].clone()), (json!(false), json!(true)));
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
fn an_end_takes_any_multiplicity_that_reads_drops_it_when_empty_and_refuses_the_rest_unchanged() {
    let mut s = session();
    let (_, _, c, _) = classes(&mut s, &["Order", "Payment"], 0);
    let a = run(&mut s, "uml.association.add", json!({ "from": c[0], "to": c[1] }));
    let set = |m: &str| json!({ "association": a, "end": 1, "role": "", "multiplicity": m, "navigable": false, "aggregation": "none" });
    let shown = |s: &mut S| relationships(s, &c[0])[0]["ends"][1]["multiplicity"].clone();
    assert_eq!(shown(&mut s), "", "a new association names no multiplicity");
    for (typed, written) in [("0", "0"), ("12", "12"), (" 2..5 ", "2..5"), ("2..*", "2..*"), ("0..*", "*"), ("*", "*"), ("0..1", "0..1")] {
        round_trip(&mut s, "uml.association.set-end", set(typed));
        assert_eq!(shown(&mut s), written, "{typed:?}");
    }
    let label = |s: &S| archforge_uml::figures::slot_text(s.doc(), obj(s, &a), "to_end");
    assert_eq!(label(&s), "0..1", "the line's end text follows the model");
    for bad in ["5..2", "-1", "1.5", "abc", "..3", "3..", "1...*", "99999999999"] {
        refused(&mut s, "uml.association.set-end", set(bad), "multiplicity");
    }
    // Empty takes the multiplicity away again, one undo step.
    round_trip(&mut s, "uml.association.set-end", set(""));
    assert_eq!(shown(&mut s), "");
    assert_eq!(label(&s), "");
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

/// A package with classes Order and Invoice and interfaces Payable and Refundable, all four on a
/// class diagram: the package, the diagram, the classifiers' ids and their nodes, in that order.
fn realizing(s: &mut S) -> (String, String, Vec<String>, Vec<String>) {
    let (pkg, d, mut ids, mut nodes) = classes(s, &["Order", "Invoice"], 2);
    for (i, name) in ["Payable", "Refundable"].into_iter().enumerate() {
        let interface = run(s, "uml.interface.add", json!({ "package": pkg, "name": name }));
        nodes.push(run(s, "diagram.place", json!({ "diagram": d, "element": interface, "x": 300 * i, "y": 400 })));
        ids.push(interface);
    }
    (pkg, d, ids, nodes)
}

/// Whether the element with the id `id` is gone from the model.
fn gone(s: &S, id: &str) -> bool {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).is_none_or(|o| !s.doc().is_alive(o))
}

/// A class realizes an interface in one undo step, owned by the class, and is drawn dashed in the
/// muted ink to a hollow triangle at the interface; several classes realize one interface; a
/// realization from anything but a class, to anything but an interface, or a second one from the
/// same class to the same interface is refused and changes nothing.
#[test]
fn a_class_realizes_an_interface_in_one_step_drawn_dashed_to_a_hollow_triangle_and_wrong_ends_are_refused() {
    use archforge_diagram::figure::EndMark as M;
    let mut s = session();
    let (_, _, c, _) = realizing(&mut s);
    let (order, invoice, payable, refundable) = (&c[0], &c[1], &c[2], &c[3]);
    round_trip(&mut s, "uml.realization.add", json!({ "class": order, "interface": payable }));
    let r = relationships(&mut s, order).into_iter().find(|r| r["kind"] == "realization").unwrap();
    assert_eq!((r["implementing"].as_str(), r["contract"].as_str()), (Some(order.as_str()), Some(payable.as_str())));
    let id = r["id"].as_str().unwrap().to_string();
    // Owned by its class, as UML has it, which makes the class its client and the interface its supplier.
    let st = s.doc();
    let (o, p, real) = (obj(&s, order), obj(&s, payable), obj(&s, &id));
    assert_eq!(st.container(real).unwrap().map(|(owner, _)| owner), Some(o));
    assert_eq!(archforge_uml::figures::ends(st, real), Some((o, p)));
    assert_eq!(st.list(real, archforge_uml::dependency::CLIENT).unwrap(), vec![Value::Ref(o)]);
    assert_eq!(st.list(real, archforge_uml::dependency::SUPPLIER).unwrap(), vec![Value::Ref(p)]);
    // Dashed in the muted ink, a hollow triangle at the interface: neither a generalization nor a dependency.
    let line = archforge_uml::figures::line(st, real).unwrap();
    assert_eq!(marks(&s, &id), [M::None, M::Triangle]);
    assert!(line.dashed && !line.styled, "{line:?}");
    assert_eq!(relationships(&mut s, payable).len(), 1, "the interface lists it too");
    // Several classes may realize one interface, and a class several interfaces.
    let second = run(&mut s, "uml.realization.add", json!({ "class": invoice, "interface": payable }));
    assert_eq!(s.view().selection, vec![obj(&s, &second)], "the new realization is selected");
    round_trip(&mut s, "uml.realization.add", json!({ "class": order, "interface": refundable }));
    assert_eq!(relationships(&mut s, payable).len(), 2);
    // What would make the model wrong is refused, saying why, and changes nothing.
    for (p, key, why) in [
        (json!({ "class": payable, "interface": refundable }), "class", "only a class implements an interface"),
        (json!({ "class": order, "interface": invoice }), "interface", "a class realizes only an interface"),
        (json!({ "class": order, "interface": payable }), "interface", "the class realizes that interface already"),
        (json!({ "class": order, "interface": order }), "interface", "only an interface"),
        (json!({ "class": "not-an-id", "interface": payable }), "class", "not an id"),
    ] {
        let before = snapshot(&s);
        let e = s.execute("uml.realization.add", &p).expect_err(&format!("{p} is refused")).to_string();
        assert!(e.contains(key) && e.contains(why), "{p}: {e}");
        assert_eq!(snapshot(&s), before, "{p} changed nothing");
    }
    // A realization has no name to rename.
    refused(&mut s, "uml.relationship.rename", json!({ "relationship": id, "name": "pays" }), "relationship");
}

/// A realization's ends reconnect with the rules kept, its line following, and it goes with
/// `element.remove` alone or with the class or interface at either end, each one undo step.
#[test]
fn a_realization_reconnects_with_its_rules_kept_and_goes_alone_or_with_either_end() {
    let mut s = session();
    let (_, d, c, nodes) = realizing(&mut s);
    let (order, invoice, payable, refundable) = (&c[0], &c[1], &c[2], &c[3]);
    let r = run(&mut s, "uml.realization.add", json!({ "class": order, "interface": payable }));
    let edge = run(&mut s, "diagram.connect", json!({ "diagram": d, "element": r, "source": nodes[0], "target": nodes[2] }));
    let end_of = |s: &S, prop| s.doc().get(obj(s, &edge), prop).unwrap().as_ref().unwrap();
    // The interface end moves to another interface, the line with it.
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "target", "class": refundable }));
    assert_eq!(end_of(&s, archforge_diagram::edge::TARGET), obj(&s, &nodes[3]));
    assert!(relationships(&mut s, payable).is_empty(), "Payable is no longer realized");
    assert_eq!(s.doc().list(obj(&s, &r), archforge_uml::dependency::SUPPLIER).unwrap(), vec![Value::Ref(obj(&s, refundable))]);
    // The class end moves to another class, which then owns it.
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "source", "class": invoice }));
    assert_eq!(end_of(&s, archforge_diagram::edge::SOURCE), obj(&s, &nodes[1]));
    assert_eq!(s.doc().container(obj(&s, &r)).unwrap().map(|(o, _)| o), Some(obj(&s, invoice)));
    assert_eq!(s.doc().list(obj(&s, &r), archforge_uml::dependency::CLIENT).unwrap(), vec![Value::Ref(obj(&s, invoice))]);
    assert!(relationships(&mut s, order).is_empty());
    // Its rules hold: a class's end only on a class, the interface's only on an interface, once.
    run(&mut s, "uml.realization.add", json!({ "class": order, "interface": refundable }));
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "source", "class": payable }), "only a class implements");
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "target", "class": order }), "only an interface");
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "source", "class": order }), "already");
    // Dropped back where it was, it stays as it is.
    run(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "target", "class": refundable }));
    // Removed alone it takes its line; the classifiers stay.
    let edges = |s: &S| s.doc().list(obj(s, &d), archforge_diagram::diagram::EDGE).unwrap().len();
    assert_eq!(edges(&s), 1);
    round_trip(&mut s, "element.remove", json!({ "ids": [r] }));
    assert!(gone(&s, &r) && edges(&s) == 0);
    s.undo().unwrap();
    // The interface removed takes every realization of it along; the class removed takes its own.
    let other = relationships(&mut s, order)[0]["id"].as_str().unwrap().to_string();
    round_trip(&mut s, "element.remove", json!({ "ids": [refundable] }));
    assert!(gone(&s, &r) && gone(&s, &other) && edges(&s) == 0);
    s.undo().unwrap();
    round_trip(&mut s, "element.remove", json!({ "ids": [invoice] }));
    assert!(gone(&s, &r) && !gone(&s, &other), "only Invoice's realization went");
    assert_eq!(edges(&s), 0);
}

/// A realization whose end that stays is wrong already, owned by something else than a class (a
/// project edited by hand), is refused when its other end moves, naming the realization's own end.
#[test]
fn a_realization_whose_staying_end_is_not_a_class_says_so_when_its_other_end_moves() {
    let mut st = store(0);
    st.create(archforge_core::MODEL).unwrap();
    let actor = st.create(archforge_uml::ACTOR).unwrap();
    let [payable, refundable] = [st.create(archforge_uml::INTERFACE).unwrap(), st.create(archforge_uml::INTERFACE).unwrap()];
    let r = st.create(archforge_uml::INTERFACE_REALIZATION).unwrap();
    st.set(r, archforge_uml::interface_realization::CONTRACT, Value::Ref(payable)).unwrap();
    st.push(actor, archforge_uml::behaviored_classifier::INTERFACE_REALIZATION, Value::Ref(r)).unwrap();
    let id = |o| st.uuid(o).unwrap().to_string();
    let (r, refundable, actor) = (id(r), id(refundable), id(actor));
    let mut s = Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE));
    let before = snapshot(&s);
    let e = s.execute("uml.relationship.reconnect", &json!({ "relationship": r, "end": "target", "class": refundable })).unwrap_err().to_string();
    let want = format!("relationship: the realization's existing implementing end {actor} is of kind Actor; it must be a Class");
    assert!(e.contains(&want), "{e}");
    assert_eq!(snapshot(&s), before, "nothing changed");
}

/// Realizations, with their lines, come back from their rows identical, as a saved project opens.
#[test]
fn realizations_come_back_from_their_rows_identical() {
    let mut s = session();
    let (_, d, c, nodes) = realizing(&mut s);
    let r = run(&mut s, "uml.realization.add", json!({ "class": c[0], "interface": c[2] }));
    run(&mut s, "uml.realization.add", json!({ "class": c[1], "interface": c[2] }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": r, "source": nodes[0], "target": nodes[2] }));
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(archforge_mof::rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let at = |id: &str| back.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    assert_eq!(archforge_uml::figures::ends(&back, at(&r)), Some((at(&c[0]), at(&c[2]))));
    assert_eq!(archforge_uml::relations::relationships_of(&back, at(&c[2])).len(), 2);
    assert_eq!(archforge_uml::figures::line(&back, at(&r)).map(|l| l.dashed), Some(true));
}

/// The command sweep, with a realization, its class and its interface among the ids, breaks nothing.
#[test]
fn the_sweep_over_realizations_breaks_nothing() {
    let mut s = session();
    let (pkg, d, c, nodes) = realizing(&mut s);
    let r = run(&mut s, "uml.realization.add", json!({ "class": c[0], "interface": c[2] }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": r, "source": nodes[0], "target": nodes[2] }));
    let ids = [pkg.as_str(), d.as_str(), c[0].as_str(), c[1].as_str(), c[2].as_str(), c[3].as_str(), r.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, |st| format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())).unwrap();
    assert!(calls > 100, "{calls}");
}

/// Whether the classifier `id` is final, as `uml.class.get` says it.
fn is_final(s: &mut S, id: &str) -> bool {
    s.execute("uml.class.get", &json!({ "class": id })).unwrap().value["final"].as_bool().unwrap()
}

/// A classifier of every kind is made final and cleared again, each one undo step, its card saying
/// `{final}` after its name while it is; it comes back from its rows final, as a saved project
/// opens; clearing changes nothing else; a built-in type is never made final (AR-137).
#[test]
fn a_classifier_of_every_kind_is_made_final_and_cleared_in_one_step_and_comes_back_final() {
    let mut s = session();
    let (pkg, d, c, _) = classes(&mut s, &["Pricing", "Order"], 0);
    let mut all = vec![c[0].clone()];
    for (cmd, name) in
        [("uml.interface.add", "Priced"), ("uml.enumeration.add", "Tier"), ("uml.data-type.add", "Money"), ("uml.primitive-type.add", "Sku")]
    {
        all.push(run(&mut s, cmd, json!({ "package": pkg, "name": name })));
    }
    all.push(run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Quote" })));
    run(&mut s, "uml.class.set", json!({ "class": c[0], "abstract": true }));
    for id in &all {
        let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": id, "x": 0, "y": 0 }));
        let card = |s: &S| archforge_uml::figures::card(s.doc(), obj(s, id), obj(s, &d)).unwrap();
        assert_eq!(card(&s).braces, None, "{id} is not final yet");
        let before = snapshot(&s);
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "final": true }));
        assert!(is_final(&mut s, id), "{id}");
        assert_eq!(card(&s).braces.as_deref(), Some("{final}"), "{id}: its card says so after its name");
        // A field left out keeps the value.
        run(&mut s, "uml.classifier.set", json!({ "classifier": id }));
        assert!(is_final(&mut s, id));
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "final": false }));
        assert!(!is_final(&mut s, id));
        assert_eq!(snapshot(&s), before, "{id}: cleared, every other value and every id is as it was");
        run(&mut s, "uml.classifier.set", json!({ "classifier": id, "final": true }));
        let _ = node;
    }
    // Final is its own setting: abstract and the rest stay.
    assert_eq!(s.execute("uml.class.get", &json!({ "class": c[0] })).unwrap().value["abstract"], true);
    // Saved and opened again, each is still final.
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(archforge_mof::rows::snapshot(&back, &tables, &junctions).unwrap(), saved);
    for id in &all {
        assert!(archforge_uml::relations::is_final(&back, back.by_uuid(Uuid::parse(id).unwrap()).unwrap()), "{id} reopened final");
    }
    // A built-in type is never made final; anything else than a classifier is no classifier.
    run(&mut s, "uml.attribute.add", json!({ "class": c[1], "name": "count", "type": "Integer" }));
    let integer = s.execute("uml.class.get", &json!({ "class": c[1] })).unwrap().value["attributes"][0]["type"]["id"].as_str().unwrap().to_string();
    refused(&mut s, "uml.classifier.set", json!({ "classifier": integer, "final": true }), "built-in type Integer");
    refused(&mut s, "uml.classifier.set", json!({ "classifier": pkg, "final": true }), "classifier");
}

/// Nothing specializes a final classifier: a new generalization to it is refused, and so is moving
/// a generalization's general end onto it; it is made final only while nothing specializes it, the
/// refusal naming what does; cleared, it may be specialized again. Every refusal changes nothing.
#[test]
fn nothing_specializes_a_final_classifier_and_one_with_subclasses_is_not_made_final() {
    let mut s = session();
    let (_, _, c, _) = classes(&mut s, &["Payment", "CardPayment", "WalletPayment", "PricingService", "DiscountService", "Service"], 0);
    let card = run(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }));
    run(&mut s, "uml.generalization.add", json!({ "specific": c[2], "general": c[0] }));
    // Payment has subclasses: refused, naming them, nothing written.
    let before = snapshot(&s);
    let e = s.execute("uml.classifier.set", &json!({ "classifier": c[0], "final": true })).unwrap_err().to_string();
    assert!(e.contains("final") && e.contains("CardPayment") && e.contains("WalletPayment"), "{e}");
    assert_eq!(snapshot(&s), before);
    assert!(!is_final(&mut s, &c[0]));
    // PricingService made final: a new generalization to it is refused, from any classifier.
    run(&mut s, "uml.classifier.set", json!({ "classifier": c[3], "final": true }));
    refused(&mut s, "uml.generalization.add", json!({ "specific": c[4], "general": c[3] }), "is final");
    let why = archforge_uml::relations::generalization_problem(s.doc(), obj(&s, &c[4]), obj(&s, &c[3]), None);
    assert_eq!(why, Some(archforge_uml::relations::GeneralizationProblem::Final));
    // A final classifier itself specializes freely.
    round_trip(&mut s, "uml.generalization.add", json!({ "specific": c[3], "general": c[5] }));
    // Moving a generalization's general end onto it is refused; its specific end moves elsewhere.
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": card, "end": "target", "class": c[3] }), "is final");
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": card, "end": "source", "class": c[4] }));
    // Cleared, it takes the generalization; its id and name stay.
    round_trip(&mut s, "uml.classifier.set", json!({ "classifier": c[3], "final": false }));
    run(&mut s, "uml.relationship.reconnect", json!({ "relationship": card, "end": "target", "class": c[3] }));
    let moved = relationships(&mut s, &c[3]).into_iter().find(|r| r["id"] == card.as_str()).unwrap();
    assert_eq!(moved["general"].as_str(), Some(c[3].as_str()));
    assert_eq!(s.execute("uml.class.get", &json!({ "class": c[3] })).unwrap().value["name"], "PricingService");
    // Its subclasses listed in the order they were made, each with its generalization.
    let below = archforge_uml::relations::specializations(s.doc(), obj(&s, &c[0]));
    assert_eq!(below.iter().map(|(_, sub)| archforge_core::name_of(s.doc(), *sub)).collect::<Vec<_>>(), ["WalletPayment"]);
}

/// Whether the classifier `id` is leaf, as `uml.class.get` says it.
fn is_leaf(s: &mut S, id: &str) -> bool {
    s.execute("uml.class.get", &json!({ "class": id })).unwrap().value["leaf"].as_bool().unwrap()
}

/// A classifier of every kind is made leaf and cleared again (AR-138), each one undo step, apart
/// from final: its card says `{leaf}` after its name, `{final, leaf}` with both; a field left out
/// keeps its value; it comes back from its rows leaf; a built-in type is never made leaf.
#[test]
fn a_classifier_of_every_kind_is_made_leaf_apart_from_final_and_comes_back_leaf() {
    let mut s = session();
    let (pkg, d, c, _) = classes(&mut s, &["Pricing", "Order"], 0);
    let mut all = vec![c[0].clone()];
    for (cmd, name) in
        [("uml.interface.add", "Priced"), ("uml.enumeration.add", "Tier"), ("uml.data-type.add", "Money"), ("uml.primitive-type.add", "Sku")]
    {
        all.push(run(&mut s, cmd, json!({ "package": pkg, "name": name })));
    }
    for id in &all {
        run(&mut s, "diagram.place", json!({ "diagram": d, "element": id, "x": 0, "y": 0 }));
        let card = |s: &S| archforge_uml::figures::card(s.doc(), obj(s, id), obj(s, &d)).unwrap();
        let before = snapshot(&s);
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "leaf": true }));
        assert!(is_leaf(&mut s, id), "{id}");
        assert!(!is_final(&mut s, id), "{id}: leaf is not final");
        assert_eq!(card(&s).braces.as_deref(), Some("{leaf}"), "{id}");
        // Final beside it: one pair of braces, final first; each left out keeps its value.
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "final": true }));
        assert!(is_leaf(&mut s, id));
        assert_eq!(card(&s).braces.as_deref(), Some("{final, leaf}"), "{id}");
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "leaf": false }));
        assert_eq!(card(&s).braces.as_deref(), Some("{final}"), "{id}");
        // Both in one call, one step; cleared, the model is as it was.
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "final": false, "leaf": true }));
        assert_eq!(card(&s).braces.as_deref(), Some("{leaf}"), "{id}");
        round_trip(&mut s, "uml.classifier.set", json!({ "classifier": id, "leaf": false }));
        assert_eq!(snapshot(&s), before, "{id}: cleared, every value and id as it was");
        run(&mut s, "uml.classifier.set", json!({ "classifier": id, "leaf": true }));
    }
    // A leaf classifier is specialized freely: leaf stops redefinition, not specialization.
    round_trip(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }));
    // Saved and opened again, each is still leaf.
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    for id in &all {
        assert!(archforge_uml::relations::is_leaf(&back, back.by_uuid(Uuid::parse(id).unwrap()).unwrap()), "{id} reopened leaf");
    }
    // A built-in type is never made leaf.
    run(&mut s, "uml.attribute.add", json!({ "class": c[1], "name": "count", "type": "Integer" }));
    let integer = s.execute("uml.class.get", &json!({ "class": c[1] })).unwrap().value["attributes"][0]["type"]["id"].as_str().unwrap().to_string();
    refused(&mut s, "uml.classifier.set", json!({ "classifier": integer, "leaf": true }), "built-in type Integer");
}

/// An operation is made leaf and cleared again (AR-138), one undo step each, its row ending in
/// `{leaf}`; added leaf in one step; `uml.class.get` says it. An operation or a classifier something
/// redefines already, as a model read from elsewhere may hold, is not made leaf: the refusal names
/// what redefines it and nothing changes; one that is not redefined is.
#[test]
fn an_operation_is_made_leaf_and_one_redefined_is_refused_naming_what_redefines_it() {
    let mut s = session();
    let (_, d, c, _) = classes(&mut s, &["Payment", "CardPayment"], 2);
    run(&mut s, "uml.generalization.add", json!({ "specific": c[1], "general": c[0] }));
    let authorise = run(&mut s, "uml.operation.add", json!({ "class": c[0], "name": "authorise", "returns": "Boolean" }));
    let card_authorise = run(&mut s, "uml.operation.add", json!({ "class": c[1], "name": "authorise", "returns": "Boolean" }));
    let total = {
        let before = snapshot(&s);
        let id = run(&mut s, "uml.operation.add", json!({ "class": c[0], "name": "total", "returns": "Real", "leaf": true }));
        s.undo().unwrap();
        assert_eq!(snapshot(&s), before, "added leaf in one step");
        s.redo().unwrap();
        id
    };
    let text = |s: &S, id: &str| archforge_uml::figures::operation_text(s.doc(), obj(s, id));
    assert_eq!(text(&s, &total), "+ total() : Real {leaf}");
    let ops = s.execute("uml.class.get", &json!({ "class": c[0] })).unwrap().value["operations"].clone();
    assert_eq!((ops[0]["leaf"].as_bool(), ops[1]["leaf"].as_bool()), (Some(false), Some(true)));
    // The card's row says it on every diagram that shows the class.
    let card = archforge_uml::figures::card(s.doc(), obj(&s, &c[0]), obj(&s, &d)).unwrap();
    assert!(card.compartments.iter().flatten().any(|r| r.text == "+ total() : Real {leaf}"));
    // Set and cleared, one step each, its other values kept as sent.
    let set = |leaf: bool| json!({ "operation": authorise, "name": "authorise", "returns": "Boolean", "abstract": true, "leaf": leaf });
    round_trip(&mut s, "uml.operation.set", set(true));
    assert_eq!(text(&s, &authorise), "+ authorise() : Boolean {leaf}");
    round_trip(&mut s, "uml.operation.set", set(false));
    assert_eq!(text(&s, &authorise), "+ authorise() : Boolean");
    // A model where CardPayment's authorise redefines Payment's, and CardPayment redefines Pricing,
    // as one read from elsewhere may hold: neither may be made leaf.
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    let at = |st: &Store, id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    let (base, over) = (at(&back, &authorise), at(&back, &card_authorise));
    back.push(over, archforge_uml::operation::REDEFINED_OPERATION, Value::Ref(base)).unwrap();
    let (payment, card_payment) = (at(&back, &c[0]), at(&back, &c[1]));
    back.push(card_payment, archforge_uml::classifier::REDEFINED_CLASSIFIER, Value::Ref(payment)).unwrap();
    let mut s = Session::with_view(registry().unwrap(), back, 1, Options::default(), ProjectView::new(ALONE));
    assert_eq!(archforge_uml::relations::redefiners(s.doc(), obj(&s, &authorise)), [obj(&s, &card_authorise)]);
    assert_eq!(archforge_uml::relations::redefiner_name(s.doc(), obj(&s, &card_authorise)), "CardPayment::authorise()");
    let before = snapshot(&s);
    let e = s.execute("uml.operation.set", &set(true)).unwrap_err().to_string();
    assert!(e.contains("leaf") && e.contains("CardPayment::authorise()") && e.contains(&card_authorise), "{e}");
    let e = s.execute("uml.classifier.set", &json!({ "classifier": c[0], "leaf": true })).unwrap_err().to_string();
    assert!(e.contains("leaf") && e.contains("CardPayment"), "{e}");
    assert_eq!(snapshot(&s), before, "nothing changed");
    // The one nothing redefines is made leaf.
    round_trip(&mut s, "uml.operation.set", json!({ "operation": card_authorise, "name": "authorise", "returns": "Boolean", "leaf": true }));
    round_trip(&mut s, "uml.classifier.set", json!({ "classifier": c[1], "leaf": true }));
}
