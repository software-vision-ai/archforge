//! Structural constraints through the engine (AR-133): added on a classifier or a package, set,
//! moved and removed, each one undo step; the expression and the language kept exactly as typed and
//! never evaluated; the only refusals an empty expression and an element outside the model (and a
//! context that owns no constraint), each changing nothing; deleting a constrained element drops
//! it from the constraint in the same step, and a constraint left naming nothing goes with it;
//! shown as braces on one diagram and as a linked note on another, its content shared; saved as
//! rows and read back identical; the sweep.

use archforge_diagram::ProjectView;
use archforge_diagram::figure::Tone;
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::constraints::{self, Form};
use archforge_uml::{ALONE, CLASS_DIAGRAM, CONSTRAINT, registry};
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

/// A model with a package `sales` holding the classes Order and Payment, Order with an attribute
/// `total`, and a class diagram of `sales` showing both: (sales, order, payment, total, diagram,
/// order's node, payment's node).
fn shop(s: &mut S) -> [String; 7] {
    let sales = run(s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let payment = run(s, "uml.class.add", json!({ "package": sales, "name": "Payment" }));
    let total = run(s, "uml.attribute.add", json!({ "class": order, "name": "total", "type": "Integer" }));
    let d = run(s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Sales", "element": sales }));
    let on = run(s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    let pn = run(s, "diagram.place", json!({ "diagram": d, "element": payment, "x": 400, "y": 0 }));
    [sales, order, payment, total, d, on, pn]
}

/// The constraints of `context`, as `uml.constraint.list` gives them.
fn listed(s: &mut S, context: &str) -> Vec<Json> {
    s.execute("uml.constraint.list", &json!({ "context": context })).unwrap().value["constraints"].as_array().unwrap().clone()
}

/// A constraint is added on a class, set, moved and removed, each one undo step; its expression and
/// language are kept exactly as typed, spaces, line breaks and all, and nothing evaluates them.
#[test]
fn a_constraint_is_added_set_moved_and_removed_each_one_step_its_text_kept_as_typed() {
    let mut s = session();
    let [_, order, payment, total, ..] = shop(&mut s);
    let typed = "  payment.amount = total()\n  when status = Paid ";
    let paid = one_step(
        &mut s,
        "uml.constraint.add",
        json!({ "context": order, "name": "paidInFull", "expression": typed, "language": "OCL", "constrained": [order, payment] }),
    );
    let st = s.doc();
    let c = obj(st, &paid);
    assert_eq!(st.class_of(c).unwrap(), CONSTRAINT);
    assert_eq!(constraints::expression(st, c), typed, "exactly as typed");
    assert_eq!(constraints::language(st, c), "OCL");
    assert_eq!(constraints::context_of(st, c), Some(obj(st, &order)));
    assert_eq!(constraints::constrained(st, c), vec![obj(st, &order), obj(st, &payment)], "in their order");
    // An empty list names the context alone; the language may be any label, or none.
    let non_empty =
        one_step(&mut s, "uml.constraint.add", json!({ "context": order, "name": "nonEmpty", "expression": "lines->notEmpty()", "constrained": [] }));
    let st = s.doc();
    assert_eq!(constraints::constrained(st, obj(st, &non_empty)), vec![obj(st, &order)]);
    assert_eq!(constraints::language(st, obj(st, &non_empty)), "", "plain text");
    let all = listed(&mut s, &order);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0]["name"], "paidInFull");
    assert_eq!(all[0]["expression"], typed);
    assert_eq!(all[0]["evaluated"], false, "a constraint is never evaluated");
    assert_eq!(all[0]["constrained"], json!([{ "id": order, "name": "Order" }, { "id": payment, "name": "Payment" }]));
    // Set: a field left out keeps its value.
    one_step(&mut s, "uml.constraint.set", json!({ "constraint": non_empty, "language": "Englisch, informell", "constrained": [total] }));
    let st = s.doc();
    let ne = obj(st, &non_empty);
    assert_eq!(constraints::expression(st, ne), "lines->notEmpty()", "kept");
    assert_eq!(constraints::language(st, ne), "Englisch, informell", "any label, as typed");
    assert_eq!(constraints::constrained(st, ne), vec![obj(st, &total)], "an attribute may be constrained");
    one_step(&mut s, "uml.constraint.set", json!({ "constraint": non_empty, "expression": "ÿ ≠ ∅", "name": "" }));
    assert_eq!(constraints::expression(s.doc(), ne), "ÿ ≠ ∅");
    assert_eq!(archforge_core::name_of(s.doc(), ne), "");
    // Moved among its context's constraints; -1, the default, is the last place.
    one_step(&mut s, "uml.constraint.move", json!({ "constraint": non_empty, "at": 0 }));
    let st = s.doc();
    assert_eq!(constraints::constraints_of(st, obj(st, &order)), vec![ne, c]);
    one_step(&mut s, "uml.constraint.move", json!({ "constraint": non_empty }));
    let st = s.doc();
    assert_eq!(constraints::constraints_of(st, obj(st, &order)), vec![c, ne], "the default goes to the last place");
    one_step(&mut s, "uml.constraint.move", json!({ "constraint": non_empty, "at": 0 }));
    // Removed through element.remove, with its specification.
    let before = s.doc().alive().count();
    one_step(&mut s, "element.remove", json!({ "ids": [paid] }));
    assert_eq!(s.doc().alive().count(), before - 2, "the constraint and its expression");
    assert_eq!(listed(&mut s, &order).len(), 1);
}

/// The refusals: an empty expression, an element outside the model, a context that owns no
/// constraint; each says why and changes nothing, history included.
#[test]
fn an_empty_expression_and_an_element_outside_the_model_are_refused_changing_nothing() {
    let mut s = session();
    let [sales, order, _, total, d, on, _] = shop(&mut s);
    let c = run(&mut s, "uml.constraint.add", json!({ "context": order, "expression": "x" }));
    let foreign = Uuid::v7(1, [9; 10]).to_string();
    let undo = s.undo_label().map(str::to_string);
    for (cmd, p, key, why) in [
        ("uml.constraint.add", json!({ "context": order, "expression": "" }), "expression", "type an expression"),
        ("uml.constraint.add", json!({ "context": order, "expression": " \n\t" }), "expression", "states no rule"),
        ("uml.constraint.set", json!({ "constraint": c, "expression": "" }), "expression", "type an expression"),
        ("uml.constraint.add", json!({ "context": order, "expression": "x", "constrained": [foreign] }), "constrained", "no element"),
        ("uml.constraint.add", json!({ "context": order, "expression": "x", "constrained": [d] }), "constrained", "outside this model"),
        ("uml.constraint.add", json!({ "context": order, "expression": "x", "constrained": [on] }), "constrained", "outside this model"),
        ("uml.constraint.set", json!({ "constraint": c, "constrained": [order, "nope"] }), "constrained", "not an id"),
        ("uml.constraint.add", json!({ "context": order, "expression": "x", "constrained": [order, order] }), "constrained", "named twice"),
        ("uml.constraint.add", json!({ "context": total, "expression": "x" }), "context", "owns no constraint"),
        ("uml.constraint.add", json!({ "context": d, "expression": "x" }), "context", "owns no constraint"),
        ("uml.constraint.set", json!({ "constraint": order }), "constraint", "not a constraint"),
        ("uml.constraint.move", json!({ "constraint": c, "at": 3 }), "at", "past the last constraint"),
    ] {
        let e = refused(&mut s, cmd, p.clone());
        assert!(e.contains(key) && e.contains(why), "{cmd} {p}: {e}");
    }
    assert_eq!(s.undo_label().map(str::to_string), undo, "the history is untouched");
    // An association class owns constraints, as the class it is.
    let st = s.doc();
    let payment = st
        .alive()
        .find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Payment")
        .map(|o| st.uuid(o).unwrap().to_string())
        .unwrap();
    let line = run(&mut s, "uml.association-class.add", json!({ "from": order, "to": payment, "name": "Settlement" }));
    let on_line = one_step(&mut s, "uml.constraint.add", json!({ "context": line, "expression": "amount > 0", "constrained": [line, order] }));
    let st = s.doc();
    assert_eq!(constraints::context_of(st, obj(st, &on_line)), Some(obj(st, &line)));
    assert_eq!(listed(&mut s, &line).len(), 1);
    // A package owns constraints too; a built-in type does not.
    run(&mut s, "uml.constraint.add", json!({ "context": sales, "expression": "every class is named" }));
    let st = s.doc();
    let integer = archforge_uml::members::types_package(st).map(|t| archforge_uml::commands::primitive_types(st, t)).unwrap_or_default();
    let integer = st.uuid(integer[0]).unwrap().to_string();
    let e = refused(&mut s, "uml.constraint.add", json!({ "context": integer, "expression": "x" }));
    assert!(e.contains("owns no constraint"), "{e}");
}

/// A constraint's own rules, refused, carry their reason typed beside the unchanged English: an
/// expression that states no rule, a context that owns none, an element no constraint names or one
/// named twice, more elements than one names, and braces with nothing to sit by. A refusal about
/// an id keeps only its sentence (AR-447).
#[test]
fn every_constraint_rule_refused_carries_its_reason_typed() {
    use archforge_engine::{EngineError, Reason};
    let mut s = session();
    let [_, order, _, total, d, on, _] = shop(&mut s);
    let c = run(&mut s, "uml.constraint.add", json!({ "context": order, "expression": "x" }));
    let check = |s: &mut S, cmd: &str, p: Json, reason: Option<Reason>, english: &str| {
        let before = picture(s.doc());
        let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
        assert_eq!(error.reason, reason, "{cmd} {p}: {}", error.message);
        assert!(error.message.contains(english), "{cmd} {p}: {}", error.message);
        assert_eq!(picture(s.doc()), before, "{cmd} {p} changed nothing");
    };
    let none = "type an expression; a constraint without one states no rule";
    check(&mut s, "uml.constraint.add", json!({ "context": order, "expression": " \n" }), Some(Reason::NoExpression), none);
    check(&mut s, "uml.constraint.set", json!({ "constraint": c, "expression": "" }), Some(Reason::NoExpression), none);
    let owns = "owns no constraint; a class, an interface";
    check(&mut s, "uml.constraint.add", json!({ "context": total, "expression": "x" }), Some(Reason::NotAContext), owns);
    // The elements a constraint names, when it is made and when it is changed.
    let made = |constrained: Json| json!({ "context": order, "expression": "x", "constrained": constrained });
    let changed = |constrained: Json| json!({ "constraint": c, "constrained": constrained });
    let outside = "is outside this model; a constraint names a package";
    check(&mut s, "uml.constraint.add", made(json!([order, d])), Some(Reason::NotConstrainable), outside);
    check(&mut s, "uml.constraint.set", changed(json!([on])), Some(Reason::NotConstrainable), outside);
    let twice = || Some(Reason::NamedTwice { kind: "constrained element", name: total.clone() });
    check(&mut s, "uml.constraint.add", made(json!([order, total, total])), twice(), "is named twice; name each element once");
    // The most a constraint names is refused one past it; at the most, the list is read on.
    let most = Some(Reason::TooMany { kind: "constrained element", max: 256 });
    check(&mut s, "uml.constraint.set", changed(json!(vec![&total; 257])), most, "a constraint names at most 256 elements; 257 were given");
    check(&mut s, "uml.constraint.set", changed(json!(vec![&total; 256])), twice(), "is named twice");
    check(&mut s, "uml.constraint.set", changed(json!([order, "nope"])), None, "is not an id");
    check(&mut s, "uml.constraint.set", json!({ "constraint": order, "name": "n" }), None, "is not a constraint of this model");
    // Braces need an element of theirs on the diagram.
    run(&mut s, "diagram.unplace", json!({ "nodes": [on] }));
    let braces = json!({ "diagram": d, "constraint": c, "as": "braces" });
    check(&mut s, "uml.constraint.show", braces, Some(Reason::BracesNoElement), "braces sit by an element they constrain");
}

/// Deleting a constrained element drops it from the constraint in the same undo step; a constraint
/// left naming nothing goes with it; undo brings both back. `dropped_by` says beforehand what
/// changes, for the toast. Removing an element from a diagram changes nothing in the model.
#[test]
fn deleting_a_constrained_element_drops_the_reference_and_an_orphan_constraint_in_one_step() {
    let mut s = session();
    let [_, order, payment, total, d, _, pn] = shop(&mut s);
    let paid =
        run(&mut s, "uml.constraint.add", json!({ "context": order, "name": "paidInFull", "expression": "a", "constrained": [order, payment] }));
    let positive =
        run(&mut s, "uml.constraint.add", json!({ "context": order, "name": "positive", "expression": "total > 0", "constrained": [total] }));
    run(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": paid, "as": "note" }));
    // From the diagram only: the model keeps it all.
    run(&mut s, "diagram.unplace", json!({ "nodes": [pn] }));
    assert_eq!(constraints::constrained(s.doc(), obj(s.doc(), &paid)).len(), 2);
    let st = s.doc();
    let (p, t) = (obj(st, &payment), obj(st, &total));
    let said = constraints::dropped_by(st, &[p, t]);
    assert_eq!(said.len(), 2);
    assert_eq!(said[0].constraint, obj(st, &paid));
    assert_eq!(said[0].remaining, vec![obj(st, &order)], "paidInFull now constrains Order only");
    assert!(said[1].remaining.is_empty(), "positive goes");
    let before = picture(s.doc());
    run(&mut s, "element.remove", json!({ "ids": [payment, total] }));
    let st = s.doc();
    assert_eq!(constraints::constrained(st, obj(st, &paid)), vec![obj(st, &order)], "the reference dropped, nothing dangling");
    assert!(st.by_uuid(Uuid::parse(&positive).unwrap()).is_none_or(|o| !st.is_alive(o)), "the orphan went with it");
    assert!(constraints::shown_on(st, obj(st, &d), obj(st, &paid)).is_some(), "the note stays");
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step brings both back");
    // Deleting the context takes its constraints with it; a constraint on its class's attribute
    // whose context is another class keeps the class.
    let other = run(&mut s, "uml.constraint.add", json!({ "context": payment, "expression": "b", "constrained": [order] }));
    let st = s.doc();
    assert!(constraints::dropped_by(st, &[obj(st, &order)]).iter().all(|c| c.constraint == obj(st, &other) && c.remaining.is_empty()));
    run(&mut s, "element.remove", json!({ "ids": [order] }));
    let st = s.doc();
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(CONSTRAINT)), "every constraint named only Order, or was Order's own");
}

