//! Primitive types through the engine (AR-124): a primitive type of the model's own added in a
//! package, renamed, moved and removed, each one undo step; its name told apart from its package's
//! other classifiers; a real PrimitiveType with no members, drawn as a card with the keyword
//! «primitive» over its name and no compartment; a type a member keeps through a rename and a move
//! and loses, undoably, with its removal; the built-in types (Integer and the others, made in the
//! package Types when first used) never duplicated, never swapped for a type of the model's own
//! nor one of those for them, and never renamed or moved; specialized only by and only of
//! primitive types, a built-in one specializing nothing, joined by no association, realizing
//! nothing; saved as rows and read back identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::{ALONE, CLASS_DIAGRAM, PRIMITIVE_TYPE, registry};
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

/// Every live primitive type named `name`, whatever its package.
fn primitives_named(st: &Store, name: &str) -> Vec<Obj> {
    st.alive().filter(|o| st.class_of(*o).ok() == Some(PRIMITIVE_TYPE) && archforge_core::name_of(st, *o) == name).collect()
}

/// The id of the built-in type `name`, made in Types when first used.
fn built_in(s: &S, name: &str) -> String {
    let st = s.doc();
    let t = st
        .alive()
        .find(|o| archforge_uml::members::is_built_in(st, *o) && archforge_core::name_of(st, *o) == name)
        .unwrap_or_else(|| panic!("{name} is made in Types when first used"));
    st.uuid(t).unwrap().to_string()
}

/// The type of the attribute `attribute` of `class`, as `uml.class.get` gives it.
fn type_of(s: &mut S, class: &str, attribute: &str) -> Json {
    let got = s.execute("uml.class.get", &json!({ "class": class })).unwrap().value;
    got["attributes"].as_array().unwrap().iter().find(|a| a["name"] == attribute).map(|a| a["type"].clone()).unwrap_or(Json::Null)
}

