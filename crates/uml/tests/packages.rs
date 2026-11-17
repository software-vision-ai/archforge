//! Packages through the engine: nested as deep as needed, names told apart among
//! siblings, classes and packages moved, a nested package removed handing its contents and
//! diagrams up to its parent (owner decision, for every notation), a top-level one taking them
//! along, either taking its default-diagram bindings along (AR-577), and a class of another package
//! drawn as an outsider; each one undo step; the sweep.

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DEFAULT_DIAGRAM_BINDING, DIAGRAM, DiagramObj, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, PropId, Store, Uuid, Value};
use archforge_uml::defaults::problems;
use archforge_uml::{ALONE, PACKAGE, STRUCTURE_DIAGRAM, registry};
use serde_json::{Value as Json, json};
type S = Session<Store, ProjectView>;
/// A session over a store holding only the project's Model.
fn session() -> S {
    let mut st = store(0);
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

fn refused(s: &mut S, cmd: &str, p: Json, words: &str) {
    let before = snapshot(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} is refused"));
    assert!(e.to_string().contains(words), "{cmd} {p}: {e}");
    assert_eq!(snapshot(s), before, "{cmd} {p} changed nothing");
}

fn obj(s: &S, id: &str) -> archforge_mof::Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn alive(s: &S, id: &str) -> bool {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| s.doc().is_alive(o))
}

/// The names of the classes a package holds.
fn class_names(s: &S, pkg: &str) -> Vec<String> {
    archforge_uml::commands::classes(s.doc(), obj(s, pkg)).into_iter().map(|c| archforge_core::name_of(s.doc(), c)).collect()
}

fn parent_of(s: &S, id: &str) -> Option<archforge_mof::Obj> {
    archforge_uml::commands::package_of(s.doc(), obj(s, id))
}

#[test]
fn packages_nest_their_names_are_told_apart_among_siblings_and_the_list_names_each_parent() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    round_trip(&mut s, "uml.package.add", json!({ "name": "orders", "parent": sales }));
    // The model's root package comes first; sales and its orders follow.
    let orders = s.execute("uml.package.list", &json!({})).unwrap().value["packages"][2]["id"].as_str().unwrap().to_string();
    let deep = run(&mut s, "uml.package.add", json!({ "name": "lines", "parent": orders }));
    // The same name in another parent is fine; twice in one parent, whatever the case, is not.
    run(&mut s, "uml.package.add", json!({ "name": "orders" }));
    refused(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }), "named orders already");
    refused(&mut s, "uml.package.add", json!({ "name": "x", "parent": "not-an-id" }), "parent");
    let listed = s.execute("uml.package.list", &json!({})).unwrap().value["packages"].as_array().unwrap().clone();
    let names: Vec<(&str, Option<&str>)> = listed.iter().map(|p| (p["name"].as_str().unwrap(), p["parent"].as_str())).collect();
    let root = listed[0]["id"].as_str().unwrap();
    assert_eq!(
        names,
        [("Model", None), ("sales", Some(root)), ("orders", Some(sales.as_str())), ("lines", Some(orders.as_str())), ("orders", Some(root))],
        "a package with no parent goes into the root, which this unnamed model calls Model"
    );
    assert_eq!(parent_of(&s, &deep), Some(obj(&s, &orders)));
}

