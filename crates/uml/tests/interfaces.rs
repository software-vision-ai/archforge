//! Interfaces through the engine: added in a package, renamed, moved and removed, each one undo
//! step; their names told apart from their package's classes and interfaces; attributes and
//! operations through the member commands, public as UML has them; drawn as a card with the
//! keyword «interface»; associated and specialized with the classifier rules (an interface
//! specializes only an interface, a class never one); offered as a type; saved as rows and read
//! back identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::{ALONE, CLASS_DIAGRAM, INTERFACE, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// Ids counted up from `start`, so two stores never share one.
fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// A store over the core, the diagrams and UML.
fn store(start: u64) -> Store {
    let universe = archforge_diagram::universe_with(vec![archforge_core::metamodel().unwrap(), archforge_uml::metamodel().unwrap()]).unwrap();
    let slabs = archforge_diagram::with_slabs(
        CompositeSlabs::new()
            .with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))
            .unwrap()
            .with(archforge_uml::INDEX, Box::new(archforge_uml::UmlSlabs::default()))
            .unwrap(),
    )
    .unwrap();
    Store::new(universe, Box::new(slabs), ids(start))
}

/// A session over a store with the project's Model, named Shop.
fn session() -> S {
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    st.set(model, archforge_core::named_element::NAME, Value::str("Shop")).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// Runs `cmd`, which must succeed, and gives the id it made or named.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// The refusal of `cmd`, which changes nothing.
fn refused(s: &mut S, cmd: &str, p: Json) -> String {
    let before = picture(s.doc());
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was not refused"));
    assert_eq!(picture(s.doc()), before, "{cmd} changed nothing");
    format!("{e:?}")
}

/// The element with the id `id`.
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

/// `cmd` changes the model as one undo step: undone it leaves the model as it was, redone as it made it.
fn one_step(s: &mut S, cmd: &str, p: Json) -> String {
    let before = picture(s.doc());
    let id = run(s, cmd, p);
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "undo of {cmd}");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after, "redo of {cmd}");
    id
}

