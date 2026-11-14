//! UML's commands through the engine over the shared core, the diagrams and UML:
//! packages and classes added, renamed, listed and refused by key; a class diagram of a package
//! showing its own classes; removing a package takes its classes and their views; every command
//! undoes and redoes; the sweep.

use archforge_diagram::{DiagramView, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{EngineError, Options, Reason, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::{ALONE, CLASS_DIAGRAM, PACKAGE, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

fn ids() -> Box<dyn FnMut() -> Uuid + Send> {
    ids_from(0)
}

/// Ids counted up from `start`, so two stores never share one.
fn ids_from(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// A store over the core, the diagrams and UML, with the project's Model.
fn store() -> Store {
    let mut st = empty_store(ids());
    let model = st.create(archforge_core::MODEL).unwrap();
    st.set(model, archforge_core::named_element::NAME, Value::str("Shop")).unwrap();
    st
}

/// A store over the core, the diagrams and UML, holding nothing, whose new ids come from `ids`.
fn empty_store(ids: Box<dyn FnMut() -> Uuid + Send>) -> Store {
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

/// The typed reason `cmd` is refused with, if it has one.
fn reason(s: &mut S, cmd: &str, p: Json) -> Option<Reason> {
    let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
    error.reason
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

/// The description of the element `id`, as Properties and the reads give it.
fn description(s: &S, id: &str) -> String {
    archforge_uml::descriptions::description_of(s.doc(), obj(s.doc(), id))
}

/// How many Comments the model holds.
fn comments(st: &Store) -> usize {
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::COMMENT)).count()
}

/// `element.describe` of `id` with `text` as one undo step: undone the model is as it was, redone
/// as the command made it.
fn describe_one_step(s: &mut S, id: &str, text: &str) {
    let before = picture(s.doc());
    run(s, "element.describe", json!({ "element": id, "text": text }));
    let after = picture(s.doc());
    assert_ne!(before, after, "describing {id} with {text:?} changed the model");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "undo of describing {id} with {text:?}");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after, "redo of describing {id} with {text:?}");
}

/// A package with a class, an interface, an enumeration with a literal, a data type, a primitive
/// type, an attribute, an operation, an association, a generalization, a dependency, a realization
/// and an association class: (package, every element's id).
fn described_sample(s: &mut S) -> (String, Vec<String>) {
    let pkg = run(s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let party = run(s, "uml.class.add", json!({ "package": pkg, "name": "Party" }));
    let priced = run(s, "uml.interface.add", json!({ "package": pkg, "name": "Priced" }));
    let status = run(s, "uml.enumeration.add", json!({ "package": pkg, "name": "Status" }));
    let open = run(s, "uml.literal.add", json!({ "enumeration": status, "name": "open" }));
    let money = run(s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let sku = run(s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Sku" }));
    let total = run(s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    let pay = run(s, "uml.operation.add", json!({ "class": order, "name": "pay" }));
    let places = run(s, "uml.association.add", json!({ "from": party, "to": order }));
    let customer = run(s, "uml.class.add", json!({ "package": pkg, "name": "Customer" }));
    let is_a = run(s, "uml.generalization.add", json!({ "specific": customer, "general": party }));
    let uses = run(s, "uml.dependency.add", json!({ "client": order, "supplier": money }));
    let realizes = run(s, "uml.realization.add", json!({ "class": order, "interface": priced }));
    let line = run(s, "uml.association-class.add", json!({ "from": order, "to": party, "name": "Line" }));
    (pkg.clone(), vec![pkg, order, priced, status, open, money, sku, total, pay, places, is_a, uses, realizes, line])
}

/// Every element Properties describes takes a description kept as a Comment it owns (AR-186):
/// described, changed and cleared, each one undo step that undoes and redoes; one Comment per
/// element, gone when cleared; the reads give it back.
#[test]
fn a_uml_element_is_described_changed_and_cleared_as_its_own_comment_each_one_undo_step() {
    let mut s = session();
    let (pkg, all) = described_sample(&mut s);
    for id in &all {
        describe_one_step(&mut s, id, "What it is.");
        assert_eq!(description(&s, id), "What it is.", "{id}");
        let st = s.doc();
        let o = obj(st, id);
        let c = archforge_uml::descriptions::description_comment(st, o).expect("a description comment");
        assert_eq!(st.container(c).unwrap(), Some((o, archforge_uml::element::OWNED_COMMENT)), "the element owns it");
        assert_eq!(st.get(c, archforge_uml::comment::BODY).unwrap(), Value::str("What it is."), "its body is the description");
        describe_one_step(&mut s, id, "What it is,\nin two lines.");
        assert_eq!(description(&s, id), "What it is,\nin two lines.");
        assert_eq!(st_comments_of(&s, id), 1, "changed in place, never a second comment");
    }
    assert_eq!(comments(s.doc()), all.len());
    // The reads give the descriptions back, for the agents.
    let order = &all[1];
    let class = s.execute("uml.class.get", &json!({ "class": order })).unwrap().value;
    assert_eq!(class["description"], "What it is,\nin two lines.");
    assert_eq!(class["attributes"][0]["description"], "What it is,\nin two lines.");
    assert_eq!(class["operations"][0]["description"], "What it is,\nin two lines.");
    let status = s.execute("uml.class.get", &json!({ "class": all[3] })).unwrap().value;
    assert_eq!(status["literals"][0]["description"], "What it is,\nin two lines.");
    let packages = s.execute("uml.package.list", &json!({})).unwrap().value;
    let listed = packages["packages"].as_array().unwrap().iter().find(|p| p["id"] == pkg.as_str()).unwrap().clone();
    assert_eq!(listed["description"], "What it is,\nin two lines.");
    let relations = s.execute("uml.relationship.list", &json!({ "class": order })).unwrap().value;
    assert!(relations["relationships"].as_array().unwrap().iter().all(|r| r["description"] == "What it is,\nin two lines."), "{relations}");
    // Cleared: the comment goes, as one undo step; clearing again changes nothing.
    for id in &all {
        describe_one_step(&mut s, id, "");
        assert_eq!(description(&s, id), "");
        assert_eq!(st_comments_of(&s, id), 0);
    }
    assert_eq!(comments(s.doc()), 0);
    let before = picture(s.doc());
    run(&mut s, "element.describe", json!({ "element": all[1], "text": "" }));
    assert_eq!(picture(s.doc()), before);
}

/// How many Comments the element `id` owns.
fn st_comments_of(s: &S, id: &str) -> usize {
    s.doc().list(obj(s.doc(), id), archforge_uml::element::OWNED_COMMENT).unwrap().len()
}

/// A description that is no description, and an element that takes none, are refused naming the
/// value and what is accepted, and change nothing.
#[test]
fn a_description_that_does_not_fit_is_refused_naming_the_value_and_the_accepted_form() {
    let mut s = session();
    let (_, all) = described_sample(&mut s);
    let order = all[1].clone();
    run(&mut s, "element.describe", json!({ "element": order, "text": "Kept." }));
    let long = "x".repeat(archforge_uml::descriptions::MAX_DESCRIPTION + 1);
    let fits = [
        (" padded ", "space", Reason::EdgeSpace),
        ("a\u{7}b", "control", Reason::ControlCharacter),
        (long.as_str(), "longer than 4096", Reason::TooLong { max: 4096 }),
    ];
    for (text, says, typed) in fits {
        let e = refused_key(&mut s, "element.describe", json!({ "element": order, "text": text }));
        assert!(e.contains("text") && e.contains(says) && e.contains("up to 4096 characters"), "{e}");
        assert_eq!(reason(&mut s, "element.describe", json!({ "element": order, "text": text })), Some(typed), "{says}");
    }
    let at_most = "y".repeat(archforge_uml::descriptions::MAX_DESCRIPTION);
    run(&mut s, "element.describe", json!({ "element": order, "text": at_most }));
    // A comment, a parameter and an association's end take none (an end goes with its
    // association, and Properties never shows one); the refusal names the id and the kinds that do.
    let c = archforge_uml::descriptions::description_comment(s.doc(), obj(s.doc(), &order)).unwrap();
    let comment = s.doc().uuid(c).unwrap().to_string();
    let op = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "refund", "parameters": [{ "name": "amount", "type": "Real" }] }));
    let st = s.doc();
    let parameter = st.uuid(archforge_uml::members::parameters(st, obj(st, &op)).0[0]).unwrap().to_string();
    let places = obj(st, &all[9]);
    let end = st.list(places, archforge_uml::association::MEMBER_END).unwrap()[0].as_ref().unwrap();
    let end = st.uuid(end).unwrap().to_string();
    for id in [&comment, &parameter, &end] {
        let e = refused_key(&mut s, "element.describe", json!({ "element": id, "text": "About it" }));
        assert!(e.contains(id.as_str()) && e.contains("takes no description") && e.contains("a package, a class"), "{e}");
        assert!(!e.contains("association end"), "an end is not offered: {e}");
        assert_eq!(reason(&mut s, "element.describe", json!({ "element": id, "text": "About it" })), Some(Reason::TakesNoDescription));
    }
}

/// Adds to `owner`, in `st`, a Comment with `body` annotating `about`, as a model read from a file
/// may hold it; gives the Comment.
fn add_comment(st: &mut Store, owner: Obj, body: &str, about: &[Obj]) -> Obj {
    let c = st.create(archforge_uml::COMMENT).unwrap();
    st.set(c, archforge_uml::comment::BODY, Value::str(body)).unwrap();
    st.push(owner, archforge_uml::element::OWNED_COMMENT, Value::Ref(c)).unwrap();
    for a in about {
        st.push(c, archforge_uml::comment::ANNOTATED_ELEMENT, Value::Ref(*a)).unwrap();
    }
    c
}

/// A model from a file whose element owns two Comments of a description's shape: the first is the
/// description; setting changes it and removes the other, clearing removes both, each one undo
/// step, so one description is left, or none.
#[test]
fn two_comments_of_a_descriptions_shape_leave_one_when_set_and_none_when_cleared() {
    let mut s = session();
    let (_, all) = described_sample(&mut s);
    let order = all[1].clone();
    let mut st = copy(s.doc());
    let o = obj(&st, &order);
    let other = add_comment(&mut st, o, "An aside.", &[]);
    let first = add_comment(&mut st, o, "First.", &[o]);
    let second = add_comment(&mut st, o, "Second.", &[o]);
    let mut s = Session::with_view(registry().unwrap(), st, 1_000, Options::default(), ProjectView::new(ALONE));
    assert_eq!(description(&s, &order), "First.", "the first of its shape");
    describe_one_step(&mut s, &order, "Changed.");
    let st = s.doc();
    assert_eq!(description(&s, &order), "Changed.");
    assert!(st.is_alive(first) && !st.is_alive(second), "the first changed, the second gone");
    assert!(st.is_alive(other), "another comment is left be");
    s.undo().unwrap();
    describe_one_step(&mut s, &order, "");
    let st = s.doc();
    assert_eq!(description(&s, &order), "", "empty clears it, and no second one shows up");
    assert!(!st.is_alive(first) && !st.is_alive(second) && st.is_alive(other));
}

/// An element owning 64 comments or more, as only a file holds them, is neither described nor
/// cleared: its description could sit past the comments looked through. The refusal says so and
/// changes nothing.
#[test]
fn an_element_owning_the_most_comments_is_neither_described_nor_cleared() {
    let mut s = session();
    let (_, all) = described_sample(&mut s);
    let order = all[1].clone();
    let mut st = copy(s.doc());
    let o = obj(&st, &order);
    for i in 0..archforge_uml::descriptions::MAX_COMMENTS {
        add_comment(&mut st, o, &format!("Aside {i}."), &[]);
    }
    let mut s = Session::with_view(registry().unwrap(), st, 1_000, Options::default(), ProjectView::new(ALONE));
    for text in ["New.", ""] {
        let e = refused_key(&mut s, "element.describe", json!({ "element": order, "text": text }));
        assert!(e.contains(&order) && e.contains("64 or more comments") && e.contains("file"), "{e}");
        assert_eq!(reason(&mut s, "element.describe", json!({ "element": order, "text": text })), Some(Reason::TooManyComments { max: 64 }));
    }
    // One fewer, and it is described again.
    let mut st = copy(s.doc());
    let o = obj(&st, &order);
    let last = st.list(o, archforge_uml::element::OWNED_COMMENT).unwrap().last().unwrap().as_ref().unwrap();
    st.destroy(last).unwrap();
    let mut s = Session::with_view(registry().unwrap(), st, 2_000, Options::default(), ProjectView::new(ALONE));
    describe_one_step(&mut s, &order, "New.");
    assert_eq!(description(&s, &order), "New.");
}

/// A built-in type takes a description as the model's own types do; an association's end reads
/// none, even with a Comment of a description's shape from a file, as describing refuses it.
#[test]
fn a_built_in_type_is_described_and_an_end_reads_none() {
    let mut s = session();
    let (_, all) = described_sample(&mut s);
    run(&mut s, "uml.attribute.add", json!({ "class": all[1], "name": "count", "type": "Integer" }));
    let st = s.doc();
    let integer = st.alive().find(|o| archforge_uml::members::is_built_in(st, *o) && archforge_core::name_of(st, *o) == "Integer").unwrap();
    let integer = st.uuid(integer).unwrap().to_string();
    describe_one_step(&mut s, &integer, "A whole number.");
    assert_eq!(description(&s, &integer), "A whole number.");
    let class = s.execute("uml.class.get", &json!({ "class": integer })).unwrap().value;
    assert_eq!(class["description"], "A whole number.");
    describe_one_step(&mut s, &integer, "");
    let mut st = copy(s.doc());
    let places = obj(&st, &all[9]);
    let end = st.list(places, archforge_uml::association::MEMBER_END).unwrap()[0].as_ref().unwrap();
    add_comment(&mut st, end, "An end's.", &[end]);
    assert_eq!(archforge_uml::descriptions::description_of(&st, end), "", "read and describe agree: an end takes none");
    assert!(archforge_uml::descriptions::description_comment(&st, end).is_none());
}

/// The storage's tables and junctions: the core's, the diagrams' and UML's.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    let tables = archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect();
    let junctions = archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect();
    (tables, junctions)
}

/// `st` saved as rows and read back into a new store, as a project is saved and reopened; each
/// copy mints its new ids from a range of its own.
fn copy(st: &Store) -> Store {
    static COPIES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(st, &tables, &junctions).unwrap();
    let start = COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) * 1_000_000;
    let mut back = empty_store(ids_from(start));
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(archforge_mof::rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    back
}

/// Saved as rows and read back, the descriptions are there, identical.
#[test]
fn descriptions_are_saved_and_read_back() {
    let mut s = session();
    let (_, all) = described_sample(&mut s);
    for (i, id) in all.iter().enumerate() {
        run(&mut s, "element.describe", json!({ "element": id, "text": format!("Element {i}.") }));
    }
    let back = copy(s.doc());
    for (i, id) in all.iter().enumerate() {
        let o = back.by_uuid(Uuid::parse(id).unwrap()).unwrap();
        assert_eq!(archforge_uml::descriptions::description_of(&back, o), format!("Element {i}."), "{id}");
    }
}

/// Removing an element takes its description Comment along, and what it holds takes theirs; a
/// nested package's description goes with it while its class, handed up, keeps its own. Undo
/// brings them back.
#[test]
fn removing_an_element_takes_its_description_along() {
    let mut s = session();
    let (pkg, all) = described_sample(&mut s);
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": pkg }));
    let kept = run(&mut s, "uml.class.add", json!({ "package": inner, "name": "Kept" }));
    for id in all.iter().chain([&inner, &kept]) {
        run(&mut s, "element.describe", json!({ "element": id, "text": "Described." }));
    }
    let (order, total) = (all[1].clone(), all[7].clone());
    let before = picture(s.doc());
    let held = comments(s.doc());
    // The attribute alone: its comment goes, the class keeps its own.
    run(&mut s, "element.remove", json!({ "ids": [total] }));
    assert_eq!(comments(s.doc()), held - 1);
    assert_eq!(description(&s, &order), "Described.");
    // The class: its comment, its operation's and those of the relationships that went with it.
    run(&mut s, "element.remove", json!({ "ids": [order] }));
    let st = s.doc();
    let orphans = st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::COMMENT)).filter(|c| st.container(*c).unwrap().is_none()).count();
    assert_eq!(orphans, 0, "no comment is left without its element");
    // A nested package: its comment goes, what it hands up keeps its own; twice, the package
    // inner within sales, then sales within the root.
    for (package, held) in [(&inner, &kept), (&pkg, &all[2])] {
        let had = comments(s.doc());
        run(&mut s, "element.remove", json!({ "ids": [package] }));
        assert_eq!(comments(s.doc()), had - 1, "the package's own comment went");
        assert_eq!(description(&s, held), "Described.", "what it handed up kept its description");
        let st = s.doc();
        assert!(
            st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::COMMENT)).all(|c| {
                st.container(c).unwrap().is_some_and(|(owner, _)| archforge_uml::descriptions::description_comment(st, owner) == Some(c))
            })
        );
    }
    for _ in 0..4 {
        s.undo().unwrap();
    }
    assert_eq!(picture(s.doc()), before, "undone, every description is back");
}

