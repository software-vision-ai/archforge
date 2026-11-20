//! Packages through the engine: nested as deep as needed, names told apart among
//! siblings, classes and packages moved, a nested package removed handing its contents and
//! diagrams up to its parent (owner decision, for every notation), a top-level one taking them
//! along, either taking its default-diagram bindings along (AR-577), and a class of another package
//! drawn as an outsider; a package's card naming its owners from the root down (AR-564); a package
//! a dependency that stays starts at or depends on kept (AR-696); each one undo step; the sweep.

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DEFAULT_DIAGRAM_BINDING, DIAGRAM, DiagramObj, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{End, Options, Session};
use archforge_mof::{CompositeSlabs, Obj, PropId, Store, Uuid, Value};
use archforge_uml::defaults::problems;
use archforge_uml::{ALONE, PACKAGE, STRUCTURE_DIAGRAM, registry};
use serde_json::{Value as Json, json};
type S = Session<Store, ProjectView>;
/// A session over a store holding only the project's Model.
fn session() -> S {
    let mut st = store(0);
    st.create(archforge_core::MODEL).unwrap();
    over(st)
}
/// An empty store of the core, the diagrams and UML, its new ids counted on from `n`.
fn store(mut n: u64) -> Store {
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
/// A session over `st`.
fn over(st: Store) -> S {
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}
fn round_trip(s: &mut S, cmd: &str, p: Json) {
    let before = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    let after = format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap());
    s.undo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), before, "undo of {cmd}");
    s.redo().unwrap();
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap()), after, "redo of {cmd}");
}
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

fn snapshot(s: &S) -> String {
    format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables().0, &tables().1).unwrap())
}

fn refused(s: &mut S, cmd: &str, p: Json, words: &str) {
    let before = snapshot(s);
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} is refused"));
    assert!(e.to_string().contains(words), "{cmd} {p}: {e}");
    assert_eq!(snapshot(s), before, "{cmd} {p} changed nothing");
}

fn obj(s: &S, id: &str) -> archforge_mof::Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn alive(s: &S, id: &str) -> bool {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).is_some_and(|o| s.doc().is_alive(o))
}

/// The names of the classes a package holds.
fn class_names(s: &S, pkg: &str) -> Vec<String> {
    archforge_uml::commands::classes(s.doc(), obj(s, pkg)).into_iter().map(|c| archforge_core::name_of(s.doc(), c)).collect()
}

fn parent_of(s: &S, id: &str) -> Option<archforge_mof::Obj> {
    archforge_uml::commands::package_of(s.doc(), obj(s, id))
}

/// Packages nest as deep as needed, a name told apart among siblings whatever the case; the list
/// names each package's parent and its owners from the root down (AR-562).
#[test]
fn packages_nest_their_names_are_told_apart_among_siblings_and_the_list_names_each_parent_and_path() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    round_trip(&mut s, "uml.package.add", json!({ "name": "orders", "parent": sales }));
    // The model's root package comes first; sales and its orders follow.
    let orders = s.execute("uml.package.list", &json!({})).unwrap().value["packages"][2]["id"].as_str().unwrap().to_string();
    let deep = run(&mut s, "uml.package.add", json!({ "name": "lines", "parent": orders }));
    // The same name in another parent is fine; twice in one parent, whatever the case, is not.
    run(&mut s, "uml.package.add", json!({ "name": "orders" }));
    refused(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }), "named orders already");
    refused(&mut s, "uml.package.add", json!({ "name": "x", "parent": "not-an-id" }), "parent");
    let listed = s.execute("uml.package.list", &json!({})).unwrap().value["packages"].as_array().unwrap().clone();
    let names: Vec<(&str, Option<&str>)> = listed.iter().map(|p| (p["name"].as_str().unwrap(), p["parent"].as_str())).collect();
    let root = listed[0]["id"].as_str().unwrap();
    assert_eq!(
        names,
        [("Model", None), ("sales", Some(root)), ("orders", Some(sales.as_str())), ("lines", Some(orders.as_str())), ("orders", Some(root))],
        "a package with no parent goes into the root, which this unnamed model calls Model"
    );
    // Each names its owners from the root down, as its card does; the root names none (AR-562).
    let paths: Vec<Vec<&str>> = listed.iter().map(|p| p["path"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect()).collect();
    assert_eq!(paths, [vec![], vec!["Model"], vec!["Model", "sales"], vec!["Model", "sales", "orders"], vec!["Model"]]);
    assert_eq!(parent_of(&s, &deep), Some(obj(&s, &orders)));
}

/// A name clash, a package into itself and a self-dependency carry their reason typed, with the
/// kind and the name of the element that has the name, for an interface to say in its own words;
/// the English sentence stays as the command line shows it (AR-429).
#[test]
fn a_clash_a_package_into_itself_and_a_self_dependency_carry_their_reason_typed() {
    use archforge_engine::{EngineError, Reason};
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let mut why = |cmd: &str, p: Json| match s.execute(cmd, &p) {
        Err(EngineError::Failed { error, .. }) => (error.reason, error.message),
        other => panic!("{cmd} {p}: {other:?}"),
    };
    let taken = Reason::NameTaken { kind: "class", name: "Order".into() };
    assert_eq!(why("uml.interface.add", json!({ "package": sales, "name": "ORDER" })), (Some(taken), "there is a class named Order already".into()));
    let into = (Some(Reason::PackageIntoItself), "a package cannot go into itself or a package it holds".to_string());
    assert_eq!(why("uml.package.move", json!({ "package": sales, "to": inner })), into);
    let itself = (Some(Reason::DependsOnItself { kind: "class" }), "a class cannot depend on itself".to_string());
    assert_eq!(why("uml.dependency.add", json!({ "client": order, "supplier": order })), itself);
    // Reconnected onto its client, a dependency would make it depend on itself too.
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    let uses = run(&mut s, "uml.dependency.add", json!({ "client": order, "supplier": line }));
    let p = json!({ "relationship": uses, "end": "target", "class": order });
    let Err(EngineError::Failed { error, .. }) = s.execute("uml.relationship.reconnect", &p) else { panic!("{p} is refused") };
    assert_eq!((error.reason, error.message), itself, "reconnected");
}

