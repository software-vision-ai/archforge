//! Association classes through the engine (AR-125): one element, UML's AssociationClass, made
//! between two classes in one undo step, owning both its ends as an association does; its ends set
//! and reconnected with the association commands, its attributes and operations with the member
//! commands, its name kept apart from its package's classifiers; refused at an interface, an
//! enumeration, a data type or a primitive type with the id and the accepted kinds; removed with
//! its ends and members, or with a class it joins; saved as rows and read back identical; the
//! sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::{ALONE, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// A store over the core, the diagrams and UML, its ids counted up from `start` so two stores never
/// share one.
fn store(start: u64) -> Store {
    let mut n: u64 = start;
    let ids = Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    });
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

/// A session over a store with the project's Model.
fn session() -> S {
    let mut st = store(0);
    st.create(archforge_core::MODEL).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// The tables and junctions of the core, the diagrams and UML, as a project stores them.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

/// The session's model as the rows it would save.
fn snapshot(s: &S) -> String {
    format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap())
}

/// Runs `cmd`, which must succeed, and gives the id it made or named.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// Runs `cmd`, which must succeed as one undo step: undone the rows are as before, redone as after.
/// Gives the id it made or named.
fn round_trip(s: &mut S, cmd: &str, p: Json) -> String {
    let before = snapshot(s);
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    let after = snapshot(s);
    s.undo().unwrap();
    assert_eq!(snapshot(s), before, "undo of {cmd}");
    s.redo().unwrap();
    assert_eq!(snapshot(s), after, "redo of {cmd}");
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// `cmd` is refused with a message holding every one of `words`, and the model is as it was.
fn refused(s: &mut S, cmd: &str, p: Json, words: &[&str]) {
    let before = snapshot(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} is refused")).to_string();
    for w in words {
        assert!(e.contains(w), "{cmd} {p}: {e} lacks {w:?}");
    }
    assert_eq!(snapshot(s), before, "{cmd} {p} changed nothing");
}

/// The element with the id `id`.
fn obj(s: &S, id: &str) -> Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// Whether the element with the id `id` is gone from the model.
fn gone(s: &S, id: &str) -> bool {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).is_none_or(|o| !s.doc().is_alive(o))
}

/// A package Shop with classes Customer, Product and Store, an interface Rated, an enumeration
/// Stars, a data type Money and a primitive type Sku, and a class diagram that shows the three
/// classes: the package, the diagram, the classifiers' ids in that order and the classes' nodes.
fn shop(s: &mut S) -> (String, String, Vec<String>, Vec<String>) {
    let pkg = run(s, "uml.package.add", json!({ "name": "Shop" }));
    let d = run(s, "diagram.add", json!({ "kind": "uml.class", "element": pkg, "name": "Shop" }));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for (i, name) in ["Customer", "Product", "Store"].into_iter().enumerate() {
        let c = run(s, "uml.class.add", json!({ "package": pkg, "name": name }));
        nodes.push(run(s, "diagram.place", json!({ "diagram": d, "element": c, "x": 400 * i, "y": 300 })));
        ids.push(c);
    }
    for (cmd, name) in
        [("uml.interface.add", "Rated"), ("uml.enumeration.add", "Stars"), ("uml.data-type.add", "Money"), ("uml.primitive-type.add", "Sku")]
    {
        ids.push(run(s, cmd, json!({ "package": pkg, "name": name })));
    }
    (pkg, d, ids, nodes)
}

/// The two ends of association class `ac`, as `uml.class.get` gives them.
fn ends(s: &mut S, ac: &str) -> Vec<Json> {
    s.execute("uml.class.get", &json!({ "class": ac })).unwrap().value["ends"].as_array().unwrap().clone()
}

