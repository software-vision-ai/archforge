//! Nested classifiers through the engine (AR-132): made inside a class, moved between a package
//! and a classifier keeping their ids, members, relationships and the types that use them, each one
//! undo step; cycles, kinds that own none or are never nested, and name clashes refused with the
//! reason, changing nothing; an owner deleted with what it nests, their relationships and views,
//! in one step; the ⊕ tie and the qualified name on a diagram; saved as rows and read back
//! identical; a model of package members alone unchanged; the sweep.

use archforge_diagram::{Notation, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::nesting::{self, OwnerProblem};
use archforge_uml::{ALONE, CLASS_DIAGRAM, UmlNotation, registry};
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

/// The storage's tables and junctions: the core's, the diagrams' and UML's.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    let tables = archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect();
    let junctions = archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect();
    (tables, junctions)
}

fn session_over(st: Store) -> S {
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// A session over a store with the project's Model, named Shop.
fn session() -> S {
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    st.set(model, archforge_core::named_element::NAME, Value::str("Shop")).unwrap();
    session_over(st)
}

fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// The refusal of `cmd`, which changes nothing.
fn refused(s: &mut S, cmd: &str, p: Json) -> String {
    let before = picture(s.doc());
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was not refused"));
    assert_eq!(picture(s.doc()), before, "{cmd} changed nothing");
    format!("{e}")
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

/// `cmd` changes the model as one undo step.
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

/// A package `sales` with the classes Order and Customer, the enumeration Status and the primitive
/// type Money, and a class diagram of `sales` showing Order: (sales, order, customer, status,
/// money, diagram).
fn shop(s: &mut S) -> [String; 6] {
    let sales = run(s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let customer = run(s, "uml.class.add", json!({ "package": sales, "name": "Customer" }));
    let status = run(s, "uml.enumeration.add", json!({ "package": sales, "name": "Status" }));
    let money = run(s, "uml.primitive-type.add", json!({ "package": sales, "name": "Money" }));
    let d = run(s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Sales", "element": sales }));
    run(s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    [sales, order, customer, status, money, d]
}

fn names(st: &Store, os: Vec<Obj>) -> Vec<String> {
    os.into_iter().map(|o| archforge_core::name_of(st, o)).collect()
}

/// A classifier of each nestable kind is made inside a class, one undo step each, selected, its
/// owner the class and its home package the class's; the kinds that own none, and an unknown kind,
/// are refused.
#[test]
fn nested_classifiers_are_made_inside_a_class_one_step_each() {
    let mut s = session();
    let [sales, order, _, status, money, _] = shop(&mut s);
    one_step(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "enumeration", "name": "Priority" }));
    s.undo().unwrap();
    let priority = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "enumeration", "name": "Priority" }));
    assert_eq!(s.view().selection.len(), 1, "it is selected");
    for (kind, name) in [("class", "Line"), ("interface", "Priced"), ("data-type", "Amount")] {
        one_step(&mut s, "uml.nested.add", json!({ "owner": order, "kind": kind, "name": name }));
    }
    let st = s.doc();
    let (o, p) = (obj(st, &order), obj(st, &priority));
    assert_eq!(names(st, nesting::nested(st, o)), ["Priority", "Line", "Priced", "Amount"]);
    assert_eq!(nesting::owner_of(st, p), Some(o));
    assert_eq!(nesting::home_package(st, p), Some(obj(st, &sales)));
    assert_eq!(nesting::qualified_name(st, p), "Order::Priority");
    assert!(archforge_uml::commands::classifiers(st, obj(st, &sales)).iter().all(|c| *c != p), "it is no member of the package");
    // Nested deeper, the path names every owner.
    let inner = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Inner" }));
    let deep = run(&mut s, "uml.nested.add", json!({ "owner": inner, "kind": "enumeration", "name": "Deep" }));
    assert_eq!(nesting::qualified_name(s.doc(), obj(s.doc(), &deep)), "Order::Inner::Deep");
    assert!(refused(&mut s, "uml.nested.add", json!({ "owner": status, "kind": "class", "name": "X" })).contains("owns no classifiers"));
    assert!(refused(&mut s, "uml.nested.add", json!({ "owner": money, "kind": "class", "name": "X" })).contains("owns no classifiers"));
    assert!(refused(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "primitive-type", "name": "X" })).contains("kind"));
    assert!(refused(&mut s, "uml.nested.add", json!({ "owner": sales, "kind": "class", "name": "X" })).contains("not a class"));
    let got = s.execute("uml.class.get", &json!({ "class": priority })).unwrap().value;
    assert_eq!((got["owner"]["name"].as_str(), got["owner"]["kind"].as_str()), (Some("Order"), Some("class")));
    assert_eq!(s.execute("uml.class.get", &json!({ "class": order })).unwrap().value["nested"].as_array().map(Vec::len), Some(5));
    let listed = s.execute("uml.nested.list", &json!({ "owner": order })).unwrap().value;
    let kinds: Vec<&str> = listed["nested"].as_array().unwrap().iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["enumeration", "class", "interface", "data_type", "class"]);
    assert_eq!(listed["nested"][4]["nested"][0]["name"], "Deep");
}

