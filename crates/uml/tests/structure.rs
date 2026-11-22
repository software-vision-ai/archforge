//! A fresh project's UML structure diagram holding packages and classifiers side by side (AR-394,
//! AR-556): each box shows its element by the element's own id, and placing owns nothing, changes no
//! element and leaves the diagram's kind and notation as they were; a dependency between a class
//! and a package is dashed with an open arrow at its supplier, whichever end is the package; every
//! step is undone and redone exactly; and the agent's commands, `diagram.add` among them, leave the
//! store the Package and classifier tools' undo steps leave.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use archforge_diagram::figure::{EndMark, Text, Tone};
use archforge_diagram::{Notation, ProjectView, diagram, edge, node};
use archforge_engine::{Options, Session};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid};
use archforge_uml::{ALONE, CLASS_SIZE, PACKAGE_SIZE, STRUCTURE_DIAGRAM, UmlNotation, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// A fresh project: a store of the core, the diagrams and UML holding only its Model, its new ids
/// counted on from 1, so two sessions given the same commands mint the same ids.
fn session() -> S {
    let mut n = 0u64;
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

/// Runs `cmd` and gives the id it made.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// The rows of the whole project, the model and its diagrams, as a save writes them, beside the
/// tables and junctions they fill, in the same order.
fn saved(s: &S) -> (archforge_mof::rows::Rows, Vec<Table>, Vec<Junction>) {
    let tables: Vec<Table> = archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect();
    let junctions: Vec<Junction> =
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect();
    (archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap(), tables, junctions)
}

/// The rows of the whole project.
fn snapshot(s: &S) -> String {
    format!("{:?}", saved(s).0)
}

/// The rows of the model alone, those of the diagrams' own tables and junctions left out, a
/// package's default-diagram binding kept, as a comparison reads the model: what placing must not
/// change.
fn model(s: &S) -> String {
    let (rows, tables, junctions) = saved(s);
    let of_model =
        |class: archforge_mof::ClassId| class.metamodel() != archforge_diagram::INDEX || class == archforge_diagram::DEFAULT_DIAGRAM_BINDING;
    let kept: Vec<_> = rows.tables.iter().zip(&tables).filter(|(_, t)| of_model(t.class)).map(|(r, _)| r).collect();
    let lists: Vec<_> = rows.junctions.iter().zip(&junctions).filter(|(_, j)| of_model(j.class)).map(|(r, _)| r).collect();
    format!("{kept:?} {lists:?}")
}

/// The object of the id `id`.
fn obj(s: &S, id: &str) -> Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// The id of what `prop` of `o` refers to.
fn referred(s: &S, o: Obj, prop: archforge_mof::PropId) -> String {
    let to = s.doc().get(o, prop).unwrap().as_ref().unwrap();
    s.doc().uuid(to).unwrap().to_string()
}

/// A package Sales holding a child package Orders, a class Order, an interface Payable and an
/// enumeration Status, and Sales's structure diagram, nothing on it yet.
struct Fresh {
    s: S,
    sales: String,
    orders: String,
    order: String,
    payable: String,
    status: String,
    d: String,
}

/// A fresh project holding [`Fresh`]'s packages, classifiers and diagram.
fn fresh() -> Fresh {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "Status" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    Fresh { s, sales, orders, order, payable, status, d }
}

/// A child package and three kinds of classifier placed on one structure diagram: each box is on it,
/// with no other, and names its element by that element's own id, no element is copied, moved or changed, the
/// diagram stays a UML structure diagram of Sales, each package's default diagram its own (Orders
/// has one already), and each card is drawn in its own kind's notation.
#[test]
fn a_child_package_and_classifiers_placed_together_keep_their_ids_owners_and_notation() {
    let Fresh { mut s, sales, orders, order, payable, status, d } = fresh();
    let elements = [&orders, &order, &payable, &status];
    let owner = |s: &S, id: &str| s.doc().container(obj(s, id)).unwrap().map(|(up, prop)| (s.doc().uuid(up).unwrap(), prop));
    let owners: Vec<_> = elements.iter().map(|e| owner(&s, e)).collect();
    // Orders has a structure diagram of its own already, its default.
    let own = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": orders, "name": "Orders" }));
    let before = model(&s);
    let nodes: Vec<String> =
        elements.iter().zip(0..).map(|(e, i)| run(&mut s, "diagram.place", json!({ "diagram": d, "element": e, "x": 300 * i, "y": 0 }))).collect();
    for (n, e) in nodes.iter().zip(elements) {
        assert_eq!(&referred(&s, obj(&s, n), node::ELEMENT), e, "the box shows the element itself");
        assert_eq!(archforge_diagram::diagram_of(s.doc(), obj(&s, n)), Some(obj(&s, &d)), "on Sales's diagram");
    }
    let listed: Vec<String> =
        s.doc().list(obj(&s, &d), diagram::NODE).unwrap().iter().map(|n| s.doc().uuid(n.as_ref().unwrap()).unwrap().to_string()).collect();
    assert_eq!(listed, nodes, "the four boxes and no other");
    assert_eq!(model(&s), before, "placing copies, moves and changes no element and no binding");
    let bound = |s: &S, p: &str, d: &str| {
        archforge_uml::defaults::resolve(s.doc(), obj(s, p)) == Ok(archforge_uml::defaults::DefaultDiagram::Bound(obj(s, d)))
    };
    assert!(bound(&s, &sales, &d) && bound(&s, &orders, &own), "each package's default its own diagram still");
    assert_eq!(elements.iter().map(|e| owner(&s, e)).collect::<Vec<_>>(), owners, "each keeps its owner");
    assert_eq!(owner(&s, &orders).map(|(up, _)| up.to_string()), Some(sales.clone()));
    let dg = obj(&s, &d);
    assert_eq!(s.doc().get(dg, diagram::KIND).unwrap().as_str(), Some(STRUCTURE_DIAGRAM), "still a structure diagram");
    assert_eq!(referred(&s, dg, diagram::ELEMENT), sales, "of Sales still");
    assert_eq!(s.view().of_kind(STRUCTURE_DIAGRAM).map(|n| n.name()), Some(UmlNotation.name()), "drawn by UML");
    let card = |e: &str| UmlNotation.card(s.doc(), obj(&s, e), dg).expect("a card");
    let looks: Vec<(Tone, Text)> = elements.iter().map(|e| card(e)).map(|c| (c.tone, c.eyebrow)).collect();
    assert_eq!(
        looks,
        [
            (Tone::Tabbed, Text::said("Package")),
            (Tone::Plain, Text::said("Class")),
            (Tone::Plain, Text::said("«interface»")),
            (Tone::Plain, Text::said("«enumeration»")),
        ]
    );
}

/// A class depending on a package and a package depending on a class: each line is dashed and
/// styled, with an open arrow at its supplier's end and none at its client's, and draws that
/// dependency on Sales's diagram from the client's box to the supplier's.
#[test]
fn a_dependency_between_a_class_and_a_package_is_dashed_with_an_open_arrow_at_its_supplier() {
    let Fresh { mut s, orders, order, d, .. } = fresh();
    let place = |s: &mut S, e: &str, x: i32| run(s, "diagram.place", json!({ "diagram": d, "element": e, "x": x, "y": 0 }));
    let (class_box, package_box) = (place(&mut s, &order, 0), place(&mut s, &orders, 400));
    for (client, supplier, from, to) in [(&order, &orders, &class_box, &package_box), (&orders, &order, &package_box, &class_box)] {
        let dep = run(&mut s, "uml.dependency.add", json!({ "client": client, "supplier": supplier }));
        let e = run(&mut s, "diagram.connect", json!({ "diagram": d, "element": dep, "source": from, "target": to }));
        let line = UmlNotation.line(s.doc(), obj(&s, &dep)).expect("a line");
        assert!(line.dashed && line.styled, "{client} to {supplier}: dashed, in the dependency's own look");
        assert_eq!([line.ends[0].mark, line.ends[1].mark], [EndMark::None, EndMark::OpenArrow], "the arrow at the supplier");
        assert_eq!(UmlNotation.ends(s.doc(), obj(&s, &dep)), Some((obj(&s, client), obj(&s, supplier))), "from client to supplier");
        let at = |prop| referred(&s, obj(&s, &e), prop);
        assert_eq!(archforge_diagram::diagram_of(s.doc(), obj(&s, &e)), Some(obj(&s, &d)), "on Sales's diagram");
        assert_eq!(at(edge::ELEMENT), dep, "the line draws this dependency");
        assert_eq!((at(edge::SOURCE), at(edge::TARGET)), (from.clone(), to.clone()), "the arrow lands on the supplier's box");
    }
}

/// Every step of building the mixed diagram (the packages, the class, interface and enumeration,
/// their boxes, and a dependency each way between the class and the package with its line) is one
/// undo step that takes the project back to the rows it had and one redo step that brings back the
/// rows it made; undone one by one, each earlier state comes back down to the fresh project, and
/// redone one by one, each later state up to the whole diagram, past which nothing is redone.
#[test]
fn every_step_of_a_mixed_diagram_undoes_and_redoes_exactly() {
    let mut s = session();
    let start = snapshot(&s);
    let mut states = vec![start.clone()];
    let mut step = |s: &mut S, cmd: &str, p: Json| {
        let before = snapshot(s);
        let id = run(s, cmd, p.clone());
        let after = snapshot(s);
        s.undo().unwrap();
        assert_eq!(snapshot(s), before, "undo of {cmd} {p}");
        s.redo().unwrap();
        assert_eq!(snapshot(s), after, "redo of {cmd} {p}");
        states.push(after);
        id
    };
    let sales = step(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let d = step(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let orders = step(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let order = step(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let package_box = step(&mut s, "diagram.place", json!({ "diagram": d, "element": orders, "x": 400, "y": 0 }));
    let class_box = step(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    let payable = step(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let status = step(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "Status" }));
    step(&mut s, "diagram.place", json!({ "diagram": d, "element": payable, "x": 0, "y": 300 }));
    step(&mut s, "diagram.place", json!({ "diagram": d, "element": status, "x": 400, "y": 300 }));
    let dep = step(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": orders }));
    step(&mut s, "diagram.connect", json!({ "diagram": d, "element": dep, "source": class_box, "target": package_box }));
    let back = step(&mut s, "uml.dependency.add", json!({ "client": orders, "supplier": order }));
    step(&mut s, "diagram.connect", json!({ "diagram": d, "element": back, "source": package_box, "target": class_box }));
    let end = states.pop().unwrap();
    for (n, state) in states.iter().enumerate().rev() {
        s.undo().unwrap();
        assert_eq!(&snapshot(&s), state, "undone back to step {n}");
    }
    assert_eq!(snapshot(&s), start, "the fresh project");
    assert!(s.undo().is_err(), "nothing older to undo");
    states.push(end);
    for (n, state) in states.iter().enumerate().skip(1) {
        s.redo().unwrap();
        assert_eq!(&snapshot(&s), state, "redone up to step {n}");
    }
    assert!(s.redo().is_err(), "nothing newer to redo");
    assert_eq!(states.last(), Some(&snapshot(&s)), "the whole diagram, as it was");
}

/// What a run reports: the ids of the package's box, the package's own diagram and the class's
/// box, and the project's rows once the package, its box and its diagram are made.
type Reported = ([String; 3], String);

/// What the Package tool and the classifier tool run on a structure diagram (`uml_ui::place_package`
/// and `place`), each one undo step: the new element, its box, and for a package its own diagram.
fn as_the_tools_do(s: &mut S, sales: &str, d: &str) -> Reported {
    let id = |v: &Json, at: usize| v[at]["id"].as_str().unwrap_or_default().to_string();
    let (w, h) = PACKAGE_SIZE;
    let package = s
        .batch("Add package", |b| {
            let made = b.execute("uml.package.add", &json!({ "name": "Package", "parent": sales }))?;
            let package = made["id"].as_str().unwrap_or_default().to_string();
            b.execute("diagram.place", &json!({ "diagram": d, "element": package, "x": 380, "y": 40, "width": w, "height": h }))?;
            b.execute("diagram.add", &json!({ "kind": STRUCTURE_DIAGRAM, "element": package, "name": "Package" }))?;
            Ok(())
        })
        .unwrap();
    let made = snapshot(s);
    let (w, h) = CLASS_SIZE;
    let class = s
        .batch("Add class", |b| {
            let c = b.execute("uml.class.add", &json!({ "package": sales, "name": "Class" }))?;
            let class = c["id"].as_str().unwrap_or_default().to_string();
            b.execute("diagram.place", &json!({ "diagram": d, "element": class, "x": 40, "y": 40, "width": w, "height": h }))?;
            Ok(())
        })
        .unwrap();
    ([id(&package.value, 1), id(&package.value, 2), id(&class.value, 1)], made)
}

/// The same package, class, boxes and package diagram made by an agent, one command at a time on
/// its own behalf, as the control channel's `engine.execute` runs them.
fn as_an_agent_does(s: &mut S, sales: &str, d: &str) -> Reported {
    let agent = |s: &mut S, cmd: &str, p: Json| {
        let out = s.execute_from(cmd, &p, Some("Agent")).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
        out.value["id"].as_str().unwrap_or_default().to_string()
    };
    let (w, h) = PACKAGE_SIZE;
    let package = agent(s, "uml.package.add", json!({ "name": "Package", "parent": sales }));
    let package_box = agent(s, "diagram.place", json!({ "diagram": d, "element": package, "x": 380, "y": 40, "width": w, "height": h }));
    let own = agent(s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": package, "name": "Package" }));
    let made = snapshot(s);
    let (w, h) = CLASS_SIZE;
    let class = agent(s, "uml.class.add", json!({ "package": sales, "name": "Class" }));
    let class_box = agent(s, "diagram.place", json!({ "diagram": d, "element": class, "x": 40, "y": 40, "width": w, "height": h }));
    ([package_box, own, class_box], made)
}

/// An agent's `diagram.add` and placements leave the store the tools leave, row for row, and report
/// the same ids: the same package and class, their boxes on Sales's diagram, each reported as it is
/// made, and the package's own structure diagram, its default by its binding, either way. The
/// class's step undone (the tools' one, the agent's two) leaves on both paths the rows they had
/// once the package, its box and its diagram were made; the rest (the tools' one, the agent's
/// three) the same fresh project.
#[test]
fn an_agents_diagram_add_and_placing_give_the_store_the_tools_give() {
    let (Fresh { s: mut tools, sales, d, .. }, mut agent) = (fresh(), fresh().s);
    let start = snapshot(&tools);
    assert_eq!(snapshot(&agent), start, "both start from the same project");
    let reported = as_the_tools_do(&mut tools, &sales, &d);
    assert_eq!(as_an_agent_does(&mut agent, &sales, &d), reported, "the same ids reported, the same rows on the way");
    assert_eq!(snapshot(&agent), snapshot(&tools), "row for row");
    let ([package_box, own, class_box], made) = reported;
    let package = archforge_uml::commands::subpackages(tools.doc(), obj(&tools, &sales)).last().copied().unwrap();
    let class = archforge_uml::commands::classes(tools.doc(), obj(&tools, &sales)).last().copied().unwrap();
    let [package, class] = [package, class].map(|o| tools.doc().uuid(o).unwrap().to_string());
    for s in [&tools, &agent] {
        let boxes: Vec<(String, String)> = s
            .doc()
            .list(obj(s, &d), diagram::NODE)
            .unwrap()
            .iter()
            .map(|n| n.as_ref().unwrap())
            .map(|n| (s.doc().uuid(n).unwrap().to_string(), referred(s, n, node::ELEMENT)))
            .collect();
        assert_eq!(boxes, [(package_box.clone(), package.clone()), (class_box.clone(), class.clone())], "both on Sales's diagram, as reported");
        let (st, p, made) = (s.doc(), obj(s, &package), obj(s, &own));
        assert_eq!(archforge_uml::defaults::resolve(st, p), Ok(archforge_uml::defaults::DefaultDiagram::Bound(made)), "its default, bound");
        assert_eq!((st.get(made, diagram::KIND).unwrap().as_str(), referred(s, made, diagram::ELEMENT)), (Some(STRUCTURE_DIAGRAM), package.clone()));
    }
    // The class's step undone on each path: the package, its box, its diagram and its binding stay.
    tools.undo().unwrap();
    for _ in 0..2 {
        agent.undo().unwrap();
    }
    assert_eq!((snapshot(&tools), snapshot(&agent)), (made.clone(), made), "the class's step undone on each path");
    for (s, steps) in [(&mut tools, 1), (&mut agent, 3)] {
        for _ in 0..steps {
            s.undo().unwrap();
        }
        assert_eq!(snapshot(s), start, "back to the start");
    }
}
