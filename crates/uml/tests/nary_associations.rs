//! N-ary associations through the engine (AR-128): made with three and four ends, each one undo
//! step; a classifier at two ends, told apart by their roles; ends added, removed and moved onto
//! another class, the legs following on every diagram; removing one of three ends refused unless
//! made binary, which turns it into a plain association with its line; its ends never
//! aggregations; Remove from this diagram apart from Delete from the model; saved as rows and read
//! back identical; binary associations as they were; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::nary;
use archforge_uml::{ALONE, CLASS_DIAGRAM, registry};
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

/// A session over `st`.
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

/// A package `shop` with Customer, Product, Catalog and Store, and two class diagrams of it,
/// Orders and Again, each showing the four classes.
struct Shop {
    customer: String,
    product: String,
    catalog: String,
    store: String,
    d: String,
    again: String,
}

fn shop(s: &mut S) -> Shop {
    let shop = run(s, "uml.package.add", json!({ "name": "shop" }));
    let class = |s: &mut S, name: &str| run(s, "uml.class.add", json!({ "package": shop, "name": name }));
    let customer = class(s, "Customer");
    let product = class(s, "Product");
    let catalog = class(s, "Catalog");
    let store = class(s, "Store");
    let mut diagrams = Vec::new();
    for name in ["Orders", "Again"] {
        let d = run(s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": name, "element": shop }));
        for (i, c) in [&customer, &product, &catalog, &store].into_iter().enumerate() {
            run(s, "diagram.place", json!({ "diagram": d, "element": c, "x": (i as f64) * 400.0, "y": 0.0 }));
        }
        diagrams.push(d);
    }
    let again = diagrams.pop().unwrap();
    let d = diagrams.pop().unwrap();
    Shop { customer, product, catalog, store, d, again }
}

/// The node of `element` on diagram `d`, as an id.
fn node(s: &S, d: &str, element: &str) -> String {
    let st = s.doc();
    let n = nary::node_in(st, obj(st, d), obj(st, element)).unwrap_or_else(|| panic!("{element} is on {d}"));
    st.uuid(n).unwrap().to_string()
}

/// Draws an n-ary association between `classes` on diagram `d` as the canvas does, in one batch:
/// the association, its diamond at (600, 400) and a leg to each end's class. Gives its id.
fn draw(s: &mut S, d: &str, classes: &[&str]) -> String {
    let cards: Vec<String> = classes.iter().map(|c| node(s, d, c)).collect();
    let classes: Vec<String> = classes.iter().map(|c| c.to_string()).collect();
    let d = d.to_string();
    let out = s
        .batch("Add n-ary association", |b| {
            let a = b.execute("uml.association.add-nary", &json!({ "classes": classes, "name": "recommends" }))?;
            let id = a["id"].as_str().unwrap_or_default().to_string();
            let ends: Vec<String> = a["ends"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect();
            let n = b.execute("diagram.place", &json!({ "diagram": d, "element": id, "x": 600.0, "y": 400.0, "width": 44.0, "height": 28.0 }))?;
            let diamond = n["id"].as_str().unwrap_or_default().to_string();
            for (end, card) in ends.iter().zip(&cards) {
                b.execute("diagram.connect", &json!({ "diagram": d, "element": end, "source": diamond, "target": card }))?;
            }
            Ok(())
        })
        .unwrap();
    out.value[0]["id"].as_str().unwrap().to_string()
}

/// The ends of association `a`: each its role, its class's name and its multiplicity.
fn ends(s: &S, a: &str) -> Vec<(String, String, String)> {
    let st = s.doc();
    nary::ends(st, obj(st, a))
        .into_iter()
        .map(|e| {
            let ty = nary::end_type(st, e).map(|t| archforge_core::name_of(st, t)).unwrap_or_default();
            (archforge_core::name_of(st, e), ty, archforge_uml::figures::multiplicity(st, e))
        })
        .collect()
}

/// The legs association `a` draws on diagram `d`: the name of the class each runs to, sorted.
fn legs(s: &S, d: &str, a: &str) -> Vec<String> {
    let st = s.doc();
    let d = obj(st, d);
    let mut out: Vec<String> = nary::ends(st, obj(st, a))
        .into_iter()
        .flat_map(|e| archforge_diagram::views_of(st, e).1)
        .filter(|leg| archforge_diagram::diagram_of(st, *leg) == Some(d))
        .map(|leg| {
            let target = st.get(leg, archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap();
            let shown = st.get(target, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap();
            archforge_core::name_of(st, shown)
        })
        .collect();
    out.sort();
    out
}

/// Whether diagram `d` shows `element` as a node.
fn shows(s: &S, d: &str, element: &str) -> bool {
    let st = s.doc();
    nary::node_in(st, obj(st, d), obj(st, element)).is_some()
}

fn triple(name: &str, class: &str, m: &str) -> (String, String, String) {
    (name.to_string(), class.to_string(), m.to_string())
}

/// Made with three ends and with four, each one undo step: its ends named after their classes at
/// `*`, navigable from none, one identity drawn as a diamond and a leg to each class.
#[test]
fn an_n_ary_association_is_made_with_three_or_four_ends_in_one_undo_step() {
    let mut s = session();
    let shop = shop(&mut s);
    let three = one_step(&mut s, "uml.association.add-nary", json!({ "classes": [shop.customer, shop.product, shop.catalog], "name": "recommends" }));
    assert_eq!(ends(&s, &three), [triple("customer", "Customer", "*"), triple("product", "Product", "*"), triple("catalog", "Catalog", "*")]);
    assert!(nary::is_nary(s.doc(), obj(s.doc(), &three)));
    let four = one_step(&mut s, "uml.association.add-nary", json!({ "classes": [shop.customer, shop.product, shop.catalog, shop.store] }));
    assert_eq!(ends(&s, &four).len(), 4);
    let st = s.doc();
    for e in nary::ends(st, obj(st, &four)) {
        assert!(!archforge_uml::figures::navigable(st, obj(st, &four), e), "navigable from none");
    }
    // Drawn as the canvas draws it: one batch, one undo step, the diamond and three legs.
    let before = picture(s.doc());
    let drawn = draw(&mut s, &shop.d, &[&shop.customer, &shop.product, &shop.catalog]);
    assert_eq!(legs(&s, &shop.d, &drawn), ["Catalog", "Customer", "Product"]);
    assert!(shows(&s, &shop.d, &drawn), "the diamond is a node of the association itself");
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step takes it all");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    // The list describes it with all its ends and no from or to.
    let listed = s.execute("uml.relationship.list", &json!({ "class": shop.catalog })).unwrap().value;
    let it = listed["relationships"].as_array().unwrap().iter().find(|r| r["id"] == drawn.as_str()).unwrap().clone();
    assert_eq!(it["kind"], "n-ary association");
    assert_eq!(it["ends"].as_array().unwrap().len(), 3);
    assert!(it.get("from").is_none());
}

/// Fewer than three classes, or a data type at an end, are refused, changing nothing.
#[test]
fn too_few_ends_and_a_data_type_are_refused() {
    let mut s = session();
    let shop = shop(&mut s);
    let why = refused(&mut s, "uml.association.add-nary", json!({ "classes": [shop.customer, shop.product] }));
    assert!(why.contains("3 to 32") && why.contains("uml.association.add"), "{why}");
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "values" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let why = refused(&mut s, "uml.association.add-nary", json!({ "classes": [shop.customer, shop.product, money] }));
    assert!(why.contains("data type"), "{why}");
}

/// A class at two ends: each end has its own role, and a role another end has is refused.
#[test]
fn a_class_repeats_at_two_ends_with_different_roles() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.customer, &shop.product]);
    assert_eq!(ends(&s, &a), [triple("customer", "Customer", "*"), triple("customer2", "Customer", "*"), triple("product", "Product", "*")]);
    assert_eq!(legs(&s, &shop.d, &a), ["Customer", "Customer", "Product"], "two legs to the one card");
    run(&mut s, "uml.association.set-end", json!({ "association": a, "end": 1, "role": "referrer", "multiplicity": "0..1", "navigable": true }));
    assert_eq!(ends(&s, &a)[1], triple("referrer", "Customer", "0..1"));
    let why = refused(&mut s, "uml.association.set-end", json!({ "association": a, "end": 1, "role": "CUSTOMER" }));
    assert!(why.contains("role customer already"), "{why}");
    let why = refused(&mut s, "uml.association.add-end", json!({ "association": a, "class": shop.store, "role": "Product" }));
    assert!(why.contains("role product already"), "{why}");
}

/// An n-ary association's ends are never aggregations; the other end fields set as a binary end's.
#[test]
fn an_n_ary_end_takes_no_aggregation() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = run(&mut s, "uml.association.add-nary", json!({ "classes": [shop.customer, shop.product, shop.catalog] }));
    let why = refused(&mut s, "uml.association.set-end", json!({ "association": a, "end": 2, "aggregation": "composite" }));
    assert!(why.contains("not aggregations"), "{why}");
    let why = refused(&mut s, "uml.association.set-end", json!({ "association": a, "end": 3, "role": "x" }));
    assert!(why.contains("0 to 2"), "{why}");
    run(
        &mut s,
        "uml.association.set-end",
        json!({ "association": a, "end": 2, "aggregation": "none", "ordered": true, "unique": false, "multiplicity": "1..*" }),
    );
    let listed = s.execute("uml.relationship.list", &json!({ "class": shop.catalog })).unwrap().value;
    let end = &listed["relationships"][0]["ends"][2];
    assert_eq!((end["ordered"].as_bool(), end["unique"].as_bool(), end["multiplicity"].as_str()), (Some(true), Some(false), Some("1..*")));
}

