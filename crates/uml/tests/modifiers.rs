//! Property modifiers through the engine (AR-126): a default value read by the property's type,
//! derived, read only, ordered and unique, and the properties it subsets and redefines, set and
//! cleared on attributes with `uml.attribute.*` and on association ends with
//! `uml.association.set-end`, one undo step each; a refusal names the value or the target and why,
//! and changes nothing; a type its default does not fit is refused, naming the default; the card's
//! and the line's notation, a static member's row underlined (AR-136); a literal that goes takes its
//! defaults along; saved as rows and read back identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid};
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
    assert_eq!(snapshot(s), before, "undo of {cmd} {p}");
    s.redo().unwrap();
    assert_eq!(snapshot(s), after, "redo of {cmd} {p}");
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// `cmd` is refused under `key` with a message holding every one of `words`, and the model is as
/// it was. Gives the message.
fn refused(s: &mut S, cmd: &str, p: Json, key: &str, words: &[&str]) -> String {
    let before = snapshot(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} is refused")).to_string();
    assert!(e.contains(&format!("{key}:")) || e.starts_with(key) || e.contains(key), "{cmd} {p}: {e} is not about {key}");
    for w in words {
        assert!(e.contains(w), "{cmd} {p}: {e} lacks {w:?}");
    }
    assert_eq!(snapshot(s), before, "{cmd} {p} changed nothing");
    e
}

/// The element with the id `id`.
fn obj(s: &S, id: &str) -> Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// The card row of attribute `id`.
fn row(s: &S, id: &str) -> String {
    archforge_uml::figures::attribute_text(s.doc(), obj(s, id))
}

/// Attribute `id` as `uml.class.get` gives it, from its class `class`.
fn read(s: &mut S, class: &str, id: &str) -> Json {
    let out = s.execute("uml.class.get", &json!({ "class": class })).unwrap();
    out.value["attributes"].as_array().unwrap().iter().find(|a| a["id"] == id).cloned().unwrap()
}

/// A package Shop with the classes Item and Order, Order specializing Item, an enumeration Status
/// with the literals Placed and Paid, and a class diagram: the package, the diagram, Item, Order
/// and Status.
fn shop(s: &mut S) -> (String, String, String, String, String) {
    let pkg = run(s, "uml.package.add", json!({ "name": "Shop" }));
    let d = run(s, "diagram.add", json!({ "kind": "uml.class", "element": pkg, "name": "Shop" }));
    let item = run(s, "uml.class.add", json!({ "package": pkg, "name": "Item" }));
    let order = run(s, "uml.class.add", json!({ "package": pkg, "name": "Order" }));
    run(s, "uml.generalization.add", json!({ "specific": order, "general": item }));
    let status = run(s, "uml.enumeration.add", json!({ "package": pkg, "name": "Status" }));
    for l in ["Placed", "Paid"] {
        run(s, "uml.literal.add", json!({ "enumeration": status, "name": l }));
    }
    (pkg, d, item, order, status)
}