/// An association class is one element made in one undo step between two classes, in the first
/// one's package, owning both its ends: no roles, no multiplicities, navigable to neither class; its
/// line is an association's, its card says Association class.
#[test]
fn an_association_class_is_one_element_made_in_one_step_owning_two_plain_ends() {
    use archforge_diagram::figure::EndMark as M;
    let mut s = session();
    let (pkg, d, c, _) = shop(&mut s);
    let before = s.doc().alive().count();
    let ac = round_trip(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    let st = s.doc();
    let a = obj(&s, &ac);
    assert_eq!(st.class_of(a).unwrap(), archforge_uml::ASSOCIATION_CLASS);
    // The association class and its two ends, nothing more.
    assert_eq!(st.alive().count(), before + 3);
    assert_eq!(archforge_uml::commands::package_of(st, a), Some(obj(&s, &pkg)));
    assert_eq!(archforge_uml::figures::ends(st, a), Some((obj(&s, &c[0]), obj(&s, &c[1]))));
    for end in archforge_uml::figures::member_ends(st, a).unwrap() {
        assert_eq!(st.container(end).unwrap(), Some((a, archforge_uml::association::OWNED_END)), "the association class owns its end");
        assert!(!archforge_uml::figures::navigable(st, a, end), "navigable to neither class");
    }
    let line = archforge_uml::figures::line(st, a).unwrap();
    assert_eq!([line.ends[0].mark, line.ends[1].mark], [M::None, M::None]);
    assert!(line.ends.iter().all(|e| e.text.is_none() && e.role.is_none()), "no roles, no multiplicities");
    let card = archforge_uml::figures::card(st, a, obj(&s, &d)).unwrap();
    assert_eq!((card.name.as_str(), card.compartments.len()), ("Review", 2));
    assert!(matches!(card.eyebrow, archforge_diagram::figure::Text::Said("Association class", _)));
    // Its classes gain no attribute, and both list it among their relationships.
    for class in &c[..2] {
        assert!(archforge_uml::members::attributes(s.doc(), obj(&s, class)).is_empty());
        let listed = s.execute("uml.relationship.list", &json!({ "class": class })).unwrap().value["relationships"].clone();
        assert_eq!(listed[0]["kind"], "association_class");
        assert_eq!(listed[0]["id"], ac.as_str());
    }
    // Read as a class, it says what it is and has its ends.
    let read = s.execute("uml.class.get", &json!({ "class": ac })).unwrap().value;
    assert_eq!(read["kind"], "association_class");
    assert_eq!(read["ends"].as_array().map(Vec::len), Some(2));
    // Without a name it is made too, as an association is.
    run(&mut s, "uml.association-class.add", json!({ "from": c[1], "to": c[2] }));
}

/// Its ends are classes only: an interface, an enumeration, a data type or a primitive type at
/// either end is refused, the refusal naming the id, what it is and the kinds an end may be.
#[test]
fn only_classes_are_at_its_ends_and_a_refusal_names_the_id_and_the_accepted_kinds() {
    let mut s = session();
    let (_, _, c, _) = shop(&mut s);
    for (other, what) in [(&c[3], "an interface"), (&c[4], "an enumeration"), (&c[5], "a data type"), (&c[6], "a primitive type")] {
        for (from, to, key) in [(&c[0], other, "to"), (other, &c[0], "from")] {
            refused(
                &mut s,
                "uml.association-class.add",
                json!({ "from": from, "to": to }),
                &[key, other.as_str(), what, "an association class joins only classes (a class or an association class)"],
            );
        }
    }
    // A class of the model's own and an association class are both classes.
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    run(&mut s, "uml.association-class.add", json!({ "from": ac, "to": c[2], "name": "Moderation" }));
}

/// Its name is one of its package's classifiers' names: a class's taken name is refused for it,
/// and its name for a class, whatever the case; it renames and moves as the class it is.
#[test]
fn its_name_is_kept_apart_from_its_package_s_classifiers_and_it_renames_and_moves() {
    let mut s = session();
    let (pkg, _, c, _) = shop(&mut s);
    refused(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "product" }), &["name", "a class named product"]);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    refused(&mut s, "uml.class.add", json!({ "package": pkg, "name": "REVIEW" }), &["an association class named REVIEW"]);
    refused(&mut s, "uml.interface.rename", json!({ "interface": c[3], "name": "review" }), &["an association class named review"]);
    refused(&mut s, "uml.association-class.rename", json!({ "association_class": ac, "name": "Store" }), &["a class named Store"]);
    round_trip(&mut s, "uml.association-class.rename", json!({ "association_class": ac, "name": "Rating" }));
    assert_eq!(archforge_core::name_of(s.doc(), obj(&s, &ac)), "Rating");
    // It may go without a name, as an association does.
    round_trip(&mut s, "uml.association-class.rename", json!({ "association_class": ac, "name": "" }));
    run(&mut s, "uml.association-class.rename", json!({ "association_class": ac, "name": "Rating" }));
    // Moved to another package, its ends stay at their classes.
    let other = run(&mut s, "uml.package.add", json!({ "name": "Reviews" }));
    round_trip(&mut s, "uml.association-class.move", json!({ "association_class": ac, "package": other }));
    assert_eq!(archforge_uml::commands::package_of(s.doc(), obj(&s, &ac)), Some(obj(&s, &other)));
    assert_eq!(archforge_uml::figures::ends(s.doc(), obj(&s, &ac)), Some((obj(&s, &c[0]), obj(&s, &c[1]))));
}