/// A name clash, a package into itself and a self-dependency carry their reason typed, with the
/// kind and the name of the element that has the name, for an interface to say in its own words;
/// the English sentence stays as the command line shows it (AR-429).
#[test]
fn a_clash_a_package_into_itself_and_a_self_dependency_carry_their_reason_typed() {
    use archforge_engine::{EngineError, Reason};
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let mut why = |cmd: &str, p: Json| match s.execute(cmd, &p) {
        Err(EngineError::Failed { error, .. }) => (error.reason, error.message),
        other => panic!("{cmd} {p}: {other:?}"),
    };
    let taken = Reason::NameTaken { kind: "class", name: "Order".into() };
    assert_eq!(why("uml.interface.add", json!({ "package": sales, "name": "ORDER" })), (Some(taken), "there is a class named Order already".into()));
    let into = (Some(Reason::PackageIntoItself), "a package cannot go into itself or a package it holds".to_string());
    assert_eq!(why("uml.package.move", json!({ "package": sales, "to": inner })), into);
    let itself = (Some(Reason::DependsOnItself { kind: "class" }), "a class cannot depend on itself".to_string());
    assert_eq!(why("uml.dependency.add", json!({ "client": order, "supplier": order })), itself);
    // Reconnected onto its client, a dependency would make it depend on itself too.
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    let uses = run(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": line }));
    let p = json!({ "relationship": uses, "end": "target", "class": order });
    let Err(EngineError::Failed { error, .. }) = s.execute("uml.relationship.reconnect", &p) else { panic!("{p} is refused") };
    assert_eq!((error.reason, error.message), itself, "reconnected");
}

#[test]
fn a_class_and_a_package_move_a_cycle_or_a_clash_is_refused_and_the_diagrams_keep_showing_them() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "sales" }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    round_trip(&mut s, "uml.class.move", json!({ "class": order, "package": billing }));
    assert_eq!(class_names(&s, &billing), ["Invoice", "Order"]);
    assert!(class_names(&s, &sales).is_empty());
    assert!(alive(&s, &node), "the diagram still shows it, now as an outsider");
    let card = archforge_uml::figures::card(s.doc(), obj(&s, &order), obj(&s, &d)).unwrap();
    assert!(card.elsewhere);
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Said("Class · {package}", vec![("package", "billing".into())]));
    // A class named like one already there is refused.
    let other = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "order" }));
    refused(&mut s, "uml.class.move", json!({ "class": other, "package": billing }), "named Order already");
    // A package goes into another, or back into the model's root; into itself or what it holds is refused.
    round_trip(&mut s, "uml.package.move", json!({ "package": billing, "to": inner }));
    assert_eq!(parent_of(&s, &billing), Some(obj(&s, &inner)));
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": billing }), "into itself");
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": sales }), "into itself");
    round_trip(&mut s, "uml.package.move", json!({ "package": billing, "to": "" }));
    assert_eq!(parent_of(&s, &billing), archforge_uml::commands::root_package(s.doc()));
    run(&mut s, "uml.package.add", json!({ "name": "inner" }));
    refused(&mut s, "uml.package.move", json!({ "package": inner, "to": "" }), "named inner already");
}

#[test]
fn a_package_removed_hands_its_contents_and_diagrams_up_to_its_parent_the_root_included() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "orders", "parent": sales }));
    let deep = run(&mut s, "uml.package.add", json!({ "name": "lines", "parent": orders }));
    let order = run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    let customer = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Customer" }));
    let a = run(&mut s, "uml.association.add", json!({ "from": customer, "to": order }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": orders, "name": "orders" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    // orders goes: Order, the package lines and the diagram move up into sales.
    round_trip(&mut s, "element.remove", json!({ "ids": [orders] }));
    assert!(!alive(&s, &orders));
    assert_eq!(class_names(&s, &sales), ["Customer", "Order"]);
    assert_eq!(parent_of(&s, &deep), Some(obj(&s, &sales)));
    assert!(alive(&s, &d) && alive(&s, &a), "the diagram and the association stay");
    let of = s.doc().get(obj(&s, &d), archforge_diagram::diagram::ELEMENT).unwrap().as_ref();
    assert_eq!(of, Some(obj(&s, &sales)), "the diagram is of the parent now");
    // A clash in the parent is refused, with nothing moved.
    let dup = run(&mut s, "uml.package.add", json!({ "name": "dup", "parent": sales }));
    run(&mut s, "uml.class.add", json!({ "package": dup, "name": "customer" }));
    refused(&mut s, "element.remove", json!({ "ids": [dup] }), "named Customer already");
    // Every package sits in the model's root package, so sales too hands everything up: into
    // the root, the relationships of its classes kept.
    let other = run(&mut s, "uml.package.add", json!({ "name": "other" }));
    let outside = run(&mut s, "uml.class.add", json!({ "package": other, "name": "Outside" }));
    let dep = run(&mut s, "uml.dependency.add", json!({ "client": outside, "supplier": customer }));
    round_trip(&mut s, "element.remove", json!({ "ids": [sales] }));
    assert!(!alive(&s, &sales));
    for kept in [&customer, &order, &deep, &a, &dep, &d, &outside] {
        assert!(alive(&s, kept), "{kept} stays");
    }
    assert_eq!(parent_of(&s, &deep), archforge_uml::commands::root_package(s.doc()));
}

#[test]
fn the_sweep_finds_no_command_that_breaks_the_store_with_packages_nested() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": inner, "name": "Order" }));
    let calls = sweep_with(&mut s, &[sales.as_str(), inner.as_str(), order.as_str(), "not-an-id"], |st| {
        format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())
    })
    .unwrap();
    assert!(calls > 100, "{calls}");
    let _ = Value::Null;
}

