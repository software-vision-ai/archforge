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
    refused(&mut s, "uml.class.add", json!({ "package": sales, "name": "order" }), "there is a class named Order already");
    refused(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "ORDER" }), "there is a class named Order already");
    refused(&mut s, "uml.data-type.add", json!({ "package": sales, "name": "state" }), "there is an enumeration named State already");
    refused(&mut s, "uml.interface.rename", json!({ "interface": payable, "name": "Line" }), "there is a class named Line already");
    refused(
        &mut s,
        "uml.association-class.add",
        json!({ "from": order, "to": line, "name": "payable" }),
        "there is an interface named Payable already",
    );
    let elsewhere = run(&mut s, "uml.package.add", json!({ "name": "Elsewhere" }));
    let other = run(&mut s, "uml.class.add", json!({ "package": elsewhere, "name": "STATE" }));
    refused(&mut s, "uml.class.move", json!({ "class": other, "package": sales }), "there is an enumeration named State already");
    // Packages.
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Billing" }));
    let audit = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Audit" }));
    refused(&mut s, "uml.package.add", json!({ "parent": sales, "name": "billing" }), "there is a package named Billing already");
    refused(&mut s, "uml.package.rename", json!({ "package": audit, "name": "BILLING" }), "there is a package named Billing already");
    let billing = run(&mut s, "uml.package.add", json!({ "parent": elsewhere, "name": "Billing" }));
    refused(&mut s, "uml.package.move", json!({ "package": billing, "to": sales }), "there is a package named Billing already");
    // Attributes, operations and literals.
    let status = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status" }));
    let total = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    refused(&mut s, "uml.attribute.add", json!({ "class": order, "name": "STATUS" }), "the class has an attribute named status already");
    refused(&mut s, "uml.attribute.set", json!({ "attribute": total, "name": "Status" }), "the class has an attribute named status already");
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
        "there is a generalization set named kinds already",
    );
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [h], "name": "KINDS" }));
    refused(&mut s, "element.remove", json!({ "ids": [gone] }), "Sales has a generalization set named kinds already");
    // A class handed up onto a classifier's name is refused as it was.
    let set = set_of(&s, &h);
    run(&mut s, "uml.generalization-set.set", json!({ "set": set, "name": "Shapes" }));
    run(&mut s, "uml.class.rename", json!({ "class": a, "name": "payable" }));
    refused(&mut s, "element.remove", json!({ "ids": [gone] }), "Sales has an interface named Payable already");
}

/// The reason `cmd` is refused with, typed as a name another element of `kind`, named `name`, has;
/// the English names that element too, and nothing changes.
fn taken(s: &mut Session, cmd: &str, p: Json, kind: &'static str, name: &str) {
    let before = snapshot(s.doc());
    let Err(archforge_engine::EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
    assert_eq!(error.reason, Some(archforge_engine::Reason::NameTaken { kind, name: name.into() }), "{cmd} {p}");
    assert!(error.message.contains(&format!("{kind} named {name}")) || error.message.contains(&format!("{kind} named {name:?}")), "{cmd}: {error:?}");
    assert_eq!(snapshot(s.doc()), before, "{cmd} {p} changed nothing");
}

/// Every name clash of UML's members, sets, nesting and hand-up, ER's tables, types, columns and
/// relationships, and skills carries its reason typed, naming the element that has the name, as
/// the English does, for an interface to say in its own words (AR-435).
#[test]
fn every_name_clash_carries_its_reason_typed_with_the_element_that_has_the_name() {
    let (mut s, sales) = project();
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let state = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "State" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status" }));
    run(&mut s, "uml.operation.add", json!({ "class": order, "name": "advance" }));
    run(&mut s, "uml.literal.add", json!({ "enumeration": state, "name": "open" }));
    taken(&mut s, "uml.attribute.add", json!({ "class": order, "name": "STATUS" }), "attribute", "status");
    taken(&mut s, "uml.operation.add", json!({ "class": order, "name": "Advance" }), "operation", "advance");
    taken(&mut s, "uml.literal.add", json!({ "enumeration": state, "name": "OPEN" }), "literal", "open");
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    let g = run(&mut s, "uml.generalization.add", json!({ "specific": line, "general": order }));
    run(&mut s, "uml.generalization-set.add", json!({ "generalizations": [g], "name": "kinds" }));
    taken(&mut s, "uml.generalization-set.add", json!({ "generalizations": [g], "name": "Kinds" }), "generalization set", "kinds");
    // Nested under Order, which holds a class Item: another Item goes under it no more.
    run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "class", "name": "Item" }));
    let item = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "ITEM" }));
    taken(&mut s, "uml.classifier.set-owner", json!({ "classifier": item, "owner": order }), "class", "Item");
    // A package removed hands up a class onto the name of one in its parent.
    let gone = run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Gone" }));
    run(&mut s, "uml.class.add", json!({ "package": gone, "name": "line" }));
    taken(&mut s, "element.remove", json!({ "ids": [gone] }), "class", "Line");
    // Skills on one subject.
    run(&mut s, "skill.add", json!({ "subject": sales, "name": "Naming" }));
    let other = run(&mut s, "skill.add", json!({ "subject": sales, "name": "Style" }));
    taken(&mut s, "skill.add", json!({ "subject": sales, "name": "NAMING" }), "skill", "Naming");
    taken(&mut s, "skill.rename", json!({ "skill": other, "name": "naming" }), "skill", "Naming");
    // ER: the interface's words, a table, a domain, a column and a relationship.
    let mut s = er_project();
    let shop = er_id(&s, archforge_er::SCHEMA, "shop");
    let (customer, order) = (er_id(&s, archforge_er::ENTITY, "Customer"), er_id(&s, archforge_er::ENTITY, "Order"));
    taken(&mut s, "er.entity.add", json!({ "schema": shop, "name": "ORDER" }), "table", "Order");
    taken(&mut s, "er.entity.rename", json!({ "entity": order, "name": "Money" }), "domain", "money");
    taken(&mut s, "er.view.add", json!({ "schema": shop, "name": "Counter", "query": "SELECT 1" }), "sequence", "counter");
    taken(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "ID", "type": "TEXT" }), "column", "id");
    taken(&mut s, "er.relationship.add", json!({ "from": customer, "to": order, "name": "Places" }), "relationship", "places");
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
    refused(&mut s, "element.remove", json!({ "ids": [billing] }), "Sales has a package named AUDIT already");
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

