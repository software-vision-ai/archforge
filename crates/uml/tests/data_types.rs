//! Data types through the engine (AR-123): added in a package, renamed, moved and removed, each one
//! undo step; their names told apart from their package's classes, interfaces and enumerations; a
//! real DataType in the model, never an enumeration or a class; drawn as a card with the keyword
//! «dataType» over attributes and operations, which the member commands add as a class's; offered
//! as a type once, which a member keeps through a rename and a move and loses, undoably, with its
//! removal; specialized only by and only of data types, joined by no association, realizing
//! nothing; the generalization rules coherent across every kind; saved as rows and read back
//! identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::{ALONE, CLASS_DIAGRAM, DATA_TYPE, registry};
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

/// The card of `element` on diagram `d`.
fn card(s: &S, element: &str, d: &str) -> archforge_diagram::figure::Card {
    let st = s.doc();
    archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(st, element), obj(st, d)).unwrap()
}

/// The rows of each compartment of `card`.
fn rows_of(card: &archforge_diagram::figure::Card) -> Vec<Vec<String>> {
    card.compartments.iter().map(|c| c.iter().map(|r| r.text.clone()).collect()).collect()
}

/// Data types are added, renamed, moved and removed, each one undo step, as real DataTypes, named
/// apart from their package's classes, interfaces and enumerations.
#[test]
fn a_data_type_is_added_renamed_moved_and_removed_each_one_step_its_name_free_among_the_classifiers() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "OrderStatus" }));
    let before = picture(s.doc());
    let money = run(&mut s, "uml.data-type.add", json!({ "package": sales, "name": "Money" }));
    let st = s.doc();
    assert_eq!(st.class_of(obj(st, &money)).unwrap(), DATA_TYPE, "a DataType, not a class or an enumeration");
    assert_eq!(s.view().selection, vec![obj(st, &money)], "the new data type is selected");
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    let st = s.doc();
    assert_eq!(
        archforge_uml::commands::classifiers(st, obj(st, &sales)),
        vec![obj(st, &order), obj(st, &payable), obj(st, &status), obj(st, &money)],
        "a classifier of its package, in its order"
    );
    // Each kind's own list holds only its kind: the data types neither the enumeration nor the class.
    assert_eq!(archforge_uml::commands::data_types(st, obj(st, &sales)), vec![obj(st, &money)]);
    assert_eq!(archforge_uml::commands::enumerations(st, obj(st, &sales)), vec![obj(st, &status)]);
    assert_eq!(archforge_uml::commands::classes(st, obj(st, &sales)), vec![obj(st, &order)]);
    let listed = s.execute("uml.data-type.list", &json!({ "package": sales })).unwrap().value;
    assert_eq!(listed["data_types"], json!([{ "id": money, "name": "Money", "attributes": 0, "operations": 0 }]));
    let packages = s.execute("uml.package.list", &json!({})).unwrap().value;
    let counted = packages["packages"].as_array().unwrap().iter().find(|p| p["id"] == sales.as_str()).unwrap().clone();
    assert_eq!(
        (counted["classes"].as_u64(), counted["interfaces"].as_u64(), counted["enumerations"].as_u64(), counted["data_types"].as_u64()),
        (Some(1), Some(1), Some(1), Some(1))
    );
    one_step(&mut s, "uml.data-type.rename", json!({ "data_type": money, "name": "Amount" }));
    assert_eq!(archforge_core::name_of(s.doc(), obj(s.doc(), &money)), "Amount");
    // Classes, interfaces, enumerations and data types share their package's names, whatever the case.
    for (cmd, p, key, why) in [
        ("uml.data-type.add", json!({ "package": sales, "name": "order" }), "name", "there is a class named order already"),
        ("uml.data-type.add", json!({ "package": sales, "name": "PAYABLE" }), "name", "there is an interface named PAYABLE already"),
        ("uml.data-type.add", json!({ "package": sales, "name": "orderstatus" }), "name", "there is an enumeration named orderstatus already"),
        ("uml.class.add", json!({ "package": sales, "name": "amount" }), "name", "there is a data type named amount already"),
        ("uml.enumeration.add", json!({ "package": sales, "name": "AMOUNT" }), "name", "there is a data type named AMOUNT already"),
        ("uml.interface.rename", json!({ "interface": payable, "name": "Amount" }), "name", "data type named Amount"),
        ("uml.data-type.rename", json!({ "data_type": money, "name": "Order" }), "name", "class named Order"),
        ("uml.data-type.add", json!({ "package": sales, "name": "" }), "name", "name"),
        ("uml.data-type.add", json!({ "package": order, "name": "X" }), "package", "not a package"),
        ("uml.data-type.rename", json!({ "data_type": order, "name": "X" }), "data_type", "not a data type"),
        ("uml.data-type.rename", json!({ "data_type": status, "name": "X" }), "data_type", "not a data type"),
        ("uml.enumeration.rename", json!({ "enumeration": money, "name": "X" }), "enumeration", "not an enumeration"),
        ("uml.class.rename", json!({ "class": money, "name": "X" }), "class", "not a class"),
        ("uml.data-type.move", json!({ "data_type": money, "package": order }), "package", "not a package"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(&format!("key: Some(\"{key}\")")) && e.contains(why), "{cmd} {p}: {e}");
    }
    // Moved to a package whose class has its name: refused; to another: one step.
    let clash = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "amount" }));
    assert!(refused(&mut s, "uml.data-type.move", json!({ "data_type": money, "package": billing })).contains("class named Amount"));
    run(&mut s, "uml.class.rename", json!({ "class": clash, "name": "Invoice" }));
    one_step(&mut s, "uml.data-type.move", json!({ "data_type": money, "package": billing }));
    let st = s.doc();
    assert_eq!(archforge_uml::commands::package_of(st, obj(st, &money)), Some(obj(st, &billing)));
    // Removed through element.remove, with its card and its members: one step.
    run(&mut s, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real" }));
    run(&mut s, "uml.operation.add", json!({ "class": money, "name": "add" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": billing }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": money, "x": 0, "y": 0 }));
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [money] })).unwrap();
    let st = s.doc();
    assert!(st.by_uuid(Uuid::parse(&money).unwrap()).is_none_or(|o| !st.is_alive(o)));
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_diagram::NODE)), "its card went with it");
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_uml::OPERATION)), "and its operation");
    assert!(
        st.alive().all(|o| st.class_of(o).ok() != Some(archforge_uml::PROPERTY) || archforge_core::name_of(st, o) != "amount"),
        "and its attribute"
    );
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// A data type's card says «dataType» over its attributes and its operations, which the member
/// commands add, set, move and remove as a class's, each one step; an agent reads them; an
/// enumeration still takes none.
#[test]
fn a_data_type_draws_its_attributes_and_operations_which_the_member_commands_edit_as_a_class_s() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": money, "x": 0, "y": 0 }));
    // With no member, two empty compartments, as an empty class has.
    let empty = card(&s, &money, &d);
    assert_eq!(empty.eyebrow, archforge_diagram::figure::Text::said("«dataType»"));
    assert_eq!((empty.name.as_str(), rows_of(&empty)), ("Money", vec![Vec::<String>::new(), Vec::new()]));
    assert_eq!(empty.more, ("+ {n} more member", "+ {n} more members"));
    assert!(!empty.slanted);
    // Its members, each one step, in DataType's own lists.
    let currency = one_step(&mut s, "uml.attribute.add", json!({ "class": money, "name": "currency", "type": "String" }));
    let amount = one_step(&mut s, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real", "at": 0 }));
    let add = one_step(
        &mut s,
        "uml.operation.add",
        json!({ "class": money, "name": "add", "parameters": [{ "name": "other", "type": money }], "returns": money }),
    );
    let st = s.doc();
    let owned = |prop| st.list(obj(st, &money), prop).unwrap().iter().filter_map(Value::as_ref).collect::<Vec<_>>();
    assert_eq!(owned(archforge_uml::data_type::OWNED_ATTRIBUTE), vec![obj(st, &amount), obj(st, &currency)]);
    assert_eq!(owned(archforge_uml::data_type::OWNED_OPERATION), vec![obj(st, &add)]);
    assert_eq!(rows_of(&card(&s, &money, &d)), [vec!["+ amount : Real", "+ currency : String"], vec!["+ add(other : Money) : Money"]]);
    // Set, moved and told apart as a class's; any visibility, as a class's members have.
    one_step(&mut s, "uml.attribute.set", json!({ "attribute": currency, "name": "code", "type": "String", "visibility": "private" }));
    one_step(&mut s, "uml.attribute.move", json!({ "attribute": currency, "at": 0 }));
    let e = refused(&mut s, "uml.attribute.add", json!({ "class": money, "name": "AMOUNT" }));
    assert!(e.contains("the data type has an attribute named AMOUNT already"), "{e}");
    assert_eq!(rows_of(&card(&s, &money, &d))[0], ["- code : String", "+ amount : Real"]);
    let got = s.execute("uml.class.get", &json!({ "class": money })).unwrap().value;
    assert_eq!((got["kind"].as_str(), got["name"].as_str(), got["abstract"].as_bool()), (Some("data_type"), Some("Money"), Some(false)));
    let names = |v: &Json| v.as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!((names(&got["attributes"]), names(&got["operations"])), (vec!["code".to_string(), "amount".into()], vec!["add".to_string()]));
    assert!(got.get("literals").is_none(), "a data type has no literals");
    // A member removed through element.remove, one step.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [add] })).unwrap();
    assert_eq!(rows_of(&card(&s, &money, &d))[1], Vec::<String>::new());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    // An enumeration, a DataType in UML's metamodel too, still keeps no attribute: its members are
    // its literals, and the refusal says what keeps them.
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let e = refused(&mut s, "uml.attribute.add", json!({ "class": status, "name": "code" }));
    assert!(e.contains(&status) && e.contains("a class, an interface, a data type or an association class does"), "{e}");
    // On another package's diagram, its package is named beside the keyword.
    let other = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": other }));
    run(&mut s, "diagram.place", json!({ "diagram": d2, "element": money, "x": 0, "y": 0 }));
    let outsider = card(&s, &money, &d2);
    assert_eq!(outsider.eyebrow, archforge_diagram::figure::Text::Said("«dataType» · {package}", vec![("package", "sales".into())]));
    assert!(outsider.elsewhere);
}

