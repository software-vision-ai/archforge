//! A dependency drawn again between the same client and supplier reuses the one there is rather
//! than adding a parallel one (AR-699): one match is selected with nothing changed and no undo
//! step, several ask for a choice with nothing changed, none adds one; a stale choice is refused
//! naming the current candidates; matching goes by identities in order, never by names.

use archforge_diagram::ProjectView;
use archforge_engine::{EngineError, Options, Reason, Session};
use archforge_mof::{CompositeSlabs, Store, Uuid, Value};
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

/// Runs `cmd`, which must succeed, and gives the id it made or named.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// Runs `uml.dependency.add`, which must succeed, and gives its whole result.
fn add(s: &mut S, p: Json) -> Json {
    s.execute("uml.dependency.add", &p).unwrap_or_else(|e| panic!("{p}: {e}")).value
}

/// The session's model as the rows it would save, its revision and its undo step's name.
fn state(s: &S) -> (String, archforge_engine::Revision, Option<String>) {
    let (tables, junctions) = tables();
    (format!("{:?}", archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap()), s.token(), s.undo_label().map(str::to_string))
}

/// The tables and junctions of the core, the diagrams and UML, as a project stores them.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    (
        archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect(),
        archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect(),
    )
}

/// The element the id `id` names in the session's model.
fn obj(s: &S, id: &str) -> archforge_mof::Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// The dependency `id` as a candidate: its id, name, client, supplier and an empty detail.
fn candidate(id: &str, name: &str, client: &str, supplier: &str) -> Json {
    json!({ "id": id, "name": name, "client": client, "supplier": supplier, "detail": {} })
}

