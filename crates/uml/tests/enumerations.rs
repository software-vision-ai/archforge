//! Enumerations through the engine: added in a package, renamed, moved and removed, each one undo
//! step; their names told apart from their package's classes, interfaces and enumerations; drawn as
//! a card with the keyword «enumeration» and one compartment of literals, empty when it has none;
//! taking no attributes or operations; offered as a type, which a member keeps through a rename and
//! a move; specialized only by and only of enumerations, and joined by no association; saved as rows
//! and read back identical; the sweep. Literals (AR-122) are added, renamed, moved and removed,
//! each one undo step, keeping their identity, their names told apart within their enumeration
//! whatever the case; their order is saved and read back; a card lists at most a bounded number
//! of them and counts the rest. Some tests put literals straight in the store, as a saved project
//! holds them, so the round trip checks the stored order rather than the order of their ids.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::{ALONE, CLASS_DIAGRAM, ENUMERATION, ENUMERATION_LITERAL, registry};
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

/// The session's model with literals named `names`, in that order, put straight in the store as
/// the enumeration `enumeration`'s, as a saved project holds them: read back from its rows into a
/// new session, ids counted from `start`. The first literal is made last and put in front, so the
/// literals' order is not the order of their ids, and a round trip that kept only ids' order shows.
fn with_literals(s: &S, enumeration: &str, names: &[&str], start: u64) -> S {
    let (tables, junctions) = tables();
    let mut st = store(start);
    rows::restore(&mut st, &rows::snapshot(s.doc(), &tables, &junctions).unwrap(), &tables, &junctions).unwrap();
    let e = obj(&st, enumeration);
    let held = archforge_uml::commands::literals(&st, e).len();
    let literal = |st: &mut Store, name: &str| {
        let l = st.create(ENUMERATION_LITERAL).unwrap();
        st.set(l, archforge_uml::named_element::NAME, Value::str(name)).unwrap();
        l
    };
    for name in names.iter().skip(1) {
        let l = literal(&mut st, name);
        st.push(e, archforge_uml::enumeration::OWNED_LITERAL, Value::Ref(l)).unwrap();
    }
    if let Some(first) = names.first() {
        let l = literal(&mut st, first);
        st.insert(e, archforge_uml::enumeration::OWNED_LITERAL, u32::try_from(held).unwrap(), Value::Ref(l)).unwrap();
    }
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

/// The card of `element` on diagram `d`.
fn card(s: &S, element: &str, d: &str) -> archforge_diagram::figure::Card {
    let st = s.doc();
    archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, element), obj(st, d)).unwrap()
}

/// The rows of each compartment of `card`.
fn rows_of(card: &archforge_diagram::figure::Card) -> Vec<Vec<String>> {
    card.compartments.iter().map(|c| c.iter().map(|r| r.text.clone()).collect()).collect()
}