/// Each modifier set on an attribute and cleared again, one undo step each, its name, type,
/// multiplicity, visibility and static kept; the card says each as the design writes it.
#[test]
fn each_modifier_is_set_and_cleared_on_an_attribute_one_undo_step_each() {
    let mut s = session();
    let (_, _, _, order, _) = shop(&mut s);
    let base = json!({ "name": "lines", "type": "Integer", "multiplicity": "1..*", "visibility": "private", "static": true });
    let lines = run(
        &mut s,
        "uml.attribute.add",
        json!({ "class": order, "name": "lines", "type": "Integer", "multiplicity": "1..*", "visibility": "private", "static": true }),
    );
    assert_eq!(row(&s, &lines), "- lines : Integer [1..*]");
    let with = |extra: Json| {
        let mut p = base.clone();
        p["attribute"] = json!(lines);
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        p
    };
    for (extra, shown) in [
        (json!({ "default": "3" }), "- lines : Integer [1..*] = 3"),
        (json!({ "derived": true }), "- / lines : Integer [1..*]"),
        (json!({ "read_only": true }), "- lines : Integer [1..*] {readOnly}"),
        (json!({ "ordered": true }), "- lines : Integer [1..*] {ordered}"),
        (json!({ "unique": false }), "- lines : Integer [1..*] {nonunique}"),
        (
            json!({ "derived": true, "default": "0", "read_only": true, "ordered": true, "unique": false }),
            "- / lines : Integer [1..*] = 0 {readOnly, ordered, nonunique}",
        ),
    ] {
        round_trip(&mut s, "uml.attribute.set", with(extra.clone()));
        assert_eq!(row(&s, &lines), shown, "{extra}");
        let got = read(&mut s, &order, &lines);
        assert_eq!(
            (got["type"]["name"].as_str(), got["multiplicity"].as_str(), got["visibility"].as_str(), got["static"].as_bool()),
            (Some("Integer"), Some("1..*"), Some("private"), Some(true)),
            "kept: {got}"
        );
        // Cleared explicitly: an empty default, the flags sent as they usually are.
        let clear = json!({ "default": "", "derived": false, "read_only": false, "ordered": false, "unique": true });
        round_trip(&mut s, "uml.attribute.set", with(clear));
        assert_eq!(row(&s, &lines), "- lines : Integer [1..*]", "cleared after {extra}");
        let got = read(&mut s, &order, &lines);
        assert_eq!(
            (got["default"].clone(), got["derived"].clone(), got["read_only"].clone(), got["ordered"].clone(), got["unique"].clone()),
            (Json::Null, json!(false), json!(false), json!(false), json!(true))
        );
    }
    // Left out, each is kept: setting one leaves the others as they were.
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": lines, "default": "4", "ordered": true }));
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": lines, "derived": true }));
    assert_eq!(row(&s, &lines), "- / lines : Integer [1..*] = 4 {ordered}");
    // Added with them in one step too.
    let total =
        round_trip(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total", "type": "Real", "default": "0.5", "derived": true }));
    assert_eq!(row(&s, &total), "+ / total : Real = 0.5");
}

/// The card row of member `id` of the classifier `owner` on `diagram`, as the canvas draws it.
fn card_row(s: &S, owner: &str, diagram: &str, id: &str) -> archforge_diagram::figure::Row {
    let st = s.doc();
    let card = archforge_uml::figures::card(st, obj(s, owner), obj(s, diagram)).unwrap();
    let member = obj(s, id);
    let rows: Vec<_> = card.compartments.into_iter().flatten().collect();
    let attributes = archforge_uml::members::attributes(st, obj(s, owner));
    let operations = archforge_uml::members::operations(st, obj(s, owner));
    let at = attributes.iter().chain(operations.iter()).position(|m| *m == member).unwrap();
    rows[at].clone()
}

/// A static attribute or operation is underlined on every card kind that lists members, on every
/// diagram that shows it, read from the model: a static abstract operation is both slanted and
/// underlined, clearing Static takes the underline away as one undo step, setting another modifier
/// keeps it, and it comes back from the saved rows (AR-136).
#[test]
fn a_static_member_is_underlined_on_its_card() {
    use archforge_diagram::figure::Row;
    let mut s = session();
    let (pkg, d, item, order, status) = shop(&mut s);
    let other = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": pkg, "name": "Other" }));
    let marks = |r: Row| (r.slanted, r.underlined);
    // A static attribute and a plain one; a static operation, a static abstract one, an abstract one.
    let count = round_trip(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer", "static": true }));
    let id = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "id", "type": "Integer" }));
    let make = round_trip(&mut s, "uml.operation.add", json!({ "class": order, "name": "make", "static": true }));
    let rule = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "rule", "static": true, "abstract": true }));
    let pay = run(&mut s, "uml.operation.add", json!({ "class": order, "name": "pay", "abstract": true }));
    for diagram in [&d, &other] {
        assert_eq!(marks(card_row(&s, &order, diagram, &count)), (false, true), "static attribute");
        assert_eq!(marks(card_row(&s, &order, diagram, &id)), (false, false), "plain attribute");
        assert_eq!(marks(card_row(&s, &order, diagram, &make)), (false, true), "static operation");
        assert_eq!(marks(card_row(&s, &order, diagram, &rule)), (true, true), "static abstract operation");
        assert_eq!(marks(card_row(&s, &order, diagram, &pay)), (true, false), "abstract operation");
    }
    // The row's text is the member's signature as before: the underline is the only static mark.
    assert_eq!(card_row(&s, &order, &d, &count).text, "+ count : Integer");
    // Another modifier set keeps Static, and so the underline.
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": count, "default": "3", "read_only": true }));
    let row = card_row(&s, &order, &d, &count);
    assert_eq!((row.text.as_str(), row.underlined), ("+ count : Integer = 3 {readOnly}", true));
    // Static cleared: the underline goes, undo brings it back, redo takes it again.
    s.execute("uml.attribute.set", &json!({ "attribute": count, "static": false })).unwrap();
    assert!(!card_row(&s, &order, &d, &count).underlined, "cleared");
    s.undo().unwrap();
    assert!(card_row(&s, &order, &d, &count).underlined, "undone");
    s.redo().unwrap();
    assert!(!card_row(&s, &order, &d, &count).underlined, "redone");
    // An operation's Static cleared keeps its Abstract, and its slant.
    s.execute("uml.operation.set", &json!({ "operation": rule, "name": "rule", "static": false, "abstract": true })).unwrap();
    assert_eq!(marks(card_row(&s, &order, &d, &rule)), (true, false), "abstract kept, static cleared");
    s.undo().unwrap();
    assert_eq!(marks(card_row(&s, &order, &d, &rule)), (true, true), "undone");
    // An interface's, a data type's and an association class's card underline theirs too.
    let payable = run(&mut s, "uml.interface.add", json!({ "package": pkg, "name": "Payable" }));
    let money = run(&mut s, "uml.data-type.add", json!({ "package": pkg, "name": "Money" }));
    let ac = run(&mut s, "uml.association-class.add", json!({ "from": order, "to": item, "name": "Review" }));
    for owner in [&payable, &money, &ac] {
        let a = run(&mut s, "uml.attribute.add", json!({ "class": owner, "name": "zero", "type": "Integer", "static": true }));
        let op = run(&mut s, "uml.operation.add", json!({ "class": owner, "name": "of", "static": true }));
        assert!(card_row(&s, owner, &d, &a).underlined, "{owner}: static attribute");
        assert!(card_row(&s, owner, &d, &op).underlined, "{owner}: static operation");
    }
    // The literals of an enumeration are never underlined.
    let status_card = archforge_uml::figures::card(s.doc(), obj(&s, &status), obj(&s, &d)).unwrap();
    assert!(status_card.compartments.iter().flatten().all(|r| !r.underlined));
    // Saved and opened again, every row is marked as before.
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    let reopened = |owner: &str| {
        let o = back.by_uuid(Uuid::parse(owner).unwrap()).unwrap();
        let card = archforge_uml::figures::card(&back, o, back.by_uuid(Uuid::parse(&d).unwrap()).unwrap()).unwrap();
        card.compartments.into_iter().flatten().map(|r| (r.text, r.slanted, r.underlined)).collect::<Vec<_>>()
    };
    let now = |owner: &str| {
        let card = archforge_uml::figures::card(s.doc(), obj(&s, owner), obj(&s, &d)).unwrap();
        card.compartments.into_iter().flatten().map(|r| (r.text, r.slanted, r.underlined)).collect::<Vec<_>>()
    };
    for owner in [order.as_str(), payable.as_str(), money.as_str(), ac.as_str()] {
        assert_eq!(reopened(owner), now(owner), "{owner} reopened");
    }
    assert!(reopened(&order).iter().any(|(_, slanted, underlined)| *slanted && *underlined), "the static abstract operation reopened");
}