/// A data type is a type an attribute, a parameter and a return may have, listed once; they keep it
/// through its rename and its move, and lose it with its removal, which undo gives back.
#[test]
fn a_data_type_types_members_listed_once_kept_through_a_rename_and_a_move_and_lost_undoably_with_its_removal() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let shared = run(&mut s, "uml.package.add", json!({ "name": "shared" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": sales, "name": "Money" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    // A built-in type made in the package Types, so both kinds of type are in the model.
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "number", "type": "Integer" }));
    let types = s.execute("uml.type.list", &json!({})).unwrap().value;
    let held = types["types"].as_array().unwrap();
    assert_eq!(held.iter().filter(|t| t["id"] == money.as_str()).count(), 1, "listed once: {types}");
    assert_eq!(held.iter().filter(|t| t["name"] == "Integer").count(), 1, "the built-in type once: {types}");
    one_step(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total", "type": money }));
    one_step(
        &mut s,
        "uml.operation.add",
        json!({ "class": order, "name": "pay", "parameters": [{ "name": "amount", "type": money }], "returns": money }),
    );
    let typed = |s: &mut S| {
        let got = s.execute("uml.class.get", &json!({ "class": order })).unwrap().value;
        (got["attributes"][1]["type"].clone(), got["operations"][0]["parameters"][0]["type"].clone(), got["operations"][0]["returns"].clone())
    };
    let want = |name: &str| json!({ "id": money, "name": name });
    assert_eq!(typed(&mut s), (want("Money"), want("Money"), want("Money")));
    // Renamed and moved, it stays their type, under its new name.
    run(&mut s, "uml.data-type.rename", json!({ "data_type": money, "name": "Amount" }));
    run(&mut s, "uml.data-type.move", json!({ "data_type": money, "package": shared }));
    assert_eq!(typed(&mut s), (want("Amount"), want("Amount"), want("Amount")));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": sales }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    assert_eq!(rows_of(&card(&s, &order, &d)), [vec!["+ number : Integer", "+ total : Amount"], vec!["+ pay(amount : Amount) : Amount"]]);
    // Removed, it leaves them untyped, as one step; undone, they are typed by it again.
    s.execute("element.remove", &json!({ "ids": [money] })).unwrap();
    assert_eq!(typed(&mut s), (Json::Null, Json::Null, Json::Null), "untyped once it is gone");
    assert_eq!(rows_of(&card(&s, &order, &d)), [vec!["+ number : Integer", "+ total"], vec!["+ pay(amount)"]]);
    s.undo().unwrap();
    assert_eq!(typed(&mut s), (want("Amount"), want("Amount"), want("Amount")), "undo types them again");
}