#[test]
fn a_class_and_a_package_move_a_cycle_or_a_clash_is_refused_and_the_diagrams_keep_showing_them() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "billing" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "sales" }));
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    round_trip(&mut s, "uml.class.move", json!({ "class": order, "package": billing }));
    assert_eq!(class_names(&s, &billing), ["Invoice", "Order"]);
    assert!(class_names(&s, &sales).is_empty());
    assert!(alive(&s, &node), "the diagram still shows it, now as an outsider");
    let card = archforge_uml::figures::card(s.doc(), obj(&s, &order), obj(&s, &d)).unwrap();
    assert!(card.elsewhere);
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Said("Class · {package}", vec![("package", "billing".into())]));
    // A class named like one already there is refused.
    let other = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "order" }));
    refused(&mut s, "uml.class.move", json!({ "class": other, "package": billing }), "named Order already");
    // A package goes into another, or back into the model's root; into itself or what it holds is refused.
    round_trip(&mut s, "uml.package.move", json!({ "package": billing, "to": inner }));
    assert_eq!(parent_of(&s, &billing), Some(obj(&s, &inner)));
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": billing }), "into itself");
    refused(&mut s, "uml.package.move", json!({ "package": sales, "to": sales }), "into itself");
    round_trip(&mut s, "uml.package.move", json!({ "package": billing, "to": "" }));
    assert_eq!(parent_of(&s, &billing), archforge_uml::commands::root_package(s.doc()));
    run(&mut s, "uml.package.add", json!({ "name": "inner" }));
    refused(&mut s, "uml.package.move", json!({ "package": inner, "to": "" }), "named inner already");
}

#[test]
fn a_package_removed_hands_its_contents_and_diagrams_up_to_its_parent_the_root_included() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "orders", "parent": sales }));
    let deep = run(&mut s, "uml.package.add", json!({ "name": "lines", "parent": orders }));
    let order = run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    let customer = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Customer" }));
    let a = run(&mut s, "uml.association.add", json!({ "from": customer, "to": order }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": orders, "name": "orders" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": order, "x": 0, "y": 0 }));
    // sales's own default takes the diagram in, an additional diagram of sales (AR-673).
    let own = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "sales" }));
    // orders goes: Order, the package lines and the diagram move up into sales.
    round_trip(&mut s, "element.remove", json!({ "ids": [orders] }));
    let default = archforge_uml::defaults::resolve(s.doc(), obj(&s, &sales));
    assert_eq!(default, Ok(archforge_uml::defaults::DefaultDiagram::Bound(obj(&s, &own))), "sales's default stays");
    assert!(!alive(&s, &orders));
    assert_eq!(class_names(&s, &sales), ["Customer", "Order"]);
    assert_eq!(parent_of(&s, &deep), Some(obj(&s, &sales)));
    assert!(alive(&s, &d) && alive(&s, &a), "the diagram and the association stay");
    let of = s.doc().get(obj(&s, &d), archforge_diagram::diagram::ELEMENT).unwrap().as_ref();
    assert_eq!(of, Some(obj(&s, &sales)), "the diagram is of the parent now");
    // A clash in the parent is refused, with nothing moved.
    let dup = run(&mut s, "uml.package.add", json!({ "name": "dup", "parent": sales }));
    run(&mut s, "uml.class.add", json!({ "package": dup, "name": "customer" }));
    refused(&mut s, "element.remove", json!({ "ids": [dup] }), "named Customer already");
    // Every package sits in the model's root package, so sales too hands everything up: into
    // the root, the relationships of its classes kept.
    let other = run(&mut s, "uml.package.add", json!({ "name": "other" }));
    let outside = run(&mut s, "uml.class.add", json!({ "package": other, "name": "Outside" }));
    let dep = run(&mut s, "uml.dependency.add", json!({ "client": outside, "supplier": customer }));
    let root = s.doc().uuid(archforge_uml::commands::root_package(s.doc()).unwrap()).unwrap().to_string();
    run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": root, "name": "model" }));
    round_trip(&mut s, "element.remove", json!({ "ids": [sales] }));
    assert!(!alive(&s, &sales));
    for kept in [&customer, &order, &deep, &a, &dep, &d, &outside] {
        assert!(alive(&s, kept), "{kept} stays");
    }
    assert_eq!(parent_of(&s, &deep), archforge_uml::commands::root_package(s.doc()));
}

#[test]
fn the_sweep_finds_no_command_that_breaks_the_store_with_packages_nested() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let inner = run(&mut s, "uml.package.add", json!({ "name": "inner", "parent": sales }));
    let order = run(&mut s, "uml.class.add", json!({ "package": inner, "name": "Order" }));
    let calls = sweep_with(&mut s, &[sales.as_str(), inner.as_str(), order.as_str(), "not-an-id"], |st| {
        format!("{:?}", archforge_mof::rows::snapshot(st, &tables().0, &tables().1).unwrap())
    })
    .unwrap();
    assert!(calls > 100, "{calls}");
    let _ = Value::Null;
}

/// A package leads to its default class diagram, the one its binding names: `uml.package.diagram`
/// refuses a package with none yet and makes nothing, then gives its first diagram, and the one
/// `uml.package.default.set` chooses once it is chosen, though another was made first. It only
/// reads: no undo step (AR-582). A diagram named like its package follows the package's renames.
#[test]
fn a_package_leads_to_its_default_class_diagram_and_entry_never_makes_one() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    refused(&mut s, "uml.package.diagram", json!({ "package": orders }), "has no class diagram yet, so none opens");
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": orders, "name": "Orders" }));
    let detail = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": orders, "name": "Detail" }));
    let entered = |s: &mut S| {
        let (at, label) = (s.token(), s.undo_label().map(str::to_string));
        let id = run(s, "uml.package.diagram", json!({ "package": orders }));
        assert_eq!((s.token(), s.undo_label().map(str::to_string)), (at, label), "it only reads");
        id
    };
    assert_eq!(entered(&mut s), d, "its first diagram, its default");
    let at = s.token();
    let token = json!({ "history": at.history, "revision": at.revision });
    run(&mut s, "uml.package.default.set", json!({ "package": orders, "diagram": detail, "expected_revision": token }));
    assert_eq!(entered(&mut s), detail, "the one chosen, though made second");
    s.undo().unwrap();
    assert_eq!(entered(&mut s), d, "undone, the first again");
    // Renamed, the package takes its diagram's name along while the diagram still has the old one.
    run(&mut s, "uml.package.rename", json!({ "package": orders, "name": "Ordering" }));
    assert_eq!(s.doc().get(obj(&s, &d), archforge_diagram::diagram::NAME).unwrap().as_str(), Some("Ordering"));
    run(&mut s, "diagram.rename", json!({ "diagram": d, "name": "Order flow" }));
    run(&mut s, "uml.package.rename", json!({ "package": orders, "name": "Orders" }));
    assert_eq!(s.doc().get(obj(&s, &d), archforge_diagram::diagram::NAME).unwrap().as_str(), Some("Order flow"), "a name of its own stays");
}

