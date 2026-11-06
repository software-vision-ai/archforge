//! Classifier templates through the engine (AR-130): template parameters added, edited, moved and
//! removed, each one undo step, the signature made with the first and gone with the last; a
//! parameter's identity kept across rename, reorder and a change of kind, with what it types; the
//! kinds not supported yet refused, never converted; a default and a constraint checked against
//! the kind; a parameter a binding substitutes kept, its kind and constraint kept fitting the
//! binding; the parameter's classifier kept out of the model's own; the signature box on two
//! diagrams; saved as rows and read back identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::templates;
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

fn alive(s: &S, id: &str) -> bool {
    let st = s.doc();
    st.by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
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

/// A package `shop` with Repository, Order, Entity (Order specializes it), the data type Money and
/// the enumeration Status, and a class diagram of `shop` showing Repository and Order.
struct Shop {
    shop: String,
    repository: String,
    order: String,
    entity: String,
    money: String,
    status: String,
    d: String,
}

fn shop(s: &mut S) -> Shop {
    let shop = run(s, "uml.package.add", json!({ "name": "shop" }));
    let class = |s: &mut S, name: &str| run(s, "uml.class.add", json!({ "package": shop, "name": name }));
    let repository = class(s, "Repository");
    let order = class(s, "Order");
    let entity = class(s, "Entity");
    run(s, "uml.generalization.add", json!({ "specific": order, "general": entity }));
    let money = run(s, "uml.data-type.add", json!({ "package": shop, "name": "Money" }));
    let status = run(s, "uml.enumeration.add", json!({ "package": shop, "name": "Status" }));
    let d = run(s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Repositories", "element": shop }));
    for (i, c) in [&repository, &order].into_iter().enumerate() {
        run(s, "diagram.place", json!({ "diagram": d, "element": c, "x": (i as f64) * 400.0, "y": 0.0 }));
    }
    Shop { shop, repository, order, entity, money, status, d }
}

/// The template parameters `uml.template-parameter.list` gives for `c`.
fn listed(s: &mut S, c: &str) -> Vec<Json> {
    s.execute("uml.template-parameter.list", &json!({ "classifier": c })).unwrap().value["parameters"].as_array().unwrap().clone()
}

/// The names of `c`'s template parameters, in their order.
fn names(s: &mut S, c: &str) -> Vec<String> {
    listed(s, c).iter().map(|p| p["name"].as_str().unwrap().to_string()).collect()
}

/// The rows of `c`'s signature box on diagram `d`.
fn rows_of(s: &S, c: &str, d: &str) -> Vec<String> {
    let st = s.doc();
    archforge_uml::figures::card(st, obj(st, c), obj(st, d)).unwrap().template
}

/// A parameter is added, edited, moved and removed, each one undo step; the signature comes with
/// the first and goes with the last, and the card's box lists the parameters in order.
#[test]
fn parameters_are_added_edited_moved_and_removed_each_one_undo_step() {
    let mut s = session();
    let shop = shop(&mut s);
    assert!(rows_of(&s, &shop.repository, &shop.d).is_empty(), "no box before it is a template");
    let t = one_step(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let st = s.doc();
    assert!(templates::is_template(st, obj(st, &shop.repository)));
    assert_eq!(rows_of(&s, &shop.repository, &shop.d), ["T : Class"]);
    let key = one_step(
        &mut s,
        "uml.template-parameter.add",
        json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type", "default": "Integer" }),
    );
    assert_eq!(rows_of(&s, &shop.repository, &shop.d), ["T : Class", "Key : DataType = Integer"]);
    let first = one_step(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "E", "kind": "enumeration", "at": 0 }));
    assert_eq!(names(&mut s, &shop.repository), ["E", "T", "Key"]);
    // Edited: a field left out is kept, an empty one cleared.
    one_step(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "name": "Id" }));
    assert_eq!(rows_of(&s, &shop.repository, &shop.d)[2], "Id : DataType = Integer", "the kind and default kept");
    one_step(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "default": "" }));
    assert_eq!(rows_of(&s, &shop.repository, &shop.d)[2], "Id : DataType");
    one_step(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "constraint": shop.entity, "default": shop.order }));
    assert_eq!(rows_of(&s, &shop.repository, &shop.d)[1], "T : Class > Entity = Order");
    // Moved.
    one_step(&mut s, "uml.template-parameter.move", json!({ "parameter": first, "at": 2 }));
    assert_eq!(names(&mut s, &shop.repository), ["T", "Id", "E"]);
    one_step(&mut s, "uml.template-parameter.move", json!({ "parameter": first, "at": 0 }));
    one_step(&mut s, "uml.template-parameter.move", json!({ "parameter": first, "at": -1 }));
    assert_eq!(names(&mut s, &shop.repository), ["T", "Id", "E"], "-1 is the last place");
    // The signature's parameter list keeps the owned order, as UML has both lists.
    let st = s.doc();
    let sig = templates::signature(st, obj(st, &shop.repository)).unwrap();
    assert_eq!(
        st.list(sig, archforge_uml::template_signature::PARAMETER).unwrap(),
        st.list(sig, archforge_uml::template_signature::OWNED_PARAMETER).unwrap()
    );
    // Removed through element.remove, one undo step each; the last takes the signature along.
    for (id, left) in [(&first, 2), (&key, 1)] {
        let before = picture(s.doc());
        s.execute("element.remove", &json!({ "ids": [id] })).unwrap();
        assert_eq!(names(&mut s, &shop.repository).len(), left);
        s.undo().unwrap();
        assert_eq!(picture(s.doc()), before, "one step");
        s.redo().unwrap();
    }
    let element = listed(&mut s, &shop.repository)[0]["element"].as_str().unwrap().to_string();
    s.execute("element.remove", &json!({ "ids": [t] })).unwrap();
    let st = s.doc();
    assert!(templates::signature(st, obj(st, &shop.repository)).is_none(), "the signature goes with the last parameter");
    assert!(!alive(&s, &element), "the parameter's classifier with it");
    assert!(rows_of(&s, &shop.repository, &shop.d).is_empty());
    assert_eq!(s.execute("uml.class.get", &json!({ "class": shop.repository })).unwrap().value["template_parameters"], json!([]));
}

