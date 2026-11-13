//! Generalization sets through the engine (AR-129): created from generalizations to one general
//! classifier, renamed, made complete and disjoint, given and losing members, each one undo step;
//! a mixed selection refused; a generalization in two sets; a reconnection of a member refused,
//! changing nothing; a set emptied by a deletion going in the same step, and a removed set keeping
//! its generalizations; its label placed on two diagrams; saved as rows and read back identical; the
//! sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::generalization_sets as sets;
use archforge_uml::{ALONE, CLASS_DIAGRAM, GENERALIZATION_SET, registry};
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

/// A model with a package `pay` holding Payment, CardPayment, BankTransfer, WalletPayment and
/// Product, each of the three a generalization to Payment, a Voucher specializing Product, and a
/// class diagram of `pay` showing them all with their lines: (pay, payment, product, the four
/// generalizations card, bank, wallet and voucher, the diagram).
struct Shop {
    pay: String,
    payment: String,
    product: String,
    card: String,
    bank: String,
    wallet: String,
    voucher: String,
    d: String,
}

fn shop(s: &mut S) -> Shop {
    let pay = run(s, "uml.package.add", json!({ "name": "pay" }));
    let class = |s: &mut S, name: &str| run(s, "uml.class.add", json!({ "package": pay, "name": name }));
    let payment = class(s, "Payment");
    let product = class(s, "Product");
    let mut gens = Vec::new();
    for (name, general) in [("CardPayment", &payment), ("BankTransfer", &payment), ("WalletPayment", &payment), ("Voucher", &product)] {
        let c = class(s, name);
        gens.push(run(s, "uml.generalization.add", json!({ "specific": c, "general": general })));
    }
    let d = run(s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Payments", "element": pay }));
    let st = s.doc();
    let classes: Vec<String> =
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS)).map(|o| st.uuid(o).unwrap().to_string()).collect();
    for (i, c) in classes.iter().enumerate() {
        run(s, "diagram.place", json!({ "diagram": d, "element": c, "x": (i as f64) * 300.0, "y": if c == &payment { 0.0 } else { 400.0 } }));
    }
    s.execute("diagram.connect-all", &json!({ "diagram": d })).unwrap();
    let [card, bank, wallet, voucher]: [String; 4] = gens.try_into().unwrap();
    Shop { pay, payment, product, card, bank, wallet, voucher, d }
}

/// The sets `uml.generalization-set.list` gives `of`.
fn listed(s: &mut S, of: &str) -> Vec<Json> {
    s.execute("uml.generalization-set.list", &json!({ "of": of })).unwrap().value["sets"].as_array().unwrap().clone()
}

fn alive(s: &S, id: &str) -> bool {
    let st = s.doc();
    st.by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
}

