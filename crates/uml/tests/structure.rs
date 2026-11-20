//! A fresh project's UML structure diagram holding packages and classifiers side by side (AR-394,
//! AR-556): each box shows its element by the element's own id, and placing owns nothing, changes no
//! element and leaves the diagram's kind and notation as they were; a dependency between a class
//! and a package is dashed with an open arrow at its supplier, whichever end is the package; every
//! step is undone and redone exactly; and the agent's commands, `diagram.add` among them, leave the
//! store the Package and classifier tools' undo steps leave.

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

/// The rows of the model alone, those of the diagrams' own tables and junctions left out: what
/// placing must not change.
fn model(s: &S) -> String {
    let (rows, tables, junctions) = saved(s);
    let of_model = |class: archforge_mof::ClassId| class.metamodel() != archforge_diagram::INDEX;
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

/// A child package and three kinds of classifier placed on one structure diagram: each box names
/// its element by that element's own id, no element is copied, moved or changed, the diagram stays
/// a UML structure diagram of Sales, and each card is drawn in its own kind's notation.
#[test]
fn a_child_package_and_classifiers_placed_together_keep_their_ids_owners_and_notation() {
    let Fresh { mut s, sales, orders, order, payable, status, d } = fresh();
    let elements = [&orders, &order, &payable, &status];
    let owner = |s: &S, id: &str| s.doc().container(obj(s, id)).unwrap().map(|(up, prop)| (s.doc().uuid(up).unwrap(), prop));
    let owners: Vec<_> = elements.iter().map(|e| owner(&s, e)).collect();
    let before = model(&s);
    let nodes: Vec<String> =
        elements.iter().zip(0..).map(|(e, i)| run(&mut s, "diagram.place", json!({ "diagram": d, "element": e, "x": 300 * i, "y": 0 }))).collect();
    for (n, e) in nodes.iter().zip(elements) {
        assert_eq!(&referred(&s, obj(&s, n), node::ELEMENT), e, "the box shows the element itself");
    }
    assert_eq!(model(&s), before, "placing copies, moves and changes no element");
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
/// styled, with an open arrow at its supplier's end and none at its client's, and runs from the
/// client's box to the supplier's.
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
        assert_eq!((at(edge::SOURCE), at(edge::TARGET)), (from.clone(), to.clone()), "the arrow lands on the supplier's box");
    }
}

/// Every step of building the mixed diagram is one undo step that takes the project back to the
/// rows it had and one redo step that brings back the rows it made; all undone, the fresh project
/// is back, and all redone, the whole diagram.
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
    let dep = step(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": orders }));
    step(&mut s, "diagram.connect", json!({ "diagram": d, "element": dep, "source": class_box, "target": package_box }));
    let end = states.pop().unwrap();
    for (n, state) in states.iter().enumerate().rev() {
        s.undo().unwrap();
        assert_eq!(&snapshot(&s), state, "undone back to step {n}");
    }
    assert_eq!(snapshot(&s), start, "the fresh project");
    assert!(s.undo().is_err(), "nothing older to undo");
    for _ in 0..states.len() {
        s.redo().unwrap();
    }
    assert_eq!(snapshot(&s), end, "the whole diagram again");
}

/// What the Package tool and the classifier tool run on a structure diagram (`uml_ui::place_package`
/// and `place`), each one undo step: the new element, its box, and for a package its own diagram.
fn as_the_tools_do(s: &mut S, sales: &str, d: &str) {
    let (w, h) = PACKAGE_SIZE;
    s.batch("Add package", |b| {
        let made = b.execute("uml.package.add", &json!({ "name": "Package", "parent": sales }))?;
        let package = made["id"].as_str().unwrap_or_default().to_string();
        b.execute("diagram.place", &json!({ "diagram": d, "element": package, "x": 380, "y": 40, "width": w, "height": h }))?;
        b.execute("diagram.add", &json!({ "kind": STRUCTURE_DIAGRAM, "element": package, "name": "Package" }))?;
        Ok(())
    })
    .unwrap();
    let (w, h) = CLASS_SIZE;
    s.batch("Add class", |b| {
        let c = b.execute("uml.class.add", &json!({ "package": sales, "name": "Class" }))?;
        let class = c["id"].as_str().unwrap_or_default().to_string();
        b.execute("diagram.place", &json!({ "diagram": d, "element": class, "x": 40, "y": 40, "width": w, "height": h }))?;
        Ok(())
    })
    .unwrap();
}

/// The same package, class, boxes and package diagram made by an agent, one command at a time on
/// its own behalf, as the control channel's `engine.execute` runs them.
fn as_an_agent_does(s: &mut S, sales: &str, d: &str) {
    let mut agent = |cmd: &str, p: Json| {
        let out = s.execute_from(cmd, &p, Some("Agent")).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
        out.value["id"].as_str().unwrap_or_default().to_string()
    };
    let (w, h) = PACKAGE_SIZE;
    let package = agent("uml.package.add", json!({ "name": "Package", "parent": sales }));
    agent("diagram.place", json!({ "diagram": d, "element": package, "x": 380, "y": 40, "width": w, "height": h }));
    agent("diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": package, "name": "Package" }));
    let (w, h) = CLASS_SIZE;
    let class = agent("uml.class.add", json!({ "package": sales, "name": "Class" }));
    agent("diagram.place", json!({ "diagram": d, "element": class, "x": 40, "y": 40, "width": w, "height": h }));
}

/// An agent's `diagram.add` and placements leave the store the tools leave, row for row: the same
/// package and class, boxes and package diagram, which is the package's default either way. The
/// tools' two steps undo in two, the agent's five in five, back to the same fresh project.
#[test]
fn an_agents_diagram_add_and_placing_give_the_store_the_tools_give() {
    let (Fresh { s: mut tools, sales, d, .. }, mut agent) = (fresh(), fresh().s);
    let start = snapshot(&tools);
    assert_eq!(snapshot(&agent), start, "both start from the same project");
    as_the_tools_do(&mut tools, &sales, &d);
    as_an_agent_does(&mut agent, &sales, &d);
    assert_eq!(snapshot(&agent), snapshot(&tools), "row for row");
    let package = archforge_uml::commands::subpackages(tools.doc(), obj(&tools, &sales)).last().copied().unwrap();
    let package = tools.doc().uuid(package).unwrap().to_string();
    let default = |s: &mut S| run(s, "uml.package.diagram", json!({ "package": package }));
    let own = default(&mut tools);
    assert_eq!(default(&mut agent), own, "the same default diagram");
    assert_eq!(tools.doc().get(obj(&tools, &own), diagram::KIND).unwrap().as_str(), Some(STRUCTURE_DIAGRAM));
    for (s, steps) in [(&mut tools, 2), (&mut agent, 5)] {
        for _ in 0..steps {
            s.undo().unwrap();
        }
        assert_eq!(snapshot(s), start, "{steps} steps back to the start");
    }
}