/// Primitive types are added, renamed, moved and removed, each one undo step, as real
/// PrimitiveTypes, named apart from their package's classes, interfaces, enumerations and data types.
#[test]
fn a_primitive_type_is_added_renamed_moved_and_removed_each_one_step_its_name_free_among_the_classifiers() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "OrderStatus" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": sales, "name": "Money" }));
    let before = picture(s.doc());
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": sales, "name": "Sku" }));
    let st = s.doc();
    assert_eq!(st.class_of(obj(st, &sku)).unwrap(), PRIMITIVE_TYPE, "a PrimitiveType, not a data type");
    assert_eq!(s.view().selection, vec![obj(st, &sku)], "the new primitive type is selected");
    assert!(!archforge_uml::members::is_built_in(st, obj(st, &sku)), "the model's own, no built-in type");
    let after = picture(s.doc());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
    s.redo().unwrap();
    assert_eq!(picture(s.doc()), after);
    let st = s.doc();
    assert_eq!(
        archforge_uml::commands::classifiers(st, obj(st, &sales)),
        vec![obj(st, &order), obj(st, &payable), obj(st, &status), obj(st, &money), obj(st, &sku)],
        "a classifier of its package, in its order"
    );
    // Each kind's own list holds only its kind: a primitive type is no data type here.
    assert_eq!(archforge_uml::commands::primitive_types(st, obj(st, &sales)), vec![obj(st, &sku)]);
    assert_eq!(archforge_uml::commands::data_types(st, obj(st, &sales)), vec![obj(st, &money)]);
    let listed = s.execute("uml.primitive-type.list", &json!({ "package": sales })).unwrap().value;
    assert_eq!(listed["primitive_types"], json!([{ "id": sku, "name": "Sku", "built_in": false }]));
    let packages = s.execute("uml.package.list", &json!({})).unwrap().value;
    let counted = packages["packages"].as_array().unwrap().iter().find(|p| p["id"] == sales.as_str()).unwrap().clone();
    assert_eq!((counted["data_types"].as_u64(), counted["primitive_types"].as_u64()), (Some(1), Some(1)));
    one_step(&mut s, "uml.primitive-type.rename", json!({ "primitive_type": sku, "name": "ProductCode" }));
    assert_eq!(archforge_core::name_of(s.doc(), obj(s.doc(), &sku)), "ProductCode");
    // Every classifier of a package shares its names, whatever the case.
    for (cmd, p, key, why) in [
        ("uml.primitive-type.add", json!({ "package": sales, "name": "order" }), "name", "there is a class named order already"),
        ("uml.primitive-type.add", json!({ "package": sales, "name": "PAYABLE" }), "name", "there is an interface named PAYABLE already"),
        ("uml.primitive-type.add", json!({ "package": sales, "name": "orderstatus" }), "name", "there is an enumeration named orderstatus already"),
        ("uml.primitive-type.add", json!({ "package": sales, "name": "MONEY" }), "name", "there is a data type named MONEY already"),
        ("uml.class.add", json!({ "package": sales, "name": "productcode" }), "name", "there is a primitive type named productcode already"),
        ("uml.data-type.add", json!({ "package": sales, "name": "PRODUCTCODE" }), "name", "there is a primitive type named PRODUCTCODE already"),
        ("uml.interface.rename", json!({ "interface": payable, "name": "ProductCode" }), "name", "primitive type named ProductCode"),
        ("uml.primitive-type.rename", json!({ "primitive_type": sku, "name": "Order" }), "name", "class named Order"),
        ("uml.primitive-type.add", json!({ "package": sales, "name": "" }), "name", "name"),
        ("uml.primitive-type.add", json!({ "package": order, "name": "X" }), "package", "not a package"),
        ("uml.primitive-type.rename", json!({ "primitive_type": money, "name": "X" }), "primitive_type", "not a primitive type"),
        ("uml.data-type.rename", json!({ "data_type": sku, "name": "X" }), "data_type", "not a data type"),
        ("uml.class.rename", json!({ "class": sku, "name": "X" }), "class", "not a class"),
        ("uml.primitive-type.move", json!({ "primitive_type": sku, "package": order }), "package", "not a package"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(&format!("key: Some(\"{key}\")")) && e.contains(why), "{cmd} {p}: {e}");
    }
    // Moved to a package whose class has its name: refused; to another: one step.
    let clash = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "productCode" }));
    assert!(refused(&mut s, "uml.primitive-type.move", json!({ "primitive_type": sku, "package": billing })).contains("class named ProductCode"));
    run(&mut s, "uml.class.rename", json!({ "class": clash, "name": "Invoice" }));
    one_step(&mut s, "uml.primitive-type.move", json!({ "primitive_type": sku, "package": billing }));
    let st = s.doc();
    assert_eq!(archforge_uml::commands::package_of(st, obj(st, &sku)), Some(obj(st, &billing)));
    // Removed through element.remove, with its card: one step.
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": billing }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": sku, "x": 0, "y": 0 }));
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [sku] })).unwrap();
    let st = s.doc();
    assert!(st.by_uuid(Uuid::parse(&sku).unwrap()).is_none_or(|o| !st.is_alive(o)));
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_diagram::NODE)), "its card went with it");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// A primitive type's card says «primitive» over its name, with no compartment; it keeps no
/// attribute, operation or literal, each refusal naming its id and what keeps them; an agent reads
/// it as a primitive type of the model's own.
#[test]
fn a_primitive_type_draws_its_name_alone_and_takes_no_member() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Sku" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": sku, "x": 0, "y": 0 }));
    // Placed with no size named, its node is a header's height, as the shell places one.
    let st = s.doc();
    let node = st.alive().find(|n| st.class_of(*n).ok() == Some(archforge_diagram::NODE)).unwrap();
    let b = archforge_diagram::grid::bounds_of(st, node);
    assert_eq!((b.width, b.height), archforge_uml::PRIMITIVE_TYPE_SIZE);
    let drawn = card(&s, &sku, &d);
    assert_eq!(drawn.eyebrow, archforge_diagram::figure::Text::said("«primitive»"));
    assert_eq!((drawn.name.as_str(), rows_of(&drawn)), ("Sku", Vec::<Vec<String>>::new()), "no compartment at all");
    assert!(!drawn.slanted && !drawn.elsewhere && drawn.hidden == 0);
    for (cmd, p, members) in [
        ("uml.attribute.add", json!({ "class": sku, "name": "code", "type": "String" }), "attributes"),
        ("uml.operation.add", json!({ "class": sku, "name": "parse" }), "operations"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains("key: Some(\"class\")") && e.contains(&sku), "{cmd}: {e}");
        assert!(
            e.contains(&format!(
                "is a primitive type and keeps no {members}, a primitive type having no members; a class, an interface, a data type or an association class does"
            )),
            "{cmd}: {e}"
        );
    }
    run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    let e = refused(&mut s, "uml.literal.add", json!({ "enumeration": sku, "name": "A" }));
    assert!(e.contains(&sku) && e.contains("not an enumeration"), "{e}");
    let got = s.execute("uml.class.get", &json!({ "class": sku })).unwrap().value;
    assert_eq!(
        (got["kind"].as_str(), got["name"].as_str(), got["built_in"].as_bool(), got["attributes"].clone(), got["operations"].clone()),
        (Some("primitive_type"), Some("Sku"), Some(false), json!([]), json!([]))
    );
    // On another package's diagram, its package is named beside the keyword.
    let other = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": other }));
    run(&mut s, "diagram.place", json!({ "diagram": d2, "element": sku, "x": 0, "y": 0 }));
    let outsider = card(&s, &sku, &d2);
    assert_eq!(outsider.eyebrow, archforge_diagram::figure::Text::Said("«primitive» · {package}", vec![("package", "sales".into())]));
    assert!(outsider.elsewhere);
}