/// A structure diagram shows any package of its model as a tabbed card, whichever package holds it
/// (AR-562).
#[test]
fn a_structure_diagram_shows_packages_from_anywhere_in_its_model_as_tabbed_cards() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    // A package Sales holds goes on its diagram, and so does one it does not hold.
    let node = run(&mut s, "diagram.place", json!({ "diagram": d, "element": orders, "x": 0, "y": 0 }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": billing, "x": 300, "y": 0 }));
    let st = s.doc();
    let n = obj(&s, &node);
    let b = archforge_diagram::grid::bounds_of(st, n);
    assert_eq!((b.width, b.height), archforge_uml::PACKAGE_SIZE, "placed at a package's size");
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, st, obj(&s, &orders), obj(&s, &d)).expect("a card");
    assert_eq!(card.tone, archforge_diagram::figure::Tone::Tabbed);
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::said("Package"));
    assert_eq!(card.name, "Orders");
    assert!(card.compartments.is_empty() && card.hidden == 0, "no rows: its card shows its own diagram instead (AR-113)");
}

/// The id of the root package.
fn root_id(s: &S) -> String {
    s.doc().uuid(archforge_uml::commands::root_package(s.doc()).unwrap()).unwrap().to_string()
}

/// The reason and the English sentence `cmd` is refused with; the store, the revision and the
/// history as they were.
fn refusal(s: &mut S, cmd: &str, p: Json) -> (Option<archforge_engine::Reason>, String) {
    let (before, revision, label) = (snapshot(s), s.revision(), s.undo_label().map(str::to_string));
    let Err(archforge_engine::EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
    assert_eq!((snapshot(s), s.revision(), s.undo_label().map(str::to_string)), (before, revision, label), "{cmd} {p} changed nothing");
    (error.reason, error.message)
}

/// The reason a package that is the `end` of a dependency that stays is kept for, naming the
/// dependency and its other end.
fn kept(package: &str, end: End, other: &str, dependency: &str) -> Option<archforge_engine::Reason> {
    let of = archforge_engine::DependencyOf { package: package.into(), end, other: other.into(), dependency: dependency.into() };
    Some(archforge_engine::Reason::DependencyEnd(Box::new(of)))
}

/// A `class` named `name` among `owner`'s packaged elements.
fn packaged(st: &mut Store, class: archforge_mof::ClassId, name: &str, owner: Obj) -> Obj {
    let o = st.create(class).unwrap();
    st.set(o, archforge_uml::named_element::NAME, Value::Str(name.into())).unwrap();
    st.push(owner, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(o)).unwrap();
    o
}

/// A dependency from `client` on `supplier`, owned by `owner`, as a loaded document may hold one
/// whose ends are packages.
fn depending(st: &mut Store, client: Obj, supplier: Obj, owner: Obj) -> Obj {
    let d = st.create(archforge_uml::DEPENDENCY).unwrap();
    st.push(owner, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(d)).unwrap();
    st.push(d, archforge_uml::dependency::CLIENT, Value::Ref(client)).unwrap();
    st.push(d, archforge_uml::dependency::SUPPLIER, Value::Ref(supplier)).unwrap();
    d
}

/// A package that a dependency which stays has as client or supplier, deeper or right under the
/// root, is kept until AR-698, the refusal naming it, the dependency and its other end; it goes
/// once the dependency goes, or its classifier client with it, a package it holds handed up with
/// its dependency, Undo and Redo exact (AR-696). The dependencies are made as a loaded document
/// holds them.
#[test]
fn a_package_a_dependency_that_stays_starts_at_or_depends_on_is_kept_until_the_dependency_goes() {
    let mut st = store(0);
    st.create(archforge_core::MODEL).unwrap();
    let root = archforge_uml::commands::ensure_root(&mut st).unwrap();
    let [sales, billing] = ["sales", "billing"].map(|name| packaged(&mut st, PACKAGE, name, root));
    let (ledger, audit) = (packaged(&mut st, PACKAGE, "ledger", billing), packaged(&mut st, PACKAGE, "audit", sales));
    let order = packaged(&mut st, archforge_uml::CLASS, "Order", sales);
    let on_ledger = depending(&mut st, order, ledger, sales);
    let on_billing = depending(&mut st, sales, billing, sales);
    let from_audit = depending(&mut st, audit, billing, audit);
    let [sales, billing, ledger, audit, order, on_ledger, on_billing, from_audit] =
        [sales, billing, ledger, audit, order, on_ledger, on_billing, from_audit].map(|o| st.uuid(o).unwrap().to_string());
    let mut s = over(st);
    let cases = [
        (&ledger, "ledger", End::Target, "Order", &on_ledger),
        (&billing, "billing", End::Target, "sales", &on_billing),
        (&sales, "sales", End::Source, "billing", &on_billing),
        (&audit, "audit", End::Source, "billing", &from_audit),
    ];
    for (package, name, end, other, d) in cases {
        let (reason, message) = refusal(&mut s, "element.remove", json!({ "ids": [package] }));
        assert_eq!(reason, kept(name, end, other, d));
        let link = if end == End::Source { "on" } else { "from" };
        assert!(message.contains(&format!("the dependency {d} {link} {other}, which stays; remove that dependency first")), "{message}");
    }
    run(&mut s, "element.remove", json!({ "ids": [on_billing, from_audit] }));
    round_trip(&mut s, "element.remove", json!({ "ids": [billing] }));
    assert!(alive(&s, &ledger) && alive(&s, &on_ledger), "ledger went up with its dependency");
    // A dependency that goes with its classifier client lets its supplier go in the same step.
    round_trip(&mut s, "element.remove", json!({ "ids": [order, ledger] }));
    assert!(!alive(&s, &on_ledger) && !alive(&s, &ledger));
    round_trip(&mut s, "element.remove", json!({ "ids": [audit] }));
    round_trip(&mut s, "element.remove", json!({ "ids": [sales] }));
}

/// A parentless package, as a loaded document may hold, takes what it holds along, the
/// dependencies inside it among them, so it is kept while a package it takes along is any supplier
/// of a dependency that stays elsewhere, and goes once that dependency goes (AR-696).
#[test]
fn a_package_with_no_parent_is_kept_while_a_package_it_takes_along_is_a_dependency_s_supplier() {
    use archforge_uml::{dependency, named_element, package};
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    let [top, inner, other] = [(); 3].map(|_| st.create(PACKAGE).unwrap());
    let [order, d, e] = [archforge_uml::CLASS, archforge_uml::DEPENDENCY, archforge_uml::DEPENDENCY].map(|c| st.create(c).unwrap());
    for p in [top, other] {
        st.push(model, archforge_core::model::MEMBER, Value::Ref(p)).unwrap();
    }
    st.set(order, named_element::NAME, Value::Str("Order".into())).unwrap();
    st.set(inner, named_element::NAME, Value::Str("ledger".into())).unwrap();
    for (owner, held) in [(top, inner), (other, order), (other, d), (inner, e)] {
        st.push(owner, package::PACKAGED_ELEMENT, Value::Ref(held)).unwrap();
    }
    st.push(d, dependency::CLIENT, Value::Ref(order)).unwrap();
    // A second supplier past a first that stays, as a loaded document may have it.
    st.push(d, dependency::SUPPLIER, Value::Ref(other)).unwrap();
    st.push(d, dependency::SUPPLIER, Value::Ref(inner)).unwrap();
    st.push(e, dependency::CLIENT, Value::Ref(inner)).unwrap();
    st.push(e, dependency::SUPPLIER, Value::Ref(other)).unwrap();
    let [top, d, e] = [top, d, e].map(|o| st.uuid(o).unwrap().to_string());
    let mut s = over(st);
    let (reason, _) = refusal(&mut s, "element.remove", json!({ "ids": [top] }));
    assert_eq!(reason, kept("ledger", End::Target, "Order", &d));
    run(&mut s, "element.remove", json!({ "ids": [d] }));
    round_trip(&mut s, "element.remove", json!({ "ids": [top] }));
    assert!(!alive(&s, &e), "the dependency inside went along with its client");
}

/// A package of another branch, the root, an owner of the diagram's package and that package
/// itself are each placed by id as an appearance of the package there is: no package made, its
/// owner kept, one undo step undone and redone exactly, and the rows read back the same (AR-562).
#[test]
fn a_package_from_another_branch_the_root_and_an_owner_appear_without_changing_owner() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let operations = run(&mut s, "uml.package.add", json!({ "name": "Operations" }));
    let archive = run(&mut s, "uml.package.add", json!({ "name": "Archive", "parent": operations }));
    let root = root_id(&s);
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": orders, "name": "Orders" }));
    let count = archforge_uml::commands::all_packages(s.doc()).len();
    for (i, id) in [&archive, &root, &sales, &orders].into_iter().enumerate() {
        let owner = parent_of(&s, id);
        round_trip(&mut s, "diagram.place", json!({ "diagram": d, "element": id, "x": 300 * i, "y": 0 }));
        let (_, _, nodes) = archforge_diagram::views_of(s.doc(), obj(&s, id));
        assert_eq!(nodes.len(), 1, "one appearance of {id}");
        assert_eq!(archforge_diagram::diagram_of(s.doc(), nodes[0]), Some(obj(&s, &d)));
        assert_eq!(parent_of(&s, id), owner, "{id} keeps its owner");
        assert_eq!(archforge_uml::commands::all_packages(s.doc()).len(), count, "no package made");
    }
    assert_eq!(parent_of(&s, &archive), Some(obj(&s, &operations)));
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(&reread(&s), &tables().0, &tables().1).unwrap()), snapshot(&s), "rows read back");
}