#[test]
fn a_package_leads_to_one_class_diagram_of_its_own_named_like_it() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    // The first ask makes it, as one undo step; the next ones find the same diagram.
    let made = s.execute("uml.package.diagram", &json!({ "package": orders })).unwrap().value;
    assert_eq!(made["made"], true);
    let d = made["id"].as_str().unwrap().to_string();
    let again = s.execute("uml.package.diagram", &json!({ "package": orders })).unwrap().value;
    assert_eq!((again["id"].as_str().unwrap(), again["made"].as_bool()), (d.as_str(), Some(false)), "never a second");
    let diagram = obj(&s, &d);
    let st = s.doc();
    assert_eq!(st.get(diagram, archforge_diagram::diagram::KIND).unwrap().as_str(), Some(archforge_uml::STRUCTURE_DIAGRAM));
    assert_eq!(st.get(diagram, archforge_diagram::diagram::ELEMENT).unwrap().as_ref(), Some(obj(&s, &orders)));
    assert_eq!(st.get(diagram, archforge_diagram::diagram::NAME).unwrap().as_str(), Some("Orders"));
    // Made as diagram.add makes one: its lines at right angles around the classes.
    let look = st.get(diagram, archforge_diagram::diagram::STYLE).unwrap().as_ref().expect("its style");
    assert_eq!(st.get(look, archforge_diagram::style::ROUTING).unwrap(), Value::Enum(archforge_diagram::Routing::RightAngled.index()));
    assert_eq!(st.get(look, archforge_diagram::style::AVOID).unwrap(), Value::Bool(true));
    assert_eq!(archforge_uml::commands::diagram_of(s.doc(), obj(&s, &orders)), Some(diagram));
    s.undo().unwrap();
    s.undo().unwrap();
    assert!(!alive(&s, &d), "undo takes the diagram back");
    s.redo().unwrap();
    s.redo().unwrap();
    // Renamed, the package takes its diagram's name along while the diagram still has the old one.
    run(&mut s, "uml.package.rename", json!({ "package": orders, "name": "Ordering" }));
    assert_eq!(s.doc().get(obj(&s, &d), archforge_diagram::diagram::NAME).unwrap().as_str(), Some("Ordering"));
    run(&mut s, "diagram.rename", json!({ "diagram": d, "name": "Order flow" }));
    run(&mut s, "uml.package.rename", json!({ "package": orders, "name": "Orders" }));
    assert_eq!(s.doc().get(obj(&s, &d), archforge_diagram::diagram::NAME).unwrap().as_str(), Some("Order flow"), "a name of its own stays");
    round_trip(&mut s, "uml.package.diagram", json!({ "package": sales }));
}

#[test]
fn a_class_diagram_shows_the_packages_its_package_holds_as_tabbed_cards() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "Sales" }));
    // A package Sales holds goes on its diagram; one it does not hold is refused.
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": orders, "x": 0, "y": 0 }));
    let e = s.execute("diagram.place", &json!({ "diagram": d, "element": billing, "x": 300, "y": 0 })).expect_err("another package's");
    assert!(!e.to_string().is_empty());
    let st = s.doc();
    let n = obj(&s, &node);
    let b = archforge_diagram::grid::bounds_of(st, n);
    assert_eq!((b.width, b.height), archforge_uml::PACKAGE_SIZE, "placed at a package's size");
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(&s, &orders), obj(&s, &d)).expect("a card");
    assert_eq!(card.tone, archforge_diagram::figure::Tone::Tabbed);
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::said("Package"));
    assert_eq!(card.name, "Orders");
    assert!(card.compartments.is_empty() && card.hidden == 0, "no rows: its card shows its own diagram instead (AR-113)");
}

