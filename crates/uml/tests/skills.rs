//! Skills over UML: packages, classes, interfaces, enumerations, data types, primitive types of the
//! model's own, association classes, attributes and operations carry them (owner decision), a
//! relationship and a built-in type do not; a skill aims at every class, interface, enumeration,
//! data type, primitive type, association class, attribute, operation or association below
//! its subject, is inherited down the package tree and replaced by a deeper one of its name; each
//! skill.add one undo step.

use archforge_core::skills::{EffectiveSkill, effective_skills, skills};
use archforge_diagram::ProjectView;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid};
use archforge_uml::{ALONE, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

fn session() -> S {
    let mut n: u64 = 0;
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
    let mut st = Store::new(universe, Box::new(slabs), ids);
    st.create(archforge_core::MODEL).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

fn obj(s: &S, id: &str) -> Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// The names of the skills that apply to `element`, each with the name of the level it comes from
/// and whether a deeper one replaces it.
fn applying(s: &S, element: &str) -> Vec<(String, String, bool)> {
    let st = s.doc();
    effective_skills(s.view(), st, obj(s, element), None)
        .into_iter()
        .map(|EffectiveSkill { skill, origin, overridden_by }| {
            let from = origin.map(|o| archforge_core::name_of(st, o)).unwrap_or_default();
            (archforge_core::skills::name_of(st, skill), from, overridden_by.is_some())
        })
        .collect()
}

struct Model {
    sales: String,
    order: String,
    total: String,
    place: String,
    association: String,
}

fn model(s: &mut S) -> Model {
    let sales = run(s, "uml.package.add", json!({ "name": "Sales" }));
    let order = run(s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let customer = run(s, "uml.class.add", json!({ "package": sales, "name": "Customer" }));
    let total = run(s, "uml.attribute.add", json!({ "class": order, "name": "total", "type": "Real" }));
    let place = run(s, "uml.operation.add", json!({ "class": order, "name": "place" }));
    let association = run(s, "uml.association.add", json!({ "from": customer, "to": order }));
    Model { sales, order, total, place, association }
}

#[test]
fn a_package_a_class_an_attribute_and_an_operation_each_carry_a_skill_in_one_undo_step() {
    let mut s = session();
    let m = model(&mut s);
    for (subject, name) in [(&m.sales, "Package naming"), (&m.order, "Class naming"), (&m.total, "Money"), (&m.place, "Verbs")] {
        let before = skills(s.doc()).len();
        let skill = run(&mut s, "skill.add", json!({ "subject": subject, "name": name, "body": "Be brief." }));
        assert_eq!(archforge_core::skills::subject_of(s.doc(), obj(&s, &skill)), Some(obj(&s, subject)), "{name}");
        s.undo().unwrap();
        assert_eq!(skills(s.doc()).len(), before, "undo of {name}");
        s.redo().unwrap();
        assert_eq!(skills(s.doc()).len(), before + 1, "redo of {name}");
    }
}

/// An association and an interface realization carry no skill: the refusal names what does, and nothing changes.
#[test]
fn a_relationship_carries_no_skill_and_the_refusal_names_what_does() {
    let mut s = session();
    let m = model(&mut s);
    let payable = run(&mut s, "uml.interface.add", json!({ "package": m.sales, "name": "Payable" }));
    let realization = run(&mut s, "uml.realization.add", json!({ "class": m.order, "interface": payable }));
    let before = skills(s.doc()).len();
    for (subject, kind) in [(&m.association, "Association"), (&realization, "InterfaceRealization")] {
        let e = s.execute("skill.add", &json!({ "subject": subject, "name": "Ends" })).unwrap_err().to_string();
        for word in [kind, "Package", "Class", "Property", "Operation"] {
            assert!(e.contains(word), "{e}");
        }
    }
    assert_eq!(skills(s.doc()).len(), before, "nothing changed");
}

/// A package skill aims at the kind it names below it, and an ER kind is refused.
#[test]
fn a_package_skill_aims_at_the_kind_it_names_below_it() {
    let mut s = session();
    let m = model(&mut s);
    for (word, name) in [("class", "Classes"), ("attribute", "Attributes"), ("operation", "Operations"), ("association", "Associations")] {
        run(&mut s, "skill.add", json!({ "subject": m.sales, "name": name, "applies_to": word }));
    }
    let names = |s: &S, e: &str| applying(s, e).into_iter().map(|(n, _, _)| n).collect::<Vec<_>>();
    assert_eq!(names(&s, &m.order), ["Classes"]);
    assert_eq!(names(&s, &m.total), ["Attributes"]);
    assert_eq!(names(&s, &m.place), ["Operations"]);
    assert_eq!(names(&s, &m.association), ["Associations"]);
    // An ER kind is not one of UML's.
    let e = s.execute("skill.add", &json!({ "subject": m.sales, "name": "Tables", "applies_to": "table" })).unwrap_err().to_string();
    assert!(e.contains("class, interface, enumeration, data type, primitive type, association class, attribute, operation, association"), "{e}");
}

/// An enumeration carries skills as an interface does, and a package skill for enumerations aims
/// at them alone.
#[test]
fn an_enumeration_carries_skills_and_a_package_skill_for_enumerations_aims_only_at_them() {
    let mut s = session();
    let m = model(&mut s);
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": m.sales, "name": "OrderStatus" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": m.sales, "name": "Payable" }));
    // The enumeration takes a skill of its own, as one undo step, and it goes with it.
    let before = skills(s.doc()).len();
    let skill = run(&mut s, "skill.add", json!({ "subject": status, "name": "States", "body": "Name them as past participles." }));
    assert_eq!(archforge_core::skills::subject_of(s.doc(), obj(&s, &skill)), Some(obj(&s, &status)));
    s.undo().unwrap();
    assert_eq!(skills(s.doc()).len(), before);
    s.redo().unwrap();
    // A package skill for enumerations aims at them alone.
    run(&mut s, "skill.add", json!({ "subject": m.sales, "name": "Enumerations", "applies_to": "enumeration" }));
    let names = |s: &S, e: &str| applying(s, e).into_iter().map(|(n, _, _)| n).collect::<Vec<_>>();
    assert_eq!(names(&s, &status), ["Enumerations", "States"]);
    assert_eq!(names(&s, &m.order), Vec::<String>::new(), "a class is no enumeration");
    assert_eq!(names(&s, &payable), Vec::<String>::new(), "nor is an interface");
    run(&mut s, "element.remove", json!({ "ids": [status] }));
    assert!(skills(s.doc()).iter().all(|k| archforge_core::skills::name_of(s.doc(), *k) != "States"), "its skill went with it");
}

