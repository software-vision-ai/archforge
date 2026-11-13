//! Qualified associations through the engine (AR-127): qualifiers added, edited, moved and removed
//! on an association's end, each one undo step; a name another qualifier of the end has, whatever
//! the case, refused with the other wrong edits, changing nothing; a type from the type selector, a
//! built-in one or the model's own; qualifiers kept apart from attributes and roles; renaming and
//! reconnecting the association keeping them; the line's qualifier box on two diagrams; saved as
//! rows and read back identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::qualifiers;
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

/// A package `shop` with Order, OrderLine and Product, an association `contains` from Order to
/// OrderLine whose OrderLine end is `line`, and a class diagram of `shop` showing them.
struct Shop {
    shop: String,
    order: String,
    line: String,
    product: String,
    contains: String,
    d: String,
}

fn shop(s: &mut S) -> Shop {
    let shop = run(s, "uml.package.add", json!({ "name": "shop" }));
    let class = |s: &mut S, name: &str| run(s, "uml.class.add", json!({ "package": shop, "name": name }));
    let order = class(s, "Order");
    let line = class(s, "OrderLine");
    let product = class(s, "Product");
    let contains = run(s, "uml.association.add", json!({ "from": order, "to": line, "name": "contains" }));
    run(s, "uml.association.set-end", json!({ "association": contains, "end": 0, "role": "order", "multiplicity": "1" }));
    run(s, "uml.association.set-end", json!({ "association": contains, "end": 1, "role": "line", "multiplicity": "0..1" }));
    let d = run(s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Orders", "element": shop }));
    for (i, c) in [&order, &line, &product].into_iter().enumerate() {
        run(s, "diagram.place", json!({ "diagram": d, "element": c, "x": (i as f64) * 400.0, "y": 0.0 }));
    }
    s.execute("diagram.connect-all", &json!({ "diagram": d })).unwrap();
    Shop { shop, order, line, product, contains, d }
}

/// The ends `uml.relationship.list` gives for `contains`, among the relationships of whichever
/// class it touches.
fn ends(s: &mut S, shop: &Shop) -> Vec<Json> {
    for class in [&shop.order, &shop.line, &shop.product] {
        let out = s.execute("uml.relationship.list", &json!({ "class": class })).unwrap().value;
        if let Some(r) = out["relationships"].as_array().unwrap().iter().find(|r| r["id"] == json!(shop.contains)) {
            return r["ends"].as_array().unwrap().clone();
        }
    }
    panic!("no class lists contains")
}

/// The names of end `i`'s qualifiers, as the read gives them.
fn names(s: &mut S, shop: &Shop, i: usize) -> Vec<String> {
    ends(s, shop)[i]["qualifiers"].as_array().unwrap().iter().map(|q| q["name"].as_str().unwrap().to_string()).collect()
}

/// The rows of the qualifier box the line draws at each end.
fn boxes(s: &S, shop: &Shop) -> [Vec<String>; 2] {
    let st = s.doc();
    let line = archforge_uml::figures::line(st, obj(st, &shop.contains)).unwrap();
    [line.ends[0].qualifiers.clone(), line.ends[1].qualifiers.clone()]
}

/// A qualifier is added to the Order end, edited, moved and removed, each one undo step; the line
/// draws its box at that end, rows in model order, and the read lists them.
#[test]
fn qualifiers_are_added_edited_moved_and_removed_each_one_undo_step() {
    let mut s = session();
    let shop = shop(&mut s);
    let line_no = one_step(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    assert_eq!(boxes(&s, &shop), [vec!["lineNo : Integer".to_string()], Vec::<String>::new()], "the box at Order, none at OrderLine");
    let kind = one_step(
        &mut s,
        "uml.qualifier.add",
        json!({ "association": shop.contains, "end": 0, "name": "kind", "type": "String", "multiplicity": "0..1" }),
    );
    assert_eq!(names(&mut s, &shop, 0), ["lineNo", "kind"]);
    assert_eq!(boxes(&s, &shop)[0], ["lineNo : Integer", "kind : String [0..1]"], "rows in model order");
    // Placed first with at.
    let first = one_step(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "batch", "at": 0 }));
    assert_eq!(names(&mut s, &shop, 0), ["batch", "lineNo", "kind"]);
    // Edited: a field left out is kept, an empty one cleared.
    one_step(&mut s, "uml.qualifier.set", json!({ "qualifier": line_no, "name": "position" }));
    assert_eq!(boxes(&s, &shop)[0][1], "position : Integer", "the type kept");
    one_step(&mut s, "uml.qualifier.set", json!({ "qualifier": kind, "type": "", "multiplicity": "" }));
    assert_eq!(boxes(&s, &shop)[0][2], "kind");
    // Moved.
    one_step(&mut s, "uml.qualifier.move", json!({ "qualifier": first, "at": 2 }));
    assert_eq!(names(&mut s, &shop, 0), ["position", "kind", "batch"]);
    one_step(&mut s, "uml.qualifier.move", json!({ "qualifier": first, "at": 0 }));
    one_step(&mut s, "uml.qualifier.move", json!({ "qualifier": first, "at": -1 }));
    assert_eq!(names(&mut s, &shop, 0), ["position", "kind", "batch"], "-1 is the last place");
    // Removed through element.remove, alone.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [kind] })).unwrap();
    assert_eq!(names(&mut s, &shop, 0), ["position", "batch"]);
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one step");
    s.redo().unwrap();
    // The read gives each qualifier's id, type and multiplicity; the far end is unchanged.
    let e = ends(&mut s, &shop);
    assert_eq!(e[0]["qualifiers"][0]["type"]["name"], json!("Integer"));
    assert_eq!(e[0]["qualifiers"][0]["id"], json!(line_no));
    assert_eq!(e[1]["multiplicity"], json!("0..1"), "the multiplicity per qualifier value stays on the far end");
    assert!(e[1]["qualifiers"].as_array().unwrap().is_empty());
}