/// A nested classifier's name is told apart from the owner's other classifiers, whatever the case,
/// made, renamed or moved there; the package's classifiers' names, a name nested elsewhere, and the
/// owner's attributes and operations, which are other kinds (AR-208), do not clash.
#[test]
fn names_clash_only_among_the_classifiers_an_owner_holds() {
    let mut s = session();
    let [sales, order, customer, ..] = shop(&mut s);
    let priority = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "enumeration", "name": "Priority" }));
    let why = refused(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "priority" }));
    assert!(why.contains("an enumeration named priority already"), "{why}");
    // Customer is a package member: a nested Customer is told apart by its owner.
    run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Customer" }));
    run(&mut s, "uml.nested.add", json!({ "owner": customer, "kind": "enumeration", "name": "Priority" }));
    let line = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Line" }));
    assert!(refused(&mut s, "uml.class.rename", json!({ "class": line, "name": "PRIORITY" })).contains("an enumeration named PRIORITY already"));
    let loose = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "LINE" }));
    assert!(refused(&mut s, "uml.classifier.set-owner", json!({ "classifier": loose, "owner": order })).contains("a class named Line already"));
    one_step(&mut s, "uml.enumeration.rename", json!({ "enumeration": priority, "name": "Urgency" }));
    // An owner's attributes and operations are other kinds: they share its nested classifiers'
    // names, each way round, and the model opens again, as D2's `priority : Priority` has it.
    one_step(&mut s, "uml.attribute.add", json!({ "class": order, "name": "urgency", "type": priority }));
    one_step(&mut s, "uml.operation.add", json!({ "class": order, "name": "LINE" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    one_step(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Total" }));
    one_step(&mut s, "uml.classifier.set-owner", json!({ "classifier": loose, "owner": customer }));
    run(&mut s, "uml.attribute.add", json!({ "class": customer, "name": "line" }));
    assert_eq!(s.doc().first_problem_by(archforge_uml::names::kind), None, "the model holds together as it would open");
}

/// A classifier moves from its package into a class, from one class to another and back to a
/// package, one undo step each: it keeps its id, its literals, the attribute it types and the
/// relationship that touches it, and its card stays on the diagram.
#[test]
fn changing_the_owner_keeps_identity_members_types_and_relationships() {
    let mut s = session();
    let [sales, order, customer, status, _, d] = shop(&mut s);
    run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Paid" }));
    let typed = run(&mut s, "uml.attribute.add", json!({ "class": customer, "name": "state", "type": status }));
    let helper = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Helper" }));
    let g = run(&mut s, "uml.generalization.add", json!({ "specific": helper, "general": order }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": status, "x": 300, "y": 0 }));
    let before_rows = |s: &S| {
        let st = s.doc();
        (
            archforge_uml::commands::literals(st, obj(st, &status)).len(),
            st.get(obj(st, &typed), archforge_uml::typed_element::TYPE).unwrap().as_ref(),
            st.is_alive(obj(st, &g)),
            st.is_alive(obj(st, &node)),
        )
    };
    let kept = before_rows(&s);
    for owner in [&order, &customer, &sales] {
        let id = one_step(&mut s, "uml.classifier.set-owner", json!({ "classifier": status, "owner": owner }));
        assert_eq!(id, status, "the same id");
        assert_eq!(before_rows(&s), kept, "literals, the type it gives, its card");
        assert_eq!(nesting::owner_of(s.doc(), obj(s.doc(), &status)), Some(obj(s.doc(), owner)));
    }
    // A class with a generalization goes into another class with it.
    one_step(&mut s, "uml.classifier.set-owner", json!({ "classifier": helper, "owner": customer }));
    assert!(s.doc().is_alive(obj(s.doc(), &g)));
    let rel = s.execute("uml.relationship.list", &json!({ "class": helper })).unwrap().value;
    assert_eq!(rel["relationships"].as_array().map(Vec::len), Some(1), "{rel}");
    // The existing package moves keep working, a nested classifier among them.
    run(&mut s, "uml.classifier.set-owner", json!({ "classifier": status, "owner": order }));
    let other = run(&mut s, "uml.package.add", json!({ "name": "other" }));
    one_step(&mut s, "uml.enumeration.move", json!({ "enumeration": status, "package": other }));
    assert_eq!(nesting::owner_of(s.doc(), obj(s.doc(), &status)), Some(obj(s.doc(), &other)));
}

/// Itself, one nested in it (a cycle), a kind that owns none, a kind never nested, a name the owner
/// has and a built-in type are each refused with the reason, changing nothing; the candidates leave
/// those nested in it out and give the others' reasons.
#[test]
fn cycles_kinds_and_name_clashes_are_refused_with_the_reason() {
    let mut s = session();
    let [sales, order, customer, status, money, _] = shop(&mut s);
    let inner = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Inner" }));
    let deep = run(&mut s, "uml.nested.add", json!({ "owner": inner, "kind": "class", "name": "Deep" }));
    run(&mut s, "uml.nested.add", json!({ "owner": customer, "kind": "class", "name": "Status" }));
    let set = |c: &str, o: &str| json!({ "classifier": c, "owner": o });
    assert!(refused(&mut s, "uml.classifier.set-owner", set(&order, &order)).contains("cannot own itself"));
    let cycle = refused(&mut s, "uml.classifier.set-owner", set(&order, &deep));
    assert!(cycle.contains("would make a cycle"), "{cycle}");
    let none = refused(&mut s, "uml.classifier.set-owner", set(&customer, &status));
    assert!(none.contains("owns no classifiers; a package, a class, an interface or an association class does"), "{none}");
    assert!(refused(&mut s, "uml.classifier.set-owner", set(&money, &order)).contains("stays in a package"));
    assert!(refused(&mut s, "uml.classifier.set-owner", set(&status, &customer)).contains("named Status already"));
    let integer = {
        run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "n", "type": "Integer" }));
        let st = s.doc();
        st.alive().find(|o| archforge_uml::members::is_built_in(st, *o)).map(|o| st.uuid(o).unwrap().to_string()).unwrap()
    };
    assert!(refused(&mut s, "uml.classifier.set-owner", set(&integer, &order)).contains("built-in"));
    assert!(refused(&mut s, "uml.classifier.set-owner", set(&order, "not-an-id")).contains("no package or classifier"));
    let st = s.doc();
    let reasons: Vec<(String, Option<OwnerProblem>)> =
        nesting::candidates(st, obj(st, &order)).into_iter().map(|(o, p)| (archforge_core::name_of(st, o), p)).collect();
    let of = |n: &str| reasons.iter().find(|(name, _)| name == n).map(|(_, p)| p.clone());
    assert_eq!(of("sales"), Some(None), "its package, where it is");
    assert_eq!(of("Order"), Some(Some(OwnerProblem::Itself)));
    assert!(reasons.iter().any(|(n, p)| n == "Status" && *p == Some(OwnerProblem::OwnsNone)), "the enumeration owns none");
    assert_eq!(of("Customer"), Some(None));
    assert_eq!(of("Inner"), None, "one nested in it is never offered");
    assert_eq!(of("Deep"), None);
    let _ = sales;
    let owners = s.execute("uml.classifier.owners", &json!({ "classifier": inner })).unwrap().value;
    assert_eq!(owners["qualified_name"], "Order::Inner");
    assert!(owners["owners"].as_array().unwrap().iter().any(|o| o["name"] == "Order" && o["current"] == true && o["allowed"] == true));
}

/// Deleting an owner deletes what it nests at any depth, their relationships and their cards, as
/// one undo step; undone, all of it is back.
#[test]
fn deleting_an_owner_takes_its_nested_classifiers_relationships_and_views_in_one_step() {
    let mut s = session();
    let [_, order, customer, _, _, d] = shop(&mut s);
    let priority = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Line" }));
    let deep = run(&mut s, "uml.nested.add", json!({ "owner": priority, "kind": "enumeration", "name": "Kind" }));
    let assoc = run(&mut s, "uml.association.add", json!({ "from": customer, "to": priority }));
    let typed = run(&mut s, "uml.attribute.add", json!({ "class": customer, "name": "kind", "type": deep }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": priority, "x": 300, "y": 0 }));
    let st = s.doc();
    assert_eq!(archforge_uml::commands::going_with(st, &[obj(st, &order)]).len(), 1, "the association of the nested class goes");
    let before = picture(s.doc());
    run(&mut s, "element.remove", json!({ "ids": [order] }));
    let st = s.doc();
    for gone in [&order, &priority, &deep, &assoc, &node] {
        assert!(st.by_uuid(Uuid::parse(gone).unwrap()).is_none_or(|o| !st.is_alive(o)), "{gone} went");
    }
    assert!(st.get(obj(st, &typed), archforge_uml::typed_element::TYPE).unwrap().as_ref().is_none(), "the type it gave is dropped");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one step");
}

/// On a diagram showing its owner, a nested classifier's card reads its name and the notation ties
/// it to the owner; where the owner is absent, the card reads `Order::Priority`. It is placed as a
/// member of its owner's package, never as an outsider.
#[test]
fn the_card_names_its_owner_where_the_owner_is_absent() {
    let mut s = session();
    let [_, order, _, _, _, d] = shop(&mut s);
    let priority = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "enumeration", "name": "Priority" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": priority, "x": 0, "y": 300 }));
    let root = run(&mut s, "uml.model.root", json!({}));
    let other = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Alone", "element": root }));
    run(&mut s, "diagram.place", json!({ "diagram": other, "element": priority, "x": 0, "y": 0 }));
    let st = s.doc();
    let (p, o) = (obj(st, &priority), obj(st, &order));
    let card = UmlNotation.card(st, p, obj(st, &d)).unwrap();
    assert_eq!((card.name.as_str(), card.elsewhere), ("Priority", false));
    assert_eq!(UmlNotation.nested_in(st, p), Some(o));
    assert_eq!(UmlNotation.nested_in(st, o), None);
    assert_eq!(UmlNotation.card(st, p, obj(st, &other)).unwrap().name, "Order::Priority");
}

/// Nested classifiers and their owners are saved as rows and read back identical, in their order.
#[test]
fn nested_classifiers_are_saved_and_read_back_identical() {
    let mut s = session();
    let [_, order, ..] = shop(&mut s);
    for name in ["B", "A", "C"] {
        run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": name }));
    }
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    assert_eq!(names(&back, nesting::nested(&back, obj(&back, &order))), ["B", "A", "C"]);
}

/// A model of package members alone stays as it was: no classifier is nested, every card reads its
/// name, the owner is each one's package, and the per-kind moves work as before.
#[test]
fn a_model_of_package_members_alone_is_unchanged() {
    let mut s = session();
    let [sales, order, customer, status, money, d] = shop(&mut s);
    let st = s.doc();
    for c in [&order, &customer, &status, &money] {
        let c = obj(st, c);
        assert_eq!(nesting::owner_of(st, c), Some(obj(st, &sales)));
        assert!(!nesting::is_nested(st, c) && nesting::nested(st, c).is_empty());
        assert_eq!(nesting::qualified_name(st, c), archforge_core::name_of(st, c));
    }
    assert_eq!(UmlNotation.card(st, obj(st, &order), obj(st, &d)).unwrap().name, "Order");
    let other = run(&mut s, "uml.package.add", json!({ "name": "other" }));
    one_step(&mut s, "uml.class.move", json!({ "class": customer, "package": other }));
}

/// The sweep over the nesting commands breaks nothing.
#[test]
fn the_sweep_over_nested_classifiers_breaks_nothing() {
    let mut s = session();
    let [sales, order, customer, status, ..] = shop(&mut s);
    let inner = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Inner" }));
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.nested.add", "uml.classifier.set-owner", "uml.nested.list", "uml.classifier.owners"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [sales.as_str(), order.as_str(), customer.as_str(), status.as_str(), inner.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}
