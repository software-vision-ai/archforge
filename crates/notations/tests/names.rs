//! Names in one owner, as UML has them (owner decision, AR-208): elements of different kinds may
//! share a name, two of one kind may not, whatever the case. What a command lets through, the
//! project's check as it opens lets through too, and what the check refuses no command makes.

use archforge_mof::{NameKind, Obj, Store, Uuid, Value, rows};
use archforge_notations::{Session, name_kind, new_project_session, session_from_rows, storage, store, universe};
use serde_json::{Value as Json, json};

fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

fn run(s: &mut Session, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

fn snapshot(st: &Store) -> rows::Rows {
    let (tables, junctions) = storage();
    rows::snapshot(st, &tables, &junctions).unwrap()
}

/// The project as it is, opened again from its rows: refused or not.
fn reopens(st: &Store) -> Result<(), String> {
    session_from_rows(&snapshot(st), ids(1 << 40)).map(|_| ())
}

/// `cmd` is refused with `words` in the reason, leaving the model and its history as they were,
/// and the project still opens.
fn refused(s: &mut Session, cmd: &str, p: Json, words: &str) {
    let (before, revision, undo) = (snapshot(s.doc()), s.revision(), s.undo_label().map(str::to_string));
    let e = s.execute(cmd, &p).map(|_| ()).expect_err(&format!("{cmd} {p} is refused"));
    assert!(e.to_string().contains(words), "{cmd} {p}: {e}");
    assert_eq!(snapshot(s.doc()), before, "{cmd} {p} changed nothing");
    assert_eq!((s.revision(), s.undo_label().map(str::to_string)), (revision, undo), "{cmd} {p} left the history as it was");
    reopens(s.doc()).unwrap();
}

/// A project with the package Sales.
fn project() -> (Session, String) {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    (s, sales)
}

/// The two projects the bug was found with: an attribute and an operation named alike, whatever
/// the case, and a package holding a package and a class of one name. Both are made and open again.
#[test]
fn an_attribute_and_an_operation_or_a_package_and_a_class_named_alike_are_made_and_open_again() {
    let (mut s, sales) = project();
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    run(&mut s, "uml.operation.add", json!({ "class": order, "name": "Total" }));
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    run(&mut s, "uml.attribute.add", json!({ "class": line, "name": "total" }));
    run(&mut s, "uml.operation.add", json!({ "class": line, "name": "total" }));
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Billing" }));
    run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Billing" }));
    reopens(s.doc()).unwrap();
}

/// Elements of different kinds share a name in one owner through every command that names, adds
/// or moves one, and the project opens again.
#[test]
fn elements_of_different_kinds_share_a_name_through_every_command() {
    let (mut s, sales) = project();
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let item = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Item" }));
    // An attribute and an operation, renamed onto each other's names; a data type's too.
    let status = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status" }));
    let advance = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "advance" }));
    run(&mut s, "uml.attribute.set", json!({ "attribute": status, "name": "Advance" }));
    run(&mut s, "uml.operation.set", json!({ "operation": advance, "name": "STATUS" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": sales, "name": "Money" }));
    run(&mut s, "uml.attribute.add", json!({ "class": money, "name": "amount" }));
    run(&mut s, "uml.operation.add", json!({ "class": money, "name": "amount" }));
    // A package and the classifiers beside it: added, renamed and moved onto each other's names.
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Billing" }));
    run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "billing" }));
    let audit = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Audit" }));
    run(&mut s, "uml.class.rename", json!({ "class": item, "name": "audit" }));
    run(&mut s, "uml.package.rename", json!({ "package": audit, "name": "ORDER" }));
    let line = run(&mut s, "uml.package.add", json!({ "name": "Line" }));
    run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    run(&mut s, "uml.package.move", json!({ "package": line, "to": sales }));
    let elsewhere = run(&mut s, "uml.package.add", json!({ "name": "Elsewhere" }));
    let tax = run(&mut s, "uml.class.add", json!({ "package": elsewhere, "name": "Tax" }));
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "tax" }));
    run(&mut s, "uml.class.move", json!({ "class": tax, "package": sales }));
    // A nested package removed hands up a class named like a package of its parent.
    let old = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Old" }));
    run(&mut s, "uml.class.add", json!({ "package": old, "name": "Archive" }));
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "archive" }));
    run(&mut s, "element.remove", json!({ "ids": [old] }));
    // A generalization set, relationships and a constraint named like a classifier, and named
    // alike: their names are labels.
    let specific = audit_class(&s, &sales);
    let g = run(&mut s, "uml.generalization.add", json!({ "specific": specific, "general": order }));
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [g], "name": "Order" }));
    run(&mut s, "uml.association.add", json!({ "from": order, "to": specific, "name": "order" }));
    run(&mut s, "uml.association.add", json!({ "from": specific, "to": order, "name": "ORDER" }));
    run(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": money, "name": "uses" }));
    run(&mut s, "uml.dependency.add", json!({ "client": money, "supplier": order, "name": "Uses" }));
    run(&mut s, "uml.constraint.add", json!({ "context": order, "name": "advance", "expression": "total >= 0" }));
    run(&mut s, "uml.constraint.add", json!({ "context": order, "name": "Advance", "expression": "true" }));
    // Diagrams named alike, by hand and as packages of one name in different parents name theirs.
    run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "Main" }));
    run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "main" }));
    run(&mut s, "uml.package.diagram", json!({ "package": line }));
    let other_line = run(&mut s, "uml.package.add", json!({ "parent": elsewhere, "name": "Line" }));
    run(&mut s, "uml.package.diagram", json!({ "package": other_line }));
    reopens(s.doc()).unwrap();
}