/// Ordered and nonunique are kept on a property that holds one value, and said in braces only once
/// it holds more; read only, subsets and redefines are said whatever it holds.
#[test]
fn ordered_and_nonunique_are_said_only_for_a_multi_valued_property() {
    let mut s = session();
    let (_, _, _, order, _) = shop(&mut s);
    let a = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "code", "type": "String", "ordered": true, "unique": false }));
    assert_eq!(row(&s, &a), "+ code : String", "kept, unsaid");
    let got = read(&mut s, &order, &a);
    assert_eq!((got["ordered"].as_bool(), got["unique"].as_bool()), (Some(true), Some(false)));
    run(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": a, "name": "code", "type": "String", "multiplicity": "0..1", "ordered": true, "unique": false, "read_only": true }),
    );
    assert_eq!(row(&s, &a), "+ code : String [0..1] {readOnly}");
    run(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": a, "name": "code", "type": "String", "multiplicity": "*", "ordered": true, "unique": false, "read_only": false }),
    );
    assert_eq!(row(&s, &a), "+ code : String [*] {ordered, nonunique}");
}

/// A field `uml.attribute.set` and `uml.association.set-end` are not given keeps what the property
/// has: an agent renaming an attribute with its name and type alone keeps its default, Derived,
/// Subsets, multiplicity, visibility and static; an end's role set alone keeps its static and
/// modifiers. Clearing is said explicitly.
#[test]
fn a_rename_without_modifier_fields_keeps_them() {
    let mut s = session();
    let (pkg, _, item, order, _) = shop(&mut s);
    let parts = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "parts", "type": "Real", "multiplicity": "*" }));
    let total = run(
        &mut s,
        "uml.attribute.add",
        json!({ "class": order, "name": "total", "type": "Real", "multiplicity": "0..*", "visibility": "private", "static": true, "default": "0", "derived": true, "subsets": [parts] }),
    );
    assert_eq!(row(&s, &total), "- / total : Real [*] = 0 {subsets parts}");
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": total, "name": "sum", "type": "Real" }));
    assert_eq!(row(&s, &total), "- / sum : Real [*] = 0 {subsets parts}", "renamed, everything else kept");
    let got = read(&mut s, &order, &total);
    assert_eq!(got["static"], json!(true));
    // Explicit clears.
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": total, "default": "", "subsets": [], "derived": false }));
    assert_eq!(row(&s, &total), "- sum : Real [*]");
    // An end's role alone keeps its static and its modifiers.
    let product = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Product" }));
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": product }));
    run(
        &mut s,
        "uml.association.set-end",
        json!({ "association": a, "end": 1, "role": "items", "multiplicity": "*", "static": true, "default": "none", "ordered": true }),
    );
    round_trip(&mut s, "uml.association.set-end", json!({ "association": a, "end": 1, "role": "goods" }));
    let end = archforge_uml::figures::member_ends(s.doc(), obj(&s, &a)).unwrap()[1];
    assert_eq!(archforge_uml::figures::end_text(s.doc(), end), "* = none {ordered}");
    assert_eq!(archforge_core::name_of(s.doc(), end), "goods");
    assert!(s.doc().get(end, archforge_uml::feature::IS_STATIC).unwrap().as_bool().unwrap(), "static kept");
    // The read's {id, name} objects go back into a set as they are.
    run(&mut s, "uml.attribute.set", json!({ "attribute": total, "subsets": [parts] }));
    let back = read(&mut s, &order, &total)["subsets"].clone();
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": total, "name": "sum2", "subsets": back }));
    assert_eq!(row(&s, &total), "- sum2 : Real [*] {subsets parts}");
}