/// A set is created from three generalizations to Payment, renamed, made complete and disjoint,
/// given and losing members, each one undo step; its label reads `method {complete, disjoint}`.
#[test]
fn a_set_is_created_edited_and_its_members_changed_each_one_undo_step() {
    let mut s = session();
    let shop = shop(&mut s);
    let before = picture(s.doc());
    let set = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.bank], "name": "method" }));
    let st = s.doc();
    let o = obj(st, &set);
    assert_eq!(st.class_of(o).unwrap(), GENERALIZATION_SET);
    assert_eq!(s.view().selection, vec![o], "the new set is selected");
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    let st = s.doc();
    assert_eq!(sets::general_of(st, o), Some(obj(st, &shop.payment)));
    assert_eq!(archforge_uml::commands::package_of(st, o), Some(obj(st, &shop.pay)), "in its general classifier's package");
    assert_eq!(sets::label(st, o), "method {incomplete, overlapping}", "UML's defaults");
    one_step(&mut s, "uml.generalization-set.set", json!({ "set": set, "complete": true, "disjoint": true }));
    assert_eq!(sets::label(s.doc(), o), "method {complete, disjoint}");
    one_step(&mut s, "uml.generalization-set.set", json!({ "set": set, "add": [shop.wallet] }));
    let st = s.doc();
    assert_eq!(sets::members(st, o), vec![obj(st, &shop.card), obj(st, &shop.bank), obj(st, &shop.wallet)]);
    assert_eq!(sets::sets_of(st, obj(st, &shop.wallet)), vec![o], "the generalization knows its set");
    one_step(&mut s, "uml.generalization-set.set", json!({ "set": set, "remove": [shop.bank], "name": "payment method" }));
    let st = s.doc();
    assert_eq!(sets::members(st, o), vec![obj(st, &shop.card), obj(st, &shop.wallet)]);
    assert!(sets::sets_of(st, obj(st, &shop.bank)).is_empty());
    assert_eq!(archforge_core::name_of(st, o), "payment method");
    let all = listed(&mut s, &shop.payment);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["label"], "payment method {complete, disjoint}");
    assert_eq!(all[0]["generalizations"].as_array().unwrap().len(), 2);
    assert_eq!(listed(&mut s, &shop.card)[0]["id"], set.as_str());
    let rel = s.execute("uml.relationship.list", &json!({ "class": shop.payment })).unwrap().value;
    assert!(rel.to_string().contains("payment method {complete, disjoint}"), "a generalization lists its sets: {rel}");
    // Removing the set keeps its generalizations; one step.
    let gens_before = s.doc().alive().filter(|g| s.doc().class_of(*g).ok() == Some(archforge_uml::GENERALIZATION)).count();
    one_step(&mut s, "element.remove", json!({ "ids": [set] }));
    let st = s.doc();
    assert!(!alive(&s, &set));
    assert_eq!(st.alive().filter(|g| st.class_of(*g).ok() == Some(archforge_uml::GENERALIZATION)).count(), gens_before, "the generalizations stay");
    assert!(sets::sets_of(st, obj(st, &shop.card)).is_empty());
    // Taking out the last member takes the set with it, in the same step.
    let lone = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card], "name": "lone" }));
    let before = picture(s.doc());
    let out = s.execute("uml.generalization-set.set", &json!({ "set": lone, "remove": [shop.card] })).unwrap().value;
    assert_eq!(out["removed"], true);
    assert!(!alive(&s, &lone) && alive(&s, &shop.card));
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
}

/// Generalizations to different classifiers, an element that is no generalization, an empty list,
/// a clashing or empty name, a member to another classifier and a member named twice are refused,
/// each saying why and changing nothing, the history included.
#[test]
fn a_mixed_selection_and_the_other_wrong_sets_are_refused_changing_nothing() {
    let mut s = session();
    let shop = shop(&mut s);
    let set = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card], "name": "method" }));
    let st = s.doc();
    let mixed = sets::shared_general(st, &[obj(st, &shop.card), obj(st, &shop.voucher)]);
    assert_eq!(mixed, Err(sets::Grouping::ParentsDiffer(vec![obj(st, &shop.card), obj(st, &shop.voucher)])));
    let undo = s.undo_label().map(str::to_string);
    for (cmd, p, key, why) in [
        (
            "uml.generalization-set.add",
            json!({ "generalizations": [shop.card, shop.voucher], "name": "x" }),
            "generalizations",
            "CardPayment → Payment and Voucher → Product specialize different classifiers",
        ),
        (
            "uml.generalization-set.add",
            json!({ "generalizations": [shop.card, shop.payment], "name": "x" }),
            "generalizations",
            "is not a generalization",
        ),
        ("uml.generalization-set.add", json!({ "generalizations": [], "name": "x" }), "generalizations", "at least one"),
        ("uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.card], "name": "x" }), "generalizations", "named twice"),
        ("uml.generalization-set.add", json!({ "generalizations": [shop.bank], "name": "Method" }), "name", "named method already"),
        ("uml.generalization-set.add", json!({ "generalizations": [shop.bank], "name": " " }), "name", "needs a name"),
        ("uml.generalization-set.set", json!({ "set": set, "add": [shop.voucher] }), "add", "specialize different classifiers"),
        ("uml.generalization-set.set", json!({ "set": set, "remove": [shop.bank] }), "remove", "is not in the set"),
        // Every member swapped for generalizations to another classifier in one call: the set
        // keeps its general classifier, which its package and its name belong to.
        (
            "uml.generalization-set.set",
            json!({ "set": set, "remove": [shop.card], "add": [shop.voucher] }),
            "add",
            "the set groups the specializations of Payment; Product is another classifier",
        ),
        ("uml.generalization-set.set", json!({ "set": shop.card }), "set", "not a generalization set"),
        ("uml.generalization-set.list", json!({ "of": shop.pay }), "of", "neither a classifier nor a generalization"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(key) && e.contains(why), "{cmd} {p}: {e}");
    }
    assert_eq!(s.undo_label().map(str::to_string), undo, "the history is untouched");
}