/// A note on a diagram is the diagram's own node, never a Comment, so it is never taken for a
/// description; and a Comment an element owns that is about something else, or about nothing, is
/// never its description: describing makes its own beside it, changing and clearing leave it be.
#[test]
fn a_note_or_another_comment_is_never_taken_for_the_description() {
    let mut s = session();
    let (pkg, all) = described_sample(&mut s);
    let (order, status) = (all[1].clone(), obj(s.doc(), &all[3]));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    run(&mut s, "diagram.note", json!({ "diagram": d, "text": "Ships in May." }));
    assert_eq!(comments(s.doc()), 0, "a note is no comment");
    assert_eq!(description(&s, &order), "", "nor a description");
    // Two other comments the class owns, ahead of any description: one about another element,
    // one about nothing.
    let mut st = copy(s.doc());
    let o = obj(&st, &order);
    let about_other = st.create(archforge_uml::COMMENT).unwrap();
    st.set(about_other, archforge_uml::comment::BODY, Value::str("See Status.")).unwrap();
    st.push(o, archforge_uml::element::OWNED_COMMENT, Value::Ref(about_other)).unwrap();
    st.push(about_other, archforge_uml::comment::ANNOTATED_ELEMENT, Value::Ref(status)).unwrap();
    let loose = st.create(archforge_uml::COMMENT).unwrap();
    st.set(loose, archforge_uml::comment::BODY, Value::str("A loose remark.")).unwrap();
    st.push(o, archforge_uml::element::OWNED_COMMENT, Value::Ref(loose)).unwrap();
    let mut s = Session::with_view(registry().unwrap(), st, 1_000, Options::default(), ProjectView::new(ALONE));
    assert_eq!(description(&s, &order), "", "neither is the description");
    run(&mut s, "element.describe", json!({ "element": order, "text": "One purchase." }));
    run(&mut s, "element.describe", json!({ "element": order, "text": "One purchase by a party." }));
    assert_eq!(description(&s, &order), "One purchase by a party.");
    let st = s.doc();
    let o = obj(st, &order);
    assert_eq!(st.list(o, archforge_uml::element::OWNED_COMMENT).unwrap().len(), 3);
    for (c, body) in [(about_other, "See Status."), (loose, "A loose remark.")] {
        assert_eq!(st.get(c, archforge_uml::comment::BODY).unwrap(), Value::str(body), "left be");
    }
    run(&mut s, "element.describe", json!({ "element": order, "text": "" }));
    let st = s.doc();
    assert_eq!(description(&s, &order), "");
    assert!(st.is_alive(about_other) && st.is_alive(loose), "clearing takes the description alone");
    assert_eq!(st.list(obj(st, &order), archforge_uml::element::OWNED_COMMENT).unwrap().len(), 2);
}

/// The command sweep, with described elements and a description comment among the ids, breaks
/// nothing.
#[test]
fn the_sweep_over_descriptions_breaks_nothing() {
    let mut s = session();
    let (pkg, all) = described_sample(&mut s);
    for id in &all {
        run(&mut s, "element.describe", json!({ "element": id, "text": "Described." }));
    }
    let st = s.doc();
    let comment = st.uuid(archforge_uml::descriptions::description_comment(st, obj(st, &all[1])).unwrap()).unwrap().to_string();
    let mut ids: Vec<&str> = vec![pkg.as_str(), all[1].as_str(), all[7].as_str(), all[4].as_str(), all[9].as_str(), comment.as_str(), "not-an-id"];
    ids.dedup();
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}