/// A default reads by its type: every built-in type's form, an enumeration's literal (kept as an
/// InstanceValue that refers to it, so it follows a rename), and opaque text for any other type or
/// none; one that does not read is refused naming the value and the accepted form, changing
/// nothing.
#[test]
fn a_default_reads_by_its_type_and_one_that_does_not_is_refused_naming_value_and_form() {
    let mut s = session();
    let (_, _, item, order, status) = shop(&mut s);
    let add = |s: &mut S, name: &str, ty: &str, default: &str| {
        let a = round_trip(s, "uml.attribute.add", json!({ "class": order, "name": name, "type": ty, "default": default }));
        row(s, &a)
    };
    assert_eq!(add(&mut s, "count", "Integer", " -7 "), "+ count : Integer = -7");
    assert_eq!(add(&mut s, "rate", "Real", "2.5e1"), "+ rate : Real = 25");
    assert_eq!(add(&mut s, "paid", "Boolean", "TRUE"), "+ paid : Boolean = true");
    assert_eq!(add(&mut s, "upto", "UnlimitedNatural", "*"), "+ upto : UnlimitedNatural = *");
    assert_eq!(add(&mut s, "few", "UnlimitedNatural", "3"), "+ few : UnlimitedNatural = 3");
    assert_eq!(add(&mut s, "note", "String", "to do"), "+ note : String = \"to do\"");
    assert_eq!(add(&mut s, "status", &status, "placed"), "+ status : Status = Placed");
    assert_eq!(add(&mut s, "owner", &item, "Item.first()"), "+ owner : Item = Item.first()");
    assert_eq!(add(&mut s, "any", "", "anything"), "+ any = anything");
    // The literal is referred to: renamed, the default follows.
    let placed = s.doc().list(obj(&s, &status), archforge_uml::enumeration::OWNED_LITERAL).unwrap()[0].as_ref().unwrap();
    let placed = s.doc().uuid(placed).unwrap().to_string();
    run(&mut s, "uml.literal.rename", json!({ "literal": placed, "name": "Open" }));
    let attrs = s.execute("uml.class.get", &json!({ "class": order })).unwrap().value["attributes"].clone();
    let st = attrs.as_array().unwrap().iter().find(|a| a["name"] == "status").unwrap().clone();
    assert_eq!(st["default"], "Open");
    let v = archforge_uml::property::DEFAULT_VALUE;
    let a = obj(&s, st["id"].as_str().unwrap());
    let value = s.doc().get(a, v).unwrap().as_ref().unwrap();
    assert_eq!(s.doc().class_of(value).unwrap(), archforge_uml::INSTANCE_VALUE);
    // Refusals name the value and the form.
    for (ty, typed, form) in [
        ("Integer", "1.5", "a whole number"),
        ("Integer", "99999999999999999999", "a whole number"),
        ("Real", "inf", "a number"),
        ("Real", "1e999", "a number"),
        ("Boolean", "yes", "true or false"),
        ("UnlimitedNatural", "-1", "0 or more, or *"),
        ("UnlimitedNatural", "many", "0 or more, or *"),
        (status.as_str(), "Shipped", "one of its literals: Open, Paid"),
    ] {
        refused(&mut s, "uml.attribute.add", json!({ "class": order, "name": "x", "type": ty, "default": typed }), "default", &[typed, form]);
    }
    let long = "x".repeat(archforge_uml::modifiers::MAX_DEFAULT + 1);
    refused(&mut s, "uml.attribute.add", json!({ "class": order, "name": "x", "type": "String", "default": long }), "default", &["at most 1000"]);
}

/// A type change its default does not fit is refused under `type`, naming the default and the
/// form, and nothing changes: the default is never cleared for it. A type it fits takes it, read
/// again by the new type.
#[test]
fn a_type_change_the_default_does_not_fit_is_refused_naming_the_default() {
    let mut s = session();
    let (_, _, _, order, status) = shop(&mut s);
    let a = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "size", "type": "Integer", "default": "3" }));
    refused(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": a, "name": "size", "type": "Boolean", "default": "3" }),
        "type",
        &["\"3\"", "Boolean", "true or false", "before changing the type"],
    );
    refused(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": a, "name": "size", "type": status, "default": "3" }),
        "type",
        &["\"3\"", "Status", "Placed, Paid"],
    );
    // Left out, the kept default is read by the new type too, and refuses it the same way.
    refused(&mut s, "uml.attribute.set", json!({ "attribute": a, "type": "Boolean" }), "type", &["\"3\"", "Boolean"]);
    assert_eq!(row(&s, &a), "+ size : Integer = 3", "kept");
    // Real takes 3, as a Real.
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": a, "name": "size", "type": "Real", "default": "3" }));
    let value = s.doc().get(obj(&s, &a), archforge_uml::property::DEFAULT_VALUE).unwrap().as_ref().unwrap();
    assert_eq!(s.doc().class_of(value).unwrap(), archforge_uml::LITERAL_REAL);
    // A new default that does not read under the same type is the default's refusal.
    refused(&mut s, "uml.attribute.set", json!({ "attribute": a, "name": "size", "type": "Real", "default": "big" }), "default", &["\"big\""]);
}