/// Adding, removing and moving an end, each one undo step, the legs following on both diagrams.
#[test]
fn ends_are_added_removed_and_reconnected_with_their_legs_on_every_diagram() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.product, &shop.catalog]);
    // The second diagram shows it too.
    s.execute("diagram.place", &json!({ "diagram": shop.again, "element": a, "x": 600.0, "y": 400.0 })).unwrap();
    s.execute("diagram.connect-all", &json!({ "diagram": shop.again })).unwrap();
    assert_eq!(legs(&s, &shop.again, &a), ["Catalog", "Customer", "Product"]);
    one_step(&mut s, "uml.association.add-end", json!({ "association": a, "class": shop.store }));
    assert_eq!(ends(&s, &a)[3], triple("store", "Store", "*"));
    for d in [&shop.d, &shop.again] {
        assert_eq!(legs(&s, d, &a), ["Catalog", "Customer", "Product", "Store"]);
    }
    one_step(&mut s, "uml.association.reconnect-end", json!({ "association": a, "end": 2, "class": shop.customer }));
    for d in [&shop.d, &shop.again] {
        assert_eq!(legs(&s, d, &a), ["Customer", "Customer", "Product", "Store"]);
    }
    one_step(&mut s, "uml.association.remove-end", json!({ "association": a, "end": 0 }));
    assert_eq!(ends(&s, &a).len(), 3);
    for d in [&shop.d, &shop.again] {
        assert_eq!(legs(&s, d, &a), ["Customer", "Product", "Store"]);
    }
    // A binary association takes no third end here; nor does an end move onto a data type.
    let binary = run(&mut s, "uml.association.add", json!({ "from": shop.customer, "to": shop.product }));
    let why = refused(&mut s, "uml.association.add-end", json!({ "association": binary, "class": shop.store }));
    assert!(why.contains("has 2 ends"), "{why}");
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "values" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let why = refused(&mut s, "uml.association.reconnect-end", json!({ "association": a, "end": 0, "class": money }));
    assert!(why.contains("data type"), "{why}");
}