// ER (AR-209): names as SQL has them, by ER's own rule (`archforge_er::names`).

/// The id of the element of `class` named `name`, from the store.
fn er_id(s: &Session, class: archforge_mof::ClassId, name: &str) -> String {
    let st = s.doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).unwrap();
    st.uuid(o).unwrap().to_string()
}

/// An ER project holding two elements of every ER kind in one owner, each pair named apart: the
/// schemas shop and billing; in shop the tables Customer and Order, the domain money (with the
/// checks positive and small), the enum type state, the sequence counter, the view report and the
/// relationships places and billed; in Customer the columns id and name, the primary key
/// pk_customer, the unique constraint uq_name, the checks named and positive_id and the indexes
/// ix_a and ix_b; in Order the foreign keys fk_places and fk_billed.
fn er_project() -> Session {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let shop = run(&mut s, "er.schema.add", json!({ "name": "shop" }));
    run(&mut s, "er.schema.add", json!({ "name": "billing" }));
    let customer = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Customer" }));
    let order = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Order" }));
    let checks = json!([{ "expression": "VALUE > 0", "name": "positive" }, { "expression": "VALUE < 100", "name": "small" }]);
    run(&mut s, "er.domain.add", json!({ "schema": shop, "name": "money", "base_type": "INTEGER", "checks": checks }));
    run(&mut s, "er.enum.add", json!({ "schema": shop, "name": "state", "literals": ["open"] }));
    run(&mut s, "er.sequence.add", json!({ "schema": shop, "name": "counter" }));
    run(&mut s, "er.view.add", json!({ "schema": shop, "name": "report", "query": "SELECT 1" }));
    let id = run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT" }));
    let name = run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "name", "type": "TEXT" }));
    run(&mut s, "er.primary-key.set", json!({ "entity": customer, "columns": [id], "name": "pk_customer" }));
    run(&mut s, "er.unique.add", json!({ "entity": customer, "columns": [name], "name": "uq_name" }));
    run(&mut s, "er.check.add", json!({ "entity": customer, "expression": "name <> ''", "name": "named" }));
    run(&mut s, "er.check.add", json!({ "entity": customer, "expression": "id > 0", "name": "positive_id" }));
    run(&mut s, "er.index.add", json!({ "entity": customer, "columns": [name], "name": "ix_a" }));
    run(&mut s, "er.index.add", json!({ "entity": customer, "columns": [id], "name": "ix_b" }));
    for (rel, role) in [("places", ""), ("billed", "payer")] {
        let p = json!({ "from": customer, "to": order, "name": rel, "from_role": role, "create_foreign_key": true });
        run(&mut s, "er.relationship.add", p);
    }
    let st = s.doc();
    let order_obj = st.by_uuid(Uuid::parse(&order).unwrap()).unwrap();
    let fks: Vec<String> = archforge_er::rules::foreign_keys(st, order_obj).into_iter().map(|fk| st.uuid(fk).unwrap().to_string()).collect();
    for (fk, name) in fks.iter().zip(["fk_places", "fk_billed"]) {
        run(&mut s, "er.foreign-key.rename", json!({ "foreign_key": fk, "name": name }));
    }
    reopens(s.doc()).unwrap();
    s
}