/// A literal that goes, or its enumeration, takes the defaults that are it along, as the
/// property's type goes; the property stays.
#[test]
fn a_literal_that_goes_takes_its_defaults_along() {
    let mut s = session();
    let (_, _, _, order, status) = shop(&mut s);
    let a = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status", "type": status, "default": "Paid" }));
    let paid = s.doc().list(obj(&s, &status), archforge_uml::enumeration::OWNED_LITERAL).unwrap()[1].as_ref().unwrap();
    let paid = s.doc().uuid(paid).unwrap().to_string();
    round_trip(&mut s, "element.remove", json!({ "ids": [paid] }));
    assert_eq!(row(&s, &a), "+ status : Status");
    run(&mut s, "uml.attribute.set", json!({ "attribute": a, "name": "status", "type": status, "default": "Placed" }));
    round_trip(&mut s, "element.remove", json!({ "ids": [status] }));
    assert_eq!(row(&s, &a), "+ status");
}

/// Subsets takes a property of the attribute's own class or of one its class specializes, named
/// otherwise, of a type its type conforms to, allowing as many values or more; Redefines one of a
/// class its class specializes, consistent with it. Each refusal names the target and why; the
/// candidates are exactly the valid ones.
#[test]
fn subsets_and_redefines_on_attributes_take_only_valid_targets() {
    let mut s = session();
    let (pkg, _, item, order, _) = shop(&mut s);
    let other = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Other" }));
    let parts = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "parts", "type": item, "multiplicity": "*" }));
    let label = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "label", "type": "String", "multiplicity": "0..2" }));
    let one = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "one", "type": order }));
    let all = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "all", "type": item, "multiplicity": "*" }));
    let elsewhere = run(&mut s, "uml.attribute.add", json!({ "class": other, "name": "away", "type": item, "multiplicity": "*" }));
    let lines = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": order, "multiplicity": "0..*" }));
    let base = |extra: Json| {
        let mut p = json!({ "attribute": lines, "name": "lines", "type": order, "multiplicity": "0..*" });
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        p
    };
    // The candidates: Order's all and Item's parts for Subsets; Item's parts for Redefines.
    let st = s.doc();
    let shape = archforge_uml::modifiers::Shape::of(st, obj(&s, &lines));
    let names = |v: Vec<Obj>| v.into_iter().map(|o| archforge_core::name_of(st, o)).collect::<Vec<_>>();
    assert_eq!(names(archforge_uml::modifiers::subsets_candidates(st, &shape)), ["all", "parts"]);
    assert_eq!(names(archforge_uml::modifiers::redefines_candidates(st, &shape)), ["parts"]);
    round_trip(&mut s, "uml.attribute.set", base(json!({ "subsets": [all, parts] })));
    assert_eq!(row(&s, &lines), "+ lines : Order [*] {subsets all, subsets parts}");
    round_trip(&mut s, "uml.attribute.set", base(json!({ "subsets": [], "redefines": [parts] })));
    assert_eq!(row(&s, &lines), "+ lines : Order [*] {redefines parts}");
    let got = read(&mut s, &order, &lines);
    assert_eq!(got["redefines"][0]["id"], json!(parts));
    round_trip(&mut s, "uml.attribute.set", base(json!({ "subsets": [], "redefines": [] })));
    assert_eq!(row(&s, &lines), "+ lines : Order [*]");
    // Refusals name the target and why.
    for (key, target, why) in [
        ("subsets", &elsewhere, "is not a property of this property's classifier"),
        ("subsets", &label, "type this property's type does not conform to"),
        ("subsets", &one, "allows fewer values"),
        ("subsets", &lines, "is the property itself"),
        ("redefines", &all, "is not a property of a classifier this property's classifier specializes"),
        ("redefines", &one, "multiplicity"),
    ] {
        refused(&mut s, "uml.attribute.set", base(json!({ key: [target] })), key, &[target.as_str(), why]);
    }
    // Same name, whatever the case.
    refused(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": lines, "name": "PARTS", "type": order, "multiplicity": "*", "subsets": [parts] }),
        "subsets",
        &[parts.as_str(), "this property's name"],
    );
    // Not a property, no element, twice, too many.
    refused(&mut s, "uml.attribute.set", base(json!({ "subsets": [order] })), "subsets", &[order.as_str(), "is not a property"]);
    refused(&mut s, "uml.attribute.set", base(json!({ "subsets": ["nope"] })), "subsets", &["nope"]);
    refused(&mut s, "uml.attribute.set", base(json!({ "subsets": [all, all] })), "subsets", &["twice"]);
    refused(&mut s, "uml.attribute.set", base(json!({ "subsets": [7] })), "subsets", &["7"]);
    let many: Vec<String> = (0..33).map(|_| all.clone()).collect();
    refused(&mut s, "uml.attribute.set", base(json!({ "subsets": many })), "subsets", &["at most 32"]);
    // A kept target is checked against the new shape: a multiplicity that breaks it is refused,
    // whether the list is resent or left out, naming the target, why and what it may name.
    run(&mut s, "uml.attribute.set", json!({ "attribute": lines, "name": "lines", "type": "String", "multiplicity": "0..1", "subsets": [label] }));
    for p in [json!({ "attribute": lines, "multiplicity": "*", "subsets": [label] }), json!({ "attribute": lines, "multiplicity": "*" })] {
        let e = refused(&mut s, "uml.attribute.set", p, "subsets", &[label.as_str(), "allows fewer values", "it may subset: "]);
        assert!(!e.contains("label (Item)"), "label is not among the accepted: {e}");
    }
    // A target that went stale through a change elsewhere (label now holds none) never blocks an
    // unrelated edit, resent or left out; added anew it is refused.
    run(&mut s, "uml.attribute.set", json!({ "attribute": label, "multiplicity": "0" }));
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": lines, "name": "rows" }));
    round_trip(&mut s, "uml.attribute.set", json!({ "attribute": lines, "subsets": [label], "read_only": true }));
    run(&mut s, "uml.attribute.set", json!({ "attribute": lines, "subsets": [] }));
    refused(&mut s, "uml.attribute.set", json!({ "attribute": lines, "subsets": [label] }), "subsets", &[label.as_str(), "allows fewer values"]);
    // Subsets never run round a cycle, however long.
    let more = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "more", "type": item, "multiplicity": "*", "subsets": [all] }));
    refused(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": all, "subsets": [more] }),
        "subsets",
        &[more.as_str(), "subsets this property already"],
    );
    let most = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "most", "type": item, "multiplicity": "*", "subsets": [more] }));
    refused(&mut s, "uml.attribute.set", json!({ "attribute": all, "subsets": [most] }), "subsets", &[most.as_str(), "through others"]);
    // An object holds id and name alone.
    refused(&mut s, "uml.attribute.set", json!({ "attribute": all, "subsets": [{ "id": parts, "kind": "x" }] }), "subsets", &["kind", "id and name"]);
}

