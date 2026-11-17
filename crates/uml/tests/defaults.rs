//! A package's default diagram through its binding (AR-575): each state built by hand in a store
//! under fixed ids, as no command makes a binding yet: bound, none yet, missing, duplicate, a dead,
//! a wrong-kind and a foreign target, a profile, and every problem of a store in a stable order.
//! Each state read through `uml.package.default.get`, on the revision its caller read, as a
//! read-only tool that leaves no undo step (AR-576).

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DEFAULT_DIAGRAM_BINDING, DIAGRAM, DiagramObj, ProjectView};
use archforge_engine::{BindingFault, EngineError, Options, Reason, Revision, Session};
use archforge_mof::rows::{self, Cell};
use archforge_mof::{ClassId, CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::defaults::{BindingProblem, DefaultDiagram, Diagnosis, problems, resolve};
use archforge_uml::{ALONE, PACKAGE, PROFILE, STRUCTURE_DIAGRAM, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// The query this file's command tests call.
const GET: &str = "uml.package.default.get";

/// An ER diagram's kind, `archforge_er::DIAGRAM_KIND`, which UML does not depend on.
const ER_DIAGRAM: &str = "er";

/// An empty store of the core, the diagrams and UML.
fn bare() -> Store {
    let mut n: u64 = 0;
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
    Store::new(universe, Box::new(slabs), ids)
}

/// A store holding the project's Model: (store, model).
fn store() -> (Store, Obj) {
    let mut st = bare();
    let model = st.create(archforge_core::MODEL).unwrap();
    (st, model)
}

/// A member of `model` of `class` under the id `[id; 16]`.
fn add(st: &mut Store, model: Obj, class: ClassId, id: u8) -> Obj {
    let o = st.create_with_uuid(class, Uuid([id; 16])).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(o)).unwrap();
    o
}

/// A diagram of `kind` of `element`.
fn diagram(st: &mut Store, model: Obj, id: u8, kind: &str, element: Obj) -> Obj {
    let d = add(st, model, DIAGRAM, id);
    DiagramObj::set_kind(st, d, Some(kind)).unwrap();
    DiagramObj::set_element(st, d, Some(element)).unwrap();
    d
}

/// A binding naming `target` the default of `owner`.
fn bind(st: &mut Store, model: Obj, id: u8, owner: Obj, target: Obj) -> Obj {
    let b = add(st, model, DEFAULT_DIAGRAM_BINDING, id);
    st.set(b, OWNER_ELEMENT, Value::Ref(owner)).unwrap();
    st.set(b, DEFAULT_DIAGRAM, Value::Ref(target)).unwrap();
    b
}

/// The package's bound diagram is the one its binding names, never the first one made; a package
/// without a structure diagram has none yet, and neither is a problem.
#[test]
fn a_bound_package_resolves_to_its_target_and_one_without_diagrams_has_none_yet() {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    let first = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, sales);
    let detail = diagram(&mut st, model, 0x12, STRUCTURE_DIAGRAM, sales);
    bind(&mut st, model, 0x13, sales, detail);
    let (empty, schema_only) = (add(&mut st, model, PACKAGE, 0x20), add(&mut st, model, PACKAGE, 0x30));
    diagram(&mut st, model, 0x31, ER_DIAGRAM, schema_only);
    assert!(st.validate_all().is_empty(), "{:?}", st.validate_all());
    assert_eq!(resolve(&st, sales), Ok(DefaultDiagram::Bound(detail)), "not the first-found {first:?}");
    assert_eq!(resolve(&st, empty), Ok(DefaultDiagram::NoDiagramYet));
    assert_eq!(resolve(&st, schema_only), Ok(DefaultDiagram::NoDiagramYet), "an ER diagram is no structure diagram");
    assert_eq!(problems(&st), []);
}

/// A package with a structure diagram but no binding is Missing, and with two bindings Duplicate,
/// naming both in id order; neither picks a diagram.
#[test]
fn a_package_with_no_binding_or_two_is_missing_or_duplicate() {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    let overview = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, sales);
    assert_eq!(resolve(&st, sales), Err(BindingProblem::Missing));
    assert_eq!(BindingProblem::Missing.to_string(), "the package has structure diagrams but no default diagram");
    assert_eq!(problems(&st), [Diagnosis { owner: Some(sales), problem: BindingProblem::Missing }]);
    // Made in the other order from their ids.
    let later = bind(&mut st, model, 0x1b, sales, overview);
    let earlier = bind(&mut st, model, 0x1a, sales, overview);
    assert_eq!(resolve(&st, sales), Err(BindingProblem::Duplicate(vec![earlier, later])));
}