/// The session `built` reloaded as a project is, each dependency of `moved` joining the client to
/// the supplier given with it instead, as a document of an earlier version may have dependencies
/// parallel to one another, which no command makes now.
fn reloaded_with(built: &S, moved: &[(&str, &str, &str)]) -> S {
    let (tables, junctions) = tables();
    let rows = archforge_mof::rows::snapshot(built.doc(), &tables, &junctions).unwrap();
    let mut st = store(1000);
    archforge_mof::rows::restore(&mut st, &rows, &tables, &junctions).unwrap();
    let at = |st: &Store, id: &str| st.by_uuid(Uuid::parse(id).unwrap()).unwrap();
    for (d, client, supplier) in moved {
        let d = at(&st, d);
        let (was_client, was_supplier) = archforge_uml::figures::ends(&st, d).unwrap();
        for (prop, was, now) in
            [(archforge_uml::dependency::CLIENT, was_client, client), (archforge_uml::dependency::SUPPLIER, was_supplier, supplier)]
        {
            let now = at(&st, now);
            st.remove_value(d, prop, &Value::Ref(was)).unwrap();
            st.push(d, prop, Value::Ref(now)).unwrap();
        }
    }
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// A dependency drawn again from the same client to the same supplier returns the one there is,
/// selected, as the outcome `selected` with it as its only candidate: nothing in the model changes,
/// no undo step is added, and its name and description stay, though another name is sent. Between
/// two packages the same.
#[test]
fn one_match_is_selected_with_nothing_changed_and_no_undo_step() {
    let mut s = session();
    let [sales, billing] = ["sales", "billing"].map(|n| run(&mut s, "uml.package.add", json!({ "name": n })));
    let [order, line] = ["Order", "Line"].map(|n| run(&mut s, "uml.class.add", json!({ "package": sales, "name": n })));
    let first = add(&mut s, json!({ "client": order, "supplier": line, "name": "reads" }));
    let dep = first["id"].as_str().unwrap().to_string();
    assert_eq!((&first["outcome"], &first["candidates"]), (&json!("created"), &json!([])));
    run(&mut s, "element.describe", json!({ "element": dep, "text": "Order reads its lines." }));
    let packages = run(&mut s, "uml.dependency.add", json!({ "client": sales, "supplier": billing }));
    run(&mut s, "view.select", json!({ "ids": [] }));
    let before = state(&s);
    let again = add(&mut s, json!({ "client": order, "supplier": line, "name": "writes" }));
    assert_eq!(again, json!({ "id": dep, "outcome": "selected", "candidates": [candidate(&dep, "reads", &order, &line)] }));
    assert_eq!(state(&s), before, "nothing changed and no undo step");
    assert_eq!(s.view().selection, [obj(&s, &dep)], "selected");
    let d = obj(&s, &dep);
    assert_eq!(archforge_uml::descriptions::description_of(s.doc(), d), "Order reads its lines.");
    let between = add(&mut s, json!({ "client": sales, "supplier": billing }));
    assert_eq!((between["id"].as_str(), &between["outcome"]), (Some(packages.as_str()), &json!("selected")));
    assert_eq!(state(&s), before, "nothing changed between packages either");
}

/// No dependency joins the client to the supplier in that order, so one is added as before: the
/// reverse direction, a realization of the same two, and a classifier of the same name elsewhere
/// are no match, and no name decides one. Each result has its id, the outcome `created` and no
/// candidates.
#[test]
fn the_reverse_direction_another_kind_or_the_same_names_elsewhere_add_a_new_dependency() {
    let mut s = session();
    let [sales, billing] = ["sales", "billing"].map(|n| run(&mut s, "uml.package.add", json!({ "name": n })));
    let [order, priced] =
        [("Order", "uml.class.add"), ("Priced", "uml.interface.add")].map(|(n, c)| run(&mut s, c, json!({ "package": sales, "name": n })));
    let other_order = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Order" }));
    let dep = add(&mut s, json!({ "client": order, "supplier": priced, "name": "uses" }))["id"].as_str().unwrap().to_string();
    run(&mut s, "uml.realization.add", json!({ "class": other_order, "interface": priced }));
    for (client, supplier) in [(&priced, &order), (&other_order, &priced)] {
        let out = add(&mut s, json!({ "client": client, "supplier": supplier, "name": "uses" }));
        assert_eq!((&out["outcome"], &out["candidates"]), (&json!("created"), &json!([])), "{client} to {supplier}");
        let id = out["id"].as_str().unwrap();
        assert!(!id.is_empty() && id != dep, "a new dependency: {out}");
        assert_eq!(s.undo_label(), Some("Add dependency"));
    }
}

/// Several dependencies joining the client to the supplier, as an earlier version may have made,
/// give the outcome `choiceRequired` with a null id and every candidate, same-named and unnamed
/// ones told apart by their ids, with nothing changed and no undo step; `existing` then picks one,
/// which is selected with nothing changed either, its reply listing it alone. The parallel ones are
/// never merged.
#[test]
fn several_matches_need_a_choice_and_existing_picks_one_with_nothing_changed() {
    let mut built = session();
    let sales = run(&mut built, "uml.package.add", json!({ "name": "sales" }));
    let [order, line, product] = ["Order", "Line", "Product"].map(|n| run(&mut built, "uml.class.add", json!({ "package": sales, "name": n })));
    let named = add(&mut built, json!({ "client": order, "supplier": line, "name": "reads" }))["id"].as_str().unwrap().to_string();
    let twin = add(&mut built, json!({ "client": order, "supplier": product, "name": "reads" }))["id"].as_str().unwrap().to_string();
    let unnamed = add(&mut built, json!({ "client": line, "supplier": product }))["id"].as_str().unwrap().to_string();
    let mut s = reloaded_with(&built, &[(&twin, &order, &line), (&unnamed, &order, &line)]);
    let before = state(&s);
    let all = json!([candidate(&named, "reads", &order, &line), candidate(&twin, "reads", &order, &line), candidate(&unnamed, "", &order, &line)]);
    let ask = add(&mut s, json!({ "client": order, "supplier": line, "name": "reads" }));
    assert_eq!(ask, json!({ "id": null, "outcome": "choiceRequired", "candidates": all }));
    assert_eq!(state(&s), before, "nothing changed and no undo step");
    assert!(s.view().selection.is_empty(), "nothing selected");
    let picked = add(&mut s, json!({ "client": order, "supplier": line, "existing": unnamed }));
    assert_eq!(picked, json!({ "id": unnamed, "outcome": "selected", "candidates": [all[2]] }), "the one selected alone");
    assert_eq!(state(&s), before, "picked with nothing changed and no undo step");
    assert_eq!(s.view().selection, [obj(&s, &unnamed)]);
}

/// A choice under `existing` that no longer joins the client to the supplier, removed or reconnected
/// since, or that never did, is refused as a stale choice under `existing`, naming the ids of those
/// that join them now, none when none does, with nothing changed; another is never picked in its
/// place.
#[test]
fn a_stale_choice_is_refused_naming_the_current_candidates_with_nothing_changed() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let [order, line, product] = ["Order", "Line", "Product"].map(|n| run(&mut s, "uml.class.add", json!({ "package": sales, "name": n })));
    let dep = add(&mut s, json!({ "client": order, "supplier": line }))["id"].as_str().unwrap().to_string();
    let moved = add(&mut s, json!({ "client": product, "supplier": line }))["id"].as_str().unwrap().to_string();
    let gone = add(&mut s, json!({ "client": line, "supplier": order }))["id"].as_str().unwrap().to_string();
    run(&mut s, "element.remove", json!({ "ids": [gone] }));
    let cases = [
        (json!({ "client": order, "supplier": line, "existing": gone }), vec![dep.clone()]),
        (json!({ "client": order, "supplier": line, "existing": moved }), vec![dep.clone()]),
        (json!({ "client": order, "supplier": line, "existing": sales }), vec![dep.clone()]),
        (json!({ "client": order, "supplier": line, "existing": "no id" }), vec![dep.clone()]),
        (json!({ "client": line, "supplier": order, "existing": gone }), vec![]),
    ];
    for (p, now) in cases {
        let before = state(&s);
        let Err(EngineError::Failed { error, .. }) = s.execute("uml.dependency.add", &p) else { panic!("{p} is refused") };
        assert_eq!(error.key.as_deref(), Some("existing"), "{p}");
        let candidates = now.iter().map(|id| id.as_str().into()).collect();
        assert_eq!(error.reason, Some(Reason::StaleChoice { candidates }), "{p}");
        let says = if now.is_empty() { "none does now".to_string() } else { format!("those that do now: {}", now.join(", ")) };
        assert!(error.message.ends_with(&says), "{p}: {}", error.message);
        assert_eq!(state(&s), before, "{p} changed nothing");
    }
}