/// A type change that breaks a kept Subsets target is refused, naming it; so is an aggregation
/// change that breaks a kept Redefines target on an end.
#[test]
fn a_type_or_aggregation_change_that_breaks_a_kept_target_is_refused() {
    let mut s = session();
    let (pkg, _, item, order, _) = shop(&mut s);
    let parts = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "parts", "type": item, "multiplicity": "*" }));
    let lines = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": order, "multiplicity": "*", "subsets": [parts] }));
    refused(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": lines, "type": "String" }),
        "subsets",
        &[parts.as_str(), "does not conform", "it may subset: "],
    );
    assert_eq!(row(&s, &lines), "+ lines : Order [*] {subsets parts}");
    // An end redefining a composite end keeps being composite.
    let product = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Product" }));
    let held = run(&mut s, "uml.association.add", json!({ "from": item, "to": product, "kind": "composition" }));
    run(&mut s, "uml.association.set-end", json!({ "association": held, "end": 0, "multiplicity": "1", "navigable": false }));
    run(&mut s, "uml.association.set-end", json!({ "association": held, "end": 1, "role": "products", "multiplicity": "1..*" }));
    let products = s.doc().uuid(archforge_uml::figures::member_ends(s.doc(), obj(&s, &held)).unwrap()[1]).unwrap().to_string();
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": product, "kind": "composition" }));
    run(&mut s, "uml.association.set-end", json!({ "association": a, "end": 0, "multiplicity": "1", "navigable": false }));
    run(&mut s, "uml.association.set-end", json!({ "association": a, "end": 1, "role": "goods", "multiplicity": "1..*", "redefines": [products] }));
    refused(
        &mut s,
        "uml.association.set-end",
        json!({ "association": a, "end": 1, "aggregation": "none" }),
        "redefines",
        &[products.as_str(), "composite"],
    );
    // And a multiplicity whose lower bound leaves the target's.
    refused(
        &mut s,
        "uml.association.set-end",
        json!({ "association": a, "end": 1, "multiplicity": "*" }),
        "redefines",
        &[products.as_str(), "lie within"],
    );
}