/// A primitive type is a type an attribute, a parameter and a return may have, listed once; they
/// keep it through its rename and its move, and lose it with its removal, which undo gives back.
#[test]
fn a_primitive_type_types_members_listed_once_kept_through_a_rename_and_a_move_and_lost_undoably_with_its_removal() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let shared = run(&mut s, "uml.package.add", json!({ "name": "shared" }));
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": sales, "name": "Sku" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "number", "type": "Integer" }));
    let types = s.execute("uml.type.list", &json!({})).unwrap().value;
    let held = types["types"].as_array().unwrap();
    assert_eq!(held.iter().filter(|t| t["id"] == sku.as_str()).count(), 1, "listed once: {types}");
    assert_eq!(held.iter().filter(|t| t["name"] == "Integer").count(), 1, "the built-in type once: {types}");
    one_step(&mut s, "uml.attribute.add", json!({ "class": order, "name": "item", "type": sku }));
    one_step(&mut s, "uml.operation.add", json!({ "class": order, "name": "find", "parameters": [{ "name": "code", "type": sku }], "returns": sku }));
    let typed = |s: &mut S| {
        let got = s.execute("uml.class.get", &json!({ "class": order })).unwrap().value;
        (got["attributes"][1]["type"].clone(), got["operations"][0]["parameters"][0]["type"].clone(), got["operations"][0]["returns"].clone())
    };
    let want = |name: &str| json!({ "id": sku, "name": name });
    assert_eq!(typed(&mut s), (want("Sku"), want("Sku"), want("Sku")));
    run(&mut s, "uml.primitive-type.rename", json!({ "primitive_type": sku, "name": "ProductCode" }));
    run(&mut s, "uml.primitive-type.move", json!({ "primitive_type": sku, "package": shared }));
    assert_eq!(typed(&mut s), (want("ProductCode"), want("ProductCode"), want("ProductCode")));
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": sales }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    assert_eq!(
        rows_of(&card(&s, &order, &d)),
        [vec!["+ number : Integer", "+ item : ProductCode"], vec!["+ find(code : ProductCode) : ProductCode"]]
    );
    s.execute("element.remove", &json!({ "ids": [sku] })).unwrap();
    assert_eq!(typed(&mut s), (Json::Null, Json::Null, Json::Null), "untyped once it is gone");
    assert_eq!(rows_of(&card(&s, &order, &d)), [vec!["+ number : Integer", "+ item"], vec!["+ find(code)"]]);
    s.undo().unwrap();
    assert_eq!(typed(&mut s), (want("ProductCode"), want("ProductCode"), want("ProductCode")), "undo types them again");
}