/// A data type carries skills as an interface and an enumeration do, and a package skill for every
/// data type aims at data types alone: not at an enumeration, which UML makes a data type too, nor
/// at a class.
#[test]
fn a_data_type_carries_skills_and_a_package_skill_for_data_types_aims_only_at_them() {
    let mut s = session();
    let m = model(&mut s);
    let money = run(&mut s, "uml.data-type.add", json!({ "package": m.sales, "name": "Money" }));
    let amount = run(&mut s, "uml.attribute.add", json!({ "class": money, "name": "amount", "type": "Real" }));
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": m.sales, "name": "OrderStatus" }));
    // The data type takes a skill of its own, as one undo step, which reaches its attributes.
    let before = skills(s.doc()).len();
    let skill = run(&mut s, "skill.add", json!({ "subject": money, "name": "Values", "body": "Keep them immutable." }));
    assert_eq!(archforge_core::skills::subject_of(s.doc(), obj(&s, &skill)), Some(obj(&s, &money)));
    s.undo().unwrap();
    assert_eq!(skills(s.doc()).len(), before);
    s.redo().unwrap();
    // A package skill for every data type aims at them alone.
    run(&mut s, "skill.add", json!({ "subject": m.sales, "name": "Data types", "applies_to": "data type" }));
    let names = |s: &S, e: &str| applying(s, e).into_iter().map(|(n, _, _)| n).collect::<Vec<_>>();
    assert_eq!(names(&s, &money), ["Data types", "Values"]);
    assert_eq!(names(&s, &amount), ["Values"], "its own skill reaches its members");
    assert_eq!(names(&s, &status), Vec::<String>::new(), "an enumeration is no data type here");
    assert_eq!(names(&s, &m.order), Vec::<String>::new(), "nor is a class");
    run(&mut s, "element.remove", json!({ "ids": [money] }));
    assert!(skills(s.doc()).iter().all(|k| archforge_core::skills::name_of(s.doc(), *k) != "Values"), "its skill went with it");
}