/// The class renamed audit in the package Sales.
fn audit_class(s: &Session, sales: &str) -> String {
    let st = s.doc();
    let sales = st.by_uuid(Uuid::parse(sales).unwrap()).unwrap();
    let c = archforge_uml::commands::classes(st, sales).into_iter().find(|c| archforge_core::name_of(st, *c) == "audit").unwrap();
    st.uuid(c).unwrap().to_string()
}

/// Two elements of one kind in one owner, whatever the case and, for classifiers, whatever their
/// metaclasses, are refused by every command that would make them, with the reason, and the model
/// and its history stay as they were.
#[test]
fn two_elements_of_one_kind_named_alike_are_refused_by_every_command() {
    let (mut s, sales) = project();
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let state = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "State" }));
    let payable = run(&mut s, "uml.interface.add", json!({ "package": sales, "name": "Payable" }));
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    // Classifiers: one kind, whatever their metaclasses.
    refused(&mut s, "uml.class.add", json!({ "package": sales, "name": "order" }), "there is a class named order already");
    refused(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "ORDER" }), "there is a class named ORDER already");
    refused(&mut s, "uml.data-type.add", json!({ "package": sales, "name": "state" }), "there is an enumeration named state already");
    refused(&mut s, "uml.interface.rename", json!({ "interface": payable, "name": "Line" }), "there is a class named Line already");
    refused(
        &mut s,
        "uml.association-class.add",
        json!({ "from": order, "to": line, "name": "payable" }),
        "there is an interface named payable already",
    );
    let elsewhere = run(&mut s, "uml.package.add", json!({ "name": "Elsewhere" }));
    let other = run(&mut s, "uml.class.add", json!({ "package": elsewhere, "name": "STATE" }));
    refused(&mut s, "uml.class.move", json!({ "class": other, "package": sales }), "there is an enumeration named STATE already");
    // Packages.
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Billing" }));
    let audit = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Audit" }));
    refused(&mut s, "uml.package.add", json!({ "parent": sales, "name": "billing" }), "there is a package named billing already");
    refused(&mut s, "uml.package.rename", json!({ "package": audit, "name": "BILLING" }), "there is a package named BILLING already");
    let billing = run(&mut s, "uml.package.add", json!({ "parent": elsewhere, "name": "Billing" }));
    refused(&mut s, "uml.package.move", json!({ "package": billing, "to": sales }), "there is a package named Billing already");
    // Attributes, operations and literals.
    let status = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status" }));
    let total = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    refused(&mut s, "uml.attribute.add", json!({ "class": order, "name": "STATUS" }), "the class has an attribute named STATUS already");
    refused(&mut s, "uml.attribute.set", json!({ "attribute": total, "name": "Status" }), "the class has an attribute named Status already");
    let advance = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "advance" }));
    let cancel = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "cancel" }));
    refused(&mut s, "uml.operation.add", json!({ "class": order, "name": "Advance" }), "the class has an operation named advance already");
    refused(&mut s, "uml.operation.set", json!({ "operation": cancel, "name": "ADVANCE" }), "the class has an operation named advance already");
    // Its own name, again, is its own.
    run(&mut s, "uml.operation.set", json!({ "operation": advance, "name": "Advance" }));
    run(&mut s, "uml.attribute.set", json!({ "attribute": status, "name": "Status" }));
    run(&mut s, "uml.literal.add", json!({ "enumeration": state, "name": "open" }));
    refused(&mut s, "uml.literal.add", json!({ "enumeration": state, "name": "Open" }), "there is a literal named open already");
    // Generalization sets, added and handed up when the package holding one goes.
    let g = run(&mut s, "uml.generalization.add", json!({ "specific": line, "general": order }));
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [g], "name": "kinds" }));
    let gone = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Gone" }));
    let (a, b) = (
        run(&mut s, "uml.class.add", json!({ "package": gone, "name": "A" })),
        run(&mut s, "uml.class.add", json!({ "package": gone, "name": "B" })),
    );
    let h = run(&mut s, "uml.generalization.add", json!({ "specific": b, "general": a }));
    refused(
        &mut s,
        "uml.generalization-set.add",
        json!({ "generalizations": [g], "name": "Kinds" }),
        "there is a generalization set named Kinds already",
    );
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [h], "name": "KINDS" }));
    refused(&mut s, "element.remove", json!({ "ids": [gone] }), "Sales has a generalization set named KINDS already");
    // A class handed up onto a classifier's name is refused as it was.
    let set = set_of(&s, &h);
    run(&mut s, "uml.generalization-set.set", json!({ "set": set, "name": "Shapes" }));
    run(&mut s, "uml.class.rename", json!({ "class": a, "name": "payable" }));
    refused(&mut s, "element.remove", json!({ "ids": [gone] }), "Sales has an interface named payable already");
}

