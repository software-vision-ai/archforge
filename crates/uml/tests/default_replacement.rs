//! A package's default diagram replaced as it is removed, through the commands alone, as the model
//! and an agent see it (AR-658): a choice read before another caller changed the default is stale
//! in both forms, and on the revision read now it is judged afresh; the interface's labelled step
//! and an agent's command give the same answer, the same ids and the same store, each one undo step
//! undone and redone exactly; removing a whole package, top-level or nested, never makes a
//! replacement.

use std::collections::{BTreeMap, BTreeSet};

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DIAGRAM, ProjectView};
use archforge_engine::{EngineError, Options, Reason, Session};
use archforge_mof::rows::{self, Rows};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::defaults::{DefaultDiagram, resolve};
use archforge_uml::{ALONE, STRUCTURE_DIAGRAM, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// A session over a store holding only the project's Model, its new ids counted from one.
fn session() -> S {
    let mut n = 0u64;
    let ids = Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    });
    let universe = archforge_diagram::universe_with(vec![archforge_core::metamodel().unwrap(), archforge_uml::metamodel().unwrap()]).unwrap();
    let slabs = CompositeSlabs::new()
        .with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))
        .and_then(|s| s.with(archforge_uml::INDEX, Box::new(archforge_uml::UmlSlabs::default())))
        .and_then(archforge_diagram::with_slabs)
        .unwrap();
    let mut st = Store::new(universe, Box::new(slabs), ids);
    st.create(archforge_core::MODEL).unwrap();
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// Runs `cmd` and gives the id it answers with.
fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

/// Sales and its Archive, Order shown on Sales's two diagrams, and Billing beside them, all in the
/// root package, which has a default of its own.
struct Scene {
    root: String,
    sales: String,
    archive: String,
    /// Sales's first diagram, bound as its default as it was made.
    overview: String,
    detail: String,
    /// Archive's own default.
    archived: String,
}