/// A model-level skill for every data type skips the built-in types: Integer is a PrimitiveType,
/// which UML's metamodel makes a DataType, but a primitive type is a skill kind of its own (AR-124),
/// the closer one. A data type still gets the skill.
#[test]
fn a_skill_for_every_data_type_skips_the_built_in_primitive_types() {
    let mut s = session();
    let m = model(&mut s);
    let money = run(&mut s, "uml.data-type.add", json!({ "package": m.sales, "name": "Money" }));
    run(&mut s, "uml.attribute.add", json!({ "class": m.order, "name": "count", "type": "Integer" }));
    run(&mut s, "skill.add", json!({ "subject": "", "name": "Values", "applies_to": "data type" }));
    let integer = {
        let st = s.doc();
        let t = st
            .alive()
            .find(|o| st.class_of(*o).ok() == Some(archforge_uml::PRIMITIVE_TYPE) && archforge_core::name_of(st, *o) == "Integer")
            .expect("Integer is made in Types when first used");
        st.uuid(t).unwrap().to_string()
    };
    let effective = |s: &mut S, element: &str| {
        let out = s.execute("skill.effective", &json!({ "element": element })).unwrap().value;
        out["skills"].as_array().unwrap().iter().map(|k| k["name"].as_str().unwrap().to_string()).collect::<Vec<_>>()
    };
    assert_eq!(effective(&mut s, &integer), Vec::<String>::new(), "a primitive type is no data type here");
    assert_eq!(effective(&mut s, &money), ["Values"], "a data type gets it");
}

/// The primitive type value of the type named `name` in the package Types, made when first used.
fn built_in(s: &S, name: &str) -> String {
    let st = s.doc();
    let t = st
        .alive()
        .find(|o| archforge_uml::members::is_built_in(st, *o) && archforge_core::name_of(st, *o) == name)
        .unwrap_or_else(|| panic!("{name} is made in Types when first used"));
    st.uuid(t).unwrap().to_string()
}

/// A primitive type of the model's own carries skills as a data type does (AR-124), and a skill for
/// every primitive type aims at those alone: never at a data type, a class or a built-in type; a
/// skill for every data type does not reach a primitive type, the closer kind.
#[test]
fn a_primitive_type_carries_skills_and_a_skill_for_every_primitive_type_aims_only_at_the_model_s_own() {
    let mut s = session();
    let m = model(&mut s);
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": m.sales, "name": "Sku" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": m.sales, "name": "Money" }));
    run(&mut s, "uml.attribute.add", json!({ "class": m.order, "name": "count", "type": "Integer" }));
    let integer = built_in(&s, "Integer");
    // Its own skill, one undo step.
    let before = skills(s.doc()).len();
    let skill = run(&mut s, "skill.add", json!({ "subject": sku, "name": "Codes", "body": "Upper case." }));
    assert_eq!(archforge_core::skills::subject_of(s.doc(), obj(&s, &skill)), Some(obj(&s, &sku)));
    s.undo().unwrap();
    assert_eq!(skills(s.doc()).len(), before);
    s.redo().unwrap();
    // The model's skills for every primitive type and for every data type each reach their kind alone.
    run(&mut s, "skill.add", json!({ "subject": "", "name": "Primitives", "applies_to": "primitive type" }));
    run(&mut s, "skill.add", json!({ "subject": "", "name": "Values", "applies_to": "data type" }));
    let names = |s: &S, e: &str| applying(s, e).into_iter().map(|(n, _, _)| n).collect::<Vec<_>>();
    assert_eq!(names(&s, &sku), ["Primitives", "Codes"], "a primitive type gets the primitive skill, never the data type one");
    assert_eq!(names(&s, &money), ["Values"], "a data type gets the data type skill alone");
    assert_eq!(names(&s, &m.order), Vec::<String>::new(), "a class neither");
    assert_eq!(names(&s, &integer), Vec::<String>::new(), "a built-in type neither");
    // Removed, it takes its skill along.
    run(&mut s, "element.remove", json!({ "ids": [sku] }));
    assert!(skills(s.doc()).iter().all(|k| archforge_core::skills::name_of(s.doc(), *k) != "Codes"), "its skill went with it");
}