/// Its ends are set with the association's command, its attributes and operations added with the
/// member commands as a class's; the ends never show among its attributes.
#[test]
fn its_ends_take_the_association_commands_and_its_members_the_class_commands() {
    let mut s = session();
    let (_, _, c, _) = shop(&mut s);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    round_trip(
        &mut s,
        "uml.association.set-end",
        json!({ "association": ac, "end": 0, "role": "reviewer", "multiplicity": "*", "navigable": false }),
    );
    round_trip(
        &mut s,
        "uml.association.set-end",
        json!({ "association": ac, "end": 1, "role": "item", "multiplicity": "1", "aggregation": "shared" }),
    );
    let e = ends(&mut s, &ac);
    assert_eq!((e[0]["role"].as_str(), e[0]["multiplicity"].as_str(), e[0]["navigable"].as_bool()), (Some("reviewer"), Some("*"), Some(false)));
    assert_eq!((e[1]["role"].as_str(), e[1]["aggregation"].as_str()), (Some("item"), Some("shared")));
    // The whole's end of the aggregation is not navigable.
    refused(&mut s, "uml.association.set-end", json!({ "association": ac, "end": 0, "navigable": true }), &["navigable"]);
    let stars = round_trip(&mut s, "uml.attribute.add", json!({ "class": ac, "name": "stars", "type": "Integer" }));
    let text = round_trip(&mut s, "uml.attribute.add", json!({ "class": ac, "name": "text", "type": "String" }));
    let publish = round_trip(&mut s, "uml.operation.add", json!({ "class": ac, "name": "publish" }));
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": stars, "name": "rating", "type": "Integer", "multiplicity": "0..1" }));
    round_trip(&mut s, "uml.attribute.move", json!({ "attribute": text, "at": 0 }));
    let read = s.execute("uml.class.get", &json!({ "class": ac })).unwrap().value;
    let names: Vec<&str> = read["attributes"].as_array().unwrap().iter().filter_map(|a| a["name"].as_str()).collect();
    assert_eq!(names, ["text", "rating"], "its attributes, never its ends");
    assert_eq!(read["operations"][0]["id"], publish.as_str());
    refused(&mut s, "uml.attribute.add", json!({ "class": ac, "name": "Text" }), &["the association class has an attribute named Text already"]);
    // Its card lists them; an end is no attribute to remove alone.
    let card = archforge_uml::figures::card(s.doc(), obj(&s, &ac), obj(&s, &ac)).unwrap();
    assert_eq!(card.compartments.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
    let end = e[0]["id"].as_str().unwrap().to_string();
    refused(&mut s, "element.remove", json!({ "ids": [end] }), &["ids"]);
}

/// Its ends reconnect through `uml.relationship.reconnect`, onto a class only and never onto
/// itself, its line following on a diagram that shows the new class; nothing specializes it, while
/// it implements an interface and specializes a class as the class it is.
#[test]
fn it_reconnects_onto_classes_only_and_keeps_the_generalization_and_realization_rules() {
    let mut s = session();
    let (_, d, c, nodes) = shop(&mut s);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    let edge = run(&mut s, "diagram.connect", json!({ "diagram": d, "element": ac, "source": nodes[0], "target": nodes[1] }));
    round_trip(&mut s, "uml.relationship.reconnect", json!({ "relationship": ac, "end": "target", "class": c[2] }));
    assert_eq!(archforge_uml::figures::ends(s.doc(), obj(&s, &ac)), Some((obj(&s, &c[0]), obj(&s, &c[2]))));
    assert_eq!(s.doc().get(obj(&s, &edge), archforge_diagram::edge::TARGET).unwrap(), Value::Ref(obj(&s, &nodes[2])));
    for (other, what) in [(&c[3], "an interface"), (&c[4], "an enumeration"), (&c[5], "a data type"), (&c[6], "a primitive type")] {
        refused(
            &mut s,
            "uml.relationship.reconnect",
            json!({ "relationship": ac, "end": "source", "class": other }),
            &["class", other.as_str(), what],
        );
    }
    refused(&mut s, "uml.relationship.reconnect", json!({ "relationship": ac, "end": "source", "class": ac }), &["the association class itself"]);
    // Nothing specializes it; it specializes a class and implements an interface.
    refused(&mut s, "uml.generalization.add", json!({ "specific": c[0], "general": ac }), &["general", "nothing specializes"]);
    let other = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Rating" }));
    refused(&mut s, "uml.generalization.add", json!({ "specific": other, "general": ac }), &["general", "nothing specializes"]);
    round_trip(&mut s, "uml.generalization.add", json!({ "specific": ac, "general": c[2] }));
    round_trip(&mut s, "uml.realization.add", json!({ "class": ac, "interface": c[3] }));
}

/// `element.remove` takes the association class with its ends, members and views as one undo step;
/// a class it joins takes it along, and what touches it goes with it in turn.
#[test]
fn removal_takes_its_ends_members_and_views_and_a_class_it_joins_takes_it_along() {
    let mut s = session();
    let (_, d, c, nodes) = shop(&mut s);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    let stars = run(&mut s, "uml.attribute.add", json!({ "class": ac, "name": "stars" }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": ac, "source": nodes[0], "target": nodes[1] }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": ac, "x": 200, "y": 0 }));
    let depends = run(&mut s, "uml.dependency.add", json!({ "client": c[2], "supplier": ac }));
    let counts = |s: &S| {
        let d = obj(s, &d);
        (s.doc().list(d, archforge_diagram::diagram::NODE).unwrap().len(), s.doc().list(d, archforge_diagram::diagram::EDGE).unwrap().len())
    };
    assert_eq!(counts(&s), (4, 1));
    let end = ends(&mut s, &ac)[0]["id"].as_str().unwrap().to_string();
    round_trip(&mut s, "element.remove", json!({ "ids": [ac] }));
    assert!(gone(&s, &ac) && gone(&s, &stars) && gone(&s, &end) && gone(&s, &depends));
    assert_eq!(counts(&s), (3, 0), "its card and its line went");
    s.undo().unwrap();
    // A class it joins takes it along, and the dependency on it with it.
    round_trip(&mut s, "element.remove", json!({ "ids": [c[1]] }));
    assert!(gone(&s, &ac) && gone(&s, &stars) && gone(&s, &depends) && !gone(&s, &c[0]));
    assert_eq!(counts(&s), (2, 0));
}