/// A package placed twice is refused as shown already; an id from another project's store names
/// no element here; a profile's diagram shows no package: each refusal typed, nothing changed and
/// no undo step (AR-562).
#[test]
fn a_package_shown_already_one_of_another_project_and_one_on_a_profiles_diagram_are_refused() {
    use archforge_engine::Reason;
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let archive = run(&mut s, "uml.package.add", json!({ "name": "Archive" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": archive, "x": 0, "y": 0 }));
    let shown = refusal(&mut s, "diagram.place", json!({ "diagram": d, "element": archive, "x": 300, "y": 0 }));
    assert_eq!(shown, (Some(Reason::ShownAlready { name: "Archive".into() }), "the diagram shows this element already".into()));
    // Another project's ids are counted elsewhere, so none of its packages is found here.
    let mut other = over({
        let mut st = store(1 << 40);
        st.create(archforge_core::MODEL).unwrap();
        st
    });
    let elsewhere = run(&mut other, "uml.package.add", json!({ "name": "Elsewhere" }));
    let foreign = refusal(&mut s, "diagram.place", json!({ "diagram": d, "element": elsewhere, "x": 300, "y": 0 }));
    assert_eq!(foreign.0, Some(Reason::NoElement { id: elsewhere }));
    // A profile's diagram shows its stereotypes and their metaclasses alone.
    let profile = run(&mut s, "uml.profile.add", json!({ "name": "Audit", "package": sales }));
    let pd = s.execute("uml.profile.diagram", &json!({ "profile": profile })).unwrap().value["id"].as_str().unwrap().to_string();
    for id in [&archive, &sales, &root_id(&s)] {
        let outside = refusal(&mut s, "diagram.place", json!({ "diagram": pd, "element": id, "x": 0, "y": 0 }));
        assert!(matches!(outside.0, Some(Reason::OutsideDiagram { .. })), "{outside:?}");
    }
}

/// A package outside the model, such as one left unowned in a store, is shown on no diagram
/// (AR-562).
#[test]
fn a_package_outside_the_model_is_shown_on_no_diagram() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    let mut st = reread(&s);
    let loose = st.create(PACKAGE).unwrap();
    let diagram = st.by_uuid(Uuid::parse(&d).unwrap()).unwrap();
    let ui = archforge_uml::UmlNotation;
    assert!(!archforge_diagram::Notation::may_show(&ui, &st, diagram, loose), "no owner, no model");
    assert!(archforge_diagram::Notation::may_show(&ui, &st, diagram, st.by_uuid(Uuid::parse(&sales).unwrap()).unwrap()));
}

/// Each appearance of the package under `id`: its node's id, the diagram holding it and its
/// bounds, in id order.
fn appearances(s: &S, id: &str) -> Vec<(String, Obj, archforge_diagram::Rect)> {
    let st = s.doc();
    let mut all: Vec<_> = archforge_diagram::views_of(st, obj(s, id))
        .2
        .into_iter()
        .map(|n| (st.uuid(n).unwrap().to_string(), archforge_diagram::diagram_of(st, n).unwrap(), archforge_diagram::grid::bounds_of(st, n)))
        .collect();
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all
}

/// How many objects of `class` the store holds alive.
fn count(s: &S, class: archforge_mof::ClassId) -> usize {
    s.doc().alive().filter(|o| s.doc().class_of(*o).ok() == Some(class)).count()
}

/// A package moved changes its owner alone (Decision 26): shown on diagrams of two branches, it
/// keeps every appearance under its id at its bounds, a note's link to one of them and the
/// default bindings their first diagrams made, its own included, and gains none on its new owner's diagram; one
/// undo step undone and redone exactly, the rows read back the same; moved back to the root alike
/// (AR-567).
#[test]
fn a_package_moved_keeps_its_appearances_their_links_and_the_bindings_and_gains_none() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let operations = run(&mut s, "uml.package.add", json!({ "name": "Operations" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    let mut diagram = |of: &str, name: &str| run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": name }));
    let (sales_d, ops_d, billing_d, orders_d) =
        (diagram(&sales, "Sales"), diagram(&operations, "Operations"), diagram(&billing, "Billing"), diagram(&orders, "Orders"));
    // Orders on its owner's diagram and on one of another branch, a note there linked to it.
    run(&mut s, "diagram.place", json!({ "diagram": sales_d, "element": orders, "x": 40, "y": 60 }));
    let there = run(&mut s, "diagram.place", json!({ "diagram": ops_d, "element": orders, "x": 400, "y": 20, "width": 260, "height": 180 }));
    let note = run(&mut s, "diagram.note", json!({ "diagram": ops_d, "text": "Kept", "x": 0, "y": 300 }));
    let link = run(&mut s, "diagram.link-note", json!({ "diagram": ops_d, "note": note, "target": there }));
    run(&mut s, "diagram.place", json!({ "diagram": billing_d, "element": operations, "x": 0, "y": 0 }));
    // The package moved, its old owner, its new one and Operations each bound to its one diagram.
    let bindings = [made(&s, &orders, &orders_d), made(&s, &sales, &sales_d), made(&s, &billing, &billing_d), made(&s, &operations, &ops_d)];
    let kept = |s: &S| {
        bindings
            .iter()
            .all(|(b, owner, target)| refers(s, b, OWNER_ELEMENT) == Some(obj(s, owner)) && refers(s, b, DEFAULT_DIAGRAM) == Some(obj(s, target)))
    };
    let (shown, nodes, edges) = (appearances(&s, &orders), count(&s, archforge_diagram::NODE), count(&s, archforge_diagram::EDGE));
    assert_eq!(shown.len(), 2);
    let unchanged = |s: &S| {
        assert_eq!(appearances(s, &orders), shown, "every appearance where it was, under its id");
        assert_eq!((count(s, archforge_diagram::NODE), count(s, archforge_diagram::EDGE)), (nodes, edges), "none added, none taken");
        assert_eq!(refers(s, &link, archforge_diagram::edge::SOURCE), Some(obj(s, &note)));
        assert_eq!(refers(s, &link, archforge_diagram::edge::TARGET), Some(obj(s, &there)), "the note's link stays");
        assert!(kept(s), "the bindings as they were");
        assert_eq!(problems(s.doc()), []);
    };
    round_trip(&mut s, "uml.package.move", json!({ "package": orders, "to": billing }));
    assert_eq!(parent_of(&s, &orders), Some(obj(&s, &billing)));
    unchanged(&s);
    assert!(shown.iter().all(|(_, d, _)| *d != obj(&s, &billing_d)), "not on its new owner's diagram");
    assert_eq!(format!("{:?}", archforge_mof::rows::snapshot(&reread(&s), &tables().0, &tables().1).unwrap()), snapshot(&s), "rows read back");
    s.undo().unwrap();
    assert_eq!(parent_of(&s, &orders), Some(obj(&s, &sales)));
    unchanged(&s);
    s.redo().unwrap();
    // Back to the root, given as nothing.
    round_trip(&mut s, "uml.package.move", json!({ "package": orders, "to": "" }));
    assert_eq!(parent_of(&s, &orders), archforge_uml::commands::root_package(s.doc()));
    unchanged(&s);
}

/// A move into the package itself, into one it holds or into a package holding one of its name is
/// refused, typed, its appearances, the store, the revision and the history as they were; one to
/// the parent it has changes nothing either (AR-567).
#[test]
fn a_refused_move_and_one_to_its_own_parent_leave_the_store_and_the_history_as_they_were() {
    use archforge_engine::Reason;
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": billing }));
    for (of, x) in [(&sales, 0), (&billing, 300)] {
        let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": "D" }));
        run(&mut s, "diagram.place", json!({ "diagram": d, "element": orders, "x": x, "y": 0 }));
    }
    let shown = appearances(&s, &orders);
    let mut why = |to: &str| refusal(&mut s, "uml.package.move", json!({ "package": orders, "to": to })).0;
    assert_eq!(why(&orders), Some(Reason::PackageIntoItself));
    assert_eq!(why(&lines), Some(Reason::PackageIntoItself), "into one it holds");
    assert!(matches!(why(&billing), Some(Reason::NameTaken { .. })), "a package named Orders there already");
    let (before, revision, label) = (snapshot(&s), s.revision(), s.undo_label().map(str::to_string));
    run(&mut s, "uml.package.move", json!({ "package": orders, "to": sales }));
    assert_eq!((snapshot(&s), s.revision(), s.undo_label().map(str::to_string)), (before, revision, label), "its own parent: nothing");
    assert_eq!(appearances(&s, &orders), shown);
}

/// The root package moved, the package Types renamed or moved away from the built-in types, a
/// package made a second Types, and one made Types while it holds a built-in type's name are each
/// refused with their reason typed and the English sentence as before, by a move, a rename and a
/// removal handing a package up; the store, the revision and the history as they were (AR-563).
#[test]
fn a_package_move_or_rename_the_root_or_the_package_types_refuses_carries_its_reason_typed() {
    use archforge_engine::Reason;
    let mut s = session();
    let lib = run(&mut s, "uml.package.add", json!({ "name": "Lib" }));
    run(&mut s, "uml.primitive-type.add", json!({ "package": lib, "name": "Integer" }));
    let outer = run(&mut s, "uml.package.add", json!({ "name": "Outer" }));
    let nested = run(&mut s, "uml.package.add", json!({ "name": "Types", "parent": outer }));
    run(&mut s, "uml.class.add", json!({ "package": nested, "name": "String" }));
    let root = root_id(&s);
    let (reason, message) = refusal(&mut s, "uml.package.move", json!({ "package": root, "to": lib }));
    assert_eq!((reason, message.as_str()), (Some(Reason::RootPackageStays), "the model's root package holds the others and stays where it is"));
    // No package Types yet: neither becomes it while holding a built-in type's name.
    let (reason, message) = refusal(&mut s, "uml.package.rename", json!({ "package": lib, "name": "Types" }));
    assert_eq!(reason, Some(Reason::HoldsBuiltInName { kind: "primitive type", name: "Integer".into() }));
    assert!(message.contains("a built-in type's name; as the package Types"), "{message}");
    let (reason, _) = refusal(&mut s, "uml.package.move", json!({ "package": nested, "to": "" }));
    assert_eq!(reason, Some(Reason::HoldsBuiltInName { kind: "class", name: "String".into() }), "moved to the root");
    // The first use of a built-in type makes the package Types, which then keeps its name and place.
    let order = run(&mut s, "uml.class.add", json!({ "package": lib, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Boolean" }));
    let types = s.doc().uuid(archforge_uml::members::types_package(s.doc()).unwrap()).unwrap().to_string();
    let (reason, message) = refusal(&mut s, "uml.package.rename", json!({ "package": types, "name": "Builtins" }));
    assert_eq!(reason, Some(Reason::TypesPackageKept));
    assert!(message.contains("keeps its name and stays directly under the root package while it holds them"), "{message}");
    assert_eq!(refusal(&mut s, "uml.package.move", json!({ "package": types, "to": outer })).0, Some(Reason::TypesPackageKept), "moved");
    // A second Types beside it, by a rename, a move and a removal handing one up.
    let (reason, message) = refusal(&mut s, "uml.package.rename", json!({ "package": outer, "name": "Types" }));
    assert_eq!(reason, Some(Reason::TypesPackageExists));
    assert!(message.contains("is the package Types already"), "{message}");
    assert_eq!(refusal(&mut s, "uml.package.move", json!({ "package": nested, "to": root })).0, Some(Reason::TypesPackageExists), "moved");
    let (reason, message) = refusal(&mut s, "element.remove", json!({ "ids": [outer] }));
    assert_eq!(reason, Some(Reason::TypesPackageExists), "handed up");
    assert!(message.starts_with("its contents cannot move up: "), "{message}");
}

#[test]
fn a_package_card_builds_no_rows_however_much_it_holds() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "element": sales, "name": "Sales" }));
    let crowded = run(&mut s, "uml.package.add", json!({ "name": "Crowded", "parent": sales }));
    for i in 0..60 {
        run(&mut s, "uml.class.add", json!({ "package": crowded, "name": format!("C{i}") }));
    }
    let card = archforge_diagram::Notation::card(&archforge_uml::UmlNotation, s.doc(), obj(&s, &crowded), obj(&s, &d)).unwrap();
    // Its card shows its own diagram in small instead (AR-113): nothing listed, nothing counted.
    assert_eq!((card.compartments.len(), card.hidden, card.name.as_str()), (0, 0, "Crowded"));
}

/// The owning path the card of the package `id` gives on diagram `d`.
fn path_of(s: &S, id: &str, d: &str) -> Vec<String> {
    archforge_diagram::Notation::card(&archforge_uml::UmlNotation, s.doc(), obj(s, id), obj(s, d)).expect("a package's card").owner_path
}

/// A package's card names its owners from the root down to its parent, whichever branch the
/// diagram shows: the root's card names none (AR-564).
#[test]
fn a_package_card_names_its_owners_from_the_root_down_to_its_parent() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let operations = run(&mut s, "uml.package.add", json!({ "name": "Operations" }));
    let archive = run(&mut s, "uml.package.add", json!({ "name": "Archive", "parent": operations }));
    let root = s.doc().uuid(archforge_uml::commands::root_package(s.doc()).unwrap()).unwrap().to_string();
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": orders, "x": 0, "y": 0 }));
    assert_eq!(path_of(&s, &sales, &d), ["Model"], "a top-level package: the root alone");
    assert_eq!(path_of(&s, &orders, &d), ["Model", "Sales"], "a local package: two owners, the root first");
    assert_eq!(path_of(&s, &lines, &d), ["Model", "Sales", "Orders"], "deeper: every owner down to its parent");
    assert_eq!(path_of(&s, &archive, &d), ["Model", "Operations"], "one of another branch than the diagram's");
    assert!(path_of(&s, &root, &d).is_empty(), "the root has no parent");
}

/// A package's path is read from the model every time, so it follows a rename of an owner and a
/// move, and Undo brings the old path back (AR-564).
#[test]
fn a_package_cards_path_follows_a_rename_and_a_move() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Sales" }));
    run(&mut s, "uml.package.rename", json!({ "package": sales, "name": "Selling" }));
    assert_eq!(path_of(&s, &orders, &d), ["Model", "Selling"]);
    assert_eq!(path_of(&s, &lines, &d), ["Model", "Selling", "Orders"]);
    run(&mut s, "uml.package.move", json!({ "package": orders, "to": billing }));
    assert_eq!(path_of(&s, &orders, &d), ["Model", "Billing"]);
    assert_eq!(path_of(&s, &lines, &d), ["Model", "Billing", "Orders"], "what it holds moves along");
    s.undo().unwrap();
    assert_eq!(path_of(&s, &lines, &d), ["Model", "Selling", "Orders"]);
}

/// A card is built every frame, so a package nested deeper than the path holds names its nearest
/// owners only, its parent last (AR-564).
#[test]
fn a_package_card_names_at_most_its_nearest_owners() {
    let mut s = session();
    let mut at = run(&mut s, "uml.package.add", json!({ "name": "P0" }));
    let d = run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": at, "name": "Deep" }));
    for i in 1..=300 {
        at = run(&mut s, "uml.package.add", json!({ "name": format!("P{i}"), "parent": at }));
    }
    let path = path_of(&s, &at, &d);
    assert_eq!(path.len(), 256);
    assert_eq!((path[0].as_str(), path[255].as_str()), ("P44", "P299"), "the root and the farthest owners are left out");
}

/// `s`'s model read back from its rows into a store of its own, to change by hand what no command
/// changes; its new ids counted far from those in use.
fn reread(s: &S) -> Store {
    let (tables, junctions) = tables();
    let mut st = store(1 << 40);
    archforge_mof::rows::restore(&mut st, &archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap(), &tables, &junctions).unwrap();
    st
}

/// A binding under the id `[id; 16]`, a member of the Model, naming `target` the default diagram
/// of `owner`; gives its id.
fn bind(st: &mut Store, id: u8, owner: Obj, target: Obj) -> String {
    let model = archforge_uml::commands::model(st).unwrap();
    let b = st.create_with_uuid(DEFAULT_DIAGRAM_BINDING, Uuid([id; 16])).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(b)).unwrap();
    st.set(b, OWNER_ELEMENT, Value::Ref(owner)).unwrap();
    st.set(b, DEFAULT_DIAGRAM, Value::Ref(target)).unwrap();
    Uuid([id; 16]).to_string()
}