/// A name the end's other qualifier has, whatever the case, is refused naming it, as are the
/// other wrong edits; nothing changes and no undo step is made.
#[test]
fn a_duplicate_name_and_the_other_wrong_edits_are_refused_changing_nothing() {
    let mut s = session();
    let shop = shop(&mut s);
    let line_no = run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    let kind = run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "kind" }));
    let undo = s.undo_label().map(str::to_string);
    let e = refused(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "LINENO", "type": "String" }));
    assert!(e.contains("has a qualifier named lineNo already") && e.contains("name"), "{e}");
    let e = refused(&mut s, "uml.qualifier.set", json!({ "qualifier": kind, "name": "lineno" }));
    assert!(e.contains("has a qualifier named lineNo already"), "{e}");
    assert!(e.contains(r#"reason: Some(NameTaken { kind: "qualifier", name: "lineNo" })"#), "typed, naming the qualifier that has it: {e}");
    assert_eq!(s.undo_label().map(str::to_string), undo, "no undo step");
    // The same name on the other end is another end's.
    run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 1, "name": "lineNo" }));
    for (cmd, p, key) in [
        ("uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "" }), "empty"),
        ("uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "x", "multiplicity": "two" }), "multiplicity"),
        ("uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "x", "type": "Decimal" }), "neither a type's id"),
        ("uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "x", "at": 5 }), "past the end"),
        ("uml.qualifier.add", json!({ "association": shop.order, "end": 0, "name": "x" }), "no association"),
        ("uml.qualifier.move", json!({ "qualifier": kind, "at": 2 }), "past the last"),
        ("uml.qualifier.set", json!({ "qualifier": kind, "multiplicity": "3..1" }), "multiplicity"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains(key), "{key}: {e}");
    }
    // An attribute is no qualifier, and a qualifier no attribute.
    let id = run(&mut s, "uml.attribute.add", json!({ "class": shop.order, "name": "id" }));
    let e = refused(&mut s, "uml.qualifier.set", json!({ "qualifier": id, "name": "y" }));
    assert!(e.contains("is no qualifier"), "{e}");
    let e = refused(&mut s, "uml.attribute.set", json!({ "attribute": line_no, "name": "y" }));
    assert!(e.contains("not a class's"), "{e}");
    let e = refused(&mut s, "uml.attribute.move", json!({ "attribute": line_no, "at": 0 }));
    assert!(e.contains("belongs to no class"), "{e}");
    // An end holds at most 32.
    for i in 2..qualifiers::MAX_QUALIFIERS {
        run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": format!("q{i}") }));
    }
    let e = refused(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "one more" }));
    assert!(e.contains("at most 32"), "{e}");
}

/// A qualifier's type comes from the type selector's choices: a built-in type by its name, made in
/// Types once, or a type of the model's own by its id; a type that goes leaves it without one.
#[test]
fn a_qualifier_takes_a_built_in_type_or_one_of_the_model_s_own() {
    let mut s = session();
    let shop = shop(&mut s);
    let a = run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "a", "type": "Integer" }));
    let b = run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "b", "type": "Integer" }));
    let st = s.doc();
    let ty = |q: &str| st.get(obj(st, q), archforge_uml::typed_element::TYPE).unwrap().as_ref().unwrap();
    assert_eq!(ty(&a), ty(&b), "one Integer");
    assert!(archforge_uml::members::is_built_in(st, ty(&a)));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": shop.shop, "name": "Status" }));
    run(&mut s, "uml.qualifier.set", json!({ "qualifier": b, "type": status }));
    assert_eq!(boxes(&s, &shop)[0], ["a : Integer", "b : Status"]);
    s.execute("element.remove", &json!({ "ids": [status] })).unwrap();
    assert_eq!(boxes(&s, &shop)[0], ["a : Integer", "b"], "the type gone, the qualifier kept");
}