/// A refusal of a target lists the properties the attribute may name instead, at most twelve and
/// how many more.
#[test]
fn a_target_refusal_lists_at_most_twelve_choices_and_how_many_more() {
    let mut s = session();
    let (_, _, item, order, _) = shop(&mut s);
    for i in 0..15 {
        run(&mut s, "uml.attribute.add", json!({ "class": item, "name": format!("p{i:02}"), "type": item, "multiplicity": "*" }));
    }
    let short = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "few", "type": item, "multiplicity": "0..1" }));
    let lines = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": order, "multiplicity": "*" }));
    let e = refused(
        &mut s,
        "uml.attribute.set",
        json!({ "attribute": lines, "subsets": [short] }),
        "subsets",
        &["it may subset: p00 (Item), p01 (Item)", "p11 (Item) and 3 more"],
    );
    assert!(!e.contains("p12 (Item)"), "{e}");
}

/// On an association's end Subsets and Redefines take the properties of the class at the other
/// end: an end of Order's subsets one of Order's attributes; one of Item's, the class Order
/// specializes, redefines a composite end only while it is composite itself. The line says it
/// beside the end, the role derived: `/items 0..* = none {ordered}`.
#[test]
fn subsets_and_redefines_on_association_ends_take_the_other_end_s_properties() {
    let mut s = session();
    let (pkg, _, item, order, _) = shop(&mut s);
    let product = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Product" }));
    let all = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "all", "type": product, "multiplicity": "*" }));
    // Item composes products; Order (an Item) has products too.
    let held = run(&mut s, "uml.association.add", json!({ "from": item, "to": product, "kind": "composition" }));
    run(&mut s, "uml.association.set-end", json!({ "association": held, "end": 0, "multiplicity": "1", "navigable": false }));
    run(
        &mut s,
        "uml.association.set-end",
        json!({ "association": held, "end": 1, "role": "products", "multiplicity": "*", "aggregation": "composite" }),
    );
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": product }));
    let set = |extra: Json| {
        let mut p = json!({ "association": a, "end": 1, "role": "items", "multiplicity": "0..*" });
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        p
    };
    let products = {
        let st = s.doc();
        let e = archforge_uml::figures::member_ends(st, obj(&s, &held)).unwrap()[1];
        st.uuid(e).unwrap().to_string()
    };
    let end = {
        let st = s.doc();
        archforge_uml::figures::member_ends(st, obj(&s, &a)).unwrap()[1]
    };
    let st = s.doc();
    let shape = archforge_uml::modifiers::Shape::of(st, end);
    assert_eq!(shape.context, [obj(&s, &order)], "its context is the class at the other end");
    round_trip(&mut s, "uml.association.set-end", set(json!({ "subsets": [all], "derived": true, "default": "none", "ordered": true })));
    let line = archforge_uml::figures::line(s.doc(), obj(&s, &a)).unwrap();
    assert_eq!((line.ends[1].role.as_deref(), line.ends[1].text.as_deref()), (Some("/items"), Some("* = none {ordered, subsets all}")));
    assert_eq!(archforge_uml::figures::slot_text(s.doc(), obj(&s, &a), "to_end"), "* = none {ordered, subsets all}");
    // Not composite: products is, so it is refused; composite, it is taken.
    refused(&mut s, "uml.association.set-end", set(json!({ "redefines": [products] })), "redefines", &[products.as_str(), "composite"]);
    run(&mut s, "uml.association.set-end", json!({ "association": a, "end": 0, "multiplicity": "1", "navigable": false }));
    round_trip(
        &mut s,
        "uml.association.set-end",
        set(json!({
            "redefines": [products], "aggregation": "composite", "static": true, "read_only": true,
            "subsets": [], "default": "", "derived": false, "ordered": false,
        })),
    );
    let got = s.execute("uml.relationship.list", &json!({ "class": order })).unwrap().value;
    let e = got["relationships"].as_array().unwrap().iter().find(|r| r["id"] == a).unwrap()["ends"][1].clone();
    assert_eq!((e["redefines"][0]["name"].as_str(), e["static"].as_bool(), e["read_only"].as_bool()), (Some("products"), Some(true), Some(true)));
    assert_eq!(archforge_uml::figures::end_text(s.doc(), end), "* {readOnly, redefines products}");
    // Out of reach: an attribute of Product, the end's own type.
    let sku = run(&mut s, "uml.attribute.add", json!({ "class": product, "name": "sku", "type": "String" }));
    refused(&mut s, "uml.association.set-end", set(json!({ "subsets": [sku] })), "subsets", &[sku.as_str(), "not a property"]);
    // Cleared again, one step.
    round_trip(&mut s, "uml.association.set-end", set(json!({ "redefines": [], "read_only": false })));
    assert_eq!(archforge_uml::figures::end_text(s.doc(), end), "*");
}