/// Two ER elements of one kind named alike in one owner, whatever the case, are refused by every
/// command that would make them, with the reason, and the model and its history stay as they were.
#[test]
fn two_er_elements_of_one_kind_named_alike_are_refused_by_every_command() {
    use archforge_er::{DOMAIN, ENTITY, ENUM_TYPE, RELATIONSHIP, SCHEMA, SEQUENCE, VIEW};
    let mut s = er_project();
    let (shop, billing) = (er_id(&s, SCHEMA, "shop"), er_id(&s, SCHEMA, "billing"));
    // Schemas: one name per schema in the model.
    refused(&mut s, "er.schema.add", json!({ "name": "SHOP" }), "there is a schema named shop already");
    refused(&mut s, "er.schema.rename", json!({ "schema": billing, "name": "Shop" }), "there is a schema named shop already");
    run(&mut s, "er.schema.rename", json!({ "schema": billing, "name": "Billing" }));
    // Tables, domains, enum types, sequences and views: one namespace per schema.
    let (order, state, counter) = (er_id(&s, ENTITY, "Order"), er_id(&s, ENUM_TYPE, "state"), er_id(&s, SEQUENCE, "counter"));
    let (money, report) = (er_id(&s, DOMAIN, "money"), er_id(&s, VIEW, "report"));
    refused(&mut s, "er.entity.add", json!({ "schema": shop, "name": "customer" }), "a table named \"Customer\" already");
    refused(&mut s, "er.entity.rename", json!({ "entity": order, "name": "MONEY" }), "a domain named \"money\" already");
    refused(&mut s, "er.domain.add", json!({ "schema": shop, "name": "STATE", "base_type": "TEXT" }), "an enum type named \"state\" already");
    refused(&mut s, "er.enum.set", json!({ "enum": state, "name": "Report", "literals": ["open"] }), "a view named \"report\" already");
    refused(&mut s, "er.sequence.set", json!({ "sequence": counter, "name": "order" }), "a table named \"Order\" already");
    refused(&mut s, "er.view.add", json!({ "schema": shop, "name": "Counter", "query": "SELECT 1" }), "a sequence named \"counter\" already");
    refused(&mut s, "er.view.set", json!({ "view": report, "name": "customer", "query": "SELECT 1" }), "a table named \"Customer\" already");
    refused(&mut s, "er.domain.set", json!({ "domain": money, "name": "Customer", "base_type": "TEXT" }), "a table named \"Customer\" already");
    let customer = er_id(&s, ENTITY, "Customer");
    let m2m = run(&mut s, "er.relationship.add", json!({ "from": customer, "to": order, "from_end": "zero-or-more" }));
    refused(&mut s, "er.relationship.resolve", json!({ "relationship": m2m, "name": "ORDER" }), "a table named \"Order\" already");
    // Columns: one name per column in a table.
    let column = |s: &Session, name: &str| {
        let st = s.doc();
        let e = st.by_uuid(Uuid::parse(&customer).unwrap()).unwrap();
        let a =
            st.list(e, archforge_er::entity::ATTRIBUTE).unwrap().iter().filter_map(Value::as_ref).find(|a| archforge_core::name_of(st, *a) == name);
        st.uuid(a.unwrap()).unwrap().to_string()
    };
    let (id, name) = (column(&s, "id"), column(&s, "name"));
    refused(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "ID", "type": "TEXT" }), "the table has a column named \"id\" already");
    refused(&mut s, "er.attribute.rename", json!({ "attribute": name, "name": "Id" }), "the table has a column named \"id\" already");
    // Relationships: one name per relationship in a schema.
    let billed = er_id(&s, RELATIONSHIP, "billed");
    refused(
        &mut s,
        "er.relationship.add",
        json!({ "from": customer, "to": order, "name": "Places" }),
        "the schema has a relationship named \"places\" already",
    );
    refused(
        &mut s,
        "er.relationship.rename",
        json!({ "relationship": billed, "name": "PLACES" }),
        "the schema has a relationship named \"places\" already",
    );
    // Keys, foreign keys, checks and indexes: one constraint name per schema, more than opening asks.
    let taken = "the schema has a constraint named";
    refused(&mut s, "er.unique.add", json!({ "entity": customer, "columns": [id], "name": "PK_CUSTOMER" }), taken);
    refused(&mut s, "er.primary-key.set", json!({ "entity": customer, "columns": [id], "name": "Uq_Name" }), taken);
    refused(&mut s, "er.check.add", json!({ "entity": order, "expression": "1 = 1", "name": "IX_A" }), taken);
    refused(&mut s, "er.index.add", json!({ "entity": customer, "columns": [name], "name": "Named" }), taken);
    let fk = {
        let st = s.doc();
        let fk = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::FOREIGN_KEY) && archforge_core::name_of(st, *o) == "fk_billed");
        st.uuid(fk.unwrap()).unwrap().to_string()
    };
    refused(&mut s, "er.foreign-key.rename", json!({ "foreign_key": fk, "name": "FK_PLACES" }), taken);
    // A domain's checks: one name per check in a domain.
    let twice = json!([{ "expression": "VALUE > 0", "name": "positive" }, { "expression": "VALUE < 9", "name": "POSITIVE" }]);
    refused(
        &mut s,
        "er.domain.add",
        json!({ "schema": shop, "name": "amount", "base_type": "INTEGER", "checks": twice }),
        "two checks are named POSITIVE",
    );
    refused(
        &mut s,
        "er.domain.set",
        json!({ "domain": money, "name": "money", "base_type": "INTEGER", "checks": twice }),
        "two checks are named POSITIVE",
    );
    // In another owner the same names are free.
    let other = run(&mut s, "er.entity.add", json!({ "schema": billing, "name": "customer" }));
    run(&mut s, "er.attribute.add", json!({ "entity": other, "name": "ID", "type": "TEXT" }));
    run(&mut s, "er.domain.add", json!({ "schema": billing, "name": "Money", "base_type": "INTEGER", "checks": [twice[0].clone()] }));
    reopens(s.doc()).unwrap();
}