/// Enumerations are added, renamed, moved and removed, each one undo step, named apart from their
/// package's classes and interfaces.
#[test]
fn an_enumeration_is_added_renamed_moved_and_removed_each_one_step_its_name_free_among_the_classifiers() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let before = picture(s.doc());
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "OrderStatus" }));
    let st = s.doc();
    assert_eq!(st.class_of(obj(st, &status)).unwrap(), ENUMERATION);
    assert_eq!(s.view().selection, vec![obj(st, &status)], "the new enumeration is selected");
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    let st = s.doc();
    assert_eq!(
        archforge_uml::commands::classifiers(st, obj(st, &sales)),
        vec![obj(st, &order), obj(st, &payable), obj(st, &status)],
        "a classifier of its package, in its order"
    );
    let listed = s.execute("uml.enumeration.list", &json!({ "package": sales })).unwrap().value;
    assert_eq!(listed["enumerations"], json!([{ "id": status, "name": "OrderStatus", "literals": [] }]));
    let packages = s.execute("uml.package.list", &json!({})).unwrap().value;
    let counted = packages["packages"].as_array().unwrap().iter().find(|p| p["id"] == sales.as_str()).unwrap().clone();
    assert_eq!((counted["classes"].as_u64(), counted["interfaces"].as_u64(), counted["enumerations"].as_u64()), (Some(1), Some(1), Some(1)));
    one_step(&mut s, "uml.enumeration.rename", json!({ "enumeration": status, "name": "State" }));
    assert_eq!(archforge_core::name_of(s.doc(), obj(s.doc(), &status)), "State");
    // Classes, interfaces and enumerations share their package's names, whatever the case.
    for (cmd, p, key, why) in [
        ("uml.enumeration.add", json!({ "package": sales, "name": "order" }), "name", "there is a class named Order already"),
        ("uml.enumeration.add", json!({ "package": sales, "name": "PAYABLE" }), "name", "there is an interface named Payable already"),
        ("uml.class.add", json!({ "package": sales, "name": "state" }), "name", "there is an enumeration named State already"),
        ("uml.interface.rename", json!({ "interface": payable, "name": "STATE" }), "name", "enumeration named State"),
        ("uml.enumeration.rename", json!({ "enumeration": status, "name": "Order" }), "name", "class named Order"),
        ("uml.enumeration.add", json!({ "package": sales, "name": "" }), "name", "name"),
        ("uml.enumeration.add", json!({ "package": order, "name": "X" }), "package", "not a package"),
        ("uml.enumeration.rename", json!({ "enumeration": order, "name": "X" }), "enumeration", "not an enumeration"),
        ("uml.class.rename", json!({ "class": status, "name": "X" }), "class", "not a class"),
        ("uml.enumeration.move", json!({ "enumeration": status, "package": order }), "package", "not a package"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains(key) && e.contains(why), "{cmd}: {e}");
    }
    // Moved to a package whose interface has its name: refused; to another: one step.
    let clash = run(&mut s, "uml.interface.add", json!({ "package": billing, "name": "state" }));
    assert!(
        refused(&mut s, "uml.enumeration.move", json!({ "enumeration": status, "package": billing })).contains("interface named state"),
        "it names the interface there"
    );
    run(&mut s, "uml.interface.rename", json!({ "interface": clash, "name": "Billable" }));
    one_step(&mut s, "uml.enumeration.move", json!({ "enumeration": status, "package": billing }));
    let st = s.doc();
    assert_eq!(archforge_uml::commands::package_of(st, obj(st, &status)), Some(obj(st, &billing)));
    // Removed through element.remove, with its card and its literals: one step.
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": billing }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": status, "x": 0, "y": 0 }));
    let mut s = with_literals(&s, &status, &["Placed", "Paid"], 1 << 20);
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [status] })).unwrap();
    let st = s.doc();
    assert!(st.by_uuid(Uuid::parse(&status).unwrap()).is_none_or(|o| !st.is_alive(o)));
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_diagram::NODE)), "its card went with it");
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(ENUMERATION_LITERAL)), "and its literals");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// An enumeration's card says «enumeration» over one compartment of its literals, empty when it has
/// none; it takes no attributes or operations, and an agent reads its literals.
#[test]
fn an_enumeration_draws_its_literals_in_one_compartment_and_takes_no_attributes_or_operations() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": status, "x": 0, "y": 0 }));
    // With no literal, one empty compartment: the card is honest about having none.
    let empty = card(&s, &status, &d);
    assert_eq!(empty.eyebrow, archforge_diagram::figure::Text::said("«enumeration»"));
    assert_eq!((empty.name.as_str(), rows_of(&empty)), ("OrderStatus", vec![Vec::<String>::new()]));
    assert_eq!(empty.more, ("+ {n} more literal", "+ {n} more literals"));
    assert!(!empty.slanted);
    // An enumeration's members are its literals: no attribute, no operation.
    for (cmd, p, why) in [
        ("uml.attribute.add", json!({ "class": status, "name": "code" }), "is an enumeration and keeps no attributes"),
        ("uml.operation.add", json!({ "class": status, "name": "next" }), "is an enumeration and keeps no operations"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(why) && e.contains(&status) && e.contains("a class, an interface, a data type or an association class does"), "{p}: {e}");
    }
    // Its literals, in their order, one row each.
    let mut s = with_literals(&s, &status, &["Placed", "Paid", "Shipped"], 1 << 20);
    let filled = card(&s, &status, &d);
    assert_eq!(rows_of(&filled), [vec!["Placed", "Paid", "Shipped"]]);
    let got = s.execute("uml.class.get", &json!({ "class": status })).unwrap().value;
    assert_eq!(got["kind"], "enumeration");
    let literals: Vec<&str> = got["literals"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert_eq!(literals, ["Placed", "Paid", "Shipped"]);
    assert_eq!((got["attributes"].as_array().map(Vec::len), got["operations"].as_array().map(Vec::len)), (Some(0), Some(0)));
    let listed = s.execute("uml.enumeration.list", &json!({ "package": pkg })).unwrap().value;
    assert_eq!(listed["enumerations"][0]["literals"], json!(["Placed", "Paid", "Shipped"]));
    // On another package's diagram, its package is named beside the keyword.
    let other = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": other }));
    run(&mut s, "diagram.place", json!({ "diagram": d2, "element": status, "x": 0, "y": 0 }));
    let outsider = card(&s, &status, &d2);
    assert_eq!(outsider.eyebrow, archforge_diagram::figure::Text::Said("«enumeration» · {package}", vec![("package", "sales".into())]));
    assert!(outsider.elsewhere);
}