/// A nested package removed hands up an element named like itself, since it goes; a clash with
/// another of its parent's elements is still refused.
#[test]
fn a_removed_package_hands_up_an_element_of_its_own_name() {
    let (mut s, sales) = project();
    let billing = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Billing" }));
    let inner = run(&mut s, "uml.package.add", json!({ "parent": billing, "name": "billing" }));
    let class = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Billing" }));
    let audit = run(&mut s, "uml.package.add", json!({ "parent": billing, "name": "Audit" }));
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "AUDIT" }));
    refused(&mut s, "element.remove", json!({ "ids": [billing] }), "Sales has a package named Audit already");
    run(&mut s, "uml.package.rename", json!({ "package": audit, "name": "Reports" }));
    run(&mut s, "element.remove", json!({ "ids": [billing] }));
    let st = s.doc();
    let sales = st.by_uuid(Uuid::parse(&sales).unwrap()).unwrap();
    for id in [&inner, &class] {
        let o = st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
        assert!(st.is_alive(o) && archforge_uml::commands::package_of(st, o) == Some(sales), "{id} is handed up to Sales");
    }
    reopens(st).unwrap();
}

/// The id of the generalization set generalization `g` is in.
fn set_of(s: &Session, g: &str) -> String {
    let st = s.doc();
    let g = st.by_uuid(Uuid::parse(g).unwrap()).unwrap();
    let set = st.referrers(g).into_iter().map(|(o, _)| o).find(|o| st.class_of(*o).ok() == Some(archforge_uml::GENERALIZATION_SET)).unwrap();
    st.uuid(set).unwrap().to_string()
}