/// The one binding `diagram.add` made for `owner`'s first diagram, which it names, `target`:
/// (binding, owner, target).
fn made(s: &S, owner: &str, target: &str) -> (String, String, String) {
    let [binding] = archforge_diagram::defaults::bindings_of(s.doc(), obj(s, owner))[..] else { panic!("{owner} has not one binding") };
    let id = s.doc().uuid(binding).unwrap().to_string();
    assert_eq!(refers(s, &id, DEFAULT_DIAGRAM), Some(obj(s, target)), "{owner}'s first diagram is its default");
    (id, owner.to_string(), target.to_string())
}

/// What `prop` of the object under `id` refers to.
fn refers(s: &S, id: &str, prop: PropId) -> Option<Obj> {
    s.doc().get(obj(s, id), prop).unwrap().as_ref()
}

/// A bound package removed takes its binding along (AR-577), as one undo step that brings the
/// package, its diagrams and its binding back under their ids. Nested, it hands its diagrams up and
/// its parent's binding stays as it was, as does that of the package it hands up; held by the root,
/// it hands them up to the root alike. A diagram that is no default goes alone. No binding problem
/// at any point.
#[test]
fn a_bound_package_removed_takes_its_binding_along_and_its_parent_keeps_its_own() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let root = archforge_uml::commands::root_package(s.doc()).map(|r| s.doc().uuid(r).unwrap().to_string()).unwrap();
    let mut diagram = |of: &str, name: &str| run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": name }));
    let (top, sales_d, detail, overview, lines_d) =
        (diagram(&root, "Model"), diagram(&sales, "Sales"), diagram(&orders, "Order detail"), diagram(&orders, "Orders"), diagram(&lines, "Lines"));
    // Each package bound to its first diagram: (binding, package, diagram).
    let (b_root, b_sales) = (made(&s, &root, &top), made(&s, &sales, &sales_d));
    let (b_orders, b_lines) = (made(&s, &orders, &detail), made(&s, &lines, &lines_d));
    let kept = |s: &S, (b, owner, target): &(String, String, String)| {
        alive(s, b) && refers(s, b, OWNER_ELEMENT) == Some(obj(s, owner)) && refers(s, b, DEFAULT_DIAGRAM) == Some(obj(s, target))
    };
    assert_eq!(problems(s.doc()), []);
    // A diagram that is no default goes alone.
    round_trip(&mut s, "diagram.remove", json!({ "diagram": overview }));
    assert!(!alive(&s, &overview) && kept(&s, &b_orders));
    // Orders goes with its binding; its diagram and Lines move up into Sales, their bindings kept.
    round_trip(&mut s, "element.remove", json!({ "ids": [orders] }));
    assert!(!alive(&s, &orders) && !alive(&s, &b_orders.0), "its binding goes with it");
    assert_eq!(refers(&s, &detail, archforge_diagram::diagram::ELEMENT), Some(obj(&s, &sales)), "its diagram is handed up");
    assert!([&b_root, &b_sales, &b_lines].into_iter().all(|b| kept(&s, b)), "the others stay as they were");
    assert_eq!(problems(s.doc()), []);
    // One undo step brings the package, its diagram and its binding back, under their ids.
    s.undo().unwrap();
    assert!(kept(&s, &b_orders) && refers(&s, &detail, archforge_diagram::diagram::ELEMENT) == Some(obj(&s, &orders)));
    assert_eq!(problems(s.doc()), []);
    s.redo().unwrap();
    // Sales, held by the root, hands up to the root in turn, whose binding stays.
    round_trip(&mut s, "element.remove", json!({ "ids": [sales] }));
    assert!(!alive(&s, &sales) && !alive(&s, &b_sales.0) && alive(&s, &sales_d) && alive(&s, &detail));
    assert!(kept(&s, &b_root) && kept(&s, &b_lines));
    assert_eq!(problems(s.doc()), []);
}

