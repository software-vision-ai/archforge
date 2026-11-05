//! Skills over UML: packages, classes, interfaces, attributes and operations carry them (owner
//! decision), a relationship does not; a skill aims at every class, interface, attribute, operation
//! or association below
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

#[test]
fn a_relationship_carries_no_skill_and_the_refusal_names_what_does() {
    let mut s = session();
    let m = model(&mut s);
    let before = skills(s.doc()).len();
    let e = s.execute("skill.add", &json!({ "subject": m.association, "name": "Ends" })).unwrap_err().to_string();
    for word in ["Association", "Package", "Class", "Property", "Operation"] {
        assert!(e.contains(word), "{e}");
    }
    assert_eq!(skills(s.doc()).len(), before, "nothing changed");
}

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
    assert!(e.contains("class, interface, attribute, operation, association"), "{e}");
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