/// Removing its card from a diagram takes its line from that diagram too: the diagram shows it
/// whole or not at all. The model is untouched.
#[test]
fn unplacing_its_card_takes_its_line_from_the_diagram_too() {
    let mut s = session();
    let (_, d, c, nodes) = shop(&mut s);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": ac, "source": nodes[0], "target": nodes[1] }));
    let card = run(&mut s, "diagram.place", json!({ "diagram": d, "element": ac, "x": 200, "y": 0 }));
    let edges = |s: &S| s.doc().list(obj(s, &d), archforge_diagram::diagram::EDGE).unwrap().len();
    round_trip(&mut s, "diagram.unplace", json!({ "nodes": [card] }));
    assert_eq!(edges(&s), 0, "its line went with its card");
    assert!(!gone(&s, &ac), "the model keeps it");
    // `diagram.connect-all` draws its line again where both its classes are shown.
    run(&mut s, "diagram.connect-all", json!({ "diagram": d }));
    assert_eq!(edges(&s), 1);
}

/// Association classes, their ends, members, cards and lines come back from their rows identical,
/// as a saved project opens.
#[test]
fn association_classes_come_back_from_their_rows_identical() {
    let mut s = session();
    let (_, d, c, nodes) = shop(&mut s);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    run(&mut s, "uml.association.set-end", json!({ "association": ac, "end": 1, "role": "item", "multiplicity": "1..*" }));
    run(&mut s, "uml.attribute.add", json!({ "class": ac, "name": "stars", "type": "Integer" }));
    run(&mut s, "uml.operation.add", json!({ "class": ac, "name": "publish" }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": ac, "source": nodes[0], "target": nodes[1] }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": ac, "x": 200, "y": 0 }));
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(archforge_mof::rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let at = |id: &str| back.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    let a = at(&ac);
    assert_eq!(back.class_of(a).unwrap(), archforge_uml::ASSOCIATION_CLASS);
    assert_eq!(archforge_uml::figures::ends(&back, a), Some((at(&c[0]), at(&c[1]))));
    assert_eq!(archforge_uml::members::attributes(&back, a).len(), 1);
    assert_eq!(archforge_uml::members::operations(&back, a).len(), 1);
    let end = archforge_uml::figures::member_ends(&back, a).unwrap()[1];
    assert_eq!((archforge_core::name_of(&back, end), archforge_uml::figures::multiplicity(&back, end)), ("item".to_string(), "1..*".to_string()));
    let (_, edges, nodes) = archforge_diagram::views_of(&back, a);
    assert_eq!((edges.len(), nodes.len()), (1, 1), "its line and its card");
}