/// ER elements of different kinds share a name in one owner, through the commands, and the project
/// opens again: a schema and its table, a table and its column, a relationship and a table, a
/// constraint and a column, a domain's check and its domain.
#[test]
fn er_elements_of_different_kinds_share_a_name_and_the_project_opens_again() {
    use archforge_er::{ENTITY, RELATIONSHIP, SCHEMA};
    let mut s = er_project();
    let shop = er_id(&s, SCHEMA, "shop");
    let table = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Shop" }));
    let column = run(&mut s, "er.attribute.add", json!({ "entity": table, "name": "SHOP", "type": "BIGINT" }));
    let places = er_id(&s, RELATIONSHIP, "places");
    run(&mut s, "er.relationship.rename", json!({ "relationship": places, "name": "Customer" }));
    run(&mut s, "er.unique.add", json!({ "entity": table, "columns": [column], "name": "shop" }));
    let order = er_id(&s, ENTITY, "Order");
    run(&mut s, "er.check.add", json!({ "entity": order, "expression": "1 = 1", "name": "order" }));
    let checks = json!([{ "expression": "VALUE > 0", "name": "amount" }]);
    run(&mut s, "er.domain.add", json!({ "schema": shop, "name": "amount", "base_type": "INTEGER", "checks": checks }));
    reopens(s.doc()).unwrap();
}

