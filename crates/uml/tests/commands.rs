//! UML's commands through the engine over the shared core, the diagrams and UML:
//! packages and classes added, renamed, listed and refused by key; a class diagram of a package
//! showing its own classes; removing a package takes its classes and their views; every command
//! undoes and redoes; the sweep.

use archforge_diagram::{DiagramView, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::{ALONE, CLASS_DIAGRAM, PACKAGE, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

fn ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 0;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// A store over the core, the diagrams and UML, with the project's Model.
fn store() -> Store {
    let universe = archforge_diagram::universe_with(vec![archforge_core::metamodel().unwrap(), archforge_uml::metamodel().unwrap()]).unwrap();
    let slabs = archforge_diagram::with_slabs(
        CompositeSlabs::new()
            .with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))
            .unwrap()
            .with(archforge_uml::INDEX, Box::new(archforge_uml::UmlSlabs::default()))
            .unwrap(),
    )
    .unwrap();
    let mut st = Store::new(universe, Box::new(slabs), ids());
    let model = st.create(archforge_core::MODEL).unwrap();
    st.set(model, archforge_core::named_element::NAME, Value::str("Shop")).unwrap();
    st
}

/// A session over [`store`] with UML's commands.
fn session() -> S {
    Session::with_view(registry().unwrap(), store(), 1, Options::default(), ProjectView::new(ALONE))
}

fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

fn refused_key(s: &mut S, cmd: &str, p: Json) -> String {
    let before = picture(s.doc());
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was not refused"));
    assert_eq!(picture(s.doc()), before, "{cmd} changed nothing");
    format!("{e:?}")
}

fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// Every live object with its class, container and stored values.
fn picture(st: &Store) -> Vec<String> {
    let u = st.universe();
    st.alive()
        .map(|o| {
            let class = st.class_of(o).unwrap();
            let values: Vec<String> = u
                .info(class)
                .unwrap()
                .stored
                .iter()
                .map(|p| format!("{:?}", st.list(o, *p).ok().or_else(|| st.get(o, *p).ok().map(|v| vec![v]))))
                .collect();
            format!("{o:?} {class:?} {:?} {values:?}", st.container(o).unwrap())
        })
        .collect()
}

#[test]
fn packages_and_classes_are_added_renamed_listed_and_refused_by_key() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Customer" }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Invoice" }));
    assert_eq!(s.view().selection, vec![obj(s.doc(), &invoice)], "the new class is selected");
    run(&mut s, "uml.class.rename", json!({ "class": order, "name": "Purchase" }));
    run(&mut s, "uml.package.rename", json!({ "package": pkg, "name": "selling" }));
    let listed = s.execute("uml.class.list", &json!({ "package": pkg })).unwrap().value;
    let names: Vec<&str> = listed["classes"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Purchase", "Customer", "Invoice"]);
    let packages = s.execute("uml.package.list", &json!({})).unwrap().value;
    // A package added with no parent goes into the model's root package, named after the project.
    assert_eq!(packages["packages"][0]["name"], "Shop");
    assert_eq!(packages["packages"][1]["name"], "selling");
    assert_eq!(packages["packages"][1]["classes"], 3);
    // Refusals name their key and change nothing.
    for (cmd, p, key) in [
        ("uml.package.add", json!({ "name": "" }), "name"),
        ("uml.package.add", json!({ "name": "Selling" }), "name"),
        ("uml.class.add", json!({ "package": pkg, "name": "customer" }), "name"),
        ("uml.class.add", json!({ "package": order, "name": "X" }), "package"),
        ("uml.class.add", json!({ "package": "not-an-id", "name": "X" }), "package"),
        ("uml.class.rename", json!({ "class": pkg, "name": "X" }), "class"),
        ("uml.class.rename", json!({ "class": order, "name": "Customer" }), "name"),
        ("uml.package.rename", json!({ "package": pkg, "name": "a\u{7}b" }), "name"),
    ] {
        let e = refused_key(&mut s, cmd, p);
        assert!(e.contains(key), "{cmd}: {e}");
    }
}