/// An enumeration is a type an attribute, a parameter and a return may have, and they keep it
/// through its rename and its move.
#[test]
fn an_enumeration_types_attributes_parameters_and_returns_and_they_keep_it_through_a_rename_and_a_move() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let shared = run(&mut s, "uml.package.add", json!({ "name": "shared" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "OrderStatus" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let types = s.execute("uml.type.list", &json!({})).unwrap().value;
    assert!(types["types"].as_array().unwrap().iter().any(|t| t["id"] == status.as_str() && t["name"] == "OrderStatus"), "{types}");
    one_step(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status", "type": status }));
    one_step(
        &mut s,
        "uml.operation.add",
        json!({ "class": order, "name": "advance", "parameters": [{ "name": "to", "type": status }], "returns": status }),
    );
    let typed = |s: &mut S| {
        let got = s.execute("uml.class.get", &json!({ "class": order })).unwrap().value;
        let attribute = got["attributes"][0]["type"].clone();
        let parameter = got["operations"][0]["parameters"][0]["type"].clone();
        let returns = got["operations"][0]["returns"].clone();
        (attribute, parameter, returns)
    };
    let want = |name: &str| json!({ "id": status, "name": name });
    assert_eq!(typed(&mut s), (want("OrderStatus"), want("OrderStatus"), want("OrderStatus")));
    // Renamed and moved, it stays their type, under its new name.
    run(&mut s, "uml.enumeration.rename", json!({ "enumeration": status, "name": "State" }));
    run(&mut s, "uml.enumeration.move", json!({ "enumeration": status, "package": shared }));
    assert_eq!(typed(&mut s), (want("State"), want("State"), want("State")));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": sales }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    assert_eq!(rows_of(&card(&s, &order, &d)), [vec!["+ status : State"], vec!["+ advance(to : State) : State"]]);
    // Removed, it leaves them untyped, as one step; undone, they are typed by it again.
    s.execute("element.remove", &json!({ "ids": [status] })).unwrap();
    assert_eq!(typed(&mut s), (Json::Null, Json::Null, Json::Null), "untyped once it is gone");
    assert_eq!(rows_of(&card(&s, &order, &d)), [vec!["+ status"], vec!["+ advance(to)"]]);
    s.undo().unwrap();
    assert_eq!(typed(&mut s), (want("State"), want("State"), want("State")), "undo types them again");
}

/// An enumeration specializes only an enumeration and is specialized only by one; no association
/// joins it, nor does it realize or get realized; a dependency may touch it. Each refusal changes
/// nothing.
#[test]
fn an_enumeration_specializes_only_enumerations_joins_no_association_and_realizes_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let closing = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "ClosingStatus" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Invoice" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    // An enumeration specializes an enumeration, one step.
    let g = one_step(&mut s, "uml.generalization.add", json!({ "specific": closing, "general": status }));
    // A dependency may touch it.
    one_step(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": status }));
    // Each refusal names the offending id, under the key of the end that breaks the rule, and what
    // is accepted instead.
    let no_association = "is an enumeration, a data type, and joins no association; an association joins classes and interfaces";
    for (cmd, p, key, id, why) in [
        (
            "uml.generalization.add",
            json!({ "specific": status, "general": order }),
            "general",
            &order,
            "is a class; an enumeration specializes only an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": status, "general": payable }),
            "general",
            &payable,
            "is an interface; an enumeration specializes only an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": order, "general": status }),
            "specific",
            &order,
            "is a class; only an enumeration specializes an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": payable, "general": status }),
            "specific",
            &payable,
            "is an interface; only an enumeration specializes an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": status, "general": status }),
            "general",
            &status,
            "an enumeration cannot specialize itself; it specializes another enumeration",
        ),
        ("uml.generalization.add", json!({ "specific": closing, "general": status }), "general", &status, "specializes that enumeration already"),
        ("uml.generalization.add", json!({ "specific": status, "general": closing }), "general", &closing, "cycle"),
        ("uml.association.add", json!({ "from": order, "to": status }), "to", &status, no_association),
        ("uml.association.add", json!({ "from": status, "to": order, "kind": "composition" }), "from", &status, no_association),
        ("uml.realization.add", json!({ "class": status, "interface": payable }), "class", &status, "only a class implements an interface"),
        ("uml.realization.add", json!({ "class": order, "interface": status }), "interface", &status, "a class realizes only an interface"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        let realization = cmd == "uml.realization.add";
        assert!(e.contains(why) && (realization || e.contains(id.as_str())), "{cmd} {p}: {e}");
        assert!(e.contains(&format!("key: Some(\"{key}\")")), "{cmd} {p} refused under {key}: {e}");
    }
    // A reconnected end keeps the same rules: an association's end never moves onto one.
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": invoice }));
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": a, "end": "target", "class": status }));
    assert!(e.contains("joins no association"), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "target", "class": order }));
    assert!(e.contains("an enumeration specializes only an enumeration"), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "source", "class": invoice }));
    assert!(e.contains("only an enumeration specializes an enumeration"), "{e}");
    // Removing it takes the relationships that touch it along, in one step.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [status] })).unwrap();
    let st = s.doc();
    let lines = [archforge_uml::GENERALIZATION, archforge_uml::DEPENDENCY];
    assert_eq!(st.alive().filter(|o| st.class_of(*o).is_ok_and(|c| lines.contains(&c))).count(), 0);
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// Enumerations with their literals, generalizations and cards come back from their rows identical.
#[test]
fn enumerations_with_literals_and_relationships_come_back_from_their_rows_identical() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let closing = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "ClosingStatus" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status", "type": status }));
    run(&mut s, "uml.generalization.add", json!({ "specific": closing, "general": status }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": status, "x": 0, "y": 0 }));
    let s = with_literals(&s, &status, &["Placed", "Paid", "Shipped"], 1 << 20);
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let e = obj(&back, &status);
    assert_eq!(back.class_of(e).unwrap(), ENUMERATION);
    let names = |os: Vec<Obj>| os.into_iter().map(|o| archforge_core::name_of(&back, o)).collect::<Vec<_>>();
    assert_eq!(names(archforge_uml::commands::literals(&back, e)), ["Placed", "Paid", "Shipped"], "in their order");
    assert_eq!(names(archforge_uml::commands::enumerations(&back, obj(&back, &pkg))), ["OrderStatus", "ClosingStatus"]);
    let back = session_over(back);
    assert_eq!(rows_of(&card(&back, &status, &d)), [vec!["Placed", "Paid", "Shipped"]]);
}

/// The names of `enumeration`'s literals, in their order.
fn literal_names(s: &S, enumeration: &str) -> Vec<String> {
    let st = s.doc();
    archforge_uml::commands::literals(st, obj(st, enumeration)).into_iter().map(|l| archforge_core::name_of(st, l)).collect()
}

/// Literals are added at the end or at a place, renamed, moved and removed, each one undo step;
/// a literal keeps its identity through a rename and a move; names clash within their enumeration
/// whatever the case, and every refusal names the value it refuses and what is accepted, changing
/// nothing.
#[test]
fn literals_are_added_renamed_moved_and_removed_each_one_step_their_names_apart_whatever_the_case() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "OrderStatus" }));
    let other = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "Priority" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let placed = run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Placed" }));
    let st = s.doc();
    assert_eq!(st.class_of(obj(st, &placed)).unwrap(), ENUMERATION_LITERAL);
    assert_eq!(s.view().selection, vec![obj(st, &placed)], "the new literal is selected");
    let shipped = one_step(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Shipped" }));
    // At a place: between the two.
    let paid = one_step(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Paid", "at": 1 }));
    assert_eq!(literal_names(&s, &status), ["Placed", "Paid", "Shipped"]);
    // The same name in another enumeration is free.
    run(&mut s, "uml.literal.add", json!({ "enumeration": other, "name": "Paid" }));
    // Renamed in place: the same element, at the same place; its own name in another case is free.
    assert_eq!(one_step(&mut s, "uml.literal.rename", json!({ "literal": paid, "name": "Settled" })), paid);
    assert_eq!(literal_names(&s, &status), ["Placed", "Settled", "Shipped"]);
    one_step(&mut s, "uml.literal.rename", json!({ "literal": paid, "name": "SETTLED" }));
    // Moved up and down: the same element at its new place.
    assert_eq!(one_step(&mut s, "uml.literal.move", json!({ "literal": shipped, "at": 0 })), shipped);
    assert_eq!(literal_names(&s, &status), ["Shipped", "Placed", "SETTLED"]);
    one_step(&mut s, "uml.literal.move", json!({ "literal": shipped, "at": 2 }));
    assert_eq!(literal_names(&s, &status), ["Placed", "SETTLED", "Shipped"]);
    let st = s.doc();
    let ids: Vec<String> = archforge_uml::commands::literals(st, obj(st, &status)).into_iter().map(|l| st.uuid(l).unwrap().to_string()).collect();
    assert_eq!(ids, [placed.clone(), paid.clone(), shipped.clone()], "each keeps its identity");
    // Refusals: each names the offending value and what is accepted, under its key.
    for (cmd, p, key, why) in [
        (
            "uml.literal.add",
            json!({ "enumeration": status, "name": "placed" }),
            "name",
            "there is a literal named Placed already in OrderStatus, and \\\"placed\\\" matches it",
        ),
        ("uml.literal.add", json!({ "enumeration": status, "name": "shipped" }), "name", "choose a name none of OrderStatus's other literals has"),
        ("uml.literal.rename", json!({ "literal": placed, "name": "Settled" }), "name", "there is a literal named SETTLED already"),
        ("uml.literal.add", json!({ "enumeration": status, "name": "" }), "name", "the name is empty"),
        ("uml.literal.add", json!({ "enumeration": status, "name": " Lost" }), "name", "starts or ends with a space"),
        (
            "uml.literal.add",
            json!({ "enumeration": status, "name": "Lost", "at": 4 }),
            "at",
            "4 is past the end of 3 literals; a place from 0 to 3, or -1 for the end",
        ),
        ("uml.literal.add", json!({ "enumeration": order, "name": "Lost" }), "enumeration", "is not an enumeration"),
        ("uml.literal.rename", json!({ "literal": order, "name": "Lost" }), "literal", "is not a literal"),
        ("uml.literal.move", json!({ "literal": placed, "at": 3 }), "at", "3 is past the last literal; a place from 0 to 2"),
        ("uml.literal.move", json!({ "literal": "nope", "at": 0 }), "literal", "\\\"nope\\\" is not an id"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(why), "{cmd} {p}: {e}");
        assert!(e.contains(&format!("key: Some(\"{key}\")")), "{cmd} {p} refused under {key}: {e}");
    }
    // The one rule the shell asks too.
    let st = s.doc();
    assert_eq!(archforge_uml::members::literal_clash(st, obj(st, &status), "pLaCeD", None), Some(obj(st, &placed)));
    assert_eq!(archforge_uml::members::literal_clash(st, obj(st, &status), "Placed", Some(obj(st, &placed))), None, "its own name");
    assert_eq!(archforge_uml::members::literal_owner(st, obj(st, &placed)), Some(obj(st, &status)));
    // Removed through element.remove, one step; the others keep their order.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [paid] })).unwrap();
    assert_eq!(literal_names(&s, &status), ["Placed", "Shipped"]);
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "undo of the removal");
    assert_eq!(literal_names(&s, &status), ["Placed", "SETTLED", "Shipped"]);
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after, "redo of the removal");
    // uml.class.get gives them in their order.
    let got = s.execute("uml.class.get", &json!({ "class": status })).unwrap().value;
    assert_eq!(
        got["literals"],
        json!([{ "id": placed, "name": "Placed", "description": "" }, { "id": shipped, "name": "Shipped", "description": "" }])
    );
}