#[test]
fn a_package_moved_leaves_the_diagrams_of_its_old_parent_and_keeps_its_own_diagram_and_contents() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    let root = archforge_uml::commands::root_package(s.doc()).map(|r| s.doc().uuid(r).unwrap().to_string()).unwrap();
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let top = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": root, "name": "Top" }));
    let card = run(&mut s, "diagram.place", json!({ "diagram": top, "element": sales, "x": 0, "y": 0 }));
    let own = s.execute("uml.package.diagram", &json!({ "package": sales })).unwrap().value["id"].as_str().unwrap().to_string();
    // Into Billing: its card leaves the root's diagram; its class and its own diagram stay with it.
    round_trip(&mut s, "uml.package.move", json!({ "package": sales, "to": billing }));
    assert_eq!(parent_of(&s, &sales), Some(obj(&s, &billing)));
    assert!(!alive(&s, &card), "no card of it on a diagram of a package that no longer holds it");
    assert!(alive(&s, &own) && alive(&s, &order));
    assert_eq!(archforge_uml::commands::diagram_of(s.doc(), obj(&s, &sales)).map(|d| s.doc().uuid(d).unwrap().to_string()), Some(own.clone()));
    // Into itself or a package it holds: refused, nothing changed.
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": sales }));
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": sales }), "cannot go into itself");
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": lines }), "cannot go into itself");
    // Back to the root, given as nothing.
    run(&mut s, "uml.package.move", json!({ "package": sales, "to": "" }));
    assert_eq!(parent_of(&s, &sales), Some(obj(&s, &root)));
}

#[test]
fn a_package_card_builds_no_rows_however_much_it_holds() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "Sales" }));
    let crowded = run(&mut s, "uml.package.add", json!({ "name": "Crowded", "parent": sales }));
    for i in 0..60 {
        run(&mut s, "uml.class.add", json!({ "package": crowded, "name": format!("C{i}") }));
    }
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, s.doc(), obj(&s, &crowded), obj(&s, &d)).unwrap();
    // Its card shows its own diagram in small instead (AR-113): nothing listed, nothing counted.
    assert_eq!((card.compartments.len(), card.hidden, card.name.as_str()), (0, 0, "Crowded"));
}

/// `s`'s model read back from its rows into a store of its own, to make bindings in by hand, as no
/// command makes one yet; its new ids counted far from those in use.
fn reread(s: &S) -> Store {
    let (tables, junctions) = tables();
    let mut st = store(1 << 40);
    archforge_mof::rows::restore(&mut st, &archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap(), &tables, &junctions).unwrap();
    st
}

/// A binding under the id `[id; 16]`, a member of the Model, naming `target` the default diagram
/// of `owner`; gives its id.
fn bind(st: &mut Store, id: u8, owner: Obj, target: Obj) -> String {
    let model = archforge_uml::commands::model(st).unwrap();
    let b = st.create_with_uuid(DEFAULT_DIAGRAM_BINDING, Uuid([id; 16])).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(b)).unwrap();
    st.set(b, OWNER_ELEMENT, Value::Ref(owner)).unwrap();
    st.set(b, DEFAULT_DIAGRAM, Value::Ref(target)).unwrap();
    Uuid([id; 16]).to_string()
}

/// What `prop` of the object under `id` refers to.
fn refers(s: &S, id: &str, prop: PropId) -> Option<Obj> {
    s.doc().get(obj(s, id), prop).unwrap().as_ref()
}