/// A generalization may be in two sets, each membership explicit; both list it, and it lists both.
#[test]
fn a_generalization_belongs_to_two_sets() {
    let mut s = session();
    let shop = shop(&mut s);
    let method = run(
        &mut s,
        "uml.generalization-set.add",
        json!({ "generalizations": [shop.card, shop.bank, shop.wallet], "name": "method", "complete": true, "disjoint": true }),
    );
    let channel = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.wallet], "name": "channel" }));
    let st = s.doc();
    assert_eq!(sets::sets_of(st, obj(st, &shop.card)), vec![obj(st, &method), obj(st, &channel)]);
    assert_eq!(sets::sets_of(st, obj(st, &shop.bank)), vec![obj(st, &method)], "membership is explicit");
    assert_eq!(sets::sets_for(st, obj(st, &shop.payment)), vec![obj(st, &method), obj(st, &channel)], "each once, in order");
    assert!(sets::sets_for(st, obj(st, &shop.product)).is_empty());
    let labels: Vec<Json> = listed(&mut s, &shop.card).into_iter().map(|j| j["label"].clone()).collect();
    assert_eq!(labels, vec![json!("method {complete, disjoint}"), json!("channel {incomplete, overlapping}")]);
}

/// Reconnecting a member, either end, is refused naming its sets, the document and the history
/// staying as they were; out of its sets it reconnects; one in no set reconnects as before.
#[test]
fn reconnecting_a_member_is_refused_and_ordinary_generalizations_reconnect_as_before() {
    let mut s = session();
    let shop = shop(&mut s);
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.bank], "name": "method" }));
    let channel = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card], "name": "channel" }));
    let undo = s.undo_label().map(str::to_string);
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.card, "end": "target", "class": shop.product }));
    assert!(e.contains("method") && e.contains("channel") && e.contains("CardPayment → Payment"), "{e}");
    let st = s.doc();
    let other = st
        .alive()
        .find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Voucher")
        .map(|o| st.uuid(o).unwrap().to_string())
        .unwrap();
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.bank, "end": "source", "class": other }));
    assert!(e.contains("method"), "{e}");
    assert_eq!(s.undo_label().map(str::to_string), undo, "the history is untouched");
    // An ordinary generalization reconnects as before.
    one_step(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.wallet, "end": "target", "class": shop.product }));
    // Taken out of its sets, a generalization reconnects.
    run(&mut s, "uml.generalization-set.set", json!({ "set": channel, "remove": [shop.card] }));
    let method = listed(&mut s, &shop.card)[0]["id"].as_str().unwrap().to_string();
    run(&mut s, "uml.generalization-set.set", json!({ "set": method, "remove": [shop.card] }));
    one_step(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.card, "end": "target", "class": shop.product }));
}