/// The built-in types work as before: a built-in type's name makes it in the package Types the
/// first time and reuses it after, never twice. A primitive type of the model's own named Integer
/// elsewhere never stands in for the built-in Integer, nor the built-in for it; in Types the
/// built-in names are kept for the built-in types, before and after they are made; and a built-in
/// type is never renamed or moved, nor does it specialize anything.
#[test]
fn the_built_in_types_are_made_once_in_types_never_swapped_for_the_model_s_own_and_never_renamed_or_moved() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    // The model's own Integer, made before the built-in one.
    let own = run(&mut s, "uml.primitive-type.add", json!({ "package": sales, "name": "Integer" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
    let integer = built_in(&s, "Integer");
    assert_ne!(integer, own, "the built-in name made the built-in type, not the model's own");
    assert_eq!(type_of(&mut s, &order, "count"), json!({ "id": integer, "name": "Integer" }));
    let st = s.doc();
    let types = archforge_uml::commands::package_of(st, obj(st, &integer)).unwrap();
    assert_eq!(archforge_core::name_of(st, types), "Types");
    // Used again, by name in any case, it is reused: never a second one.
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": "integer" }));
    run(&mut s, "uml.operation.add", json!({ "class": order, "name": "size", "returns": "INTEGER" }));
    assert_eq!(type_of(&mut s, &order, "lines"), json!({ "id": integer, "name": "Integer" }));
    assert_eq!(primitives_named(s.doc(), "Integer").len(), 2, "the built-in one and the model's own, no more");
    // The model's own, typed by its id, stays the model's own.
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "code", "type": own }));
    assert_eq!(type_of(&mut s, &order, "code"), json!({ "id": own, "name": "Integer" }));
    // The type list says each once, the built-in one apart.
    let listed = s.execute("uml.type.list", &json!({})).unwrap().value;
    let named: Vec<&Json> = listed["types"].as_array().unwrap().iter().filter(|t| t["name"] == "Integer").collect();
    assert_eq!(named.len(), 2, "{listed}");
    let types_id = s.doc().uuid(types).unwrap().to_string();
    let listed = s.execute("uml.primitive-type.list", &json!({ "package": types_id })).unwrap().value;
    assert_eq!(listed["primitive_types"], json!([{ "id": integer, "name": "Integer", "built_in": true }]));
    // In Types the built-in names are kept: made already (Integer) or not yet (Real), whatever the case.
    let kept = "kept for it in the package Types; name it otherwise or put it in another package";
    for (cmd, p, key, value) in [
        ("uml.primitive-type.add", json!({ "package": types_id, "name": "integer" }), "name", "integer"),
        ("uml.primitive-type.add", json!({ "package": types_id, "name": "Real" }), "name", "Real"),
        ("uml.class.add", json!({ "package": types_id, "name": "STRING" }), "name", "STRING"),
        ("uml.primitive-type.move", json!({ "primitive_type": own, "package": types_id }), "package", "Integer"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(&format!("key: Some(\"{key}\")")) && e.contains(value) && e.contains(kept), "{cmd} {p}: {e}");
    }
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": types_id, "name": "Sku" }));
    let e = refused(&mut s, "uml.primitive-type.rename", json!({ "primitive_type": sku, "name": "Boolean" }));
    assert!(e.contains("Boolean") && e.contains(kept), "{e}");
    // A built-in type keeps its name and its package, and specializes nothing.
    for (cmd, p, changed) in [
        ("uml.primitive-type.rename", json!({ "primitive_type": integer, "name": "Int" }), "renamed"),
        ("uml.primitive-type.move", json!({ "primitive_type": integer, "package": sales }), "moved"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains("key: Some(\"primitive_type\")") && e.contains(&integer), "{cmd}: {e}");
        assert!(e.contains(&format!("is the built-in type Integer, which is never {changed}")), "{cmd}: {e}");
    }
    let e = refused(&mut s, "uml.generalization.add", json!({ "specific": integer, "general": own }));
    assert!(
        e.contains("key: Some(\"specific\")") && e.contains(&integer) && e.contains("is the built-in type Integer, which specializes nothing"),
        "{e}"
    );
    // A built-in type has no member either.
    let e = refused(&mut s, "uml.attribute.add", json!({ "class": integer, "name": "bits" }));
    assert!(e.contains(&integer) && e.contains("a primitive type having no members"), "{e}");
    // Renaming the model's own to another built-in name elsewhere is its own business, and a
    // built-in name still makes the built-in type.
    run(&mut s, "uml.primitive-type.rename", json!({ "primitive_type": own, "name": "String" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "label", "type": "String" }));
    let string = built_in(&s, "String");
    assert_ne!(string, own);
    assert_eq!(type_of(&mut s, &order, "label"), json!({ "id": string, "name": "String" }));
    assert_eq!(type_of(&mut s, &order, "code"), json!({ "id": own, "name": "String" }), "the model's own keeps typing what it typed");
    assert_eq!(primitives_named(s.doc(), "Integer").len(), 1, "the built-in Integer once");
}