/// A nested package removed hands up what it holds but takes its own constraints along: they are
/// not counted as changed, whatever they name, and nothing else is touched.
#[test]
fn a_removed_nested_package_takes_its_own_constraints_and_reports_none_as_changed() {
    let mut s = session();
    let [sales, order, payment, ..] = shop(&mut s);
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let held = run(&mut s, "uml.class.add", json!({ "package": inner, "name": "Refund" }));
    let own = run(&mut s, "uml.constraint.add", json!({ "context": inner, "expression": "refunds are rare", "constrained": [inner, order] }));
    let kept = run(&mut s, "uml.constraint.add", json!({ "context": order, "expression": "x", "constrained": [order, inner] }));
    let lone = run(&mut s, "uml.constraint.add", json!({ "context": payment, "expression": "y", "constrained": [inner] }));
    let st = s.doc();
    let said = constraints::dropped_by(st, &[obj(st, &inner)]);
    let changed: Vec<Obj> = said.iter().map(|d| d.constraint).collect();
    assert!(!changed.contains(&obj(st, &own)), "the package's own constraint goes with it, not reported: {said:?}");
    assert_eq!(changed, vec![obj(st, &kept), obj(st, &lone)]);
    assert_eq!(said[0].remaining, vec![obj(st, &order)]);
    assert!(said[1].remaining.is_empty());
    let before = picture(s.doc());
    run(&mut s, "element.remove", json!({ "ids": [inner] }));
    let st = s.doc();
    let alive = |id: &str| st.by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o));
    assert!(!alive(&own) && !alive(&lone), "its own constraint and the one naming it alone go");
    assert!(alive(&kept) && alive(&held), "the other stays, and what it held moved up");
    assert_eq!(constraints::constrained(st, obj(st, &kept)), vec![obj(st, &order)]);
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one undo step");
}