/// Removing one of three ends is never silent: refused unless made binary, which turns it into a
/// plain association with a named line where its diamond was, as one undo step.
#[test]
fn removing_one_of_three_ends_keeps_them_or_makes_it_binary() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.product, &shop.catalog]);
    let why = refused(&mut s, "uml.association.remove-end", json!({ "association": a, "end": 2 }));
    assert!(why.contains("binary: true") && why.contains("catalog"), "{why}");
    let before = picture(s.doc());
    let out = s.execute("uml.association.remove-end", &json!({ "association": a, "end": 2, "binary": true })).unwrap().value;
    assert_eq!(out["lines"].as_array().unwrap().len(), 1, "a line where the diamond was");
    let st = s.doc();
    let it = obj(st, &a);
    assert!(!nary::is_nary(st, it));
    assert_eq!(archforge_uml::figures::ends(st, it), Some((obj(st, &shop.customer), obj(st, &shop.product))), "one identity, now binary");
    assert!(!shows(&s, &shop.d, &a), "no diamond");
    let (_, lines, _) = archforge_diagram::views_of(s.doc(), it);
    assert_eq!(lines.len(), 1);
    let labels = s.doc().list(lines[0], archforge_diagram::edge::LABEL).unwrap();
    assert_eq!(labels.len(), 1, "its name at the middle");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step brings the three ends and the diamond back");
    // With four ends, binary is not said.
    run(&mut s, "uml.association.add-end", json!({ "association": a, "class": shop.store }));
    let why = refused(&mut s, "uml.association.remove-end", json!({ "association": a, "end": 0, "binary": true }));
    assert!(why.contains("stays n-ary"), "{why}");
}