/// Interfaces are added, renamed, moved and removed, each one undo step, named apart from their package's classes.
#[test]
fn an_interface_is_added_renamed_moved_and_removed_each_one_step_its_name_free_among_classes_and_interfaces() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let before = picture(s.doc());
    let payable = run(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let st = s.doc();
    assert_eq!(st.class_of(obj(st, &payable)).unwrap(), INTERFACE);
    assert_eq!(s.view().selection, vec![obj(st, &payable)], "the new interface is selected");
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    let st = s.doc();
    assert_eq!(archforge_uml::commands::classifiers(st, obj(st, &sales)), vec![obj(st, &order), obj(st, &payable)]);
    let listed = s.execute("uml.interface.list", &json!({ "package": sales })).unwrap().value;
    assert_eq!(listed["interfaces"][0]["name"], "Payable");
    assert_eq!(s.execute("uml.class.list", &json!({ "package": sales })).unwrap().value["classes"].as_array().unwrap().len(), 1);
    one_step(&mut s, "uml.interface.rename", json!({ "interface": payable, "name": "Billable" }));
    assert_eq!(archforge_core::name_of(s.doc(), obj(s.doc(), &payable)), "Billable");
    // A class and an interface share their package's names, whatever the case.
    for (cmd, p, key, why) in [
        ("uml.interface.add", json!({ "package": sales, "name": "order" }), "name", "there is a class named Order already"),
        ("uml.class.add", json!({ "package": sales, "name": "BILLABLE" }), "name", "there is an interface named Billable already"),
        ("uml.interface.rename", json!({ "interface": payable, "name": "Order" }), "name", "class named Order"),
        ("uml.class.rename", json!({ "class": order, "name": "billable" }), "name", "interface named Billable"),
        ("uml.interface.add", json!({ "package": sales, "name": "" }), "name", "name"),
        ("uml.interface.add", json!({ "package": order, "name": "X" }), "package", "not a package"),
        ("uml.interface.rename", json!({ "interface": order, "name": "X" }), "interface", "not an interface"),
        ("uml.class.rename", json!({ "class": payable, "name": "X" }), "class", "not a class"),
        ("uml.interface.move", json!({ "interface": payable, "package": order }), "package", "not a package"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains(key) && e.contains(why), "{cmd}: {e}");
    }
    // Moved to a package whose class has its name: refused; to another: one step.
    let clash = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Billable" }));
    assert!(refused(&mut s, "uml.interface.move", json!({ "interface": payable, "package": billing })).contains("class named Billable"));
    run(&mut s, "uml.class.rename", json!({ "class": clash, "name": "Invoice" }));
    one_step(&mut s, "uml.interface.move", json!({ "interface": payable, "package": billing }));
    let st = s.doc();
    assert_eq!(archforge_uml::commands::package_of(st, obj(st, &payable)), Some(obj(st, &billing)));
    // Removed through element.remove, with its views: one step.
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": billing }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": payable, "x": 0, "y": 0 }));
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [payable] })).unwrap();
    let st = s.doc();
    assert!(st.by_uuid(Uuid::parse(&payable).unwrap()).is_none_or(|o| !st.is_alive(o)));
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_diagram::NODE)), "its card went with it");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// An interface's members are public, go through the member commands and show on its card; it is a type.
#[test]
fn an_interface_takes_public_attributes_and_operations_through_the_member_commands_and_shows_them_on_its_card() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let money = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Money" }));
    let due = one_step(&mut s, "uml.attribute.add", json!({ "class": payable, "name": "due", "type": money }));
    let pay = one_step(
        &mut s,
        "uml.operation.add",
        json!({ "class": payable, "name": "pay", "parameters": [{ "name": "amount", "type": money }], "returns": "Boolean" }),
    );
    one_step(&mut s, "uml.operation.add", json!({ "class": payable, "name": "refund", "returns": "Boolean" }));
    one_step(&mut s, "uml.operation.move", json!({ "operation": pay, "at": 1 }));
    one_step(&mut s, "uml.attribute.set", json!({ "attribute": due, "name": "dueOn", "type": money }));
    let st = s.doc();
    let i = obj(st, &payable);
    assert_eq!(archforge_uml::members::attributes(st, i), vec![obj(st, &due)]);
    assert_eq!(archforge_uml::members::operations(st, i).len(), 2);
    let got = s.execute("uml.class.get", &json!({ "class": payable })).unwrap().value;
    assert_eq!((got["kind"].as_str(), got["abstract"].as_bool()), (Some("interface"), Some(false)));
    let ops: Vec<&str> = got["operations"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap()).collect();
    assert_eq!(ops, ["refund", "pay"]);
    // An interface's members are public; its attribute names are its own.
    for (cmd, p, key) in [
        ("uml.attribute.add", json!({ "class": payable, "name": "secret", "visibility": "private" }), "visibility"),
        ("uml.operation.set", json!({ "operation": pay, "name": "pay", "visibility": "protected" }), "visibility"),
        ("uml.attribute.add", json!({ "class": payable, "name": "DUEON" }), "name"),
        ("uml.class.set", json!({ "class": payable, "abstract": true }), "class"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains(key), "{cmd}: {e}");
    }
    assert!(refused(&mut s, "uml.attribute.add", json!({ "class": payable, "name": "x", "visibility": "private" })).contains("public"));
    // Its card: «interface» in the eyebrow, its attributes, then its operations.
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": payable, "x": 0, "y": 0 }));
    let st = s.doc();
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, &payable), obj(st, &d)).unwrap();
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::said("«interface»"));
    let rows: Vec<Vec<&str>> = card.compartments.iter().map(|c| c.iter().map(|r| r.text.as_str()).collect()).collect();
    assert_eq!(rows, [vec!["+ dueOn : Money"], vec!["+ refund() : Boolean", "+ pay(amount : Money) : Boolean"]]);
    assert!(!card.slanted);
    // On another package's diagram, its package is named beside the keyword.
    let other = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": other }));
    run(&mut s, "diagram.place", json!({ "diagram": d2, "element": payable, "x": 0, "y": 0 }));
    let st = s.doc();
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, &payable), obj(st, &d2)).unwrap();
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Said("«interface» · {package}", vec![("package", "sales".into())]));
    // It is a type a member may have.
    let types = s.execute("uml.type.list", &json!({})).unwrap().value;
    assert!(types["types"].as_array().unwrap().iter().any(|t| t["id"] == payable.as_str()));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "payment", "type": payable }));
    let got = s.execute("uml.class.get", &json!({ "class": order })).unwrap().value;
    assert_eq!(got["attributes"][0]["type"]["name"], "Payable");
}