/// A primitive type specializes only a primitive type, a built-in one among them, and is
/// specialized only by one; no association joins it, nor does it realize or get realized; a
/// dependency may touch it. Each refusal names the offending id under the key of the end that
/// breaks the rule, and changes nothing.
#[test]
fn a_primitive_type_specializes_only_primitive_types_joins_no_association_and_realizes_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Sku" }));
    let code = run(&mut s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Code" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Invoice" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": pkg, "name": "OrderStatus" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "label", "type": "String" }));
    let string = built_in(&s, "String");
    // A primitive type specializes a primitive type, a built-in one too, one step each; a
    // dependency may touch it either way.
    let g = one_step(&mut s, "uml.generalization.add", json!({ "specific": sku, "general": code }));
    one_step(&mut s, "uml.generalization.add", json!({ "specific": code, "general": string }));
    one_step(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": sku }));
    one_step(&mut s, "uml.dependency.add", json!({ "client": sku, "supplier": payable }));
    let no_association = "is a primitive type and joins no association; an association joins classes and interfaces";
    for (cmd, p, key, id, why) in [
        (
            "uml.generalization.add",
            json!({ "specific": sku, "general": order }),
            "general",
            &order,
            "is a class; a primitive type specializes only a primitive type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": sku, "general": payable }),
            "general",
            &payable,
            "is an interface; a primitive type specializes only a primitive type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": order, "general": sku }),
            "specific",
            &order,
            "is a class; only a primitive type specializes a primitive type",
        ),
        // Between a primitive type and a data type or an enumeration, their own rule speaks.
        (
            "uml.generalization.add",
            json!({ "specific": sku, "general": money }),
            "specific",
            &sku,
            "is a primitive type; only a data type specializes a data type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": money, "general": sku }),
            "general",
            &sku,
            "is a primitive type; a data type specializes only a data type",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": sku, "general": status }),
            "specific",
            &sku,
            "is a primitive type; only an enumeration specializes an enumeration",
        ),
        (
            "uml.generalization.add",
            json!({ "specific": sku, "general": sku }),
            "general",
            &sku,
            "a primitive type cannot specialize itself; it specializes another primitive type",
        ),
        ("uml.generalization.add", json!({ "specific": sku, "general": code }), "general", &code, "specializes that primitive type already"),
        ("uml.generalization.add", json!({ "specific": code, "general": sku }), "general", &sku, "cycle"),
        ("uml.association.add", json!({ "from": order, "to": sku }), "to", &sku, no_association),
        ("uml.association.add", json!({ "from": sku, "to": order, "kind": "composition" }), "from", &sku, no_association),
        ("uml.association.add", json!({ "from": order, "to": string }), "to", &string, no_association),
        (
            "uml.realization.add",
            json!({ "class": sku, "interface": payable }),
            "class",
            &sku,
            "is a primitive type; only a class implements an interface",
        ),
        (
            "uml.realization.add",
            json!({ "class": order, "interface": sku }),
            "interface",
            &sku,
            "is a primitive type; a class realizes only an interface",
        ),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(why) && e.contains(id.as_str()), "{cmd} {p}: {e}");
        assert!(e.contains(&format!("key: Some(\"{key}\")")), "{cmd} {p} refused under {key}: {e}");
    }
    // A reconnected end keeps the same rules.
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": invoice }));
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": a, "end": "target", "class": sku }));
    assert!(e.contains(no_association) && e.contains(&sku), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "target", "class": order }));
    assert!(e.contains("a primitive type specializes only a primitive type"), "{e}");
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": g, "end": "source", "class": string }));
    assert!(e.contains("is the built-in type String, which specializes nothing"), "{e}");
    let r = run(&mut s, "uml.realization.add", json!({ "class": order, "interface": payable }));
    let e = refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": r, "end": "source", "class": sku }));
    assert!(e.contains("only a class implements an interface") && e.contains(&sku), "{e}");
    // Removing it takes the relationships that touch it along, in one step.
    let before = picture(s.doc());
    let gone = obj(s.doc(), &sku);
    s.execute("element.remove", &json!({ "ids": [sku] })).unwrap();
    let st = s.doc();
    let touching = st
        .alive()
        .filter(|o| st.class_of(*o).is_ok_and(|c| [archforge_uml::GENERALIZATION, archforge_uml::DEPENDENCY].contains(&c)))
        .filter(|o| archforge_uml::figures::ends(st, *o).is_none_or(|(a, b)| a == gone || b == gone))
        .count();
    assert_eq!(touching, 0, "the relationships that touched it went with it");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// Primitive types, a generalization between them and a card come back from their rows identical,