/// A data type specializes only a data type and is specialized only by one; no association joins
/// it, nor does it realize or get realized; a dependency may touch it. Each refusal names the
/// offending id under the key of the end that breaks the rule, and changes nothing.
#[test]
fn a_data_type_specializes_only_data_types_joins_no_association_and_realizes_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let price = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Price" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Invoice" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    // A data type specializes a data type, one step; a dependency may touch it either way.
    let g = one_step(&mut s, "uml.generalization.add", json!({ "specific": price, "general": money }));
    one_step(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": money }));
    one_step(&mut s, "uml.dependency.add", json!({ "client": money, "supplier": payable }));
    let no_association = "is a data type and joins no association; an association joins classes and interfaces";
    for (cmd, p, key, id, why) in [
        (
            "uml.generalization.add",
            json!({ "specific": money, "general": order }),
            "general",
            &order,
            "is a class; a data type specializes only a data type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": money, "general": payable }),
            "general",
            &payable,
            "is an interface; a data type specializes only a data type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": order, "general": money }),
            "specific",
            &order,
            "is a class; only a data type specializes a data type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": payable, "general": money }),
            "specific",
            &payable,
            "is an interface; only a data type specializes a data type",
        ),
        // Between a data type and an enumeration the enumeration's own rule speaks.
        (
            "uml.generalization.add",
            json!({ "specific": money, "general": status }),
            "specific",
            &money,
            "is a data type; only an enumeration specializes an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": status, "general": money }),
            "general",
            &money,
            "is a data type; an enumeration specializes only an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": money, "general": money }),
            "general",
            &money,
            "a data type cannot specialize itself; it specializes another data type",
        ),
        ("uml.generalization.add", json!({ "specific": price, "general": money }), "general", &money, "specializes that data type already"),
        ("uml.generalization.add", json!({ "specific": money, "general": price }), "general", &price, "cycle"),
        ("uml.association.add", json!({ "from": order, "to": money }), "to", &money, no_association),
        ("uml.association.add", json!({ "from": money, "to": order, "kind": "aggregation" }), "from", &money, no_association),
        (
            "uml.realization.add",
            json!({ "class": money, "interface": payable }),
            "class",
            &money,
            "is a data type; only a class implements an interface",
        ),
        (
            "uml.realization.add",
            json!({ "class": order, "interface": money }),
            "interface",
            &money,
            "is a data type; a class realizes only an interface",
        ),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(why) && e.contains(id.as_str()), "{cmd} {p}: {e}");
        assert!(e.contains(&format!("key: Some(\"{key}\")")), "{cmd} {p} refused under {key}: {e}");
    }
    // The enumeration's refusal keeps its own wording beside the data type's.
    let e = refused(&mut s, "uml.association.add", json!({ "from": order, "to": status }));
    assert!(e.contains("is an enumeration, a data type, and joins no association"), "{e}");
    // A reconnected end keeps the same rules.
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": invoice }));
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": a, "end": "target", "class": money }));
    assert!(e.contains(no_association) && e.contains(&money), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "target", "class": order }));
    assert!(e.contains("a data type specializes only a data type"), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "source", "class": invoice }));
    assert!(e.contains("only a data type specializes a data type"), "{e}");
    let r = run(&mut s, "uml.realization.add", json!({ "class": order, "interface": payable }));
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "source", "class": money }));
    assert!(e.contains("only a class implements an interface") && e.contains(&money), "{e}");
    // Removing it takes the relationships that touch it along, in one step.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [money] })).unwrap();
    let st = s.doc();
    let lines = [archforge_uml::GENERALIZATION, archforge_uml::DEPENDENCY];
    assert_eq!(st.alive().filter(|o| st.class_of(*o).is_ok_and(|c| lines.contains(&c))).count(), 0);
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// Generalization stays coherent across every kind: between two classifiers of one kind it is
/// drawn, between two of different kinds refused, and the refusal's key names the end whose
/// classifier it names, the one whose kind's rule is broken.
#[test]
fn generalization_joins_two_classifiers_of_one_kind_and_every_other_pair_is_refused_naming_the_end_that_breaks_the_rule() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "kinds" }));
    let kinds = [
        ("class", "uml.class.add"),
        ("interface", "uml.interface.add"),
        ("enumeration", "uml.enumeration.add"),
        ("data type", "uml.data-type.add"),
        ("primitive type", "uml.primitive-type.add"),
    ];
    let made: Vec<(&str, String, String)> = kinds
        .iter()
        .map(|(kind, cmd)| {
            let a = run(&mut s, cmd, json!({ "package": pkg, "name": format!("{kind} a") }));
            let b = run(&mut s, cmd, json!({ "package": pkg, "name": format!("{kind} b") }));
            (*kind, a, b)
        })
        .collect();
    for (specific_kind, specific, _) in &made {
        for (general_kind, _, general) in &made {
            let p = json!({ "specific": specific, "general": general });
            if specific_kind == general_kind {
                let g = one_step(&mut s, "uml.generalization.add", p);
                s.execute("element.remove", &json!({ "ids": [g] })).unwrap();
                continue;
            }
            let e = refused(&mut s, "uml.generalization.add", p);
            let (key, named) = if e.contains("key: Some(\"specific\")") { ("specific", specific) } else { ("general", general) };
            let kind = if key == "specific" { specific_kind } else { general_kind };
            assert!(e.contains(named.as_str()), "{specific_kind} to {general_kind}: the {key} end is named: {e}");
            let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            assert!(e.contains(&format!("{named} is {article} {kind};")), "{specific_kind} to {general_kind}: says what it is: {e}");
            // The rule said is the kind's whose rule is broken: never the class's rule for another kind.
            assert!(!e.contains("a class cannot specialize an interface") || *specific_kind == "class", "{e}");
        }
    }
}