/// Renaming, reordering and changing the kind keep the parameter's id and what it types: the
/// template's own members typed by it follow it.
#[test]
fn identity_and_references_are_kept_across_rename_reorder_and_kind() {
    let mut s = session();
    let shop = shop(&mut s);
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let key = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type" }));
    let element =
        |s: &mut S, p: &str| listed(s, &shop.repository).iter().find(|x| x["id"] == json!(p)).unwrap()["element"].as_str().unwrap().to_string();
    let t_el = element(&mut s, &t);
    let key_el = element(&mut s, &key);
    let items = run(&mut s, "uml.attribute.add", json!({ "class": shop.repository, "name": "items", "type": t_el, "multiplicity": "*" }));
    run(
        &mut s,
        "uml.operation.add",
        json!({ "class": shop.repository, "name": "find", "parameters": [{ "name": "key", "type": key_el }], "returns": t_el }),
    );
    let card = |s: &S| {
        let st = s.doc();
        archforge_uml::figures::card(st, obj(st, &shop.repository), obj(st, &shop.d))
            .unwrap()
            .compartments
            .iter()
            .flatten()
            .map(|r| r.text.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(card(&s), ["+ items : T [*]", "+ find(key : Key) : T"]);
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "name": "Item" }));
    run(&mut s, "uml.template-parameter.move", json!({ "parameter": key, "at": 0 }));
    assert_eq!(element(&mut s, &t), t_el, "a rename keeps the classifier");
    assert_eq!(card(&s), ["+ items : Item [*]", "+ find(key : Key) : Item"]);
    assert_eq!(names(&mut s, &shop.repository), ["Key", "Item"]);
    // The parameter's id names it, and so does its classifier's.
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": key_el, "name": "Id" }));
    assert_eq!(names(&mut s, &shop.repository), ["Id", "Item"]);
    // A kind changed: the parameter keeps its id, its new classifier types what the old one did.
    one_step(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "kind": "interface" }));
    let new_el = element(&mut s, &t);
    assert_ne!(new_el, t_el);
    assert!(!alive(&s, &t_el), "the old classifier goes");
    let st = s.doc();
    assert_eq!(st.class_of(obj(st, &new_el)).unwrap(), archforge_uml::INTERFACE);
    assert_eq!(st.get(obj(st, &items), archforge_uml::typed_element::TYPE).unwrap(), Value::Ref(obj(st, &new_el)));
    assert_eq!(card(&s), ["+ items : Item [*]", "+ find(key : Id) : Item"]);
    assert_eq!(rows_of(&s, &shop.repository, &shop.d), ["Id : DataType", "Item : Interface"]);
}