/// the built-in ones still built-in.
#[test]
fn primitive_types_with_relationships_come_back_from_their_rows_identical() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Sku" }));
    let code = run(&mut s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Code" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "item", "type": sku }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
    run(&mut s, "uml.generalization.add", json!({ "specific": sku, "general": code }));
    let integer = built_in(&s, "Integer");
    let d = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "element": pkg }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": sku, "x": 0, "y": 0 }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    assert_eq!(back.class_of(obj(&back, &sku)).unwrap(), PRIMITIVE_TYPE, "still a PrimitiveType");
    assert!(!archforge_uml::members::is_built_in(&back, obj(&back, &sku)) && archforge_uml::members::is_built_in(&back, obj(&back, &integer)));
    let names = |os: Vec<Obj>| os.into_iter().map(|o| archforge_core::name_of(&back, o)).collect::<Vec<_>>();
    assert_eq!(names(archforge_uml::commands::primitive_types(&back, obj(&back, &pkg))), ["Sku", "Code"]);
    let mut back = session_over(back);
    let drawn = card(&back, &sku, &d);
    assert_eq!((drawn.eyebrow, drawn.name.as_str(), drawn.compartments.len()), (archforge_diagram::figure::Text::said("«primitive»"), "Sku", 0));
    assert_eq!(type_of(&mut back, &order, "item"), json!({ "id": sku, "name": "Sku" }));
    // A built-in name after reopening still finds the one in Types: no second Integer.
    run(&mut back, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": "Integer" }));
    assert_eq!(primitives_named(back.doc(), "Integer").len(), 1);
    let got = back.registry().infos(&back.facts()).into_iter().any(|i| i.id == "uml.primitive-type.add");
    assert!(got, "the commands are offered over the project read back");
}

/// The command sweep, with a primitive type of the model's own, a built-in one and a class among
/// the ids, breaks nothing.
#[test]
fn the_sweep_over_primitive_types_breaks_nothing() {
    let mut s = session();
    let pkg = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": pkg, "name": "Sku" }));
    let class = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    let attribute = run(&mut s, "uml.attribute.add", json!({ "class": class, "name": "count", "type": "Integer" }));
    let integer = built_in(&s, "Integer");
    // The primitive type's commands are in the registry the sweep walks.
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.primitive-type.add", "uml.primitive-type.rename", "uml.primitive-type.move", "uml.primitive-type.list"] {
        assert!(listed.iter().any(|c| c == cmd), "{cmd} is swept");
    }
    let ids = [pkg.as_str(), sku.as_str(), integer.as_str(), attribute.as_str(), class.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}

/// The package `name` added under `parent` (the root package when empty), by `uml.package.add`.
fn package(s: &mut S, name: &str, parent: &str) -> String {
    run(s, "uml.package.add", json!({ "name": name, "parent": parent }))
}

/// The id of the package Types, as `members::types_package` finds it.
fn types_id(s: &S) -> Option<String> {
    let st = s.doc();
    archforge_uml::members::types_package(st).map(|t| st.uuid(t).unwrap().to_string())
}