/// A binding naming an ER diagram is of the wrong kind, and one naming another package's
/// structure diagram of the wrong owner.
#[test]
fn a_target_of_another_kind_or_another_package_is_refused() {
    let (mut st, model) = store();
    let (sales, billing) = (add(&mut st, model, PACKAGE, 0x10), add(&mut st, model, PACKAGE, 0x20));
    let schema = diagram(&mut st, model, 0x11, ER_DIAGRAM, sales);
    let theirs = diagram(&mut st, model, 0x21, STRUCTURE_DIAGRAM, billing);
    let binding = bind(&mut st, model, 0x12, sales, schema);
    assert_eq!(resolve(&st, sales), Err(BindingProblem::WrongKind { binding, diagram: schema }));
    st.set(binding, DEFAULT_DIAGRAM, Value::Ref(theirs)).unwrap();
    assert_eq!(resolve(&st, sales), Err(BindingProblem::WrongOwner { binding, diagram: theirs }));
}

/// A profile has no default: not missing without a binding, and a binding it has is wrong.
#[test]
fn a_profile_has_no_default_and_a_binding_of_one_is_a_problem() {
    let (mut st, model) = store();
    let profile = add(&mut st, model, PROFILE, 0x10);
    let own = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, profile);
    assert_eq!(resolve(&st, profile), Ok(DefaultDiagram::DoesNotApply));
    assert_eq!(problems(&st), []);
    let binding = bind(&mut st, model, 0x12, profile, own);
    assert_eq!(resolve(&st, profile), Ok(DefaultDiagram::DoesNotApply));
    assert_eq!(problems(&st), [Diagnosis { owner: Some(profile), problem: BindingProblem::WrongOwner { binding, diagram: own } }]);
}

/// A binding's dead target comes only from rows: a reference to an id the project lacks is
/// refused as the rows are read (the open path, 396-L), and an emptied one reads back as a binding
/// naming no diagram.
#[test]
fn a_binding_naming_no_live_diagram_has_a_dead_target() {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    let overview = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, sales);
    bind(&mut st, model, 0x12, sales, overview);
    let tables: Vec<_> = archforge_core::TABLES.iter().chain(archforge_diagram::TABLES).chain(archforge_uml::TABLES).copied().collect();
    let junctions: Vec<_> = archforge_core::JUNCTIONS.iter().chain(archforge_diagram::JUNCTIONS).chain(archforge_uml::JUNCTIONS).copied().collect();
    let saved = rows::snapshot(&st, &tables, &junctions).unwrap();
    let at = tables.iter().position(|t| t.class == DEFAULT_DIAGRAM_BINDING).unwrap();
    let column = tables[at].columns.iter().position(|c| c.prop == Some(DEFAULT_DIAGRAM)).unwrap();
    let with_target = |cell: Cell| {
        let mut edited = saved.clone();
        edited.tables[at][0][column] = cell;
        let mut back = bare();
        rows::restore(&mut back, &edited, &tables, &junctions).map(|()| back)
    };
    let dangling = with_target(Cell::Id(Uuid([0xdd; 16]))).err().map(|e| e.to_string()).unwrap_or_default();
    assert!(dangling.contains("which is not in the project"), "a dangling reference is not read: {dangling}");
    let back = with_target(Cell::Null).unwrap();
    let (package, binding) = (back.by_uuid(Uuid([0x10; 16])).unwrap(), back.by_uuid(Uuid([0x12; 16])).unwrap());
    assert_eq!(resolve(&back, package), Err(BindingProblem::DeadTarget { binding }));
    assert_eq!(problems(&back), [Diagnosis { owner: Some(package), problem: BindingProblem::DeadTarget { binding } }]);
}