/// Value and Operation are named kinds, refused as not supported yet, and nothing is made of them.
#[test]
fn the_kinds_not_supported_yet_are_refused_never_converted() {
    let mut s = session();
    let shop = shop(&mut s);
    for kind in ["value", "operation"] {
        let e = refused(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "N", "kind": kind }));
        assert!(e.contains("not supported yet") && e.contains("never turned into another kind"), "{e}");
    }
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let e = refused(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "kind": "value" }));
    assert!(e.contains("Value template parameters are not supported yet"), "{e}");
    assert_eq!(templates::Kind::ALL.iter().filter(|k| !k.supported()).map(|k| k.key()).collect::<Vec<_>>(), ["value", "operation"]);
    let e = refused(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "N", "kind": "number" }));
    assert!(e.contains("kind"), "{e}");
}

/// A default is a classifier of the model of the parameter's kind that fits its constraint; the
/// other wrong edits are refused too, changing nothing.
#[test]
fn a_default_must_be_a_classifier_of_the_kind_and_the_other_wrong_edits_are_refused() {
    let mut s = session();
    let shop = shop(&mut s);
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let key = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type" }));
    let undo = s.undo_label().map(str::to_string);
    for (cmd, p, said) in [
        // Of the kind: a data type's default is a data type (a primitive or an enumeration is one).
        ("uml.template-parameter.set", json!({ "parameter": key, "default": shop.order }), "a default of a data type parameter is a data type"),
        ("uml.template-parameter.set", json!({ "parameter": t, "default": shop.money }), "a default of a class parameter is a class"),
        ("uml.template-parameter.set", json!({ "parameter": t, "default": "Integer" }), "a default of a class parameter"),
        ("uml.template-parameter.set", json!({ "parameter": t, "default": "Decimal" }), "neither a type's id"),
        ("uml.template-parameter.set", json!({ "parameter": t, "default": shop.shop }), "not a type"),
        // Fitting the constraint: Order specializes Entity, Repository does not.
        (
            "uml.template-parameter.set",
            json!({ "parameter": t, "constraint": shop.entity, "default": shop.repository }),
            "neither is Entity nor specializes it",
        ),
        ("uml.template-parameter.set", json!({ "parameter": key, "constraint": shop.entity }), "constrain it to a data type"),
        // Names, places, owners.
        ("uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "t" }), "has a template parameter named T already"),
        ("uml.template-parameter.set", json!({ "parameter": key, "name": "T" }), "has a template parameter named T already"),
        ("uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "" }), "empty"),
        ("uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "X", "at": 5 }), "past the end"),
        ("uml.template-parameter.move", json!({ "parameter": t, "at": 2 }), "past the last"),
        ("uml.template-parameter.add", json!({ "classifier": shop.status, "name": "X" }), "is no template"),
        ("uml.template-parameter.set", json!({ "parameter": shop.order, "name": "X" }), "no template parameter"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains(said), "{said}: {e}");
    }
    assert_eq!(s.undo_label().map(str::to_string), undo, "no undo step");
    // Fitting ones are taken: a primitive type and an enumeration are data types, Order fits Entity.
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "default": "Integer" }));
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "default": shop.status }));
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "constraint": shop.entity, "default": shop.order }));
    let p = &listed(&mut s, &shop.repository)[0];
    assert_eq!((p["constraint"]["name"].as_str(), p["default"]["name"].as_str()), (Some("Entity"), Some("Order")));
    // A default that goes leaves the parameter without one.
    s.execute("element.remove", &json!({ "ids": [shop.status] })).unwrap();
    assert_eq!(listed(&mut s, &shop.repository)[1]["default"], Json::Null);
    // A template holds at most 32.
    for i in 2..templates::MAX_PARAMETERS {
        run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": format!("P{i}") }));
    }
    let e = refused(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "One more" }));
    assert!(e.contains("at most 32"), "{e}");
}

/// A parameter's classifier is its template's own: no other classifier is typed by it, and the
/// classifier commands, the diagram and the skills leave it alone.
#[test]
fn a_parameter_s_classifier_stays_its_template_s_own() {
    let mut s = session();
    let shop = shop(&mut s);
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let el = listed(&mut s, &shop.repository)[0]["element"].as_str().unwrap().to_string();
    for (cmd, p, said) in [
        ("uml.attribute.add", json!({ "class": shop.order, "name": "x", "type": el }), "types only that template's own"),
        ("uml.attribute.add", json!({ "class": el, "name": "x" }), "is the template parameter T of Repository"),
        ("uml.class.rename", json!({ "class": el, "name": "U" }), "is the template parameter T"),
        ("uml.class.move", json!({ "class": el, "package": shop.shop }), "is the template parameter T"),
        ("element.remove", json!({ "ids": [el] }), "is the template parameter T"),
        ("diagram.place", json!({ "diagram": shop.d, "element": el, "x": 0.0, "y": 600.0 }), ""),
        ("skill.add", json!({ "subject": el, "name": "Rule", "body": "x" }), "a template parameter carries no skill"),
        ("uml.template-parameter.set", json!({ "parameter": t, "default": el }), "types only that template's own"),
    ] {
        let e = refused(&mut s, cmd, p);
        assert!(e.contains(said), "{cmd} {said}: {e}");
    }
    let types = s.execute("uml.type.list", &json!({})).unwrap().value;
    assert!(types["types"].as_array().unwrap().iter().all(|t| t["id"] != json!(el)), "the type list leaves it out");
}