/// A built-in type carries no skill: skill.add on Integer is refused, naming its id and saying a
/// primitive type of the model's own does, and nothing changes; the model's own primitive type
/// beside it takes one.
#[test]
fn a_built_in_type_refuses_a_skill_naming_it_and_what_carries_one() {
    let mut s = session();
    let m = model(&mut s);
    run(&mut s, "uml.attribute.add", json!({ "class": m.order, "name": "count", "type": "Integer" }));
    let integer = built_in(&s, "Integer");
    let before = skills(s.doc()).len();
    let e = s.execute("skill.add", &json!({ "subject": integer, "name": "Ints" })).unwrap_err().to_string();
    assert!(e.contains(&integer) && e.contains("a built-in type carries no skill; a primitive type of the model's own does"), "{e}");
    assert_eq!(skills(s.doc()).len(), before, "nothing changed");
    let st = s.doc();
    assert!(archforge_core::skills::subject_problem(s.view(), st, obj(&s, &integer)).is_some(), "the shell asks the same rule before it offers +");
    let sku = run(&mut s, "uml.primitive-type.add", json!({ "package": m.sales, "name": "Sku" }));
    assert!(archforge_core::skills::subject_problem(s.view(), s.doc(), obj(&s, &sku)).is_none(), "a primitive type of the model's own carries one");
}

/// An association class carries skills as the class it is, its members too; it is a kind of its
/// own (AR-125): UML's AssociationClass conforms to Class and to Association equally, so the core's
/// closer-kind rule keeps a skill for every class and one for every association off it, and a skill
/// for every association class reaches it alone. Its ends carry none, as an association's do not.
#[test]
fn an_association_class_carries_skills_and_only_a_skill_for_association_classes_aims_at_it() {
    let mut s = session();
    let m = model(&mut s);
    let buyer = run(&mut s, "uml.class.add", json!({ "package": m.sales, "name": "Buyer" }));
    let review = run(&mut s, "uml.association-class.add", json!({ "from": buyer, "to": m.order, "name": "Review" }));
    let stars = run(&mut s, "uml.attribute.add", json!({ "class": review, "name": "stars", "type": "Integer" }));
    // It takes a skill of its own, as one undo step, which reaches its members.
    let before = skills(s.doc()).len();
    let skill = run(&mut s, "skill.add", json!({ "subject": review, "name": "Reviews", "body": "Keep them short." }));
    assert_eq!(archforge_core::skills::subject_of(s.doc(), obj(&s, &skill)), Some(obj(&s, &review)));
    s.undo().unwrap();
    assert_eq!(skills(s.doc()).len(), before);
    s.redo().unwrap();
    for (word, name) in
        [("class", "Classes"), ("association", "Associations"), ("association class", "Association classes"), ("attribute", "Attributes")]
    {
        run(&mut s, "skill.add", json!({ "subject": m.sales, "name": name, "applies_to": word }));
    }
    let names = |s: &S, e: &str| applying(s, e).into_iter().map(|(n, _, _)| n).collect::<Vec<_>>();
    assert_eq!(names(&s, &review), ["Association classes", "Reviews"], "neither every class nor every association reaches it");
    assert_eq!(names(&s, &m.order), ["Classes"], "a plain class is no association class");
    assert_eq!(names(&s, &m.association), ["Associations"], "nor is a plain association");
    assert_eq!(names(&s, &stars), ["Attributes", "Reviews"]);
    // Its end carries no skill, and no attribute skill aims at it.
    let end = s.execute("uml.class.get", &json!({ "class": review })).unwrap().value["ends"][0]["id"].as_str().unwrap().to_string();
    assert!(s.execute("skill.add", &json!({ "subject": end, "name": "Ends" })).is_err());
    assert!(applying(&s, &end).iter().all(|(n, _, _)| n != "Attributes"));
    run(&mut s, "element.remove", json!({ "ids": [review] }));
    assert!(skills(s.doc()).iter().all(|k| archforge_core::skills::name_of(s.doc(), *k) != "Reviews"), "its skill went with it");
}