/// Every check a new dependency meets comes before the match: a name refused, the wrong end and a
/// dependency on itself are refused though a dependency between them exists, with nothing changed.
#[test]
fn a_call_that_would_be_refused_is_refused_though_a_match_exists() {
    let mut s = session();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let [order, line] = ["Order", "Line"].map(|n| run(&mut s, "uml.class.add", json!({ "package": sales, "name": n })));
    let total = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    add(&mut s, json!({ "client": order, "supplier": line }));
    for (p, key) in [
        (json!({ "client": order, "supplier": line, "name": " padded " }), "name"),
        (json!({ "client": order, "supplier": total }), "supplier"),
        (json!({ "client": order, "supplier": order }), "supplier"),
    ] {
        let before = state(&s);
        let Err(EngineError::Failed { error, .. }) = s.execute("uml.dependency.add", &p) else { panic!("{p} is refused") };
        assert_eq!(error.key.as_deref(), Some(key), "{p}");
        assert_eq!(state(&s), before, "{p} changed nothing");
    }
}

/// A stale choice among more candidates than a sentence names, as a damaged or old document may
/// hold, names the first twelve and how many more; its reason holds them all.
#[test]
fn a_stale_choice_among_many_names_the_first_twelve_and_how_many_more() {
    let mut built = session();
    let sales = run(&mut built, "uml.package.add", json!({ "name": "sales" }));
    let order = run(&mut built, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let suppliers: Vec<String> = (0..13).map(|i| run(&mut built, "uml.class.add", json!({ "package": sales, "name": format!("S{i}") }))).collect();
    let deps: Vec<String> =
        suppliers.iter().map(|su| add(&mut built, json!({ "client": order, "supplier": su }))["id"].as_str().unwrap().to_string()).collect();
    let moved: Vec<(&str, &str, &str)> = deps.iter().map(|d| (d.as_str(), order.as_str(), suppliers[0].as_str())).collect();
    let mut s = reloaded_with(&built, &moved);
    let Err(EngineError::Failed { error, .. }) =
        s.execute("uml.dependency.add", &json!({ "client": order, "supplier": suppliers[0], "existing": sales }))
    else {
        panic!("a stale choice is refused")
    };
    assert!(error.message.ends_with(&format!("those that do now: {} and 1 more", deps[..12].join(", "))), "{}", error.message);
    assert_eq!(error.reason, Some(Reason::StaleChoice { candidates: deps.iter().map(|d| d.as_str().into()).collect() }));
}