/// Literals added and moved through the commands keep their order through a save and a reopen.
#[test]
fn literals_added_and_moved_keep_their_order_through_save_and_reopen() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": status, "x": 0, "y": 0 }));
    for name in ["Paid", "Shipped", "Cancelled"] {
        run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": name }));
    }
    let placed = run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Placed", "at": 0 }));
    let cancelled = archforge_uml::commands::literals(s.doc(), obj(s.doc(), &status))[3];
    let cancelled = s.doc().uuid(cancelled).unwrap().to_string();
    run(&mut s, "uml.literal.move", json!({ "literal": cancelled, "at": 1 }));
    let order = ["Placed", "Cancelled", "Paid", "Shipped"];
    assert_eq!(literal_names(&s, &status), order);
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    let back = session_over(back);
    assert_eq!(literal_names(&back, &status), order, "in their order after the reopen");
    assert_eq!(rows_of(&card(&back, &status, &d)), [order.to_vec()]);
    let st = back.doc();
    assert_eq!(st.uuid(archforge_uml::commands::literals(st, obj(st, &status))[0]).unwrap().to_string(), placed, "the same literals");
}

/// An enumeration holds at most `MAX_LITERALS` literals: the add past it is refused, naming the
/// enumeration and the cap, and changes nothing; one removed makes room again.
#[test]
fn an_enumeration_takes_no_literal_past_the_cap() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "Codes" }));
    let max = archforge_uml::members::MAX_LITERALS;
    assert_eq!(max, 256, "the cap ER's enum types keep too");
    let names: Vec<String> = (0..max - 1).map(|i| format!("C{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut s = with_literals(&s, &status, &names, 1 << 20);
    let last = one_step(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Last" }));
    assert_eq!(literal_names(&s, &status).len(), max);
    let e = refused(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "More" }));
    assert!(e.contains(&format!("{status} holds 256 literals already, and an enumeration holds at most 256")), "{e}");
    assert!(e.contains("key: Some(\"enumeration\")"), "{e}");
    s.execute("element.remove", &json!({ "ids": [last] })).unwrap();
    run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "More" }));
}

/// A card lists at most `MAX_CARD_ROWS` literals and counts the rest in `hidden`, so the rows built
/// every frame stay bounded; a class's members are capped the same way.
#[test]
fn a_card_lists_a_bounded_number_of_rows_and_counts_the_rest() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "Codes" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    let max = archforge_uml::figures::MAX_CARD_ROWS;
    let names: Vec<String> = (0..max + 3).map(|i| format!("C{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let s = with_literals(&s, &status, &names, 1 << 20);
    let c = card(&s, &status, &d);
    assert_eq!((c.compartments[0].len(), c.hidden), (max, 3), "the rest counted, not listed");
    assert_eq!(c.compartments[0][0].text, "C0", "the first ones listed, in their order");
}

/// The command sweep, with an enumeration, one of its literals and a class among the ids, breaks
/// nothing.
#[test]
fn the_sweep_over_enumerations_breaks_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let class = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let literal = run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Placed" }));
    run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "Paid" }));
    // The literal commands are in the registry the sweep walks.
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.literal.add", "uml.literal.rename", "uml.literal.move"] {
        assert!(listed.iter().any(|c| c == cmd), "{cmd} is swept");
    }
    let ids = [pkg.as_str(), status.as_str(), literal.as_str(), class.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}