/// There is one package Types, directly under the root package, and renames and moves keep it so:
/// it keeps its name and its place while it holds built-in types; a package becomes Types only
/// while there is none and when it holds no classifier with a built-in type's name; a package named
/// Types elsewhere is ordinary. Each refusal names the ids involved and changes nothing.
#[test]
fn the_package_types_stays_one_through_renames_and_moves() {
    let mut s = session();
    let lib = package(&mut s, "Lib", "");
    let own = run(&mut s, "uml.primitive-type.add", json!({ "package": lib, "name": "Integer" }));
    let order_pkg = package(&mut s, "Sales", "");
    let order = run(&mut s, "uml.class.add", json!({ "package": order_pkg, "name": "Order" }));
    // No Types yet: Lib, holding the model's own Integer, does not become it.
    let e = refused(&mut s, "uml.package.rename", json!({ "package": lib, "name": "Types" }));
    assert!(e.contains(&lib) && e.contains(&own) && e.contains("a built-in type's name") && e.contains("key: Some(\"name\")"), "{e}");
    // The first use makes it; then Lib cannot take its name, whatever Lib holds.
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
    let types = types_id(&s).expect("made directly under the root package");
    let integer = built_in(&s, "Integer");
    run(&mut s, "uml.primitive-type.rename", json!({ "primitive_type": own, "name": "Count" }));
    let e = refused(&mut s, "uml.package.rename", json!({ "package": lib, "name": "Types" }));
    assert!(e.contains(&types) && e.contains(&lib) && e.contains("is the package Types already"), "{e}");
    // Types keeps its name and its place while it holds the built-in Integer.
    let e = refused(&mut s, "uml.package.rename", json!({ "package": types, "name": "Builtins" }));
    assert!(e.contains(&types) && e.contains("keeps its name and stays directly under the root package while it holds them"), "{e}");
    let e = refused(&mut s, "uml.package.move", json!({ "package": types, "to": lib }));
    assert!(e.contains(&types) && e.contains("key: Some(\"to\")"), "{e}");
    // So the next Integer is the same one, never a second.
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": "Integer" }));
    assert_eq!(built_in(&s, "Integer"), integer);
    assert_eq!(primitives_named(s.doc(), "Integer").len(), 1);
    // A package named Types elsewhere is ordinary: it takes a type of the model's own named
    // Integer, which stays the model's own, and a built-in name still means the one in Types.
    let nested = package(&mut s, "Types", &lib);
    let nested_integer = run(&mut s, "uml.primitive-type.add", json!({ "package": nested, "name": "Integer" }));
    assert!(!archforge_uml::members::is_built_in(s.doc(), obj(s.doc(), &nested_integer)));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "size", "type": "Integer" }));
    assert_eq!(type_of(&mut s, &order, "size"), json!({ "id": integer, "name": "Integer" }));
    // Nor does it come up beside Types, by a move or by its parent's removal handing it up.
    let e = refused(&mut s, "uml.package.move", json!({ "package": nested, "to": "" }));
    assert!(e.contains(&types) && e.contains(&nested) && e.contains("is the package Types already"), "{e}");
    let e = refused(&mut s, "element.remove", json!({ "ids": [lib] }));
    assert!(e.contains(&types) && e.contains("is the package Types already"), "{e}");
}

/// Without a package Types, a package named Types comes directly under the root only when it holds
/// no classifier with a built-in type's name, by a move or by a removal handing it up; an empty one
/// renamed Types is the package Types. A Types that holds no built-in type is an ordinary package to
/// rename; once the first built-in type is made there, it keeps its name.
#[test]
fn a_package_becomes_types_only_without_a_built_in_name_in_it() {
    let mut s = session();
    // An empty package directly under the root renamed Types becomes it, and renames back freely.
    let spare = package(&mut s, "Spare", "");
    one_step(&mut s, "uml.package.rename", json!({ "package": spare, "name": "Types" }));
    assert_eq!(types_id(&s).as_deref(), Some(spare.as_str()));
    one_step(&mut s, "uml.package.rename", json!({ "package": spare, "name": "Spare" }));
    assert_eq!(types_id(&s), None);
    let outer = package(&mut s, "Outer", "");
    let nested = package(&mut s, "Types", &outer);
    let own = run(&mut s, "uml.class.add", json!({ "package": nested, "name": "string" }));
    let e = refused(&mut s, "uml.package.move", json!({ "package": nested, "to": "" }));
    assert!(e.contains(&nested) && e.contains(&own) && e.contains("a built-in type's name"), "{e}");
    let e = refused(&mut s, "element.remove", json!({ "ids": [outer] }));
    assert!(e.contains(&nested) && e.contains(&own), "{e}");
    assert_eq!(types_id(&s), None, "no package Types came of them");
    // Renamed, the class no longer stands in the way: the move makes it the package Types.
    run(&mut s, "uml.class.rename", json!({ "class": own, "name": "Text" }));
    one_step(&mut s, "uml.package.move", json!({ "package": nested, "to": "" }));
    assert_eq!(types_id(&s).as_deref(), Some(nested.as_str()));
    // The first built-in type goes there.
    let sales = package(&mut s, "Sales", "");
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "code", "type": "String" }));
    let string = built_in(&s, "String");
    let st = s.doc();
    assert_eq!(archforge_uml::commands::package_of(st, obj(st, &string)), Some(obj(st, &nested)));
    // Holding it, Types keeps its name.
    let e = refused(&mut s, "uml.package.rename", json!({ "package": nested, "name": "Library" }));
    assert!(e.contains(&nested) && e.contains("keeps its name"), "{e}");
}