/// A binding (AR-131) built in the store: OrderRepository binds Repository's Key to Integer and T
/// to Order. Gives the session over it and the binding's id.
fn bound(s: &S, shop: &Shop) -> (S, String) {
    bind(s, shop, true)
}

/// A binding built in the store as [`bound`] builds it, with its substitutions or, without
/// `substitute`, none: OrderRepository naming Repository's signature alone.
fn bind(s: &S, shop: &Shop, substitute: bool) -> (S, String) {
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut st = store(1 << 30);
    rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
    let pkg = obj(&st, &shop.shop);
    let repo = obj(&st, &shop.repository);
    let bound_class = st.create(archforge_uml::CLASS).unwrap();
    st.set(bound_class, archforge_uml::named_element::NAME, Value::str("OrderRepository")).unwrap();
    st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(bound_class)).unwrap();
    let sig = templates::signature(&st, repo).unwrap();
    let b = st.create(archforge_uml::TEMPLATE_BINDING).unwrap();
    st.set(b, archforge_uml::template_binding::SIGNATURE, Value::Ref(sig)).unwrap();
    st.push(bound_class, archforge_uml::templateable_element::TEMPLATE_BINDING, Value::Ref(b)).unwrap();
    let integer = st.alive().find(|o| archforge_core::name_of(&st, *o) == "Integer");
    let actuals = if substitute { vec![obj(&st, &shop.order), integer.unwrap()] } else { Vec::new() };
    for (p, actual) in templates::parameters(&st, repo).into_iter().zip(actuals) {
        let sub = st.create(archforge_uml::TEMPLATE_PARAMETER_SUBSTITUTION).unwrap();
        st.set(sub, archforge_uml::template_parameter_substitution::FORMAL, Value::Ref(p)).unwrap();
        st.set(sub, archforge_uml::template_parameter_substitution::ACTUAL, Value::Ref(actual)).unwrap();
        st.push(b, archforge_uml::template_binding::PARAMETER_SUBSTITUTION, Value::Ref(sub)).unwrap();
    }
    assert!(st.first_problem().is_none(), "{:?}", st.first_problem());
    let id = st.uuid(b).unwrap().to_string();
    (session_over(st), id)
}

/// A parameter a binding substitutes is never removed, and its kind and constraint change only
/// while the binding's argument fits; renaming, reordering and its default change freely.
#[test]
fn a_parameter_a_binding_uses_is_kept_and_kept_fitting() {
    let mut s = session();
    let shop = shop(&mut s);
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let key = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type" }));
    // Integer, made in Types by its first use.
    run(&mut s, "uml.attribute.add", json!({ "class": shop.order, "name": "id", "type": "Integer" }));
    let (mut s, binding) = bound(&s, &shop);
    let e = refused(&mut s, "element.remove", json!({ "ids": [key] }));
    assert!(e.contains("OrderRepository binds Key to Integer; change or remove that binding first, then remove Key"), "{e}");
    let e = refused(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "kind": "enumeration" }));
    assert!(e.contains("OrderRepository binds Key to Integer") && e.contains("it is not an enumeration"), "{e}");
    let e = refused(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "constraint": shop.repository }));
    assert!(e.contains("OrderRepository binds T to Order") && e.contains("constrained to Repository"), "{e}");
    // What still fits is taken: Order specializes Entity, Integer is a primitive type.
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": t, "constraint": shop.entity }));
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "kind": "primitive-type" }));
    // Renaming, reordering and the default change freely: the binding follows the parameter.
    run(&mut s, "uml.template-parameter.set", json!({ "parameter": key, "name": "Id", "default": "" }));
    run(&mut s, "uml.template-parameter.move", json!({ "parameter": key, "at": 0 }));
    let p = &listed(&mut s, &shop.repository)[0];
    assert_eq!(p["name"], json!("Id"));
    assert_eq!(p["bound_by"][0]["binding"], json!(binding));
    assert_eq!(p["bound_by"][0]["actual"]["name"], json!("Integer"));
    // The template removed takes its binding along, the bound class kept.
    let before = picture(s.doc());
    s.execute("element.remove", &json!({ "ids": [shop.repository] })).unwrap();
    assert!(!alive(&s, &binding));
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
}