/// Attributes and ends with every modifier come back from their rows identical, as a saved project
/// opens.
#[test]
fn modifiers_come_back_from_their_rows_identical() {
    let mut s = session();
    let (pkg, _, item, order, status) = shop(&mut s);
    let product = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Product" }));
    let parts = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "parts", "type": item, "multiplicity": "*" }));
    let mut made = Vec::new();
    for (name, ty, default) in [
        ("a", "Integer", "1"),
        ("b", "Real", "0.25"),
        ("c", "Boolean", "false"),
        ("d", "UnlimitedNatural", "*"),
        ("e", "String", "x y"),
        ("f", status.as_str(), "Paid"),
        ("g", product.as_str(), "new Product()"),
    ] {
        made.push(run(&mut s, "uml.attribute.add", json!({ "class": order, "name": name, "type": ty, "default": default, "multiplicity": "*", "derived": true, "read_only": true, "ordered": true, "unique": false })));
    }
    let lines = run(
        &mut s,
        "uml.attribute.add",
        json!({ "class": order, "name": "lines", "type": order, "multiplicity": "*", "subsets": [parts], "redefines": [parts] }),
    );
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": product }));
    run(
        &mut s,
        "uml.association.set-end",
        json!({ "association": a, "end": 1, "role": "items", "multiplicity": "*", "default": "none", "static": true, "unique": false }),
    );
    let (tables, junctions) = tables();
    let saved = archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 20);
    archforge_mof::rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(archforge_mof::rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    for id in made.iter().chain([&lines]) {
        let o = back.by_uuid(Uuid::parse(id).unwrap()).unwrap();
        assert_eq!(archforge_uml::figures::attribute_text(&back, o), row(&s, id), "{id}");
    }
    let end = archforge_uml::figures::member_ends(&back, back.by_uuid(Uuid::parse(&a).unwrap()).unwrap()).unwrap()[1];
    assert_eq!(archforge_uml::figures::end_text(&back, end), "* = none {nonunique}");
}

/// The command sweep, with attributes and ends carrying modifiers, a literal default and the
/// targets among the ids, breaks nothing.
#[test]
fn the_sweep_over_modifiers_breaks_nothing() {
    let mut s = session();
    let (pkg, d, item, order, status) = shop(&mut s);
    let parts = run(&mut s, "uml.attribute.add", json!({ "class": item, "name": "parts", "type": item, "multiplicity": "*" }));
    let st = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "status", "type": status, "default": "Paid", "subsets": [] }));
    let lines = run(
        &mut s,
        "uml.attribute.add",
        json!({ "class": order, "name": "lines", "type": order, "multiplicity": "*", "subsets": [parts], "derived": true }),
    );
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": item }));
    run(
        &mut s,
        "uml.association.set-end",
        json!({ "association": a, "end": 1, "role": "items", "multiplicity": "*", "default": "none", "subsets": [parts] }),
    );
    let ids = [
        pkg.as_str(),
        d.as_str(),
        item.as_str(),
        order.as_str(),
        status.as_str(),
        parts.as_str(),
        st.as_str(),
        lines.as_str(),
        a.as_str(),
        "not-an-id",
    ];
    let calls = sweep_with(&mut s, &ids, |st| format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())).unwrap();
    assert!(calls > 100, "{calls}");
}

/// An end that subsets a property of the class at the other end loses it, in the same step, when
/// that end moves to another class, and the reconnection says so; a role edit afterwards still
/// works (the model never keeps a target that blocks it).
#[test]
fn a_reconnection_drops_what_an_end_no_longer_subsets_and_a_role_edit_still_works() {
    let mut s = session();
    let (pkg, _, item, order, _) = shop(&mut s);
    let customer = run(&mut s, "uml.class.add", json!({ "package": pkg, "name": "Customer" }));
    let lines = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": item, "multiplicity": "*" }));
    let a = run(&mut s, "uml.association.add", json!({ "from": order, "to": item }));
    run(&mut s, "uml.association.set-end", json!({ "association": a, "end": 1, "role": "items", "multiplicity": "*", "subsets": [lines] }));
    let end = archforge_uml::figures::member_ends(s.doc(), obj(&s, &a)).unwrap()[1];
    assert_eq!(archforge_uml::figures::end_text(s.doc(), end), "* {subsets lines}");
    let before = snapshot(&s);
    let out = s.execute("uml.relationship.reconnect", &json!({ "relationship": a, "end": "source", "class": customer })).unwrap();
    let dropped = out.value["dropped"].as_array().cloned().unwrap_or_default();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert_eq!(
        (dropped[0]["list"].as_str(), dropped[0]["name"].as_str(), dropped[0]["id"].as_str()),
        (Some("subsets"), Some("lines"), Some(lines.as_str()))
    );
    assert_eq!(archforge_uml::figures::end_text(s.doc(), end), "*", "dropped in the same step");
    let after = snapshot(&s);
    s.undo().unwrap();
    assert_eq!(snapshot(&s), before, "one undo step brings the line and its subsets back");
    s.redo().unwrap();
    assert_eq!(snapshot(&s), after);
    round_trip(&mut s, "uml.association.set-end", json!({ "association": a, "end": 1, "role": "goods" }));
    // A reconnection that keeps every target drops nothing.
    let out = s.execute("uml.relationship.reconnect", &json!({ "relationship": a, "end": "source", "class": order })).unwrap();
    assert_eq!(out.value["dropped"], json!([]));
}