/// The project opens by the very rule ER's commands ask: for every named ER metaclass the kind is
/// ER's, and for every kind two elements named alike in one owner are what the commands refuse
/// ([`archforge_er::names::clash`], and for constraints the wider rule of the schema) and what the
/// check as it opens refuses; a name that clashes with nothing of its kind opens.
#[test]
fn er_opens_by_the_rule_its_commands_ask() {
    use archforge_er::names::{self, CONSTRAINT, RELATION, clash};
    use archforge_er::{
        ATTRIBUTE, CHECK_CONSTRAINT, DOMAIN, DOMAIN_CHECK, ENTITY, ENUM_TYPE, FOREIGN_KEY, INDEX, PRIMARY_KEY, RELATIONSHIP, SCHEMA, SEQUENCE,
        TABLE_INDEX, UNIQUE_CONSTRAINT, VIEW,
    };
    let u = universe().unwrap();
    let mut named = 0;
    for c in u.classes().filter(|c| c.metamodel() == INDEX) {
        if u.info(c).and_then(|i| i.name_prop).is_some() {
            named += 1;
            assert_eq!(name_kind(c), names::kind(c), "{}", u.class_name(c));
        }
    }
    assert_eq!(named, 16, "every named ER metaclass is asked, the abstract NamedElement and Key among them");
    for c in [ENTITY, DOMAIN, ENUM_TYPE, SEQUENCE, VIEW] {
        assert_eq!(name_kind(c), Some(RELATION), "{}", u.class_name(c));
    }
    for c in [PRIMARY_KEY, UNIQUE_CONSTRAINT, FOREIGN_KEY, CHECK_CONSTRAINT, TABLE_INDEX] {
        assert_eq!(name_kind(c), Some(CONSTRAINT), "{}", u.class_name(c));
    }
    for c in [SCHEMA, ATTRIBUTE, RELATIONSHIP, DOMAIN_CHECK] {
        assert_eq!(name_kind(c), Some(NameKind::Class(c)), "{}", u.class_name(c));
    }
    let s = er_project();
    let st = s.doc();
    let named_obj = |class, name: &str| st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).unwrap();
    let shop = named_obj(SCHEMA, "shop");
    // (the element whose name is taken, another of its kind in its owner, whether it is a constraint)
    let same_kind = [
        (named_obj(SCHEMA, "shop"), named_obj(SCHEMA, "billing"), false),
        (named_obj(ENTITY, "Customer"), named_obj(ENTITY, "Order"), false),
        (named_obj(ENTITY, "Customer"), named_obj(DOMAIN, "money"), false),
        (named_obj(ENUM_TYPE, "state"), named_obj(VIEW, "report"), false),
        (named_obj(VIEW, "report"), named_obj(SEQUENCE, "counter"), false),
        (named_obj(ATTRIBUTE, "id"), named_obj(ATTRIBUTE, "name"), false),
        (named_obj(RELATIONSHIP, "places"), named_obj(RELATIONSHIP, "billed"), false),
        (named_obj(PRIMARY_KEY, "pk_customer"), named_obj(UNIQUE_CONSTRAINT, "uq_name"), true),
        (named_obj(FOREIGN_KEY, "fk_places"), named_obj(FOREIGN_KEY, "fk_billed"), true),
        (named_obj(CHECK_CONSTRAINT, "named"), named_obj(CHECK_CONSTRAINT, "positive_id"), true),
        (named_obj(TABLE_INDEX, "ix_a"), named_obj(TABLE_INDEX, "ix_b"), true),
        (named_obj(DOMAIN_CHECK, "positive"), named_obj(DOMAIN_CHECK, "small"), false),
    ];
    let (tables, junctions) = storage();
    for (first, second, constraint) in same_kind {
        let taken = archforge_core::name_of(st, first).to_uppercase();
        let (owner, _) = st.container(second).unwrap().unwrap();
        let class = st.class_of(second).unwrap();
        let what = u.class_name(class);
        // What the commands ask of the name.
        assert_eq!(clash(st, owner, class, &taken, Some(second)), Some(first), "{what} {taken}: the commands refuse it");
        if constraint {
            assert!(!archforge_er::rules::constraint_name_free(st, shop, &taken, Some(second)), "{what} {taken}: the schema's constraints refuse it");
        }
        // What the check as it opens makes of it, written straight into the store.
        let mut copy = store(ids(1 << 41)).unwrap();
        rows::restore(&mut copy, &snapshot(st), &tables, &junctions).unwrap();
        let prop = copy.universe().info(class).and_then(|i| i.name_prop).unwrap();
        let second = copy.by_uuid(st.uuid(second).unwrap()).unwrap();
        copy.set(second, prop, Value::str(&taken)).unwrap();
        let e = reopens(&copy).expect_err(&format!("{what} {taken}: the project does not open"));
        assert!(e.contains("DuplicateName"), "{what} {taken}: {e}");
        // A name of no element of its kind there passes both.
        let free = format!("{taken}_free");
        assert_eq!(clash(&copy, copy.container(second).unwrap().unwrap().0, class, &free, Some(second)), None, "{what} {free}");
        copy.set(second, prop, Value::str(&free)).unwrap();
        reopens(&copy).unwrap_or_else(|e| panic!("{what} {free}: {e}"));
    }
}