/// A built-in type is not removed, since its next use would make it again under a new id, nor is a
/// package holding one (the package Types), whose removal would hand them to the root as types of
/// the model's own; each refusal names the id and changes nothing. An empty Types, or one holding
/// only the model's own types, goes as any package, one undo step.
#[test]
fn a_built_in_type_and_the_package_types_holding_one_are_not_removed() {
    let mut s = session();
    let sales = package(&mut s, "Sales", "");
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
    let types = types_id(&s).unwrap();
    let integer = built_in(&s, "Integer");
    let e = refused(&mut s, "element.remove", json!({ "ids": [types] }));
    assert!(e.contains(&types) && e.contains("it stays while it holds them") && e.contains("key: Some(\"ids\")"), "{e}");
    let e = refused(&mut s, "element.remove", json!({ "ids": [integer] }));
    assert!(e.contains(&integer) && e.contains("is the built-in type Integer") && e.contains("make it again under a new id"), "{e}");
    let e = refused(&mut s, "element.remove", json!({ "ids": [order, integer] }));
    assert!(e.contains(&integer), "{e}");
    assert_eq!(type_of(&mut s, &order, "count"), json!({ "id": integer, "name": "Integer" }), "still typed by it");
    // An empty Types, and one holding only the model's own types, goes as any package.
    let mut s = session();
    let types = package(&mut s, "Types", "");
    assert_eq!(types_id(&s).as_deref(), Some(types.as_str()));
    one_step(&mut s, "element.remove", json!({ "ids": [types] }));
    let types = package(&mut s, "Types", "");
    run(&mut s, "uml.primitive-type.add", json!({ "package": types, "name": "Sku" }));
    one_step(&mut s, "element.remove", json!({ "ids": [types] }));
    assert_eq!(types_id(&s), None);
}

/// A project from before keeps, in its package Types, a class named Integer or a primitive type
/// named integer: the first use of the built-in name is refused, naming that element and asking
/// for it to be renamed, rather than making a second classifier of that name beside it.
#[test]
fn a_built_in_type_is_not_made_beside_a_same_named_classifier_an_older_types_holds() {
    for (class, held) in [(archforge_uml::CLASS, "Integer"), (PRIMITIVE_TYPE, "integer")] {
        let mut st = store(0);
        let model = st.create(archforge_core::MODEL).unwrap();
        st.set(model, archforge_core::named_element::NAME, Value::str("Shop")).unwrap();
        let named = |st: &mut Store, class, name: &str| {
            let o = st.create(class).unwrap();
            st.set(o, archforge_uml::named_element::NAME, Value::str(name)).unwrap();
            o
        };
        let sales = named(&mut st, archforge_uml::PACKAGE, "Sales");
        let types = named(&mut st, archforge_uml::PACKAGE, "Types");
        for p in [sales, types] {
            st.push(model, archforge_core::model::MEMBER, Value::Ref(p)).unwrap();
        }
        let order = named(&mut st, archforge_uml::CLASS, "Order");
        st.push(sales, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(order)).unwrap();
        let clash = named(&mut st, class, held);
        st.push(types, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(clash)).unwrap();
        let (order, clash) = (st.uuid(order).unwrap().to_string(), st.uuid(clash).unwrap().to_string());
        let mut s = session_over(st);
        let e = refused(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
        assert!(e.contains(&clash) && e.contains(&format!("named {held}")) && e.contains("rename it to use the built-in type"), "{e}");
        assert!(e.contains("key: Some(\"type\")"), "{e}");
        assert_eq!(primitives_named(s.doc(), "Integer").len(), usize::from(held == "Integer" && class == PRIMITIVE_TYPE), "no second one made");
        // Renamed, the built-in type is made there.
        let rename = if class == PRIMITIVE_TYPE { "uml.primitive-type.rename" } else { "uml.class.rename" };
        let key = if class == PRIMITIVE_TYPE { "primitive_type" } else { "class" };
        run(&mut s, rename, json!({ key: clash, "name": "Count" }));
        run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
        built_in(&s, "Integer");
    }
}
