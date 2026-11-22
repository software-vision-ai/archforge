//! A package's ownership and its appearances are separate (Decision 26): a move changes the owner
//! and the owning paths alone, keeping every appearance in every branch under its id, at its bounds,
//! with the routes beside it, and adds none; placing it on the new owner's diagram is a later step
//! of its own; each undone and redone exactly, the rows read back the same, an earlier version read
//! from its rows untouched, the package's removal taking every appearance in one step, and an
//! agent's commands giving the store the interface's give (AR-572).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use archforge_diagram::{ProjectView, Rect, diagram, edge, point};
use archforge_engine::{EngineError, Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::{ALONE, STRUCTURE_DIAGRAM, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// A session over a store holding only the project's Model, its new ids counted on from `n`.
fn session(n: u64) -> S {
    let mut st = store(n);
    st.create(archforge_core::MODEL).unwrap();
    over(st)
}

/// An empty store of the core, the diagrams and UML, its new ids counted on from `n`.
fn store(mut n: u64) -> Store {
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

/// A session over `st`.
fn over(st: Store) -> S {
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// Runs `cmd` and gives the id it answers with.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// The tables and junctions of the core, the diagrams and UML.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

/// `st` as rows.
fn rows(st: &Store) -> archforge_mof::rows::Rows {
    archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap()
}

/// `s`'s store as rows, to compare.
fn snapshot(s: &S) -> String {
    format!("{:?}", rows(s.doc()))
}

/// A store built from `rows`, as the host builds a saved or an earlier version; its new ids counted
/// far from those in use.
fn restored(from: &archforge_mof::rows::Rows) -> Store {
    let mut st = store(1 << 40);
    archforge_mof::rows::restore(&mut st, from, &tables().0, &tables().1).unwrap();
    st
}

/// The object under `id`.
fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// The package holding the package under `id`, by name.
fn owner(st: &Store, id: &str) -> String {
    archforge_uml::commands::package_of(st, obj(st, id)).map(|p| archforge_core::name_of(st, p)).unwrap_or_default()
}

/// The owners the card of the package `id` names on diagram `d`, from the root down.
fn path(st: &Store, id: &str, d: &str) -> Vec<String> {
    archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, id), obj(st, d)).expect("a package's card").owner_path
}

/// What a move must keep: each appearance of the package under `id` (its node's id, the diagram
/// holding it and its bounds, in id order, by id) and the waypoints of the edge under `edge`.
fn layout(st: &Store, id: &str, edge: &str) -> (Vec<(String, String, Rect)>, Vec<(Value, Value)>) {
    let mut shown: Vec<_> = archforge_diagram::views_of(st, obj(st, id))
        .2
        .into_iter()
        .map(|n| {
            (
                st.uuid(n).unwrap().to_string(),
                st.uuid(archforge_diagram::diagram_of(st, n).unwrap()).unwrap().to_string(),
                archforge_diagram::grid::bounds_of(st, n),
            )
        })
        .collect();
    shown.sort_by(|a, b| a.0.cmp(&b.0));
    let way = st.list(obj(st, edge), edge::WAYPOINT).unwrap().into_iter().filter_map(|w| w.as_ref());
    (shown, way.map(|w| (st.get(w, point::X).unwrap(), st.get(w, point::Y).unwrap())).collect())
}

/// The ids of what a scene is made of.
struct Scene {
    orders: String,
    billing: String,
    operations: String,
    sales_d: String,
    ops_d: String,
    billing_d: String,
    route: String,
}

/// Orders, held by Sales, shown on the diagrams of Sales and of Operations, two branches; its class
/// Order joined to Billing's Invoice by an association drawn on the Operations diagram along two
/// waypoints. Billing's diagram shows neither.
fn scene(s: &mut S) -> Scene {
    let sales = run(s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let billing = run(s, "uml.package.add", json!({ "name": "Billing" }));
    let operations = run(s, "uml.package.add", json!({ "name": "Operations" }));
    let order = run(s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    let invoice = run(s, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
    let association = run(s, "uml.association.add", json!({ "from": order, "to": invoice }));
    let mut diagram = |of: &str, name: &str| run(s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": name }));
    let (sales_d, ops_d, billing_d) = (diagram(&sales, "Sales"), diagram(&operations, "Operations"), diagram(&billing, "Billing"));
    run(s, "diagram.place", json!({ "diagram": sales_d, "element": orders, "x": 40, "y": 60 }));
    run(s, "diagram.place", json!({ "diagram": ops_d, "element": orders, "x": 400, "y": 20, "width": 260, "height": 180 }));
    let from = run(s, "diagram.place", json!({ "diagram": ops_d, "element": order, "x": 0, "y": 300 }));
    let to = run(s, "diagram.place", json!({ "diagram": ops_d, "element": invoice, "x": 500, "y": 300 }));
    let route = run(s, "diagram.connect", json!({ "diagram": ops_d, "element": association, "source": from, "target": to }));
    run(s, "diagram.route", json!({ "edge": route, "points": [{ "x": 250, "y": 280 }, { "x": 250, "y": 460 }] }));
    Scene { orders, billing, operations, sales_d, ops_d, billing_d, route }
}

/// Moved to Billing, whose diagram lacks it, Orders gains no appearance there; moved on to
/// Operations, whose diagram has it, that appearance stays the one it was. Through both, its
/// appearances in both branches keep their ids and bounds, the route beside them its waypoints,
/// and the paths follow the owner; each move one undo step undone and redone exactly.
#[test]
fn a_move_adds_no_appearance_and_keeps_every_one_in_every_branch() {
    let mut s = session(0);
    let c = scene(&mut s);
    let kept = layout(s.doc(), &c.orders, &c.route);
    let on: Vec<&str> = kept.0.iter().map(|(_, d, _)| d.as_str()).collect();
    assert_eq!(on.len(), 2);
    assert!(on.contains(&c.sales_d.as_str()) && on.contains(&c.ops_d.as_str()), "on the Sales and the Operations diagrams, not Billing's");
    let counts = |s: &S| [&c.sales_d, &c.ops_d, &c.billing_d].map(|d| s.doc().list(obj(s.doc(), d), diagram::NODE).unwrap().len());
    let held = counts(&s);
    for (to, d, words) in [(&c.billing, &c.billing_d, "Billing"), (&c.operations, &c.ops_d, "Operations")] {
        let (before, was) = (snapshot(&s), owner(s.doc(), &c.orders));
        run(&mut s, "uml.package.move", json!({ "package": c.orders, "to": to }));
        let after = snapshot(&s);
        assert_eq!(owner(s.doc(), &c.orders), words);
        assert_eq!(layout(s.doc(), &c.orders, &c.route), kept, "every appearance where it was, the route as it was");
        assert_eq!(counts(&s), held, "no node added on any diagram, the one of {words} included");
        assert_eq!(path(s.doc(), &c.orders, d), ["Model", words]);
        assert_eq!(path(s.doc(), &c.orders, &c.sales_d), ["Model", words], "the path follows in the other branch too");
        s.undo().unwrap();
        assert_eq!((snapshot(&s), owner(s.doc(), &c.orders)), (before, was), "undo of the move to {words}");
        s.redo().unwrap();
        assert_eq!(snapshot(&s), after, "redo of the move to {words}");
    }
}

/// Orders moved into Billing, then placed on Billing's diagram, each by `step`: the rows before,
/// after the move and after the placement.
fn move_then_place(s: &mut S, c: &Scene, step: impl Fn(&mut S, &str, Json)) -> [String; 3] {
    let start = snapshot(s);
    step(s, "uml.package.move", json!({ "package": c.orders, "to": c.billing }));
    let moved = snapshot(s);
    let p = json!({ "diagram": c.billing_d, "element": c.orders, "x": 300, "y": 40, "width": 220, "height": 160 });
    step(s, "diagram.place", p);
    [start, moved, snapshot(s)]
}

/// A move and a later placement on the new owner's diagram are two steps: the first Undo takes the
/// placement alone, the owner staying Billing, the second the move; Redo brings each back in turn,
/// exactly. The rows read back give the same paths, appearances, bounds and route.
#[test]
fn a_move_and_a_later_placement_are_two_steps_undone_and_redone_each_exactly() {
    let mut s = session(0);
    let c = scene(&mut s);
    let [start, moved, placed] = move_then_place(&mut s, &c, |s, cmd, p| {
        run(s, cmd, p);
    });
    let shown = layout(s.doc(), &c.orders, &c.route);
    assert_eq!(shown.0.len(), 3, "the placement added the one appearance");
    s.undo().unwrap();
    assert_eq!((snapshot(&s), owner(s.doc(), &c.orders)), (moved.clone(), "Billing".into()), "the placement undone alone");
    s.undo().unwrap();
    assert_eq!(snapshot(&s), start, "then the move");
    s.redo().unwrap();
    assert_eq!(snapshot(&s), moved);
    s.redo().unwrap();
    assert_eq!(snapshot(&s), placed);
    // Read back from its rows, as a save and a reopening do.
    let back = restored(&rows(s.doc()));
    assert_eq!(format!("{:?}", rows(&back)), placed, "rows read back");
    assert_eq!(layout(&back, &c.orders, &c.route), shown, "the same nodes, bounds and route");
    for d in [&c.sales_d, &c.ops_d, &c.billing_d] {
        assert_eq!(path(&back, &c.orders, d), ["Model", "Billing"]);
    }
}

/// An earlier version, built from its rows into a read-only session as the host shows one, keeps
/// the owner, the path and the appearances it had while the work goes on, refuses a move and a
/// placement of its own, and leaves the work as it was.
#[test]
fn an_earlier_version_stays_as_it_was_while_the_work_moves_and_places() {
    let mut s = session(0);
    let c = scene(&mut s);
    let then = rows(s.doc());
    let mut earlier = over(restored(&then));
    earlier.set_read_only(true).unwrap();
    let shown = layout(earlier.doc(), &c.orders, &c.route);
    move_then_place(&mut s, &c, |s, cmd, p| {
        run(s, cmd, p);
    });
    let now = snapshot(&s);
    assert_eq!(format!("{:?}", rows(earlier.doc())), format!("{then:?}"), "the version as it was");
    assert_eq!((owner(earlier.doc(), &c.orders), path(earlier.doc(), &c.orders, &c.ops_d)), ("Sales".into(), vec!["Model".into(), "Sales".into()]));
    assert_eq!(layout(earlier.doc(), &c.orders, &c.route), shown);
    let place = json!({ "diagram": c.billing_d, "element": c.orders, "x": 0, "y": 0 });
    for (cmd, p) in [("uml.package.move", json!({ "package": c.orders, "to": c.billing })), ("diagram.place", place)] {
        assert!(matches!(earlier.execute(cmd, &p), Err(EngineError::ReadOnly)), "{cmd} on a version");
    }
    assert_eq!(format!("{:?}", rows(earlier.doc())), format!("{then:?}"));
    assert_eq!(snapshot(&s), now, "the work as it was");
}

/// Shown on three diagrams after the move and the placement, Orders removed takes every appearance
/// along in one undo step, which brings each back under its id at its bounds.
#[test]
fn a_package_removed_takes_every_appearance_along_in_one_undo_step() {
    let mut s = session(0);
    let c = scene(&mut s);
    let [.., placed] = move_then_place(&mut s, &c, |s, cmd, p| {
        run(s, cmd, p);
    });
    let shown = layout(s.doc(), &c.orders, &c.route).0;
    assert_eq!(shown.len(), 3);
    run(&mut s, "element.remove", json!({ "ids": [c.orders] }));
    let gone = snapshot(&s);
    assert!(shown.iter().all(|(n, ..)| s.doc().by_uuid(Uuid::parse(n).unwrap()).is_none_or(|n| !s.doc().is_alive(n))), "every appearance goes");
    s.undo().unwrap();
    assert_eq!(snapshot(&s), placed, "one undo step brings all back");
    assert_eq!(layout(s.doc(), &c.orders, &c.route).0, shown);
    s.redo().unwrap();
    assert_eq!(snapshot(&s), gone);
}

/// The move and the placement an agent runs through the registry's commands, one by one, give the
/// store, step by step and undone, that the interface's labelled steps give: Properties' "Move
/// package" and the Models drop's "Place package".
#[test]
fn an_agents_move_and_placement_give_the_store_the_interfaces_give() {
    let mut ui = session(0);
    let c = scene(&mut ui);
    let by_ui = move_then_place(&mut ui, &c, |s, cmd, p| {
        let label = if cmd == "diagram.place" { "Place package" } else { "Move package" };
        s.batch(label, |b| b.execute(cmd, &p).map(|_| ())).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    });
    let mut agent = session(0);
    scene(&mut agent);
    let by_agent = move_then_place(&mut agent, &c, |s, cmd, p| {
        s.execute_from(cmd, &p, Some("Claude")).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    });
    assert_eq!(by_agent, by_ui);
    for at in [&by_ui[1], &by_ui[0]] {
        ui.undo().unwrap();
        agent.undo().unwrap();
        assert_eq!((&snapshot(&agent), &snapshot(&ui)), (at, at), "each one undo step");
    }
}