/// Qualifiers are neither attributes nor roles: Order's card and attributes leave them out, its
/// end's role stays, a skill is refused on one, and they go with their association.
#[test]
fn qualifiers_stay_apart_from_attributes_and_roles_and_go_with_their_association() {
    let mut s = session();
    let shop = shop(&mut s);
    let q = run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    let st = s.doc();
    let order = obj(st, &shop.order);
    assert!(archforge_uml::members::attributes(st, order).is_empty());
    let card = archforge_uml::figures::card(st, order, obj(st, &shop.d)).unwrap();
    assert!(card.compartments.iter().flatten().all(|r| !r.text.contains("lineNo")), "{card:?}");
    assert!(qualifiers::is_qualifier(st, obj(st, &q)));
    assert_eq!(qualifiers::association_of(st, obj(st, &q)), Some((obj(st, &shop.contains), 0)));
    assert_eq!(ends(&mut s, &shop)[0]["role"], json!("order"), "the role is the end's own");
    let e = s.execute("skill.add", &json!({ "subject": q, "name": "Rule", "body": "x" })).unwrap_err().to_string();
    assert!(e.contains("a qualifier carries no skill"), "{e}");
    // Deleting the association takes its qualifiers along, and undo brings them back.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [shop.contains] })).unwrap();
    assert!(!alive(&s, &q));
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// Renaming the association, its end's role, and reconnecting either end keep the qualifiers; a
/// reconnection refused for its own reason leaves them as they were.
#[test]
fn renaming_and_reconnecting_keep_the_qualifiers() {
    let mut s = session();
    let shop = shop(&mut s);
    run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    run(&mut s, "uml.relationship.rename", json!({ "relationship": shop.contains, "name": "holds" }));
    run(&mut s, "uml.association.set-end", json!({ "association": shop.contains, "end": 0, "role": "owner" }));
    assert_eq!(names(&mut s, &shop, 0), ["lineNo"]);
    // The far end moves to Product: the box stays at Order.
    run(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.contains, "end": "target", "class": shop.product }));
    assert_eq!(boxes(&s, &shop)[0], ["lineNo : Integer"]);
    // The qualified end moves to OrderLine: its qualifiers go with it.
    run(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.contains, "end": "source", "class": shop.line }));
    assert_eq!(ends(&mut s, &shop)[0]["class"], json!(shop.line));
    assert_eq!(boxes(&s, &shop)[0], ["lineNo : Integer"]);
    // Onto an enumeration: refused, the qualifiers as they were.
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": shop.shop, "name": "Status" }));
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": shop.contains, "end": "source", "class": status }));
    assert_eq!(names(&mut s, &shop, 0), ["lineNo"]);
}

/// Every diagram that shows the association draws the same box: the content is the model's.
#[test]
fn the_box_is_drawn_on_every_diagram_that_shows_the_line() {
    let mut s = session();
    let shop = shop(&mut s);
    run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Overview", "element": shop.shop }));
    for (c, x) in [(&shop.line, 0.0), (&shop.order, 500.0)] {
        run(&mut s, "diagram.place", json!({ "diagram": d2, "element": c, "x": x, "y": 200.0 }));
    }
    s.execute("diagram.connect-all", &json!({ "diagram": d2 })).unwrap();
    let st = s.doc();
    let edges = archforge_diagram::views_of(st, obj(st, &shop.contains)).1;
    let on: Vec<Obj> = edges.iter().filter_map(|e| archforge_diagram::diagram_of(st, *e)).collect();
    assert!(on.contains(&obj(st, &shop.d)) && on.contains(&obj(st, &d2)), "a line on each diagram");
    let view = ProjectView::new(ALONE);
    for e in edges {
        let element = st.get(e, archforge_diagram::edge::ELEMENT).unwrap().as_ref().unwrap();
        let line = view.of_element(st, element).unwrap().line(st, element).unwrap();
        assert_eq!(line.ends[0].qualifiers, ["lineNo : Integer"]);
    }
}

/// Qualifiers, their types, multiplicities and order, saved as rows, read back identical.
#[test]
fn qualifiers_are_saved_and_read_back_identical() {
    let mut s = session();
    let shop = shop(&mut s);
    run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "kind", "type": shop.product, "multiplicity": "0..1" }));
    run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "batch", "at": 0 }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let mut back = session_over(back);
    assert_eq!(ends(&mut back, &shop), ends(&mut s, &shop), "the same qualifiers, in the same order");
    assert_eq!(boxes(&back, &shop)[0], ["batch", "lineNo : Integer", "kind : Product [0..1]"]);
}

/// The sweep over the qualifier commands breaks nothing.
#[test]
fn the_sweep_over_qualifiers_breaks_nothing() {
    let mut s = session();
    let shop = shop(&mut s);
    let q = run(&mut s, "uml.qualifier.add", json!({ "association": shop.contains, "end": 0, "name": "lineNo", "type": "Integer" }));
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.qualifier.add", "uml.qualifier.set", "uml.qualifier.move"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [q.as_str(), shop.contains.as_str(), shop.order.as_str(), shop.d.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}

fn alive(s: &S, id: &str) -> bool {
    let st = s.doc();
    st.by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
}