/// A bound package with no parent removed takes its diagrams along, and with them its binding and
/// that of the package it holds (AR-577): never refused, as one undo step that brings all back.
#[test]
fn a_package_with_no_parent_takes_its_diagrams_and_every_binding_in_it_along() {
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    let member = |st: &mut Store, class, id: u8| {
        let o = st.create_with_uuid(class, Uuid([id; 16])).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(o)).unwrap();
        o
    };
    let outer = member(&mut st, PACKAGE, 0x10);
    let inner = st.create_with_uuid(PACKAGE, Uuid([0x20; 16])).unwrap();
    st.push(outer, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(inner)).unwrap();
    for (package, id) in [(outer, 0x11), (inner, 0x21)] {
        let d = member(&mut st, DIAGRAM, id);
        DiagramObj::set_kind(&mut st, d, Some(STRUCTURE_DIAGRAM)).unwrap();
        DiagramObj::set_element(&mut st, d, Some(package)).unwrap();
        bind(&mut st, id + 1, package, d);
    }
    let mut s = over(st);
    let ids: Vec<String> = [0x10, 0x11, 0x12, 0x20, 0x21, 0x22].map(|n| Uuid([n; 16]).to_string()).into();
    assert!(parent_of(&s, &ids[0]).is_none() && problems(s.doc()).is_empty());
    round_trip(&mut s, "element.remove", json!({ "ids": [ids[0]] }));
    assert!(ids.iter().all(|id| !alive(&s, id)), "the packages, their diagrams and their bindings go");
    assert_eq!(problems(s.doc()), []);
    s.undo().unwrap();
    assert!(ids.iter().all(|id| alive(&s, id)));
    assert_eq!(refers(&s, &ids[5], OWNER_ELEMENT), Some(obj(&s, &ids[3])));
    assert_eq!(refers(&s, &ids[5], DEFAULT_DIAGRAM), Some(obj(&s, &ids[4])));
    assert_eq!(problems(s.doc()), []);
}