/// An end removed takes the links on it and a constraint that named only it, leaving no link or
/// reference behind; undo brings them all back.
#[test]
fn a_removed_end_takes_its_links_and_its_own_constraint_along() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = run(&mut s, "uml.association.add-nary", json!({ "classes": [shop.customer, shop.product, shop.catalog, shop.store] }));
    let end = s.doc().uuid(nary::ends(s.doc(), obj(s.doc(), &a))[3]).unwrap().to_string();
    let only = run(&mut s, "uml.constraint.add", json!({ "context": shop.store, "expression": "size > 0", "constrained": [end] }));
    let both = run(&mut s, "uml.constraint.add", json!({ "context": shop.store, "expression": "open", "constrained": [end, shop.store] }));
    // A trace from the end to Customer, as a file may hold one.
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut st = store(1 << 40);
    rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
    let link = st.create(archforge_core::LINK).unwrap();
    st.set(link, archforge_core::link::SOURCE, Value::Ref(obj(&st, &end))).unwrap();
    st.set(link, archforge_core::link::TARGET, Value::Ref(obj(&st, &shop.customer))).unwrap();
    let model = archforge_uml::commands::model(&st).unwrap();
    st.push(model, archforge_core::model::LINK, Value::Ref(link)).unwrap();
    let mut s = session_over(st);
    let before = picture(s.doc());
    run(&mut s, "uml.association.remove-end", json!({ "association": a, "end": 3 }));
    let st = s.doc();
    assert!(!alive(&s, &end) && !alive(&s, &only), "the end and the constraint naming only it go");
    assert!(alive(&s, &both), "a constraint naming something else stays");
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_core::LINK)), "no link left");
    assert_eq!(st.first_problem_by(archforge_uml::names::kind), None);
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "undo brings the end, its link and its constraint back");
}

/// Remove from this diagram takes the diamond and its legs off one diagram, the model kept; Delete
/// from the model takes the association, its ends and every diamond and leg; an end alone is not
/// deleted with element.remove.
#[test]
fn remove_from_this_diagram_and_delete_from_the_model_keep_their_meanings() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.product, &shop.catalog]);
    s.execute("diagram.place", &json!({ "diagram": shop.again, "element": a, "x": 600.0, "y": 400.0 })).unwrap();
    s.execute("diagram.connect-all", &json!({ "diagram": shop.again })).unwrap();
    let diamond = node(&s, &shop.d, &a);
    run(&mut s, "diagram.unplace", json!({ "nodes": [diamond] }));
    assert!(!shows(&s, &shop.d, &a) && legs(&s, &shop.d, &a).is_empty(), "gone from Orders");
    assert_eq!(legs(&s, &shop.again, &a).len(), 3, "kept on Again");
    assert!(alive(&s, &a) && ends(&s, &a).len() == 3, "kept in the model");
    let end = s.doc().uuid(nary::ends(s.doc(), obj(s.doc(), &a))[0]).unwrap().to_string();
    let why = refused(&mut s, "element.remove", json!({ "ids": [end] }));
    assert!(why.contains("uml.association.remove-end"), "{why}");
    run(&mut s, "element.remove", json!({ "ids": [a] }));
    assert!(!alive(&s, &a) && !alive(&s, &end));
    let st = s.doc();
    let again = obj(st, &shop.again);
    assert_eq!(st.list(again, archforge_diagram::diagram::NODE).unwrap().len(), 4, "its diamonds go everywhere, the four cards stay");
    assert!(st.list(again, archforge_diagram::diagram::EDGE).unwrap().is_empty(), "and its legs");
    assert!(alive(&s, &shop.customer), "its classes stay");
}