/// An interface carries skills as a class does, and a package skill for interfaces aims at them alone.
#[test]
fn an_interface_and_its_members_carry_skills_and_a_package_skill_for_interfaces_aims_only_at_them() {
    let mut s = session();
    let m = model(&mut s);
    let payable = run(&mut s, "uml.interface.add", json!({ "package": m.sales, "name": "Payable" }));
    let due = run(&mut s, "uml.attribute.add", json!({ "class": payable, "name": "due", "type": "Real" }));
    let pay = run(&mut s, "uml.operation.add", json!({ "class": payable, "name": "pay" }));
    // The interface takes a skill of its own, as one undo step, and it goes with the interface.
    let before = skills(s.doc()).len();
    let skill = run(&mut s, "skill.add", json!({ "subject": payable, "name": "Contracts", "body": "Name them for a capability." }));
    assert_eq!(archforge_core::skills::subject_of(s.doc(), obj(&s, &skill)), Some(obj(&s, &payable)));
    s.undo().unwrap();
    assert_eq!(skills(s.doc()).len(), before);
    s.redo().unwrap();
    // A package skill for interfaces aims at them alone; one for attributes aims at an
    // interface's attributes too, and the interface's own skill reaches its members.
    run(&mut s, "skill.add", json!({ "subject": m.sales, "name": "Interfaces", "applies_to": "interface" }));
    run(&mut s, "skill.add", json!({ "subject": m.sales, "name": "Attributes", "applies_to": "attribute" }));
    let names = |s: &S, e: &str| applying(s, e).into_iter().map(|(n, _, _)| n).collect::<Vec<_>>();
    assert_eq!(names(&s, &payable), ["Interfaces", "Contracts"]);
    assert_eq!(names(&s, &m.order), Vec::<String>::new(), "a class is no interface");
    assert_eq!(names(&s, &due), ["Attributes", "Contracts"]);
    assert_eq!(names(&s, &pay), ["Contracts"]);
    run(&mut s, "element.remove", json!({ "ids": [payable] }));
    assert!(skills(s.doc()).iter().all(|k| archforge_core::skills::name_of(s.doc(), *k) != "Contracts"), "its skill went with it");
}

#[test]
fn skills_are_inherited_down_the_package_tree_and_a_deeper_one_of_the_name_replaces_them() {
    let mut s = session();
    let m = model(&mut s);
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": m.sales }));
    let line = run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Line" }));
    let qty = run(&mut s, "uml.attribute.add", json!({ "class": line, "name": "qty", "type": "Integer" }));
    run(&mut s, "skill.add", json!({ "subject": "", "name": "House style" }));
    run(&mut s, "skill.add", json!({ "subject": m.sales, "name": "Naming" }));
    run(&mut s, "skill.add", json!({ "subject": line, "name": "Naming" }));
    run(&mut s, "skill.add", json!({ "subject": qty, "name": "Units" }));
    assert_eq!(
        applying(&s, &qty),
        [
            ("House style".to_string(), String::new(), false),
            ("Naming".to_string(), "Sales".to_string(), true),
            ("Naming".to_string(), "Line".to_string(), false),
            ("Units".to_string(), "qty".to_string(), false),
        ]
    );
    // The other package's class keeps the outer one.
    assert_eq!(
        applying(&s, &m.order).iter().map(|(n, from, _)| (n.as_str(), from.as_str())).collect::<Vec<_>>(),
        [("House style", ""), ("Naming", "Sales")]
    );
}