#[test]
fn a_class_diagram_shows_its_packages_classes_and_removing_the_package_takes_them_and_their_views() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
    assert!(s.view().kinds().contains(&CLASS_DIAGRAM));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": sales, "name": "Sales" }));
    assert!(s.execute("diagram.add", &json!({ "kind": CLASS_DIAGRAM })).is_err(), "a class diagram is of a package");
    assert!(s.execute("diagram.add", &json!({ "kind": CLASS_DIAGRAM, "element": order })).is_err(), "not of a class");
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    let size = s.execute("diagram.get", &json!({ "diagram": d })).unwrap().value;
    assert_eq!(
        (size["nodes"][0]["width"].as_f64(), size["nodes"][0]["height"].as_f64()),
        (Some(archforge_uml::CLASS_SIZE.0), Some(archforge_uml::CLASS_SIZE.1))
    );
    // Another package's class is welcome, drawn as an outsider.
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": invoice, "x": 300, "y": 0 }));
    let st = s.doc();
    assert!(archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, &invoice), obj(st, &d)).unwrap().elsewhere);
    // A card: the class's name, its attributes and its operations.
    let st = s.doc();
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, &order), obj(st, &d)).unwrap();
    assert_eq!((card.name.as_str(), card.compartments.len()), ("Order", 2));
    // Called directly, a removal naming something UML does not remove yet changes nothing, not
    // even what comes before it in the list.
    let mut st = store();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let pkg = st.create(PACKAGE).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(pkg)).unwrap();
    let before = picture(&st);
    assert!(archforge_uml::commands::remove_elements(&mut st, &[pkg, model]).is_err());
    assert_eq!(picture(&st), before, "nothing went");
    // Every package sits in the model's root, so the package goes and its class and its diagram
    // move up into the root, in one undo step.
    let before = picture(s.doc());
    let out = s.execute("element.remove", &json!({ "ids": [sales] })).unwrap();
    assert_eq!(out.value["removed"], 1, "{}", out.value);
    let st = s.doc();
    let root = archforge_uml::commands::root_package(st).unwrap();
    let order_obj = obj(st, &order);
    assert_eq!(archforge_uml::commands::package_of(st, order_obj), Some(root));
    let diagram = obj(st, &d);
    assert_eq!(st.get(diagram, archforge_diagram::diagram::ELEMENT).unwrap().as_ref(), Some(root));
    assert_eq!(st.alive().filter(|o| st.class_of(*o).ok() == Some(PACKAGE)).count(), 2, "billing and the root are left");
    // The root itself stays.
    let root_id = st.uuid(root).unwrap().to_string();
    assert!(s.execute("element.remove", &json!({ "ids": [root_id] })).is_err());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    s.redo().unwrap();
}

#[test]
fn every_command_undoes_and_redoes_and_the_sweep_breaks_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let class = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    for (cmd, p) in [
        ("uml.package.add", json!({ "name": "billing" })),
        ("uml.class.add", json!({ "package": pkg, "name": "Invoice" })),
        ("uml.class.rename", json!({ "class": class, "name": "Purchase" })),
        ("uml.package.rename", json!({ "package": pkg, "name": "selling" })),
        ("diagram.place", json!({ "diagram": d, "element": class, "x": 10, "y": 10 })),
    ] {
        let before = picture(s.doc());
        s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd}: {e}"));
        let after = picture(s.doc());
        s.undo().unwrap();
        assert_eq!(picture(s.doc()), before, "undo of {cmd}");
        s.redo().unwrap();
        assert_eq!(picture(s.doc()), after, "redo of {cmd}");
    }
    let ids = [pkg.as_str(), class.as_str(), d.as_str(), "not-an-id", "00000000-0000-7000-8000-000000000000"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}

#[test]
fn one_root_package_named_after_the_project_holds_what_has_no_other_home() {
    let mut s = session();
    assert!(archforge_uml::commands::root_package(s.doc()).is_none(), "a new project has none yet");
    let first = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let root = archforge_uml::commands::root_package(s.doc()).expect("made with the first package");
    let st = s.doc();
    assert_eq!(archforge_core::name_of(st, root), "Shop");
    assert_eq!(archforge_uml::commands::packages(st), vec![root], "the root alone is at the top level");
    assert_eq!(archforge_uml::commands::subpackages(st, root), vec![obj(st, &first)]);
    // Asking again makes no second root, whoever asks.
    let again = run(&mut s, "uml.model.root", json!({}));
    run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let st = s.doc();
    assert_eq!(obj(st, &again), root);
    assert_eq!(archforge_uml::commands::packages(st), vec![root]);
    assert_eq!(st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_core::TAG)).count(), 1);
    // The root stays where it is; a package moved to nothing goes back into it.
    let root_id = st.uuid(root).unwrap().to_string();
    assert!(refused_key(&mut s, "uml.package.move", json!({ "package": root_id, "to": first })).contains("package"));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": first }));
    run(&mut s, "uml.package.move", json!({ "package": inner, "to": "" }));
    let st = s.doc();
    assert!(archforge_uml::commands::subpackages(st, root).contains(&obj(st, &inner)));
    // Undoing the first package takes the root with it: one step.
    while s.undo().is_ok() {}
    assert!(archforge_uml::commands::root_package(s.doc()).is_none());
}

#[test]
fn an_older_model_s_top_level_packages_go_into_the_root_and_one_named_after_the_project_becomes_it() {
    // Two top-level packages, as projects had them before the root.
    let mut st = store();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let mut top = Vec::new();
    for name in ["Commerce", "Billing"] {
        let p = st.create(PACKAGE).unwrap();
        st.set(p, archforge_uml::named_element::NAME, Value::str(name)).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(p)).unwrap();
        top.push(p);
    }
    let root = archforge_uml::commands::ensure_root(&mut st).unwrap();
    assert_eq!(archforge_core::name_of(&st, root), "Shop");
    assert_eq!(archforge_uml::commands::packages(&st), vec![root]);
    assert_eq!(archforge_uml::commands::subpackages(&st, root), top, "both, in their order");
    // One already named after the project is the root, and keeps what it holds.
    let mut st = store();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let shop = st.create(PACKAGE).unwrap();
    st.set(shop, archforge_uml::named_element::NAME, Value::str("Shop")).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(shop)).unwrap();
    assert_eq!(archforge_uml::commands::ensure_root(&mut st).unwrap(), shop);
}