/// A binding that names the template's signature with no substitution still needs that
/// signature: removing the last parameter, which would take the signature along, is refused before
/// anything is written, the store and the history as they were; so is removing every parameter at
/// once. A parameter that is not the last goes, and the template removed takes the binding along.
#[test]
fn a_binding_without_substitutions_keeps_the_last_parameter() {
    let mut s = session();
    let shop = shop(&mut s);
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let key = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type" }));
    let (mut s, binding) = bind(&s, &shop, false);
    let st = s.doc();
    assert!(templates::users(st, obj(st, &t)).is_empty() && templates::users(st, obj(st, &key)).is_empty(), "no substitution");
    // Both at once would take the signature the binding names.
    let undo = s.undo_label().map(str::to_string);
    let e = refused(&mut s, "element.remove", json!({ "ids": [t, key] }));
    assert!(e.contains("OrderRepository binds Repository") && e.contains("its last template parameter"), "{e}");
    // One that is not the last goes.
    run(&mut s, "element.remove", json!({ "ids": [key] }));
    assert_eq!(names(&mut s, &shop.repository), ["T"]);
    // The last is refused before anything is written: the store and the history as they were.
    let undo_after = s.undo_label().map(str::to_string);
    assert_ne!(undo_after, undo);
    let e = refused(&mut s, "element.remove", json!({ "ids": [t] }));
    assert!(e.contains("OrderRepository binds Repository, and T is its last template parameter"), "{e}");
    assert_eq!(s.undo_label().map(str::to_string), undo_after, "no undo step");
    assert!(alive(&s, &binding) && names(&mut s, &shop.repository) == ["T"]);
    let st = s.doc();
    assert!(templates::last_parameter_problem(st, obj(st, &t)).is_some(), "the shell asks the same rule");
    // The template and its last parameter removed together take the binding along.
    run(&mut s, "element.remove", json!({ "ids": [shop.repository, t] }));
    assert!(!alive(&s, &binding) && !alive(&s, &shop.repository));
}

/// Every diagram that shows the template draws the same box: the content is the model's.
#[test]
fn the_signature_box_is_drawn_on_every_diagram_that_shows_the_template() {
    let mut s = session();
    let shop = shop(&mut s);
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type", "default": "Integer" }));
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Overview", "element": shop.shop }));
    run(&mut s, "diagram.place", json!({ "diagram": d2, "element": shop.repository, "x": 100.0, "y": 300.0 }));
    let st = s.doc();
    let nodes = archforge_diagram::views_of(st, obj(st, &shop.repository)).2;
    assert_eq!(nodes.len(), 2, "a card on each diagram");
    let view = ProjectView::new(ALONE);
    for n in nodes {
        let d = archforge_diagram::diagram_of(st, n).unwrap();
        let element = obj(st, &shop.repository);
        let card = view.of_element(st, element).unwrap().card(st, element, d).unwrap();
        assert_eq!(card.template, ["T : Class", "Key : DataType = Integer"]);
    }
}

/// Template parameters, their kinds, constraints, defaults and order, saved as rows, read back
/// identical, and a model with them opens.
#[test]
fn templates_are_saved_and_read_back_identical() {
    let mut s = session();
    let shop = shop(&mut s);
    run(
        &mut s,
        "uml.template-parameter.add",
        json!({ "classifier": shop.repository, "name": "T", "constraint": shop.entity, "default": shop.order }),
    );
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "Key", "kind": "data-type", "default": "Integer" }));
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "S", "kind": "enumeration", "at": 0 }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    assert!(back.first_problem().is_none(), "{:?}", back.first_problem());
    let mut back = session_over(back);
    assert_eq!(listed(&mut back, &shop.repository), listed(&mut s, &shop.repository), "the same parameters, in the same order");
    assert_eq!(rows_of(&back, &shop.repository, &shop.d), ["S : Enumeration", "T : Class > Entity = Order", "Key : DataType = Integer"]);
}

/// The sweep over the template commands breaks nothing.
#[test]
fn the_sweep_over_templates_breaks_nothing() {
    let mut s = session();
    let shop = shop(&mut s);
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": shop.repository, "name": "T" }));
    let el = listed(&mut s, &shop.repository)[0]["element"].as_str().unwrap().to_string();
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.template-parameter.add", "uml.template-parameter.set", "uml.template-parameter.move", "uml.template-parameter.list"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [t.as_str(), el.as_str(), shop.repository.as_str(), shop.money.as_str(), shop.d.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}