/// The command sweep, with an association class, its classes, a member and the kinds it refuses
/// among the ids, breaks nothing.
#[test]
fn the_sweep_over_association_classes_breaks_nothing() {
    let mut s = session();
    let (pkg, d, c, nodes) = shop(&mut s);
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": c[0], "to": c[1], "name": "Review" }));
    let stars = run(&mut s, "uml.attribute.add", json!({ "class": ac, "name": "stars" }));
    run(&mut s, "diagram.connect", json!({ "diagram": d, "element": ac, "source": nodes[0], "target": nodes[1] }));
    let card = run(&mut s, "diagram.place", json!({ "diagram": d, "element": ac, "x": 200, "y": 0 }));
    // The association class's own commands are in the registry the sweep walks.
    let names: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.association-class.add", "uml.association-class.rename", "uml.association-class.move"] {
        assert!(names.iter().any(|n| n == cmd), "{cmd}");
    }
    let ids = [
        pkg.as_str(),
        d.as_str(),
        c[0].as_str(),
        c[1].as_str(),
        c[3].as_str(),
        c[4].as_str(),
        ac.as_str(),
        stars.as_str(),
        card.as_str(),
        "not-an-id",
    ];
    let calls = sweep_with(&mut s, &ids, |st| format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())).unwrap();
    assert!(calls > 100, "{calls}");
}