/// Every problem of a store, a binding with no owner's among them, comes ordered by its owner's id
/// and then its binding's, the same in a store read back from the rows with other handles.
#[test]
fn a_stores_problems_come_in_the_order_of_their_ids() {
    let (mut st, model) = store();
    let missing = add(&mut st, model, PACKAGE, 0x30);
    diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, missing);
    let foreign = add(&mut st, model, PACKAGE, 0x20);
    let theirs = diagram(&mut st, model, 0x32, STRUCTURE_DIAGRAM, missing);
    bind(&mut st, model, 0x21, foreign, theirs);
    let profile = add(&mut st, model, PROFILE, 0x10);
    let own = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, profile);
    // Two of the profile's, made in the other order from their ids.
    bind(&mut st, model, 0x13, profile, own);
    bind(&mut st, model, 0x12, profile, own);
    // One naming no owner and a structure diagram of no element, which must not read as a match.
    let loose = add(&mut st, model, DIAGRAM, 0x41);
    DiagramObj::set_kind(&mut st, loose, Some(STRUCTURE_DIAGRAM)).unwrap();
    let ownerless = bind(&mut st, model, 0x40, profile, loose);
    st.set(ownerless, OWNER_ELEMENT, Value::Null).unwrap();
    // (owner, problem, binding) by id.
    let ids = |st: &Store| -> Vec<(Option<Uuid>, &str, Option<Uuid>)> {
        let id = |o: Obj| st.uuid(o).unwrap();
        let said = |d: Diagnosis| match d.problem {
            BindingProblem::Missing => (d.owner.map(id), "Missing", None),
            BindingProblem::WrongOwner { binding, .. } => (d.owner.map(id), "WrongOwner", Some(id(binding))),
            other => panic!("{other:?}"),
        };
        problems(st).into_iter().map(said).collect()
    };
    let u = |n: u8| Some(Uuid([n; 16]));
    let expected = [
        (None, "WrongOwner", u(0x40)),
        (u(0x10), "WrongOwner", u(0x12)),
        (u(0x10), "WrongOwner", u(0x13)),
        (u(0x20), "WrongOwner", u(0x21)),
        (u(0x30), "Missing", None),
    ];
    assert_eq!(ids(&st), expected);
    let (tables, junctions) = (
        [archforge_uml::TABLES, archforge_diagram::TABLES, archforge_core::TABLES].concat(),
        [archforge_core::JUNCTIONS, archforge_diagram::JUNCTIONS, archforge_uml::JUNCTIONS].concat(),
    );
    let mut back = bare();
    rows::restore(&mut back, &rows::snapshot(&st, &tables, &junctions).unwrap(), &tables, &junctions).unwrap();
    assert_eq!(ids(&back), ids(&st), "read back with other handles");
}