/// What `element.remove` of `ids` answers, checked as one undo step (AR-675): undone and redone, the
/// rows read back as they were each time, and removed again after one more undo, it answers the
/// same; `removed` is every object that went.
fn removal(s: &mut S, ids: &[&str]) -> Json {
    let p = json!({ "ids": ids });
    let (before, live) = (snapshot(s), s.doc().alive().count());
    let answer = s.execute("element.remove", &p).unwrap_or_else(|e| panic!("{p}: {e}")).value;
    let after = snapshot(s);
    assert_eq!(answer["removed"], json!(live - s.doc().alive().count()), "every object that went is counted: {answer}");
    s.undo().unwrap();
    assert_eq!(snapshot(s), before, "undone");
    s.redo().unwrap();
    assert_eq!(snapshot(s), after, "redone");
    s.undo().unwrap();
    assert_eq!(s.execute("element.remove", &p).unwrap().value, answer, "the same counts after Undo and Redo");
    assert_eq!(snapshot(s), after);
    answer
}

/// The counts `element.remove` answers beside `removed`: (moved elements, moved diagrams,
/// bindings, relationships).
fn counts(answer: &Json) -> (u64, u64, u64, u64) {
    let n = |key: &str| answer[key].as_u64().unwrap_or_else(|| panic!("{key} in {answer}"));
    (n("moved_elements"), n("moved_diagrams"), n("bindings"), n("relationships"))
}