/// Deleting a member takes it out of its sets in the same step; a set left with no member goes
/// with it; deleting a class takes its generalizations, and so the sets they alone made up.
#[test]
fn a_set_emptied_by_a_deletion_goes_in_the_same_step() {
    let mut s = session();
    let shop = shop(&mut s);
    let method = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.bank], "name": "method" }));
    let lone = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card], "name": "lone" }));
    run(&mut s, "uml.generalization-set.place", json!({ "diagram": shop.d, "set": lone, "x": 10, "y": 200 }));
    let st = s.doc();
    assert_eq!(sets::emptied_by(st, &[obj(st, &shop.card)]), vec![obj(st, &lone)]);
    let before = picture(s.doc());
    run(&mut s, "element.remove", json!({ "ids": [shop.card] }));
    let st = s.doc();
    assert!(!alive(&s, &lone), "the set left empty went with its member, its label too");
    assert!(st.alive().all(|o| !sets::is_label(st, o)));
    assert_eq!(sets::members(st, obj(st, &method)), vec![obj(st, &shop.bank)], "the other set keeps the rest");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step brings both back");
    // Deleting the general classifier takes every generalization to it, and so both sets.
    run(&mut s, "element.remove", json!({ "ids": [shop.payment] }));
    assert!(!alive(&s, &method) && !alive(&s, &lone));
    assert!(alive(&s, &shop.voucher) && alive(&s, &shop.product));
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// The label sits on each diagram that shows the grouped lines, placed per diagram: its node
/// nested in the general classifier's node, moved there on that diagram alone.
#[test]
fn the_label_is_placed_on_two_diagrams_each_its_own_place() {
    let mut s = session();
    let shop = shop(&mut s);
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Overview", "element": shop.pay }));
    let st = s.doc();
    let classes: Vec<String> =
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS)).map(|o| st.uuid(o).unwrap().to_string()).collect();
    for (i, c) in classes.iter().enumerate() {
        run(&mut s, "diagram.place", json!({ "diagram": d2, "element": c, "x": (i as f64) * 250.0, "y": (i as f64) * 50.0 }));
    }
    s.execute("diagram.connect-all", &json!({ "diagram": d2 })).unwrap();
    let set = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.bank], "name": "method", "complete": true }));
    let n1 = one_step(&mut s, "uml.generalization-set.place", json!({ "diagram": shop.d, "set": set, "x": 100, "y": 250 }));
    let n2 = one_step(&mut s, "uml.generalization-set.place", json!({ "diagram": d2, "set": set, "x": -40, "y": 90 }));
    let st = s.doc();
    let (s_obj, d1, d2o) = (obj(st, &set), obj(st, &shop.d), obj(st, &d2));
    assert_eq!(sets::label_node(st, d1, s_obj), Some(obj(st, &n1)));
    assert_eq!(sets::label_node(st, d2o, s_obj), Some(obj(st, &n2)));
    let payment_node =
        |d: Obj| archforge_diagram::views_of(st, obj(st, &shop.payment)).2.into_iter().find(|n| archforge_diagram::diagram_of(st, *n) == Some(d));
    assert_eq!(st.container(obj(st, &n1)).unwrap().map(|(o, _)| o), payment_node(d1), "nested in the general's node");
    let at = |s: &S, n: &str| {
        let r = archforge_diagram::grid::bounds_of(s.doc(), obj(s.doc(), n));
        (r.x, r.y)
    };
    assert_eq!((at(&s, &n1), at(&s, &n2)), ((100.0, 250.0), (-40.0, 90.0)), "each diagram its own place");
    // Moved again on one diagram: the same node, the other untouched.
    let again = run(&mut s, "uml.generalization-set.place", json!({ "diagram": shop.d, "set": set, "x": 120, "y": 260 }));
    assert_eq!(again, n1);
    assert_eq!((at(&s, &n1), at(&s, &n2)), ((120.0, 260.0), (-40.0, 90.0)));
    // The label fits its text when the set changes, on every diagram.
    let width = |s: &S, n: &str| archforge_diagram::grid::bounds_of(s.doc(), obj(s.doc(), n)).width;
    let before = width(&s, &n2);
    run(&mut s, "uml.generalization-set.set", json!({ "set": set, "name": "the method of payment" }));
    assert!(width(&s, &n1) > before && width(&s, &n2) > before);
    // A diagram showing none of its classifiers has nowhere to put it.
    let empty = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Empty", "element": shop.pay }));
    let e = refused(&mut s, "uml.generalization-set.place", json!({ "diagram": empty, "set": set, "x": 0, "y": 0 }));
    assert!(e.contains("nothing to sit by"), "{e}");
}

/// Sets, their flags, members and placed labels, saved as rows, read back identical.
#[test]
fn sets_are_saved_and_read_back_identical() {
    let mut s = session();
    let shop = shop(&mut s);
    let method = run(
        &mut s,
        "uml.generalization-set.add",
        json!({ "generalizations": [shop.card, shop.bank, shop.wallet], "name": "method", "complete": true, "disjoint": true }),
    );
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.wallet, shop.card], "name": "channel" }));
    run(&mut s, "uml.generalization-set.place", json!({ "diagram": shop.d, "set": method, "x": 300, "y": 250 }));
    run(&mut s, "element.describe", json!({ "element": method, "text": "How a payment is made" }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let mut back = session_over(back);
    assert_eq!(listed(&mut back, &shop.payment), listed(&mut s, &shop.payment), "the same sets, flags, members and description");
    let st = back.doc();
    assert!(sets::label_node(st, obj(st, &shop.d), obj(st, &method)).is_some(), "the label's place too");
}

/// The sweep over the set commands breaks nothing.
#[test]
fn the_sweep_over_generalization_sets_breaks_nothing() {
    let mut s = session();
    let shop = shop(&mut s);
    let set = run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [shop.card, shop.bank], "name": "method" }));
    run(&mut s, "uml.generalization-set.place", json!({ "diagram": shop.d, "set": set, "x": 0, "y": 300 }));
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.generalization-set.add", "uml.generalization-set.set", "uml.generalization-set.place", "uml.generalization-set.list"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [shop.card.as_str(), shop.voucher.as_str(), shop.payment.as_str(), set.as_str(), shop.d.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}