/// A bound package removed takes its binding along (AR-577), as one undo step that brings the
/// package, its diagrams and its binding back under their ids. Nested, it hands its diagrams up and
/// its parent's binding stays as it was, as does that of the package it hands up; held by the root,
/// it hands them up to the root alike. A diagram that is no default goes alone. No binding problem
/// at any point.
#[test]
fn a_bound_package_removed_takes_its_binding_along_and_its_parent_keeps_its_own() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let root = archforge_uml::commands::root_package(s.doc()).map(|r| s.doc().uuid(r).unwrap().to_string()).unwrap();
    let mut diagram = |of: &str, name: &str| run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": name }));
    let (top, sales_d, overview, detail, lines_d) =
        (diagram(&root, "Model"), diagram(&sales, "Sales"), diagram(&orders, "Orders"), diagram(&orders, "Order detail"), diagram(&lines, "Lines"));
    // Each package bound to one of its diagrams: (binding, package, diagram).
    let mut st = reread(&s);
    let mut bind_at = |id: u8, owner: &String, target: &String| {
        let (o, t) = (st.by_uuid(Uuid::parse(owner).unwrap()).unwrap(), st.by_uuid(Uuid::parse(target).unwrap()).unwrap());
        (bind(&mut st, id, o, t), owner.clone(), target.clone())
    };
    let (b_root, b_sales) = (bind_at(0xb0, &root, &top), bind_at(0xb1, &sales, &sales_d));
    let (b_orders, b_lines) = (bind_at(0xb2, &orders, &detail), bind_at(0xb3, &lines, &lines_d));
    let mut s = over(st);
    let kept = |s: &S, (b, owner, target): &(String, String, String)| {
        alive(s, b) && refers(s, b, OWNER_ELEMENT) == Some(obj(s, owner)) && refers(s, b, DEFAULT_DIAGRAM) == Some(obj(s, target))
    };
    assert_eq!(problems(s.doc()), []);
    // A diagram that is no default goes alone.
    round_trip(&mut s, "diagram.remove", json!({ "diagram": overview }));
    assert!(!alive(&s, &overview) && kept(&s, &b_orders));
    // Orders goes with its binding; its diagram and Lines move up into Sales, their bindings kept.
    round_trip(&mut s, "element.remove", json!({ "ids": [orders] }));
    assert!(!alive(&s, &orders) && !alive(&s, &b_orders.0), "its binding goes with it");
    assert_eq!(refers(&s, &detail, archforge_diagram::diagram::ELEMENT), Some(obj(&s, &sales)), "its diagram is handed up");
    assert!([&b_root, &b_sales, &b_lines].into_iter().all(|b| kept(&s, b)), "the others stay as they were");
    assert_eq!(problems(s.doc()), []);
    // One undo step brings the package, its diagram and its binding back, under their ids.
    s.undo().unwrap();
    assert!(kept(&s, &b_orders) && refers(&s, &detail, archforge_diagram::diagram::ELEMENT) == Some(obj(&s, &orders)));
    assert_eq!(problems(s.doc()), []);
    s.redo().unwrap();
    // Sales, held by the root, hands up to the root in turn, whose binding stays.
    round_trip(&mut s, "element.remove", json!({ "ids": [sales] }));
    assert!(!alive(&s, &sales) && !alive(&s, &b_sales.0) && alive(&s, &sales_d) && alive(&s, &detail));
    assert!(kept(&s, &b_root) && kept(&s, &b_lines));
    assert_eq!(problems(s.doc()), []);
}

/// A bound package with no parent removed takes its diagrams along, and with them its binding and
/// that of the package it holds (AR-577): never refused, as one undo step that brings all back.
#[test]
fn a_package_with_no_parent_takes_its_diagrams_and_every_binding_in_it_along() {
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    let member = |st: &mut Store, class, id: u8| {
        let o = st.create_with_uuid(class, Uuid([id; 16])).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(o)).unwrap();
        o
    };
    let outer = member(&mut st, PACKAGE, 0x10);
    let inner = st.create_with_uuid(PACKAGE, Uuid([0x20; 16])).unwrap();
    st.push(outer, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(inner)).unwrap();
    for (package, id) in [(outer, 0x11), (inner, 0x21)] {
        let d = member(&mut st, DIAGRAM, id);
        DiagramObj::set_kind(&mut st, d, Some(STRUCTURE_DIAGRAM)).unwrap();
        DiagramObj::set_element(&mut st, d, Some(package)).unwrap();
        bind(&mut st, id + 1, package, d);
    }
    let mut s = over(st);
    let ids: Vec<String> = [0x10, 0x11, 0x12, 0x20, 0x21, 0x22].map(|n| Uuid([n; 16]).to_string()).into();
    assert!(parent_of(&s, &ids[0]).is_none() && problems(s.doc()).is_empty());
    round_trip(&mut s, "element.remove", json!({ "ids": [ids[0]] }));
    assert!(ids.iter().all(|id| !alive(&s, id)), "the packages, their diagrams and their bindings go");
    assert_eq!(problems(s.doc()), []);
    s.undo().unwrap();
    assert!(ids.iter().all(|id| alive(&s, id)));
    assert_eq!(refers(&s, &ids[5], OWNER_ELEMENT), Some(obj(&s, &ids[3])));
    assert_eq!(refers(&s, &ids[5], DEFAULT_DIAGRAM), Some(obj(&s, &ids[4])));
    assert_eq!(problems(s.doc()), []);
}