/// A session over `st`, which nothing has changed yet.
fn over(st: Store) -> S {
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// The id `[n; 16]` as a command takes it.
fn id(n: u8) -> String {
    Uuid([n; 16]).to_string()
}

/// `uml.package.default.get` of `package` given `p` besides, checked to have changed nothing and
/// left no undo step: its answer, or the refusal's parameter, reason and English.
fn get(s: &mut S, package: &str, mut p: Json) -> Result<Json, (Option<String>, Option<Reason>, String)> {
    let at = s.token();
    p["package"] = json!(package);
    let out = s.execute(GET, &p);
    assert_eq!((s.token(), s.can_undo()), (at, false), "{p} changed nothing and left no undo step");
    match out {
        Ok(o) => Ok(o.value),
        Err(EngineError::Failed { error, .. }) => Err((error.key, error.reason, error.message)),
        Err(other) => panic!("{p}: {other}"),
    }
}

/// Every state a package's default can be in, read through the command: the bound diagram with
/// its binding, none yet, a profile's that does not apply, and each unsound binding refused about
/// `package` as its fault, naming the ids concerned and the choice that mends it; never the first
/// diagram found.
#[test]
fn the_command_answers_every_state_of_a_default_and_changes_nothing() {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, sales);
    let detail = diagram(&mut st, model, 0x12, STRUCTURE_DIAGRAM, sales);
    bind(&mut st, model, 0x13, sales, detail);
    add(&mut st, model, PACKAGE, 0x20);
    let profile = add(&mut st, model, PROFILE, 0x30);
    diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, profile);
    let missing = add(&mut st, model, PACKAGE, 0x40);
    diagram(&mut st, model, 0x41, STRUCTURE_DIAGRAM, missing);
    let twice = add(&mut st, model, PACKAGE, 0x50);
    let both = diagram(&mut st, model, 0x51, STRUCTURE_DIAGRAM, twice);
    bind(&mut st, model, 0x53, twice, both);
    bind(&mut st, model, 0x52, twice, both);
    let schema_bound = add(&mut st, model, PACKAGE, 0x60);
    let schema = diagram(&mut st, model, 0x61, ER_DIAGRAM, schema_bound);
    bind(&mut st, model, 0x62, schema_bound, schema);
    let foreign = add(&mut st, model, PACKAGE, 0x70);
    bind(&mut st, model, 0x72, foreign, detail);
    let dead = add(&mut st, model, PACKAGE, 0x80);
    let gone = diagram(&mut st, model, 0x81, STRUCTURE_DIAGRAM, dead);
    let emptied = bind(&mut st, model, 0x82, dead, gone);
    st.set(emptied, DEFAULT_DIAGRAM, Value::Null).unwrap();
    let mut s = over(st);
    let answer = |state: &str, package: u8, binding: Option<u8>, diagram: Option<u8>| {
        Ok(json!({ "package": id(package), "state": state, "binding": binding.map(id), "diagram": diagram.map(id) }))
    };
    assert_eq!(get(&mut s, &id(0x10), json!({})), answer("bound", 0x10, Some(0x13), Some(0x12)), "not the first-found 0x11");
    assert_eq!(get(&mut s, &id(0x20), json!({})), answer("no_diagram_yet", 0x20, None, None));
    assert_eq!(get(&mut s, &id(0x30), json!({})), answer("does_not_apply", 0x30, None, None));
    let own = "choose one of its own structure diagrams as its default";
    for (package, fault, named, mend) in [
        (0x40, BindingFault::Missing, format!("(package {})", id(0x40)), own),
        (0x50, BindingFault::Duplicate, format!("bindings {}, {})", id(0x52), id(0x53)), "choose which of its structure diagrams stays its default"),
        (0x60, BindingFault::WrongKind, format!("binding {}, diagram {})", id(0x62), id(0x61)), own),
        (0x70, BindingFault::WrongOwner, format!("binding {}, diagram {})", id(0x72), id(0x12)), own),
        (0x80, BindingFault::DeadTarget, format!("binding {})", id(0x82)), own),
    ] {
        let Err((key, reason, said)) = get(&mut s, &id(package), json!({})) else { panic!("{fault:?} answered") };
        assert_eq!((key.as_deref(), reason), (Some("package"), Some(Reason::InvalidBinding(fault))), "{said}");
        assert!(said.contains(&named) && said.ends_with(mend), "{fault:?}: {said}");
    }
    let Err((key, None, said)) = get(&mut s, &id(0x11), json!({})) else { panic!("a diagram answered") };
    assert_eq!((key.as_deref(), said), (Some("package"), format!("{} is not a package", id(0x11))));
}

/// Given the revision its caller read, the command answers only on it: another revision, another
/// history or a token of another shape is refused as stale, naming the revision it runs on; left
/// out or null, it answers.
#[test]
fn the_command_answers_only_on_the_revision_its_caller_read() {
    let (mut st, model) = store();
    add(&mut st, model, PACKAGE, 0x10);
    let mut s = over(st);
    let at = s.token();
    let none_yet = Ok(json!({ "package": id(0x10), "state": "no_diagram_yet", "binding": null, "diagram": null }));
    let token = |r: Revision| json!({ "history": r.history, "revision": r.revision });
    assert_eq!(get(&mut s, &id(0x10), json!({ "expected_revision": token(at) })), none_yet);
    assert_eq!(get(&mut s, &id(0x10), json!({ "expected_revision": null })), none_yet);
    let (later, other) = (Revision { revision: at.revision + 1, ..at }, Revision { history: at.history + 1, ..at });
    for (given, expected) in [(token(later), Some(later)), (token(other), Some(other)), (json!({ "history": at.history }), None)] {
        let Err((key, reason, _)) = get(&mut s, &id(0x10), json!({ "expected_revision": given })) else { panic!("{given} answered") };
        assert_eq!((key.as_deref(), reason), (Some("expected_revision"), Some(Reason::StaleRevision { expected, current: at })), "{given}");
    }
}

/// The command is a query: never journaled nor undoable, which the MCP server offers as a
/// read-only tool (`readOnlyHint`), its `expected_revision` an object it may leave out.
#[test]
fn the_command_is_offered_as_a_read_only_tool() {
    let (st, _) = store();
    let s = over(st);
    assert_eq!(s.registry().is_query(GET), Some(true));
    let info = s.registry().infos(&s.facts()).into_iter().find(|i| i.id == GET).unwrap();
    assert_eq!((info.journal, info.undoable, info.enabled), (false, false, true));
    let revision = info.params.iter().find(|p| p.key == "expected_revision").unwrap();
    assert_eq!((revision.kind, revision.optional), ("object", true));
}