#[test]
fn an_association_end_carries_no_skill_and_no_attribute_skill_aims_at_it() {
    let mut s = session();
    let m = model(&mut s);
    let a = obj(&s, &m.association);
    let end = {
        let st = s.doc();
        let ends = st.list(a, archforge_uml::association::MEMBER_END).unwrap();
        st.uuid(ends[0].as_ref().unwrap()).unwrap().to_string()
    };
    let before = skills(s.doc()).len();
    let e = s.execute("skill.add", &json!({ "subject": end, "name": "Ends" })).unwrap_err().to_string();
    assert!(e.contains("association's end"), "{e}");
    assert_eq!(skills(s.doc()).len(), before, "nothing changed");
    run(&mut s, "skill.add", json!({ "subject": m.sales, "name": "Attributes", "applies_to": "attribute" }));
    assert!(applying(&s, &end).iter().all(|(n, _, _)| n != "Attributes"), "{:?}", applying(&s, &end));
    assert_eq!(applying(&s, &m.total)[0].0, "Attributes");
}

/// How many skills there are, and how many apply to the whole model.
fn sitting(s: &S) -> (usize, usize) {
    let st = s.doc();
    let all = skills(st);
    let model_wide = all.iter().filter(|k| archforge_core::skills::subject_of(st, **k).is_none()).count();
    (all.len(), model_wide)
}

#[test]
fn a_skill_goes_with_the_element_it_sits_on_and_comes_back_with_undo() {
    let mut s = session();
    let m = model(&mut s);
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": m.sales }));
    let line = run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Line" }));
    for subject in [&m.total, &m.place, &m.order, &orders, &line, &m.sales] {
        run(&mut s, "skill.add", json!({ "subject": subject, "name": "Skill" }));
    }
    assert_eq!(sitting(&s), (6, 0));
    // A member and a class each take their own skill along.
    for (gone, left) in [(&m.total, 5), (&m.order, 3)] {
        let before = sitting(&s);
        run(&mut s, "element.remove", json!({ "ids": [gone] }));
        assert_eq!(sitting(&s), (left, 0), "none turns model-wide");
        s.undo().unwrap();
        assert_eq!(sitting(&s), before, "undo brings them back");
    }
    // A nested package's skill goes with it (owner decision); its class moves up with its own.
    run(&mut s, "element.remove", json!({ "ids": [orders] }));
    assert_eq!(sitting(&s), (5, 0));
    let st = s.doc();
    assert!(skills(st).into_iter().any(|k| archforge_core::skills::subject_of(st, k) == Some(obj(&s, &line))), "Line keeps its skill");
    s.undo().unwrap();
    // sales sits in the model's root package too: its own skill goes, and everything below it
    // moves up into the root with its skills.
    run(&mut s, "element.remove", json!({ "ids": [m.sales] }));
    assert_eq!(sitting(&s), (5, 0));
    s.undo().unwrap();
    assert_eq!(sitting(&s), (6, 0));
}

/// The core's name and text rules carry their reason typed beside the English sentence, which
/// stays as it was: a skill's empty name, a class's name with a hidden character or too long, and a
/// skill's body with a control character (AR-434).
#[test]
fn the_cores_name_rules_carry_their_reason_typed() {
    use archforge_engine::{EngineError, Reason};
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let max = archforge_uml::commands::MAX_NAME;
    for (cmd, p, reason, message) in [
        ("skill.add", json!({ "subject": sales, "name": "" }), Reason::NameEmpty, "the name is empty".to_string()),
        (
            "uml.class.add",
            json!({ "package": sales, "name": "Or\u{200B}der" }),
            Reason::ControlCharacter,
            "the name holds a control or hidden character".into(),
        ),
        (
            "uml.class.add",
            json!({ "package": sales, "name": "n".repeat(max + 1) }),
            Reason::TooLong { max },
            format!("the name is longer than {max} characters"),
        ),
        ("uml.class.add", json!({ "package": sales, "name": "Order " }), Reason::EdgeSpace, "the name starts or ends with a space".into()),
        (
            "skill.add",
            json!({ "subject": sales, "name": "Brief", "body": "a\u{7}b" }),
            Reason::ControlCharacter,
            "the body holds a control or hidden character".into(),
        ),
    ] {
        let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
        assert_eq!((error.reason, error.message), (Some(reason), message), "{cmd} {p}");
    }
}