/// The project opens by the very rule UML's commands ask, and its kinds are UML's.
#[test]
fn the_project_opens_by_the_rule_the_commands_ask() {
    let u = universe().unwrap();
    let uml_name = archforge_uml::named_element::NAME;
    let mut named = 0;
    for c in u.classes() {
        if u.info(c).and_then(|i| i.name_prop) == Some(uml_name) {
            named += 1;
            assert_eq!(name_kind(c), archforge_uml::names::kind(c), "{}", u.class_name(c));
        }
    }
    assert!(named > 100, "every named UML metaclass is asked: {named}");
    let classifier = Some(archforge_uml::names::CLASSIFIER);
    for c in archforge_uml::CLASSIFIERS {
        assert_eq!(name_kind(c), classifier, "{}", u.class_name(c));
    }
    let own = [
        archforge_uml::PACKAGE,
        archforge_uml::PROPERTY,
        archforge_uml::OPERATION,
        archforge_uml::PARAMETER,
        archforge_uml::ENUMERATION_LITERAL,
        archforge_uml::GENERALIZATION_SET,
    ];
    for c in own {
        assert_eq!(name_kind(c), Some(NameKind::Class(c)), "{}", u.class_name(c));
    }
    for c in [archforge_uml::ASSOCIATION, archforge_uml::DEPENDENCY, archforge_uml::INTERFACE_REALIZATION, archforge_uml::CONSTRAINT] {
        assert_eq!(name_kind(c), None, "{}: a label", u.class_name(c));
    }
    assert_eq!(name_kind(archforge_diagram::DIAGRAM), None, "a diagram's name is a label");
    assert_eq!(name_kind(archforge_core::SKILL), None, "skills keep their own rule");
    assert!(name_kind(archforge_er::ENTITY).is_some(), "the ER names keep theirs");
}

/// `name` given to a new `class` element pushed into `owner`'s `prop`.
fn put(st: &mut Store, owner: Obj, prop: archforge_mof::PropId, class: archforge_mof::ClassId, name: &str) -> Obj {
    let o = st.create(class).unwrap();
    st.set(o, archforge_uml::named_element::NAME, Value::str(name)).unwrap();
    st.push(owner, prop, Value::Ref(o)).unwrap();
    o
}

/// A project saved with names the check as it opened refused, written straight into the store as
/// no command would, opens now; one with two elements of one kind named alike still does not.
#[test]
fn a_project_saved_with_names_the_old_check_refused_opens_and_one_kind_named_twice_does_not() {
    use archforge_uml::{CLASS, OPERATION, PACKAGE, PROPERTY, class, package};
    let (s, sales) = project();
    let mut st = store(ids(1 << 41)).unwrap();
    let (tables, junctions) = storage();
    rows::restore(&mut st, &snapshot(s.doc()), &tables, &junctions).unwrap();
    let sales = st.by_uuid(Uuid::parse(&sales).unwrap()).unwrap();
    let order = put(&mut st, sales, package::PACKAGED_ELEMENT, CLASS, "Order");
    put(&mut st, order, class::OWNED_ATTRIBUTE, PROPERTY, "total");
    put(&mut st, order, class::OWNED_OPERATION, OPERATION, "Total");
    put(&mut st, sales, package::PACKAGED_ELEMENT, PACKAGE, "order");
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    for name in ["Main", "MAIN"] {
        archforge_diagram::commands::new_diagram(&mut st, "uml.class", name, Some(sales), Some((model, archforge_core::model::MEMBER))).unwrap();
    }
    assert!(st.first_problem().is_some(), "the check as it was refuses it");
    reopens(&st).unwrap();
    // Two operations of one name, or two classifiers, still do not open.
    let mut twice = store(ids(1 << 42)).unwrap();
    rows::restore(&mut twice, &snapshot(&st), &tables, &junctions).unwrap();
    let order = twice.alive().find(|o| twice.class_of(*o).ok() == Some(CLASS)).unwrap();
    put(&mut twice, order, class::OWNED_OPERATION, OPERATION, "TOTAL");
    assert!(reopens(&twice).unwrap_err().contains("DuplicateName"));
    let mut twice = store(ids(1 << 43)).unwrap();
    rows::restore(&mut twice, &snapshot(&st), &tables, &junctions).unwrap();
    let sales = twice.alive().find(|o| twice.class_of(*o).ok() == Some(PACKAGE) && archforge_core::name_of(&twice, *o) == "Sales").unwrap();
    put(&mut twice, sales, package::PACKAGED_ELEMENT, archforge_uml::ENUMERATION, "ORDER");
    assert!(reopens(&twice).unwrap_err().contains("DuplicateName"));
}