/// Associations join interfaces; generalization follows the classifier rules, refusals change nothing.
#[test]
fn interfaces_associate_with_anything_and_specialize_only_interfaces_a_refusal_saying_why_and_changing_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let refundable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Refundable" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Invoice" }));
    // Associations and dependencies between interfaces, and with classes.
    one_step(&mut s, "uml.association.add", json!({ "from": payable, "to": refundable, "kind": "directed" }));
    one_step(&mut s, "uml.association.add", json!({ "from": order, "to": payable }));
    one_step(&mut s, "uml.dependency.add", json!({ "client": refundable, "supplier": order }));
    // An interface specializes an interface.
    let g = one_step(&mut s, "uml.generalization.add", json!({ "specific": refundable, "general": payable }));
    let related = s.execute("uml.relationship.list", &json!({ "class": payable })).unwrap().value;
    assert_eq!(related["relationships"].as_array().unwrap().len(), 3, "{related}");
    // What would make the model wrong is refused, saying why.
    for (p, why) in [
        (json!({ "specific": payable, "general": order }), "an interface specializes only an interface"),
        (json!({ "specific": order, "general": payable }), "a class cannot specialize an interface"),
        (json!({ "specific": payable, "general": payable }), "an interface cannot specialize itself"),
        (json!({ "specific": payable, "general": refundable }), "cycle"),
        (json!({ "specific": refundable, "general": payable }), "specializes that interface already"),
    ] {
        let e = refused(&mut s, "uml.generalization.add", p.clone());
        assert!(e.contains(why), "{p}: {e}");
    }
    // A refusal between kinds names the class that breaks the rule, under its own key.
    for (p, key) in [(json!({ "specific": order, "general": payable }), "specific"), (json!({ "specific": payable, "general": order }), "general")] {
        let e = refused(&mut s, "uml.generalization.add", p.clone());
        assert!(e.contains(order.as_str()) && e.contains(&format!("key: Some(\"{key}\")")), "{p}: {e}");
    }
    // A reconnected end keeps the same rules.
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "target", "class": invoice }));
    assert!(e.contains("an interface specializes only an interface"), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "source", "class": invoice }));
    assert!(e.contains("a class cannot specialize an interface"), "{e}");
    // Removing an interface takes the relationships that touch it along, in one step.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [payable] })).unwrap();
    let st = s.doc();
    let lines = [archforge_uml::ASSOCIATION, archforge_uml::GENERALIZATION];
    assert_eq!(st.alive().filter(|o| st.class_of(*o).is_ok_and(|c| lines.contains(&c))).count(), 0);
    assert_eq!(st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::DEPENDENCY)).count(), 1, "the dependency stays");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// Interfaces with their members, relationships and card come back from their rows.
#[test]
fn interfaces_with_members_and_relationships_come_back_from_their_rows_identical() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let refundable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Refundable" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": payable, "name": "due", "type": "Integer", "multiplicity": "0..1" }));
    run(&mut s, "uml.operation.add", json!({ "class": payable, "name": "pay", "parameters": [{ "name": "amount", "type": "Real" }] }));
    run(&mut s, "uml.operation.add", json!({ "class": payable, "name": "refund", "returns": "Boolean" }));
    run(&mut s, "uml.generalization.add", json!({ "specific": refundable, "general": payable }));
    run(&mut s, "uml.association.add", json!({ "from": order, "to": payable, "kind": "directed" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": payable, "x": 0, "y": 0 }));
    let tables: Vec<_> = archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect();
    let junctions: Vec<_> = archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let i = obj(&back, &payable);
    assert_eq!(back.class_of(i).unwrap(), INTERFACE);
    let names = |os: Vec<Obj>| os.into_iter().map(|o| archforge_core::name_of(&back, o)).collect::<Vec<_>>();
    assert_eq!(names(archforge_uml::members::attributes(&back, i)), ["due"]);
    assert_eq!(names(archforge_uml::members::operations(&back, i)), ["pay", "refund"], "in their order");
    assert_eq!(names(archforge_uml::commands::interfaces(&back, obj(&back, &pkg))), ["Payable", "Refundable"]);
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, &back, i, obj(&back, &d)).unwrap();
    assert_eq!(card.compartments.iter().map(Vec::len).collect::<Vec<_>>(), [1, 2]);
}

/// The command sweep, with an interface and its attribute among the ids, breaks nothing.
#[test]
fn the_sweep_over_interfaces_breaks_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let attribute = run(&mut s, "uml.attribute.add", json!({ "class": payable, "name": "due" }));
    let class = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let ids = [pkg.as_str(), payable.as_str(), attribute.as_str(), class.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}