/// A class's card taken off a diagram takes its legs; placed again, connect-all draws them back.
#[test]
fn a_class_card_takes_its_legs_and_brings_them_back() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.product, &shop.catalog]);
    let card = node(&s, &shop.d, &shop.catalog);
    run(&mut s, "diagram.unplace", json!({ "nodes": [card] }));
    assert_eq!(legs(&s, &shop.d, &a), ["Customer", "Product"]);
    run(&mut s, "diagram.place", json!({ "diagram": shop.d, "element": shop.catalog, "x": 0.0, "y": 600.0 }));
    s.execute("diagram.connect-all", &json!({ "diagram": shop.d })).unwrap();
    assert_eq!(legs(&s, &shop.d, &a), ["Catalog", "Customer", "Product"]);
}

/// Binary associations are as they were: two ends, a line, never a diamond.
#[test]
fn a_binary_association_is_still_a_line() {
    let mut s = session();
    let shop = shop(&mut s);
    let b = run(&mut s, "uml.association.add", json!({ "from": shop.customer, "to": shop.product, "kind": "composition" }));
    let st = s.doc();
    assert!(archforge_uml::figures::member_ends(st, obj(st, &b)).is_some());
    assert!(!nary::is_nary(st, obj(st, &b)));
    let why = refused(&mut s, "diagram.place", json!({ "diagram": shop.d, "element": b, "x": 0.0, "y": 0.0 }));
    assert!(why.contains("outside what this diagram shows"), "{why}");
    s.execute("diagram.connect-all", &json!({ "diagram": shop.d })).unwrap();
    let (_, lines, _) = archforge_diagram::views_of(s.doc(), obj(s.doc(), &b));
    assert_eq!(lines.len(), 1, "one line");
    run(&mut s, "uml.association.set-end", json!({ "association": b, "end": 0, "multiplicity": "1" }));
}

/// An n-ary association, its ends, diamonds and legs on two diagrams, saved as rows, read back
/// identical.
#[test]
fn an_n_ary_association_is_saved_and_read_back_identical() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.customer, &shop.product, &shop.catalog]);
    s.execute("diagram.place", &json!({ "diagram": shop.again, "element": a, "x": 100.0, "y": 300.0 })).unwrap();
    s.execute("diagram.connect-all", &json!({ "diagram": shop.again })).unwrap();
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let back = session_over(back);
    assert_eq!(ends(&back, &a), ends(&s, &a), "the same ends, in the same order");
    for d in [&shop.d, &shop.again] {
        assert_eq!(legs(&back, d, &a), ["Catalog", "Customer", "Customer", "Product"]);
    }
}

/// The sweep over the n-ary commands breaks nothing.
#[test]
fn the_sweep_over_n_ary_associations_breaks_nothing() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = draw(&mut s, &shop.d, &[&shop.customer, &shop.product, &shop.catalog]);
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.association.add-nary", "uml.association.add-end", "uml.association.remove-end", "uml.association.reconnect-end"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [a.as_str(), shop.customer.as_str(), shop.store.as_str(), shop.d.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}

fn alive(s: &S, id: &str) -> bool {
    let st = s.doc();
    st.by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
}