/// The scene every test starts from, built by the commands a person or an agent runs.
fn scene(s: &mut S) -> Scene {
    let sales = run(s, "uml.package.add", json!({ "name": "Sales" }));
    let archive = run(s, "uml.package.add", json!({ "name": "Archive", "parent": sales }));
    let billing = run(s, "uml.package.add", json!({ "name": "Billing" }));
    let order = run(s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let (st, sales_obj) = (s.doc(), s.doc().by_uuid(Uuid::parse(&sales).unwrap()).unwrap());
    let root = st.uuid(st.container(sales_obj).unwrap().unwrap().0).unwrap().to_string();
    let mut diagram = |of: &str, name: &str| run(s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": of, "name": name }));
    let (overview, detail) = (diagram(&sales, "Sales overview"), diagram(&sales, "Sales detail"));
    let archived = diagram(&archive, "Archive overview");
    diagram(&billing, "Billing overview");
    diagram(&root, "Model overview");
    run(s, "diagram.place", json!({ "diagram": overview, "element": order, "x": 40, "y": 60 }));
    run(s, "diagram.place", json!({ "diagram": detail, "element": order, "x": 300, "y": 20 }));
    Scene { root, sales, archive, overview, detail, archived }
}

/// The revision the session is at, as `expected_revision` takes it.
fn token(s: &S) -> Json {
    let at = s.token();
    json!({ "history": at.history, "revision": at.revision })
}

/// The store's rows, to compare one state with another.
fn rows_of(s: &S) -> Rows {
    let tables = [archforge_core::TABLES, archforge_diagram::TABLES, archforge_uml::TABLES].concat();
    let junctions = [archforge_core::JUNCTIONS, archforge_diagram::JUNCTIONS, archforge_uml::JUNCTIONS].concat();
    rows::snapshot(s.doc(), &tables, &junctions).unwrap()
}

/// The ids of the live diagrams.
fn diagrams(s: &S) -> BTreeSet<String> {
    let st = s.doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(DIAGRAM)).map(|o| st.uuid(o).unwrap().to_string()).collect()
}

/// Every live diagram's element, by their ids.
fn elements(s: &S) -> BTreeMap<String, String> {
    let st = s.doc();
    let id = |o: Obj| st.uuid(o).unwrap().to_string();
    let element = |d: Obj| match st.get(d, archforge_diagram::diagram::ELEMENT) {
        Ok(Value::Ref(e)) => id(e),
        other => panic!("{other:?}"),
    };
    st.alive().filter(|o| st.class_of(*o).ok() == Some(DIAGRAM)).map(|d| (id(d), element(d))).collect()
}

/// Every binding by its owner's id: (the binding's id, the diagram's).
fn bound(s: &S) -> BTreeMap<String, (String, String)> {
    let st = s.doc();
    let id = |o: Obj| st.uuid(o).unwrap().to_string();
    let end = |b: Obj, prop| match st.get(b, prop) {
        Ok(Value::Ref(o)) => id(o),
        other => panic!("{other:?}"),
    };
    let ends = |b: Obj| (end(b, OWNER_ELEMENT), (id(b), end(b, DEFAULT_DIAGRAM)));
    archforge_diagram::defaults::bindings(st).into_iter().map(ends).collect()
}

/// The object the id `id` names.
fn at(s: &S, id: &str) -> Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// `p` with the keys of `form` added: one of the two forms of a replacement.
fn joined(mut p: Json, form: &Json) -> Json {
    p.as_object_mut().unwrap().extend(form.as_object().unwrap().clone());
    p
}

/// The two forms of a replacement in the scene: Sales's other diagram, and a new one.
fn forms(c: &Scene) -> [Json; 2] {
    [json!({ "replacement_diagram": c.detail }), json!({ "create_replacement": { "name": "Sales map" } })]
}

/// The diagram the package `id` opens to, by its id.
fn default_of(s: &S, id: &str) -> Option<String> {
    let st = s.doc();
    match resolve(st, at(s, id)) {
        Ok(DefaultDiagram::Bound(d)) => Some(st.uuid(d).unwrap().to_string()),
        _ => None,
    }
}

/// `diagram.remove` given `p`, checked to be refused with the store, the revision and the undo step
/// on top as they were: the refusal's parameter and reason.
fn refused(s: &mut S, p: Json) -> (Option<String>, Option<Reason>) {
    let (at, before, top) = (s.token(), rows_of(s), s.undo_label().map(str::to_string));
    let Err(EngineError::Failed { error, .. }) = s.execute("diagram.remove", &p) else { panic!("{p} was not refused") };
    assert_eq!((s.token(), s.undo_label().map(str::to_string)), (at, top), "{p} left no undo step");
    assert!(rows_of(s) == before, "{p} changed nothing");
    (error.key, error.reason)
}

/// A choice read before an agent chose another default is stale in both forms, naming the revision
/// read and the one now. On the revision read now the old default needs no replacement, a new one's
/// invalid name is refused, and a replacement removed since is no diagram, each with nothing made.
#[test]
fn a_choice_read_before_a_concurrent_default_set_is_refused_and_judged_afresh() {
    let mut s = session();
    let c = scene(&mut s);
    let (read, at) = (token(&s), s.token());
    let set = json!({ "package": c.sales, "diagram": c.detail, "expected_revision": read });
    s.execute_from("uml.package.default.set", &set, Some("Claude")).unwrap();
    let now = s.token();
    for form in forms(&c) {
        let stale = Reason::StaleRevision { expected: Some(at), current: now };
        let p = joined(json!({ "diagram": c.overview, "expected_revision": read }), &form);
        assert_eq!(refused(&mut s, p), (Some("expected_revision".into()), Some(stale)), "{form}");
    }
    let p = json!({ "diagram": c.overview, "replacement_diagram": c.archived, "expected_revision": token(&s) });
    assert_eq!(refused(&mut s, p).1, Some(Reason::ReplacementNotNeeded { diagram: "Sales overview".into() }));
    let fresh = token(&s);
    let named = |name: &str| json!({ "diagram": c.detail, "create_replacement": { "name": name }, "expected_revision": fresh });
    assert_eq!(refused(&mut s, named(" ")).1, Some(Reason::NameEmpty));
    assert_eq!(refused(&mut s, named("Sales\u{7}map")).1, Some(Reason::ControlCharacter));
    s.execute("diagram.remove", &json!({ "diagram": c.overview })).unwrap();
    let p = json!({ "diagram": c.detail, "replacement_diagram": c.overview, "expected_revision": token(&s) });
    assert_eq!(refused(&mut s, p), (Some("replacement_diagram".into()), Some(Reason::NoElement { id: c.overview.clone() })));
    assert_eq!(default_of(&s, &c.sales), Some(c.detail.clone()), "the agent's choice stays");
}

/// Both forms run as the interface's labelled step and as an agent's command give the same answer,
/// the same ids and the same store; one undo step brings the old default back and redo the new one,
/// under the binding's id and the replacement's.
#[test]
fn an_agents_removal_gives_what_the_interfaces_step_gives_undone_and_redone_exactly() {
    for n in 0..2 {
        let (mut ui, mut agent) = (session(), session());
        let (c, _) = (scene(&mut ui), scene(&mut agent));
        let form = forms(&c)[n].clone();
        // Each session reads its own revision, as the dialog and the agent each do.
        let asked = |s: &S| joined(json!({ "diagram": c.overview, "expected_revision": token(s) }), &form);
        let (p, by_agent_p) = (asked(&ui), asked(&agent));
        let (before, binding) = (rows_of(&ui), bound(&ui)[&c.sales].0.clone());
        let mut answer = Json::Null;
        ui.batch("Delete diagram", |b| b.execute("diagram.remove", &p).map(|v| answer = v)).unwrap();
        let by_agent = agent.execute_from("diagram.remove", &by_agent_p, Some("Claude")).unwrap().value;
        assert_eq!(by_agent, answer, "{form}");
        let after = rows_of(&ui);
        assert!(rows_of(&agent) == after, "{form}: the same store");
        let replacement = answer["replacement"].as_str().unwrap().to_string();
        assert_eq!((&answer["binding"], &answer["created"]), (&json!(binding), &json!(n == 1)), "{answer}");
        assert_eq!(replacement == c.detail, n == 0, "{answer}");
        for s in [&mut ui, &mut agent] {
            assert_eq!(default_of(s, &c.sales), Some(replacement.clone()), "{form}");
            s.undo().unwrap();
            assert!(rows_of(s) == before, "{form}: one undo step");
            assert_eq!((default_of(s, &c.sales), s.can_undo()), (Some(c.overview.clone()), true), "{form}: only that step undone");
            s.redo().unwrap();
            assert!(rows_of(s) == after, "{form}: redo is exact");
            assert_eq!(default_of(s, &c.sales), Some(replacement.clone()), "{form}: under the same id");
        }
    }
}

/// A package removed whole hands its diagrams up and makes none: nested Archive to Sales, and
/// top-level Sales to the root, Archive moving up beside them with its own default. The removed
/// package's binding goes; every other stays as it was, under its id and naming the same diagram;
/// one undo step brings it all back.
#[test]
fn removing_a_whole_package_never_makes_a_replacement() {
    let mut s = session();
    let c = scene(&mut s);
    let (all, was, before, of) = (diagrams(&s), bound(&s), rows_of(&s), elements(&s));
    let owner = |s: &S, of: &str| s.doc().container(at(s, of)).unwrap().map(|(o, _)| s.doc().uuid(o).unwrap().to_string());
    for (removed, up, handed) in [(&c.archive, &c.sales, 1), (&c.sales, &c.root, 2)] {
        s.execute("element.remove", &json!({ "ids": [removed] })).unwrap();
        assert!(s.doc().by_uuid(Uuid::parse(removed).unwrap()).is_none_or(|o| !s.doc().is_alive(o)), "{removed} goes");
        assert_eq!(diagrams(&s), all, "{removed}: nothing made");
        let mut moved = of.clone();
        moved.values_mut().filter(|e| *e == removed).for_each(|e| *e = up.clone());
        assert_eq!(elements(&s), moved, "{removed}: its {handed} diagrams handed up, the others where they were");
        assert_eq!(of.values().filter(|e| *e == removed).count(), handed, "{removed}");
        let mut kept = was.clone();
        kept.remove(removed);
        assert_eq!(bound(&s), kept, "{removed}: its binding goes, the others stay");
        s.undo().unwrap();
        assert!(rows_of(&s) == before, "{removed}: one undo step");
    }
    s.execute("element.remove", &json!({ "ids": [c.sales] })).unwrap();
    assert_eq!(owner(&s, &c.archive), Some(c.root.clone()), "what Sales held moves up");
    assert_eq!(default_of(&s, &c.archive), Some(c.archived.clone()));
}