/// Data types with their members, a generalization and a card come back from their rows identical.
#[test]
fn data_types_with_members_and_relationships_come_back_from_their_rows_identical() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let price = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Price" }));
    run(&mut s, "uml.attribute.add", json!({ "class": money, "name": "currency", "type": "String" }));
    run(&mut s, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real", "at": 0 }));
    run(&mut s, "uml.operation.add", json!({ "class": money, "name": "add", "parameters": [{ "name": "other", "type": money }], "returns": money }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total", "type": money }));
    run(&mut s, "uml.generalization.add", json!({ "specific": price, "general": money }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": money, "x": 0, "y": 0 }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let t = obj(&back, &money);
    assert_eq!(back.class_of(t).unwrap(), DATA_TYPE, "still a DataType");
    let names = |os: Vec<Obj>| os.into_iter().map(|o| archforge_core::name_of(&back, o)).collect::<Vec<_>>();
    assert_eq!(names(archforge_uml::members::attributes(&back, t)), ["amount", "currency"], "in their order");
    assert_eq!(names(archforge_uml::commands::data_types(&back, obj(&back, &pkg))), ["Money", "Price"]);
    let back = session_over(back);
    assert_eq!(rows_of(&card(&back, &money, &d)), [vec!["+ amount : Real", "+ currency : String"], vec!["+ add(other : Money) : Money"]]);
    let got = back.registry().infos(&back.facts()).into_iter().any(|i| i.id == "uml.data-type.add");
    assert!(got, "the commands are offered over the project read back");
}

/// The command sweep, with a data type, its attribute and operation and a class among the ids,
/// breaks nothing.
#[test]
fn the_sweep_over_data_types_breaks_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let class = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let attribute = run(&mut s, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real" }));
    let operation = run(&mut s, "uml.operation.add", json!({ "class": money, "name": "add" }));
    // The data type's commands are in the registry the sweep walks.
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.data-type.add", "uml.data-type.rename", "uml.data-type.move", "uml.data-type.list"] {
        assert!(listed.iter().any(|c| c == cmd), "{cmd} is swept");
    }
    let ids = [pkg.as_str(), money.as_str(), attribute.as_str(), operation.as_str(), class.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}