/// Each diagram shows a constraint its own way, braces or a note, while its content is the
/// model's: an edit shows on both. The braces nest in their element's node and read `{expression}`;
/// the note reads its name and expression and links to its elements' nodes.
#[test]
fn braces_on_one_diagram_and_a_note_on_another_share_the_content() {
    let mut s = session();
    let [sales, order, payment, _, d, on, pn] = shop(&mut s);
    let d2 = run(&mut s, "diagram.add", json!({ "kind": CLASS_DIAGRAM, "name": "Overview", "element": sales }));
    let on2 = run(&mut s, "diagram.place", json!({ "diagram": d2, "element": order, "x": 0, "y": 0 }));
    let pn2 = run(&mut s, "diagram.place", json!({ "diagram": d2, "element": payment, "x": 400, "y": 300 }));
    let c = run(&mut s, "uml.constraint.add", json!({ "context": order, "name": "nonEmpty", "expression": "lines->notEmpty()" }));
    let st = s.doc();
    assert_eq!(s.view().selection, vec![obj(st, &c)], "the new constraint is selected");
    assert_eq!(constraints::suggested_form(st, obj(st, &c)), Form::Braces, "short, one line, one element");
    let braces = one_step(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": c, "as": "braces" }));
    let note = one_step(&mut s, "uml.constraint.show", json!({ "diagram": d2, "constraint": c, "as": "note" }));
    let st = s.doc();
    let (b, n) = (obj(st, &braces), obj(st, &note));
    assert_eq!(st.container(b).unwrap().map(|(o, _)| o), Some(obj(st, &on)), "the braces sit in Order's node, moving with it");
    assert_eq!(st.get(b, archforge_diagram::node::KIND).unwrap().as_str(), Some(constraints::BRACES));
    let card = |s: &S, d: &str| archforge_diagram::Notation::card(&archforge_uml::UmlNotation, s.doc(), obj(s.doc(), &c), obj(s.doc(), d)).unwrap();
    let (on_d, on_d2) = (card(&s, &d), card(&s, &d2));
    assert_eq!((on_d.tone, on_d2.tone), (Tone::Braces, Tone::Constraint));
    assert_eq!(on_d.compartments[0][0].text, "{lines->notEmpty()}");
    assert_eq!(on_d2.name, "nonEmpty");
    assert_eq!(on_d2.compartments[0][0].text, "{ lines->notEmpty() }");
    let links = |s: &S, d: &str| archforge_diagram::Notation::links(&archforge_uml::UmlNotation, s.doc(), obj(s.doc(), &c), obj(s.doc(), d));
    assert!(links(&s, &d).is_empty(), "braces have no links");
    assert_eq!(links(&s, &d2), vec![obj(s.doc(), &on2)]);
    // The content is shared: one edit shows on both diagrams, and the braces fit their new text.
    run(&mut s, "uml.constraint.set", json!({ "constraint": c, "expression": "lines->size() > 0", "constrained": [order, payment] }));
    assert_eq!(card(&s, &d).compartments[0][0].text, "{lines->size() > 0}");
    assert_eq!(card(&s, &d2).compartments[0][0].text, "{ lines->size() > 0 }");
    assert_eq!(links(&s, &d2), vec![obj(s.doc(), &on2), obj(s.doc(), &pn2)], "linked to every element shown");
    let width = archforge_diagram::grid::bounds_of(s.doc(), obj(s.doc(), &braces)).width;
    assert!(width > 100.0, "{width}");
    // Switching the form on one diagram leaves the other as it was, one undo step.
    let again = one_step(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": c, "as": "note", "x": 700, "y": 40 }));
    let st = s.doc();
    assert!(!st.is_alive(b), "the braces gave way");
    assert_eq!(constraints::shown_on(st, obj(st, &d), obj(st, &c)), Some((obj(st, &again), Form::Note)));
    assert_eq!(constraints::shown_on(st, obj(st, &d2), obj(st, &c)), Some((n, Form::Note)));
    assert_eq!(links(&s, &d), vec![obj(s.doc(), &on), obj(s.doc(), &pn)]);
    // Braces need an element of theirs on the diagram.
    run(&mut s, "diagram.unplace", json!({ "nodes": [on, pn] }));
    let e = refused(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": c, "as": "braces" }));
    assert!(e.contains("braces sit by an element"), "{e}");
}

/// A constraint, its expression, language, elements and how each diagram shows it, saved as rows,
/// read back identical.
#[test]
fn constraints_are_saved_and_read_back_identical() {
    let mut s = session();
    let [_, order, payment, total, d, ..] = shop(&mut s);
    let c = run(
        &mut s,
        "uml.constraint.add",
        json!({ "context": order, "name": "paidInFull", "expression": "a\n  b ", "language": "OCL", "constrained": [payment, order, total] }),
    );
    let short = run(&mut s, "uml.constraint.add", json!({ "context": order, "expression": "x > 0" }));
    run(&mut s, "uml.constraint.move", json!({ "constraint": short, "at": 0 }));
    run(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": c, "as": "note" }));
    run(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": short, "as": "braces" }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let mut back = session_over(back);
    assert_eq!(listed(&mut back, &order), listed(&mut s, &order), "the same constraints, in their order, shown the same way");
    let st = back.doc();
    let ids = |os: Vec<Obj>| os.into_iter().map(|o| st.uuid(o).unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(ids(constraints::constrained(st, obj(st, &c))), [payment.clone(), order.clone(), total.clone()]);
    assert_eq!(constraints::expression(st, obj(st, &c)), "a\n  b ");
}

/// The sweep over the constraint commands breaks nothing.
#[test]
fn the_sweep_over_constraints_breaks_nothing() {
    let mut s = session();
    let [_, order, payment, _, d, ..] = shop(&mut s);
    let c = run(&mut s, "uml.constraint.add", json!({ "context": order, "expression": "x", "constrained": [order, payment] }));
    run(&mut s, "uml.constraint.show", json!({ "diagram": d, "constraint": c }));
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.constraint.add", "uml.constraint.set", "uml.constraint.move", "uml.constraint.show", "uml.constraint.list"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [order.as_str(), payment.as_str(), c.as_str(), d.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 100, "{calls}");
}