/// A nested package removed reports what it handed up to its parent, its contents and its
/// diagrams, and the one binding that went, its own; the binding of a package it hands up stays
/// and is not counted. Removed with one of its classes, the class is no longer counted as handed
/// up, and its association is counted as gone. Each answer is the same after Undo and Redo (AR-675).
#[test]
fn a_nested_package_removed_reports_what_it_handed_up_and_its_one_binding() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let orders = run(&mut s, "uml.package.add", json!({ "name": "Orders", "parent": sales }));
    let lines = run(&mut s, "uml.package.add", json!({ "name": "Lines", "parent": orders }));
    let order = run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Order" }));
    run(&mut s, "uml.class.add", json!({ "package": orders, "name": "Line" }));
    let customer = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Customer" }));
    run(&mut s, "uml.association.add", json!({ "from": customer, "to": order }));
    for (of, name) in [(&sales, "Sales"), (&orders, "Order detail"), (&orders, "Orders"), (&lines, "Lines")] {
        run(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": name }));
    }
    // Order, Line and Lines move up, with both diagrams of Orders; its binding goes, Lines's stays.
    let answer = removal(&mut s, &[&orders]);
    assert_eq!(counts(&answer), (3, 2, 1, 0), "{answer}");
    assert_eq!(answer.as_object().map(|o| o.len()), Some(5), "removed and the four counts: {answer}");
    // Order goes in the same step: it is not handed up, and its association goes with it.
    s.undo().unwrap();
    let answer = removal(&mut s, &[&orders, &order]);
    assert_eq!(counts(&answer), (2, 2, 1, 1), "{answer}");
    assert_eq!(problems(s.doc()), []);
}

/// A package with no parent removed hands nothing up and reports every binding in it and every
/// relationship that went: the generalization and the dependency inside it, and a generalization
/// onto one of its classes from a class that stays elsewhere. That generalization named alone is
/// counted too, both its ends staying. Each answer is the same after Undo and Redo (AR-675).
#[test]
fn a_package_with_no_parent_removed_reports_its_bindings_and_relationships() {
    use archforge_uml::{CLASS, GENERALIZATION, classifier, generalization};
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    let [outer, other] = [0x10, 0x30].map(|id| {
        let p = st.create_with_uuid(PACKAGE, Uuid([id; 16])).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(p)).unwrap();
        p
    });
    let inner = packaged(&mut st, PACKAGE, "inner", outer);
    let (party, customer, guest) =
        (packaged(&mut st, CLASS, "Party", outer), packaged(&mut st, CLASS, "Customer", inner), packaged(&mut st, CLASS, "Guest", other));
    let [_, guest_is_party] = [customer, guest].map(|specific| {
        let g = st.create(GENERALIZATION).unwrap();
        st.push(specific, classifier::GENERALIZATION, Value::Ref(g)).unwrap();
        st.set(g, generalization::GENERAL, Value::Ref(party)).unwrap();
        g
    });
    depending(&mut st, party, customer, outer);
    for (package, id) in [(outer, 0x11), (inner, 0x21)] {
        let d = st.create_with_uuid(DIAGRAM, Uuid([id; 16])).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(d)).unwrap();
        DiagramObj::set_kind(&mut st, d, Some(STRUCTURE_DIAGRAM)).unwrap();
        DiagramObj::set_element(&mut st, d, Some(package)).unwrap();
        bind(&mut st, id + 1, package, d);
    }
    let [guest, guest_is_party] = [guest, guest_is_party].map(|o| st.uuid(o).unwrap().to_string());
    let mut s = over(st);
    // A relationship named alone goes and is counted, though both its ends stay.
    let answer = removal(&mut s, &[&guest_is_party]);
    assert_eq!(counts(&answer), (0, 0, 0, 1), "{answer}");
    assert!(alive(&s, &guest), "its specific end stays");
    s.undo().unwrap();
    let answer = removal(&mut s, &[&Uuid([0x10; 16]).to_string()]);
    assert_eq!(counts(&answer), (0, 0, 2, 3), "{answer}");
    assert!(alive(&s, &guest), "the class elsewhere stays");
    assert_eq!(problems(s.doc()), []);
}

/// Two packages of one chain removed in one call, `Root > Parent > Child` with a class and a bound
/// diagram under Child: whichever is named first, the class and the diagram are handed up through
/// both to Root and each is counted once, and Child's binding is the one that goes. With Root,
/// which has no parent, in the same call, they go with it and neither is counted; Root's binding
/// goes too (AR-675).
#[test]
fn packages_of_one_chain_removed_together_count_what_they_hand_up_once() {
    let mut st = store(0);
    let model = st.create(archforge_core::MODEL).unwrap();
    let root = st.create_with_uuid(PACKAGE, Uuid([0x40; 16])).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(root)).unwrap();
    let parent = packaged(&mut st, PACKAGE, "Parent", root);
    let child = packaged(&mut st, PACKAGE, "Child", parent);
    let class = packaged(&mut st, archforge_uml::CLASS, "Order", child);
    // Root and Child each bound to a diagram of their own: Root needs one to take Child's.
    let [_, d] = [(root, 0x41), (child, 0x51)].map(|(package, id)| {
        let d = st.create_with_uuid(DIAGRAM, Uuid([id; 16])).unwrap();
        st.push(model, archforge_core::model::MEMBER, Value::Ref(d)).unwrap();
        DiagramObj::set_kind(&mut st, d, Some(STRUCTURE_DIAGRAM)).unwrap();
        DiagramObj::set_element(&mut st, d, Some(package)).unwrap();
        bind(&mut st, id + 1, package, d);
        d
    });
    let [root, parent, child, class, diagram] = [root, parent, child, class, d].map(|o| st.uuid(o).unwrap().to_string());
    let mut s = over(st);
    for ids in [[child.as_str(), parent.as_str()], [parent.as_str(), child.as_str()]] {
        let answer = removal(&mut s, &ids);
        assert_eq!(counts(&answer), (1, 1, 1, 0), "{ids:?}: {answer}");
        assert!(alive(&s, &class) && alive(&s, &diagram), "{ids:?}: both are handed up to Root");
        assert_eq!(parent_of(&s, &class), Some(obj(&s, &root)), "{ids:?}");
        assert_eq!(refers(&s, &diagram, archforge_diagram::diagram::ELEMENT), Some(obj(&s, &root)), "{ids:?}");
        assert_eq!(problems(s.doc()), []);
        s.undo().unwrap();
    }
    let answer = removal(&mut s, &[&child, &parent, &root]);
    assert_eq!(counts(&answer), (0, 0, 2, 0), "{answer}");
    assert!(!alive(&s, &class) && !alive(&s, &diagram), "both go with Root");
    assert_eq!(problems(s.doc()), []);
}
