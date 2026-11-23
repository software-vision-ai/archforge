//! A package's default diagram through its binding (AR-575): each state built by hand in a store
//! under fixed ids: bound, none yet, missing, duplicate, a dead, a wrong-kind and a foreign target,
//! a profile, and every problem of a store in a stable order, which UML reports as a project opens
//! in the words every notation uses (AR-782).
//! Each state read through `uml.package.default.get`, on the revision its caller read, as a
//! read-only tool that leaves no undo step (AR-576). A diagram a binding names stays: removing it,
//! or an element of another one's, is refused typed with nothing changed (AR-578), an association
//! class's that goes with its end among them. A package's first class diagram is bound as its
//! default as it is made, and only that one (AR-579). `uml.package.default.set` makes the binding,
//! keeps its id when it names another diagram, changes nothing for the same one, mends several into
//! one, and refuses anything but one of the package's own class diagrams on the revision read
//! (AR-580). A nested package handing a structure diagram up to a package with no sound default,
//! the root among them, is refused with nothing changed; one handing up none goes (AR-673).
//! Below a package a profile holds, read back from rows, the handoff passes the profile when it
//! stays and judges nothing when it or that package goes (AR-745).
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DEFAULT_DIAGRAM_BINDING, DIAGRAM, DiagramObj, Notation, OpenProblem, ProjectView};
use archforge_engine::{BindingFault, EngineError, Options, Reason, Revision, Session};
use archforge_mof::rows::{self, Cell};
use archforge_mof::{ClassId, CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::defaults::{BindingProblem, DefaultDiagram, Diagnosis, problems, resolve};
use archforge_uml::{ALONE, PACKAGE, PROFILE, STRUCTURE_DIAGRAM, UmlNotation, registry};
use serde_json::{Value as Json, json};

type S = Session<Store, ProjectView>;

/// The query this file's command tests call.
const GET: &str = "uml.package.default.get";

/// The query entry asks for the diagram a package opens to (AR-582).
const ENTRY: &str = "uml.package.diagram";

/// The kind of a diagram of another notation, as an ER diagram is: a word of no notation, since UML
/// depends on none and `archforge_er::DIAGRAM_KIND` is out of its reach.
const OTHER_DIAGRAM: &str = "other.kind";

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
    diagram(&mut st, model, 0x31, OTHER_DIAGRAM, schema_only);
    assert!(st.validate_all().is_empty(), "{:?}", st.validate_all());
    assert_eq!(resolve(&st, sales), Ok(DefaultDiagram::Bound(detail)), "not the first-found {first:?}");
    assert_eq!(resolve(&st, empty), Ok(DefaultDiagram::NoDiagramYet));
    assert_eq!(resolve(&st, schema_only), Ok(DefaultDiagram::NoDiagramYet), "a diagram of another kind is no structure diagram");
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

/// A binding naming a diagram of another notation is of the wrong kind, and one naming another
/// package's structure diagram of the wrong owner.
#[test]
fn a_target_of_another_kind_or_another_package_is_refused() {
    let (mut st, model) = store();
    let (sales, billing) = (add(&mut st, model, PACKAGE, 0x10), add(&mut st, model, PACKAGE, 0x20));
    let schema = diagram(&mut st, model, 0x11, OTHER_DIAGRAM, sales);
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

/// AR-782: as a project opens, UML reports each binding problem in the words every notation uses,
/// in the order of `problems`: the package it concerns, a code of its own, the bindings and the
/// diagram it names, and its English words. A store with none reports none.
#[test]
fn uml_reports_each_binding_problem_in_the_words_every_notation_uses() {
    let (mut st, model) = store();
    assert_eq!(UmlNotation.open_problems(&st), []);
    let missing = add(&mut st, model, PACKAGE, 0x10);
    diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, missing);
    let duplicate = add(&mut st, model, PACKAGE, 0x20);
    let twice = diagram(&mut st, model, 0x21, STRUCTURE_DIAGRAM, duplicate);
    // Made in the other order from their ids.
    let (later, earlier) = (bind(&mut st, model, 0x23, duplicate, twice), bind(&mut st, model, 0x22, duplicate, twice));
    let dead = add(&mut st, model, PACKAGE, 0x30);
    let gone = diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, dead);
    let emptied = bind(&mut st, model, 0x32, dead, gone);
    st.set(emptied, DEFAULT_DIAGRAM, Value::Null).unwrap();
    let wrong_kind = add(&mut st, model, PACKAGE, 0x40);
    let other = diagram(&mut st, model, 0x41, OTHER_DIAGRAM, wrong_kind);
    let to_other = bind(&mut st, model, 0x42, wrong_kind, other);
    let wrong_owner = add(&mut st, model, PACKAGE, 0x50);
    let theirs = diagram(&mut st, model, 0x51, STRUCTURE_DIAGRAM, missing);
    let to_theirs = bind(&mut st, model, 0x52, wrong_owner, theirs);
    let open = |owner, code, objects: Vec<Obj>, problem: BindingProblem| OpenProblem {
        notation: "UML",
        owner: Some(owner),
        code,
        objects,
        message: problem.to_string(),
    };
    let expected = [
        open(missing, "default_diagram_missing", vec![], BindingProblem::Missing),
        open(duplicate, "default_diagram_duplicate", vec![earlier, later], BindingProblem::Duplicate(Vec::new())),
        open(dead, "default_diagram_dead_target", vec![emptied], BindingProblem::DeadTarget { binding: emptied }),
        open(wrong_kind, "default_diagram_wrong_kind", vec![to_other, other], BindingProblem::WrongKind { binding: to_other, diagram: other }),
        open(wrong_owner, "default_diagram_wrong_owner", vec![to_theirs, theirs], BindingProblem::WrongOwner { binding: to_theirs, diagram: theirs }),
    ];
    assert_eq!(UmlNotation.open_problems(&st), expected);
    assert_eq!(problems(&st).len(), expected.len(), "one for each of UML's own");
}

/// A session over `st`, which nothing has changed yet.
fn over(st: Store) -> S {
    Session::with_view(registry().unwrap(), st, 1, Options::default(), ProjectView::new(ALONE))
}

/// The id `[n; 16]` as a command takes it.
fn id(n: u8) -> String {
    Uuid([n; 16]).to_string()
}

/// `uml.package.default.get` of `package` given `p` besides, checked as [`ask`] checks it.
fn get(s: &mut S, package: &str, p: Json) -> Result<Json, (Option<String>, Option<Reason>, String)> {
    ask(s, GET, package, p)
}

/// The query `cmd` of `package` given `p` besides, checked to have changed nothing and left no undo
/// step: its answer, or the refusal's parameter, reason and English.
fn ask(s: &mut S, cmd: &str, package: &str, mut p: Json) -> Result<Json, (Option<String>, Option<Reason>, String)> {
    let at = s.token();
    p["package"] = json!(package);
    let out = s.execute(cmd, &p);
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
/// diagram found. `uml.package.diagram`, which entry asks, gives the same bound diagram and refuses
/// the same way, a package with no diagram yet as `NoDiagramYet`, and makes nothing (AR-582).
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
    // Six, made in the other order from their ids: the refusal names five and counts the sixth.
    for n in (0x52..=0x57).rev() {
        bind(&mut st, model, n, twice, both);
    }
    let schema_bound = add(&mut st, model, PACKAGE, 0x60);
    let schema = diagram(&mut st, model, 0x61, OTHER_DIAGRAM, schema_bound);
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
    let own = "choose one of its own structure diagrams as its default with uml.package.default.set";
    for (package, fault, named, mend) in [
        (0x40, BindingFault::Missing, format!("(package {})", id(0x40)), own),
        (
            0x50,
            BindingFault::Duplicate,
            format!("bindings {} and 1 more)", (0x52..=0x56).map(id).collect::<Vec<_>>().join(", ")),
            "choose which of its structure diagrams stays its default with uml.package.default.set, which removes the other bindings",
        ),
        (0x60, BindingFault::WrongKind, format!("binding {}, diagram {})", id(0x62), id(0x61)), own),
        (0x70, BindingFault::WrongOwner, format!("binding {}, diagram {})", id(0x72), id(0x12)), own),
        (0x80, BindingFault::DeadTarget, format!("binding {})", id(0x82)), own),
    ] {
        let Err((key, reason, said)) = get(&mut s, &id(package), json!({})) else { panic!("{fault:?} answered") };
        assert_eq!((key.as_deref(), reason.clone()), (Some("package"), Some(Reason::InvalidBinding(fault))), "{said}");
        assert!(said.contains(&named) && said.ends_with(mend), "{fault:?}: {said}");
        assert_eq!(ask(&mut s, ENTRY, &id(package), json!({})), Err((key, reason, said)), "{fault:?}: entry refused alike");
    }
    assert_eq!(ask(&mut s, ENTRY, &id(0x10), json!({})), Ok(json!({ "id": id(0x12) })), "entry opens the bound, not 0x11");
    let none_yet = format!("the package {} has no class diagram yet, so none opens; make one for it with diagram.add first", id(0x20));
    let no_diagram = Reason::NoDiagramYet { package: id(0x20).into() };
    assert_eq!(ask(&mut s, ENTRY, &id(0x20), json!({})), Err((Some("package".into()), Some(no_diagram), none_yet)));
    assert!(ask(&mut s, ENTRY, &id(0x30), json!({})).is_err(), "a profile opens no default");
    assert_eq!(s.registry().is_query(ENTRY), Some(true), "offered as a read-only tool");
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

/// While a gesture shows a preview, the command runs on the preview's own revision: a token read
/// before it is refused as stale, though the preview left the session's revision as it was.
#[test]
fn a_token_read_before_a_preview_is_stale_during_it() {
    let (mut st, model) = store();
    add(&mut st, model, PACKAGE, 0x10);
    let mut s = over(st);
    let at = s.token();
    let before = json!({ "package": id(0x10), "expected_revision": { "history": at.history, "revision": at.revision } });
    s.begin_interaction("diagram.add").unwrap();
    s.preview(&json!({ "kind": STRUCTURE_DIAGRAM, "element": id(0x10) })).unwrap();
    assert_eq!(s.revision(), at.revision);
    let Err(EngineError::Failed { error, .. }) = s.execute(GET, &before) else { panic!("the token read before the preview matched it") };
    assert_eq!(error.reason, Some(Reason::StaleRevision { expected: Some(at), current: s.token() }));
    s.cancel().unwrap();
    assert!(s.execute(GET, &before).is_ok(), "after a cancel, it matches again");
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

/// The tables and junctions of the core, the diagrams and UML.
fn tables() -> (Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    let tables = [archforge_uml::TABLES, archforge_diagram::TABLES, archforge_core::TABLES].concat();
    (tables, [archforge_core::JUNCTIONS, archforge_diagram::JUNCTIONS, archforge_uml::JUNCTIONS].concat())
}

/// The store's rows, to compare a store before and after a command.
fn rows_of(st: &Store) -> rows::Rows {
    let (tables, junctions) = tables();
    rows::snapshot(st, &tables, &junctions).unwrap()
}

/// `cmd` given `p`, checked to be refused with the store as it was and no undo step: the refusal's
/// parameter, reason and English.
fn refused(s: &mut S, cmd: &str, p: Json) -> (Option<String>, Option<Reason>, String) {
    let (at, before) = (s.token(), rows_of(s.doc()));
    let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} was not refused") };
    assert_eq!((s.token(), s.can_undo()), (at, false), "{cmd} {p} left no undo step");
    assert!(rows_of(s.doc()) == before, "{cmd} {p} changed nothing");
    (error.key, error.reason, error.message)
}

/// Sales, bound to the second of its two structure diagrams: (store, model, sales, its first
/// diagram, its default, the binding).
fn bound_sales() -> (Store, Obj, Obj, Obj, Obj, Obj) {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    st.set(sales, archforge_uml::named_element::NAME, Value::str("Sales")).unwrap();
    let first = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, sales);
    let detail = diagram(&mut st, model, 0x12, STRUCTURE_DIAGRAM, sales);
    let binding = bind(&mut st, model, 0x13, sales, detail);
    (st, model, sales, first, detail, binding)
}

/// A package's default diagram is not removed without a replacement: `diagram.remove` is refused
/// about the diagram, typed with the package's name and naming the parameters it needs, before
/// anything changes; its other diagram goes, with no revision, and leaves the binding as it was
/// (AR-578, AR-640).
#[test]
fn removing_a_default_diagram_is_refused_and_another_leaves_the_binding() {
    let (st, _, sales, first, detail, binding) = bound_sales();
    let mut s = over(st);
    let (key, reason, said) = refused(&mut s, "diagram.remove", json!({ "diagram": id(0x12) }));
    let default_of = Reason::DefaultDiagramOf { package: "Sales".into(), fault: None };
    assert_eq!((key.as_deref(), reason), (Some("diagram"), Some(default_of)), "{said}");
    let named =
        "name another of its diagrams as replacement_diagram, or a new one as create_replacement, with the expected_revision the project state gives";
    assert_eq!(said, format!("the diagram {} is the default diagram of Sales; {named}", id(0x12)));
    let answer = s.execute("diagram.remove", &json!({ "diagram": id(0x11) })).unwrap().value;
    let plain =
        json!({ "removed": answer["removed"], "diagram": id(0x11), "replacement": null, "binding": null, "bindings_removed": [], "created": false });
    assert_eq!(answer, plain);
    let st = s.doc();
    assert!(!st.is_alive(first) && st.is_alive(detail));
    assert_eq!(archforge_diagram::defaults::bindings_of(st, sales), [binding]);
    assert_eq!(resolve(st, sales), Ok(DefaultDiagram::Bound(detail)));
}

/// A binding of Billing that names a diagram of Sales (WrongOwner) keeps that diagram: removing
/// it, with a replacement too (AR-640), or Sales with it, is refused typed with the binding's owner and its fault, never with the
/// store's own error. A package nested in Sales whose diagram Billing names goes, as it hands that
/// diagram up; removing Billing takes its bindings along and leaves Sales's diagrams (AR-578).
#[test]
fn a_diagram_another_packages_binding_names_keeps_it_and_its_package() {
    let (mut st, model, sales, _, detail, _) = bound_sales();
    let billing = add(&mut st, model, PACKAGE, 0x20);
    st.set(billing, archforge_uml::named_element::NAME, Value::str("Billing")).unwrap();
    let foreign = bind(&mut st, model, 0x21, billing, detail);
    let archive = st.create_with_uuid(PACKAGE, Uuid([0x30; 16])).unwrap();
    st.push(sales, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(archive)).unwrap();
    let handed_up = diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, archive);
    bind(&mut st, model, 0x32, billing, handed_up);
    let mut s = over(st);
    let wrong = Some(Reason::DefaultDiagramOf { package: "Billing".into(), fault: Some(BindingFault::WrongOwner) });
    let (key, reason, said) = refused(&mut s, "diagram.remove", json!({ "diagram": id(0x12) }));
    assert_eq!((key.as_deref(), &reason), (Some("diagram"), &wrong), "{said}");
    let replaced = json!({ "diagram": id(0x12), "replacement_diagram": id(0x11), "expected_revision": token(&s) });
    assert_eq!(refused(&mut s, "diagram.remove", replaced).1, wrong, "a replacement does not mend another's binding");
    let (key, reason, said) = refused(&mut s, "element.remove", json!({ "ids": [id(0x10)] }));
    assert_eq!((key.as_deref(), &reason), (Some("ids"), &wrong), "{said}");
    assert!(said.contains(&format!("binding {} of Billing names the diagram {}", id(0x21), id(0x12))), "{said}");
    s.execute("element.remove", &json!({ "ids": [id(0x30)] })).unwrap();
    assert_eq!(s.doc().get(handed_up, archforge_diagram::diagram::ELEMENT).unwrap(), Value::Ref(sales), "handed up");
    s.execute("element.remove", &json!({ "ids": [id(0x20)] })).unwrap();
    let st = s.doc();
    assert!(!st.is_alive(foreign) && st.is_alive(detail) && st.is_alive(handed_up) && st.is_alive(sales));
    assert_eq!(resolve(st, sales), Ok(DefaultDiagram::Bound(detail)));
}

/// Billing, whose binding names a diagram of Sales, removed together with Sales goes, its binding
/// with it, whichever of the two is selected first; one undo step brings everything back. Sales
/// alone is still refused while Billing stays (AR-578).
#[test]
fn a_package_and_the_one_whose_diagram_its_binding_names_go_together_in_either_order() {
    for order in [[0x20, 0x10], [0x10, 0x20]] {
        let (mut st, model, _, _, detail, _) = bound_sales();
        let billing = add(&mut st, model, PACKAGE, 0x20);
        bind(&mut st, model, 0x21, billing, detail);
        let mut s = over(st);
        let before = rows_of(s.doc());
        let wrong = Some(Reason::DefaultDiagramOf { package: id(0x20).into(), fault: Some(BindingFault::WrongOwner) });
        assert_eq!(refused(&mut s, "element.remove", json!({ "ids": [id(0x10)] })).1, wrong, "{order:x?}");
        s.execute("element.remove", &json!({ "ids": order.map(id) })).unwrap();
        let st = s.doc();
        assert!(!st.is_alive(billing) && !st.is_alive(detail), "{order:x?}");
        assert_eq!((archforge_diagram::defaults::bindings(st), problems(st)), (vec![], vec![]), "{order:x?}");
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(rows_of(s.doc()) == before, "{order:x?}: one undo step brings all back");
    }
}

/// The revision the session is at, as `expected_revision` takes it.
fn token(s: &S) -> Json {
    let at = s.token();
    json!({ "history": at.history, "revision": at.revision })
}

/// A default removed with another diagram of its package named as its replacement goes in one undo
/// step: the binding keeps its id and names the replacement, whose boxes stay where they were; the
/// classes the removed diagram showed stay in the model. Undo brings the diagram, its box and the
/// old default back; Redo repeats it exactly (AR-640).
#[test]
fn a_default_removed_with_a_replacement_retargets_its_binding_in_one_undo_step() {
    let (st, _, sales, first, detail, binding) = bound_sales();
    let mut s = over(st);
    let order = made(&mut s, "uml.class.add", json!({ "package": id(0x10), "name": "Order" }));
    let item = made(&mut s, "uml.class.add", json!({ "package": id(0x10), "name": "Item" }));
    s.execute("diagram.place", &json!({ "diagram": id(0x11), "element": order, "x": 40, "y": 60 })).unwrap();
    s.execute("diagram.place", &json!({ "diagram": id(0x12), "element": item })).unwrap();
    let shown = |s: &mut S| s.execute("diagram.get", &json!({ "diagram": id(0x11) })).unwrap().value;
    let (before, layout, alive) = (rows_of(s.doc()), shown(&mut s), s.doc().alive().count());
    let p = json!({ "diagram": id(0x12), "replacement_diagram": id(0x11), "expected_revision": token(&s) });
    let answer = s.execute("diagram.remove", &p).unwrap().value;
    assert_eq!(answer["removed"], json!(alive - s.doc().alive().count()), "every object that went is counted");
    let (removed, replacement) = (answer["removed"].clone(), id(0x11));
    let expected =
        json!({ "removed": removed, "diagram": id(0x12), "replacement": replacement, "binding": id(0x13), "bindings_removed": [], "created": false });
    assert_eq!(answer, expected);
    let st = s.doc();
    assert!(!st.is_alive(detail) && st.is_alive(at(&s, &item)), "the class stays in the model");
    assert_eq!((archforge_diagram::defaults::bindings_of(st, sales), resolve(st, sales)), (vec![binding], Ok(DefaultDiagram::Bound(first))));
    assert_eq!(shown(&mut s), layout, "the replacement's boxes as they were");
    let after = rows_of(s.doc());
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == before, "one undo step brings the diagram, its box and the old default back");
    s.execute("edit.redo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == after, "redo repeats it exactly");
}

/// A package with several bindings, the one naming the default removed not the first by id, keeps
/// the first by id, now naming the replacement, and loses the others in the same undo step, each
/// named in the answer, as `uml.package.default.set` mends them (AR-580, AR-640). One of them the
/// end of a trace link goes with its link, and the count of objects removed takes in the bindings
/// and the link.
#[test]
fn a_default_removed_with_a_replacement_mends_duplicate_bindings_as_choosing_one_does() {
    let (mut st, model, sales, first, detail, binding) = bound_sales();
    let kept = bind(&mut st, model, 0x05, sales, first);
    let unlinked = bind(&mut st, model, 0x14, sales, first);
    let trace = archforge_core::link(&mut st, model, archforge_core::LinkKind::Trace, binding, sales, None).unwrap();
    let mut s = over(st);
    let (before, alive) = (rows_of(s.doc()), s.doc().alive().count());
    let p = json!({ "diagram": id(0x12), "replacement_diagram": id(0x11), "expected_revision": token(&s) });
    let answer = s.execute("diagram.remove", &p).unwrap().value;
    assert_eq!((&answer["binding"], &answer["bindings_removed"]), (&json!(id(0x05)), &json!([id(0x13), id(0x14)])), "{answer}");
    assert_eq!(answer["removed"], json!(alive - s.doc().alive().count()), "the bindings and the link counted");
    let st = s.doc();
    assert!(!st.is_alive(detail) && !st.is_alive(binding) && !st.is_alive(unlinked) && !st.is_alive(trace));
    assert_eq!((archforge_diagram::defaults::bindings_of(st, sales), resolve(st, sales)), (vec![kept], Ok(DefaultDiagram::Bound(first))));
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == before, "one undo step brings both bindings back");
}

/// Every replacement that cannot take the default's place is refused typed, about the parameter
/// at fault, with the store and its history as they were: none, the diagram itself, an id of
/// nothing, another package's diagram, a diagram of another kind, one named for a diagram that is
/// no default, and a revision absent with a replacement or stale on a plain removal (AR-640).
#[test]
fn a_replacement_that_cannot_take_the_defaults_place_is_refused_with_nothing_changed() {
    let (mut st, model, sales, ..) = bound_sales();
    let billing = add(&mut st, model, PACKAGE, 0x20);
    diagram(&mut st, model, 0x21, STRUCTURE_DIAGRAM, billing);
    diagram(&mut st, model, 0x22, OTHER_DIAGRAM, sales);
    let mut s = over(st);
    let (at, token) = (s.token(), token(&s));
    let stale = Revision { revision: at.revision + 1, ..at };
    let not_own = |d: u8| Reason::NotReplacement { diagram: id(d).into(), package: "Sales".into() };
    let stale_given = json!({ "history": stale.history, "revision": stale.revision });
    let (r, e) = ("replacement_diagram", "expected_revision");
    let cases = [
        (0x12, String::new(), Json::Null, "diagram", Reason::DefaultDiagramOf { package: "Sales".into(), fault: None }),
        (0x12, id(0x12), token.clone(), r, Reason::ReplacementIsRemoved),
        (0x12, id(0x7f), token.clone(), r, Reason::NoElement { id: id(0x7f) }),
        (0x12, id(0x21), token.clone(), r, not_own(0x21)),
        (0x12, id(0x22), token.clone(), r, not_own(0x22)),
        (0x11, id(0x12), token.clone(), r, Reason::ReplacementNotNeeded { diagram: id(0x11).into() }),
        (0x12, id(0x11), Json::Null, e, Reason::StaleRevision { expected: None, current: at }),
        (0x11, String::new(), stale_given, e, Reason::StaleRevision { expected: Some(stale), current: at }),
    ];
    for (removed, replacement, revision, key, reason) in cases {
        let p = json!({ "diagram": id(removed), "replacement_diagram": replacement, "expected_revision": revision });
        let (k, why, said) = refused(&mut s, "diagram.remove", p.clone());
        assert_eq!((k.as_deref(), why), (Some(key), Some(reason)), "{p}: {said}");
    }
}

/// A default removed with a `create_replacement` goes in one undo step with a new empty diagram of
/// its kind and package, made in the Model beside it and named as asked, which the binding, keeping
/// its id, now names; the package's other diagram is left as it was. Undo takes the new diagram away
/// and brings the old default back, never the diagram that was there before; Redo makes it again
/// under the same id (AR-641).
#[test]
fn a_default_removed_with_a_new_replacement_makes_binds_and_removes_in_one_undo_step() {
    let (st, model, sales, first, detail, binding) = bound_sales();
    let mut s = over(st);
    let item = made(&mut s, "uml.class.add", json!({ "package": id(0x10), "name": "Item" }));
    s.execute("diagram.place", &json!({ "diagram": id(0x12), "element": item })).unwrap();
    let (before, alive) = (rows_of(s.doc()), s.doc().alive().count());
    let p = json!({ "diagram": id(0x12), "create_replacement": { "name": "Sales map" }, "expected_revision": token(&s) });
    let answer = s.execute("diagram.remove", &p).unwrap().value;
    let made = answer["replacement"].as_str().unwrap_or_else(|| panic!("{answer}")).to_string();
    let (removed, new) = (json!(alive + 2 - s.doc().alive().count()), at(&s, &made));
    let expected =
        json!({ "removed": removed, "diagram": id(0x12), "replacement": made, "binding": id(0x13), "bindings_removed": [], "created": true });
    assert_eq!(answer, expected, "the diagram and its style made are not counted as removed");
    let st = s.doc();
    assert!(!st.is_alive(detail) && st.is_alive(first) && st.is_alive(at(&s, &item)), "the class and the other diagram stay");
    assert_eq!((archforge_diagram::defaults::bindings_of(st, sales), resolve(st, sales)), (vec![binding], Ok(DefaultDiagram::Bound(new))));
    assert!(st.list(model, archforge_core::model::MEMBER).unwrap().contains(&Value::Ref(new)), "made in the Model");
    let shown = s.execute("diagram.get", &json!({ "diagram": made })).unwrap().value;
    let head = (&shown["kind"], &shown["name"], &shown["element"], &shown["nodes"]);
    assert_eq!(head, (&json!(STRUCTURE_DIAGRAM), &json!("Sales map"), &json!(id(0x10)), &json!([])), "an empty class diagram of Sales");
    let after = rows_of(s.doc());
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == before, "one undo step takes the new diagram away and brings the old default back");
    assert!(!s.doc().is_alive(new) && s.doc().is_alive(first), "the diagram there before stays");
    s.execute("edit.redo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == after, "redo makes it again, under the same id");
    assert_eq!(resolve(s.doc(), sales), Ok(DefaultDiagram::Bound(at(&s, &made))));
}

/// A new replacement is refused typed, about the parameter at fault, with no diagram made and the
/// store and its history as they were: given with an existing one too, neither given, a name blank,
/// left out, no text, holding a control character or past the limit, a key other than the name, one for a
/// diagram that is no default, and a revision left out (AR-641).
#[test]
fn a_new_replacement_given_twice_misnamed_or_not_needed_is_refused_with_nothing_changed() {
    let (st, ..) = bound_sales();
    let mut s = over(st);
    let (at, token) = (s.token(), token(&s));
    let (c, max) = ("create_replacement", archforge_diagram::rules::MAX_NAME);
    let named = |name: &str| json!({ "name": name });
    let unknown = Reason::UnknownKey { key: "title".into(), takes: &["name"] };
    let cases = [
        (0x12, id(0x11), named("Sales map"), token.clone(), c, Reason::OneReplacement),
        (0x12, String::new(), Json::Null, token.clone(), "diagram", Reason::DefaultDiagramOf { package: "Sales".into(), fault: None }),
        (0x12, String::new(), named(""), token.clone(), c, Reason::NameEmpty),
        (0x12, String::new(), named("  "), token.clone(), c, Reason::NameEmpty),
        (0x12, String::new(), json!({}), token.clone(), c, Reason::NameEmpty),
        (0x12, String::new(), json!({ "name": 5 }), token.clone(), c, Reason::NameNotText),
        (0x12, String::new(), named("Sales\u{7}map"), token.clone(), c, Reason::ControlCharacter),
        (0x12, String::new(), named(&"m".repeat(max + 1)), token.clone(), c, Reason::TooLong { max }),
        (0x12, String::new(), json!({ "name": "Sales map", "title": "x" }), token.clone(), c, unknown),
        (0x11, String::new(), named("Sales map"), token.clone(), c, Reason::ReplacementNotNeeded { diagram: id(0x11).into() }),
        (0x12, String::new(), named("Sales map"), Json::Null, "expected_revision", Reason::StaleRevision { expected: None, current: at }),
    ];
    for (removed, replacement, create, revision, key, reason) in cases {
        let p = json!({ "diagram": id(removed), "replacement_diagram": replacement, "create_replacement": create, "expected_revision": revision });
        let (k, why, said) = refused(&mut s, "diagram.remove", p.clone());
        assert_eq!((k.as_deref(), why), (Some(key), Some(reason)), "{p}: {said}");
    }
}

/// An association class goes with a class at one of its ends, and its diagram with it, so a
/// binding of a package that stays naming that diagram refuses the removal before anything changes,
/// typed, the store and its history as they were (AR-578).
#[test]
fn a_binding_keeps_the_diagram_of_an_association_class_going_with_its_end() {
    let mut s = over(store().0);
    let made = |s: &mut S, cmd: &str, p: Json| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap().to_string();
    let sales = made(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let order = made(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let item = made(&mut s, "uml.class.add", json!({ "package": sales, "name": "Item" }));
    let line = made(&mut s, "uml.association-class.add", json!({ "from": order, "to": item, "name": "Line" }));
    // Read back into a store of its own, to give the association class a diagram and bind it by hand.
    let (tables, junctions) = tables();
    let mut st = bare();
    rows::restore(&mut st, &rows_of(s.doc()), &tables, &junctions).unwrap();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let billing = add(&mut st, model, PACKAGE, 0x20);
    let of_line = st.by_uuid(Uuid::parse(&line).unwrap()).unwrap();
    let kept = diagram(&mut st, model, 0x21, STRUCTURE_DIAGRAM, of_line);
    bind(&mut st, model, 0x22, billing, kept);
    let mut s = over(st);
    let (key, reason, said) = refused(&mut s, "element.remove", json!({ "ids": [order] }));
    let wrong = Reason::DefaultDiagramOf { package: id(0x20).into(), fault: Some(BindingFault::WrongOwner) };
    assert_eq!((key.as_deref(), reason), (Some("ids"), Some(wrong)), "{said}");
}

/// The command that chooses a package's default diagram.
const SET: &str = "uml.package.default.set";

/// `uml.package.default.set` of the diagram `[diagram; 16]` for the package `[package; 16]`, on
/// the revision the session is at: its answer.
fn set(s: &mut S, package: u8, diagram: u8) -> Json {
    let at = s.token();
    let token = json!({ "history": at.history, "revision": at.revision });
    s.execute(SET, &json!({ "package": id(package), "diagram": id(diagram), "expected_revision": token })).unwrap().value
}

/// Sales with two structure diagrams and no binding yet: (store, model, sales, overview, detail).
fn unbound_sales() -> (Store, Obj, Obj, Obj, Obj) {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    st.set(sales, archforge_uml::named_element::NAME, Value::str("Sales")).unwrap();
    let overview = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, sales);
    let detail = diagram(&mut st, model, 0x12, STRUCTURE_DIAGRAM, sales);
    (st, model, sales, overview, detail)
}

/// Choosing makes the one binding, a member of the project's Model; choosing another keeps its id;
/// choosing the same again changes nothing and leaves no undo step. Undo and Redo step back and
/// forth exactly, and the rows read back to the same default (AR-580).
#[test]
fn choosing_a_default_makes_its_binding_then_retargets_it() {
    let (st, model, sales, overview, detail) = unbound_sales();
    let mut s = over(st);
    let none = rows_of(s.doc());
    assert_eq!(resolve(s.doc(), sales), Err(BindingProblem::Missing));
    let made = set(&mut s, 0x10, 0x11);
    let binding = Uuid::parse(made["binding"].as_str().unwrap()).unwrap();
    assert_eq!(made, json!({ "package": id(0x10), "binding": binding.to_string(), "diagram": id(0x11), "removed": [], "links_removed": [] }));
    let st = s.doc();
    let b = st.by_uuid(binding).unwrap();
    assert_eq!((resolve(st, sales), st.container(b).unwrap()), (Ok(DefaultDiagram::Bound(overview)), Some((model, archforge_core::model::MEMBER))));
    assert!(st.validate_all().is_empty(), "{:?}", st.validate_all());
    let first = rows_of(st);
    assert_eq!(set(&mut s, 0x10, 0x12)["binding"], json!(binding.to_string()), "the binding keeps its id");
    assert_eq!(resolve(s.doc(), sales), Ok(DefaultDiagram::Bound(detail)));
    let second = rows_of(s.doc());
    let at = s.token();
    assert_eq!(set(&mut s, 0x10, 0x12)["binding"], json!(binding.to_string()));
    assert!(s.token() == at && rows_of(s.doc()) == second, "the same default again changed nothing and left no undo step");
    for (step, expected) in [("edit.undo", &first), ("edit.undo", &none), ("edit.redo", &first), ("edit.redo", &second)] {
        s.execute(step, &json!({})).unwrap();
        assert!(rows_of(s.doc()) == *expected, "{step} gives the rows exactly");
    }
    let (tables, junctions) = tables();
    let mut back = bare();
    rows::restore(&mut back, &second, &tables, &junctions).unwrap();
    let (package, target) = (back.by_uuid(Uuid([0x10; 16])).unwrap(), back.by_uuid(Uuid([0x12; 16])).unwrap());
    assert_eq!(
        (resolve(&back, package), archforge_diagram::defaults::bindings_of(&back, package)),
        (Ok(DefaultDiagram::Bound(target)), vec![back.by_uuid(binding).unwrap()])
    );
}

/// A package with more than one binding is mended by the choice: the first binding by id names the
/// chosen diagram, though a later one named it already, and the others go, all as one undo step.
#[test]
fn choosing_a_default_of_a_package_with_several_keeps_one_binding() {
    let (mut st, model, sales, overview, detail) = unbound_sales();
    let later = bind(&mut st, model, 0x1c, sales, detail);
    let middle = bind(&mut st, model, 0x1b, sales, detail);
    let first = bind(&mut st, model, 0x1a, sales, overview);
    let mut s = over(st);
    let before = rows_of(s.doc());
    let made = set(&mut s, 0x10, 0x12);
    assert_eq!(made, json!({ "package": id(0x10), "binding": id(0x1a), "diagram": id(0x12), "removed": [id(0x1b), id(0x1c)], "links_removed": [] }));
    let st = s.doc();
    assert!(st.is_alive(first) && !st.is_alive(middle) && !st.is_alive(later));
    assert_eq!((resolve(st, sales), problems(st)), (Ok(DefaultDiagram::Bound(detail)), vec![]));
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == before, "one undo step brings all three back");
}

/// Each choice that is not one of the package's own live class diagrams, of no package, or on a
/// revision the caller did not read is refused, typed, with the store and its history as they were;
/// a read-only session, as a version viewed, refuses the command before it runs (AR-580).
#[test]
fn a_default_that_is_not_the_packages_own_class_diagram_is_refused() {
    let (mut st, model, sales, ..) = unbound_sales();
    let schema = diagram(&mut st, model, 0x13, OTHER_DIAGRAM, sales);
    DiagramObj::set_name(&mut st, schema, Some("Schema")).unwrap();
    let billing = add(&mut st, model, PACKAGE, 0x20);
    st.set(billing, archforge_uml::named_element::NAME, Value::str("Billing")).unwrap();
    let theirs = diagram(&mut st, model, 0x21, STRUCTURE_DIAGRAM, billing);
    DiagramObj::set_name(&mut st, theirs, Some("Invoices")).unwrap();
    add(&mut st, model, PROFILE, 0x30);
    let mut s = over(st);
    let at = s.token();
    let token = json!({ "history": at.history, "revision": at.revision });
    let class_diagram = |name: &str| Reason::NotA { kind: "class diagram", name: name.into() };
    let stale = Revision { revision: at.revision + 1, ..at };
    const EXPECTED: &str = "expected_revision";
    for (package, diagram, revision, key, reason) in [
        (id(0x10), id(0x13), token.clone(), "diagram", class_diagram("Schema")),
        (id(0x10), id(0x10), token.clone(), "diagram", class_diagram("Sales")),
        (id(0x10), id(0x21), token.clone(), "diagram", Reason::NotOwnDiagram { diagram: "Invoices".into(), package: "Sales".into() }),
        (id(0x10), id(0xee), token.clone(), "diagram", Reason::NoElement { id: id(0xee) }),
        (id(0x10), String::new(), token.clone(), "diagram", Reason::IdEmpty { kind: "class diagram" }),
        (id(0x30), id(0x11), token.clone(), "package", Reason::NotA { kind: "package", name: id(0x30) }),
        (String::new(), id(0x11), token.clone(), "package", Reason::IdEmpty { kind: "package" }),
        (
            id(0x10),
            id(0x11),
            json!({ "history": stale.history, "revision": stale.revision }),
            EXPECTED,
            Reason::StaleRevision { expected: Some(stale), current: at },
        ),
        (id(0x10), id(0x11), Json::Null, EXPECTED, Reason::StaleRevision { expected: None, current: at }),
    ] {
        let p = json!({ "package": package, "diagram": diagram, "expected_revision": revision });
        let (said_key, said_reason, said) = refused(&mut s, SET, p);
        assert_eq!((said_key.as_deref(), said_reason.as_ref()), (Some(key), Some(&reason)), "{said}");
    }
    s.set_read_only(true).unwrap();
    let p = json!({ "package": id(0x10), "diagram": id(0x11), "expected_revision": token });
    assert!(matches!(s.execute(SET, &p), Err(EngineError::ReadOnly)));
    assert_eq!((s.token(), s.can_undo()), (at, false));
}

/// The id `cmd` given `p` answers with, as it made something.
fn made(s: &mut S, cmd: &str, p: Json) -> String {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}")).value["id"].as_str().unwrap().to_string()
}

/// The object the id `id` names in the session's store.
fn at(s: &S, id: &str) -> Obj {
    s.doc().by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

/// A package's first class diagram is its default from the start: `diagram.add` makes the binding
/// with it, a member of the project's Model naming the package and the diagram, which
/// `uml.package.default.get` reads back. One undo step takes the diagram and its binding back
/// together, and redo brings both back under their ids, as do the project's rows read back (AR-579).
#[test]
fn a_packages_first_class_diagram_is_bound_as_its_default_in_the_same_step() {
    let mut s = over(store().0);
    let model = s.doc().alive().find(|o| s.doc().class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let package = made(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let before = rows_of(s.doc());
    let d = made(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": package }));
    let st = s.doc();
    let [binding] = archforge_diagram::defaults::bindings_of(st, at(&s, &package))[..] else { panic!("not one binding") };
    assert_eq!(st.container(binding).unwrap(), Some((model, archforge_core::model::MEMBER)), "a member of the Model");
    assert_eq!(resolve(st, at(&s, &package)), Ok(DefaultDiagram::Bound(at(&s, &d))));
    let binding = st.uuid(binding).unwrap().to_string();
    let answer = s.execute(GET, &json!({ "package": package })).unwrap().value;
    assert_eq!(answer, json!({ "package": package, "state": "bound", "binding": binding, "diagram": d }));
    let after = rows_of(s.doc());
    s.undo().unwrap();
    assert!(rows_of(s.doc()) == before, "one undo step takes the diagram and its binding back");
    s.redo().unwrap();
    assert!(rows_of(s.doc()) == after, "redo brings both back under their ids");
    // Saved and read back: the binding under its id, still naming both by theirs.
    let mut back = bare();
    rows::restore(&mut back, &after, &tables().0, &tables().1).unwrap();
    let again = back.by_uuid(Uuid::parse(&binding).unwrap()).expect("the binding keeps its id");
    assert_eq!(
        resolve(&back, back.by_uuid(Uuid::parse(&package).unwrap()).unwrap()),
        Ok(DefaultDiagram::Bound(back.by_uuid(Uuid::parse(&d).unwrap()).unwrap()))
    );
    assert!(back.container(again).unwrap().is_some() && rows_of(&back) == after, "a member still, the rows as written");
}

/// Only a package's first class diagram is bound: a second leaves its default as it was, a
/// profile's diagram binds nothing whether `diagram.add` or `uml.profile.diagram` makes it, a
/// package with class diagrams but no binding stays without one, its default still missing rather
/// than bound silently, and one that has a binding already gets no second (AR-579).
#[test]
fn a_second_diagram_a_profiles_or_one_of_a_package_without_a_binding_binds_nothing() {
    let mut s = over(store().0);
    let sales = made(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let first = made(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales }));
    let binding = archforge_diagram::defaults::bindings(s.doc());
    made(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": sales, "name": "Second" }));
    assert_eq!(s.execute(ENTRY, &json!({ "package": sales })).unwrap().value["id"], first.as_str(), "the first is entered");
    assert_eq!(resolve(s.doc(), at(&s, &sales)), Ok(DefaultDiagram::Bound(at(&s, &first))), "the second does not take over");
    let persistence = made(&mut s, "uml.profile.add", json!({ "name": "Persistence" }));
    made(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": persistence }));
    let audit = made(&mut s, "uml.profile.add", json!({ "name": "Audit" }));
    made(&mut s, "uml.profile.diagram", json!({ "profile": audit }));
    assert_eq!(archforge_diagram::defaults::bindings(s.doc()), binding, "Sales's binding alone");
    assert_eq!(resolve(s.doc(), at(&s, &persistence)), Ok(DefaultDiagram::DoesNotApply));
    // A package whose class diagram came without a binding, as one made before bindings existed,
    // and one with no diagram but a binding naming another's, which a second would duplicate.
    let (mut st, model) = store();
    let orders = add(&mut st, model, PACKAGE, 0x30);
    let unbound = diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, orders);
    let billing = add(&mut st, model, PACKAGE, 0x40);
    let foreign = bind(&mut st, model, 0x41, billing, unbound);
    let mut s = over(st);
    made(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": id(0x30) }));
    let Err(EngineError::Failed { error, .. }) = s.execute(ENTRY, &json!({ "package": id(0x30) })) else { panic!("entry made a diagram") };
    assert_eq!(error.reason, Some(Reason::InvalidBinding(BindingFault::Missing)), "entry refuses, binding nothing");
    made(&mut s, "diagram.add", json!({ "kind": STRUCTURE_DIAGRAM, "element": id(0x40) }));
    assert_eq!(archforge_diagram::defaults::bindings(s.doc()), [foreign], "nothing bound silently");
    assert_eq!(resolve(s.doc(), orders), Err(BindingProblem::Missing));
}

/// A package under the id `[id; 16]` nested in `parent`, named `name` unless it is empty.
fn nested(st: &mut Store, parent: Obj, id: u8, name: &str) -> Obj {
    let p = st.create_with_uuid(PACKAGE, Uuid([id; 16])).unwrap();
    st.push(parent, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(p)).unwrap();
    if !name.is_empty() {
        st.set(p, archforge_uml::named_element::NAME, Value::str(name)).unwrap();
    }
    p
}

/// A nested package handing a structure diagram up to a parent with no sound default, each fault
/// of its binding or no structure diagram yet, is refused before anything moves: typed with the
/// parent's name and fault, the bindings, the diagrams and the undo stack as they were (AR-673).
#[test]
fn a_handoff_into_a_parent_with_no_sound_default_is_refused_and_changes_nothing() {
    use BindingFault::{DeadTarget, Duplicate, Missing, WrongKind, WrongOwner};
    for fault in [None, Some(Missing), Some(Duplicate), Some(DeadTarget), Some(WrongKind), Some(WrongOwner)] {
        let (mut st, model) = store();
        let sales = add(&mut st, model, PACKAGE, 0x10);
        st.set(sales, archforge_uml::named_element::NAME, Value::str("Sales")).unwrap();
        let archive = nested(&mut st, sales, 0x30, "Archive");
        let handed = diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, archive);
        bind(&mut st, model, 0x32, archive, handed);
        let billing = add(&mut st, model, PACKAGE, 0x40);
        let theirs = diagram(&mut st, model, 0x41, STRUCTURE_DIAGRAM, billing);
        let kind = if fault == Some(WrongKind) { OTHER_DIAGRAM } else { STRUCTURE_DIAGRAM };
        let own = diagram(&mut st, model, 0x11, kind, sales);
        let target = if fault == Some(WrongOwner) { theirs } else { own };
        let bindings: &[u8] = match fault {
            None => {
                st.destroy(own).unwrap();
                &[]
            }
            Some(Missing) => &[],
            Some(Duplicate) => &[0x12, 0x13],
            Some(_) => &[0x12],
        };
        for n in bindings {
            let b = bind(&mut st, model, *n, sales, target);
            if fault == Some(DeadTarget) {
                st.set(b, DEFAULT_DIAGRAM, Value::Null).unwrap();
            }
        }
        let mut s = over(st);
        let (key, reason, said) = refused(&mut s, "element.remove", json!({ "ids": [id(0x30)] }));
        let unsound = Reason::ParentDefaultUnsound { parent: "Sales".into(), fault };
        assert_eq!((key.as_deref(), reason), (Some("ids"), Some(unsound)), "{said}");
        assert!(said.starts_with("its diagrams cannot move up: ") && said.contains("Sales"), "{fault:?}: {said}");
    }
}

/// Every user package sits under the root: one directly under a root whose default is Missing
/// keeps its diagram there, refused, until the root's default is chosen; then it goes, and its
/// diagram is one more of the root's beside the root's default, which stays (AR-673).
#[test]
fn a_package_under_a_root_with_no_default_goes_once_the_root_has_one() {
    let (mut st, model) = store();
    let root = add(&mut st, model, PACKAGE, 0x10);
    st.set(root, archforge_uml::named_element::NAME, Value::str("Model")).unwrap();
    assert_eq!(archforge_uml::commands::ensure_root(&mut st).ok(), Some(root));
    let overview = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, root);
    let sales = nested(&mut st, root, 0x20, "Sales");
    let handed = diagram(&mut st, model, 0x21, STRUCTURE_DIAGRAM, sales);
    bind(&mut st, model, 0x22, sales, handed);
    let mut s = over(st);
    let (_, reason, said) = refused(&mut s, "element.remove", json!({ "ids": [id(0x20)] }));
    let unsound = Reason::ParentDefaultUnsound { parent: "Model".into(), fault: Some(BindingFault::Missing) };
    assert_eq!(reason, Some(unsound), "{said}");
    assert!(said.ends_with("as its default with uml.package.default.set first"), "{said}");
    set(&mut s, 0x10, 0x11);
    s.execute("element.remove", &json!({ "ids": [id(0x20)] })).unwrap();
    let st = s.doc();
    assert!(!st.is_alive(sales) && st.is_alive(handed));
    assert_eq!(st.get(handed, archforge_diagram::diagram::ELEMENT).unwrap(), Value::Ref(root), "handed up");
    assert_eq!((resolve(st, root), problems(st)), (Ok(DefaultDiagram::Bound(overview)), vec![]));
}

/// A package handing up no structure diagram goes whatever its parent's default: under a parent
/// with no diagram yet, and with only a diagram of another notation under one whose default is
/// Missing, left so. Removed with its parent, a package's diagram goes to the first package above
/// that stays, judged by that one's default: refused alone, it goes with its parent into a bound
/// grandparent (AR-673).
#[test]
fn a_handoff_of_no_class_diagram_goes_and_one_past_a_parent_going_too_is_judged_above() {
    let (mut st, model) = store();
    let sales = add(&mut st, model, PACKAGE, 0x10);
    nested(&mut st, sales, 0x20, "Empty");
    let billing = add(&mut st, model, PACKAGE, 0x30);
    diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, billing);
    let schema_only = nested(&mut st, billing, 0x40, "Schema");
    diagram(&mut st, model, 0x41, OTHER_DIAGRAM, schema_only);
    let shop = add(&mut st, model, PACKAGE, 0x50);
    let shop_default = diagram(&mut st, model, 0x51, STRUCTURE_DIAGRAM, shop);
    bind(&mut st, model, 0x52, shop, shop_default);
    let accounts = nested(&mut st, shop, 0x60, "");
    let ledger = nested(&mut st, accounts, 0x70, "Ledger");
    let handed = diagram(&mut st, model, 0x71, STRUCTURE_DIAGRAM, ledger);
    bind(&mut st, model, 0x72, ledger, handed);
    let mut s = over(st);
    let unsound = Reason::ParentDefaultUnsound { parent: id(0x60).into(), fault: None };
    assert_eq!(refused(&mut s, "element.remove", json!({ "ids": [id(0x70)] })).1, Some(unsound), "Accounts has no diagram yet");
    s.execute("element.remove", &json!({ "ids": [id(0x20)] })).unwrap();
    s.execute("element.remove", &json!({ "ids": [id(0x40)] })).unwrap();
    assert_eq!((resolve(s.doc(), sales), resolve(s.doc(), billing)), (Ok(DefaultDiagram::NoDiagramYet), Err(BindingProblem::Missing)));
    s.execute("element.remove", &json!({ "ids": [id(0x70), id(0x60)] })).unwrap();
    let st = s.doc();
    assert_eq!(st.get(handed, archforge_diagram::diagram::ELEMENT).unwrap(), Value::Ref(shop), "past Accounts, into Shop");
    assert_eq!(resolve(st, shop), Ok(DefaultDiagram::Bound(shop_default)));
}

/// Ledger in Accounts (no diagram yet) in Shop, a package with no parent: removing Shop with Ledger
/// takes everything along, Accounts and Ledger's diagram among them, exactly as removing Shop
/// alone does, so nothing is refused. Under Sales, a package that stays, Shop hands Accounts up to
/// Sales, but Ledger's diagram goes past Accounts, below Shop, to Sales, which is judged (AR-673).
#[test]
fn a_handoff_inside_a_package_with_no_parent_that_goes_is_not_judged() {
    let shop_with = |under: bool| {
        let (mut st, model) = store();
        let shop = add(&mut st, model, PACKAGE, 0x50);
        let sales = under.then(|| {
            let sales = add(&mut st, model, PACKAGE, 0x40);
            let default = diagram(&mut st, model, 0x41, STRUCTURE_DIAGRAM, sales);
            bind(&mut st, model, 0x42, sales, default);
            st.remove_value(model, archforge_core::model::MEMBER, &Value::Ref(shop)).unwrap();
            st.push(sales, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(shop)).unwrap();
            (sales, default)
        });
        let accounts = nested(&mut st, shop, 0x60, "");
        let ledger = nested(&mut st, accounts, 0x70, "Ledger");
        let handed = diagram(&mut st, model, 0x71, STRUCTURE_DIAGRAM, ledger);
        bind(&mut st, model, 0x72, ledger, handed);
        (over(st), sales, accounts, handed)
    };
    let ((mut both, ..), (mut alone, ..)) = (shop_with(false), shop_with(false));
    both.execute("element.remove", &json!({ "ids": [id(0x50), id(0x70)] })).unwrap();
    alone.execute("element.remove", &json!({ "ids": [id(0x50)] })).unwrap();
    assert!(rows_of(both.doc()) == rows_of(alone.doc()), "the same as Shop alone");
    assert!(both.doc().by_uuid(Uuid([0x71; 16])).is_none_or(|d| !both.doc().is_alive(d)), "Ledger's diagram went with Shop");
    let (mut kept, Some((sales, default)), accounts, handed) = shop_with(true) else { panic!("Sales was made") };
    kept.execute("element.remove", &json!({ "ids": [id(0x50), id(0x70)] })).unwrap();
    let st = kept.doc();
    assert_eq!((st.container(accounts).unwrap().map(|(o, _)| o), resolve(st, accounts)), (Some(sales), Ok(DefaultDiagram::NoDiagramYet)));
    assert_eq!(
        (st.get(handed, archforge_diagram::diagram::ELEMENT).unwrap(), resolve(st, sales)),
        (Value::Ref(sales), Ok(DefaultDiagram::Bound(default)))
    );
}

/// Removing T, in R, with C, in P below T: T hands P up to R, but C's class diagram goes past P to R,
/// the first package above all that go, judged in P's place: refused, named, with no diagram yet; with
/// a bound one T and C go in either order, P lands in R diagramless, C's diagram is R's (AR-673).
#[test]
fn a_handoff_past_a_removed_ancestor_goes_to_the_first_package_above_it_that_stays() {
    let built = |bound: bool| {
        let (mut st, model) = store();
        let r = add(&mut st, model, PACKAGE, 0x10);
        st.set(r, archforge_uml::named_element::NAME, Value::str("R")).unwrap();
        let own = bound.then(|| diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, r));
        if let Some(own) = own {
            bind(&mut st, model, 0x12, r, own);
        }
        let t = nested(&mut st, r, 0x20, "T");
        let p = nested(&mut st, t, 0x30, "P");
        let c = nested(&mut st, p, 0x40, "C");
        let handed = diagram(&mut st, model, 0x41, STRUCTURE_DIAGRAM, c);
        bind(&mut st, model, 0x42, c, handed);
        (over(st), [r, t, p, c], own, handed)
    };
    let unsound = Reason::ParentDefaultUnsound { parent: "R".into(), fault: None };
    assert_eq!(refused(&mut built(false).0, "element.remove", json!({ "ids": [id(0x20), id(0x40)] })).1, Some(unsound), "R takes it");
    for ids in [[id(0x20), id(0x40)], [id(0x40), id(0x20)]] {
        let (mut s, [r, t, p, c], own, handed) = built(true);
        s.execute("element.remove", &json!({ "ids": ids })).unwrap();
        let st = s.doc();
        assert!(!st.is_alive(t) && !st.is_alive(c), "{ids:?}: T and C went");
        assert_eq!((st.container(p).unwrap().map(|(o, _)| o), resolve(st, p)), (Some(r), Ok(DefaultDiagram::NoDiagramYet)), "{ids:?}");
        let r_default = own.map(|d| Ok(DefaultDiagram::Bound(d)));
        assert_eq!((st.get(handed, archforge_diagram::diagram::ELEMENT).unwrap(), Some(resolve(st, r))), (Value::Ref(r), r_default), "{ids:?}");
    }
}

/// Inner, a package a profile holds directly, which no command makes but a project's rows may
/// hold, read back from them: Leaf in Inner hands its class diagram up past the profile as past any
/// parent. Removed with the profile, or with Inner, Leaf goes and so does its diagram, taken along
/// with them, nothing judged, R's lack of a diagram no more than Inner's; removed alone, Inner judges
/// it, refused. With the profile in Shop, a nested package, removing Shop with Leaf hands the diagram
/// past Inner and the profile, which only stay as Shop hands them up, to R, the first package above,
/// which judges it: refused, named, while R has no diagram yet, and R's once it has one (AR-745).
#[test]
fn a_package_a_profile_holds_hands_its_diagrams_up_past_the_profile() {
    let built = |in_shop: bool, r_bound: bool| {
        let (mut st, model) = store();
        let r = add(&mut st, model, PACKAGE, 0x10);
        st.set(r, archforge_uml::named_element::NAME, Value::str("R")).unwrap();
        if r_bound {
            let own = diagram(&mut st, model, 0x11, STRUCTURE_DIAGRAM, r);
            bind(&mut st, model, 0x12, r, own);
        }
        let holder = if in_shop { nested(&mut st, r, 0x20, "Shop") } else { r };
        let profile = st.create_with_uuid(PROFILE, Uuid([0x30; 16])).unwrap();
        st.push(holder, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(profile)).unwrap();
        let inner = nested(&mut st, profile, 0x40, "Inner");
        let leaf = nested(&mut st, inner, 0x50, "Leaf");
        let handed = diagram(&mut st, model, 0x51, STRUCTURE_DIAGRAM, leaf);
        bind(&mut st, model, 0x52, leaf, handed);
        let mut back = bare();
        let (tables, junctions) = tables();
        rows::restore(&mut back, &rows_of(&st), &tables, &junctions).unwrap();
        assert_eq!(back.first_problem(), None, "the rows read back whole");
        let at = |n: u8| back.by_uuid(Uuid([n; 16])).unwrap();
        let (r, profile, inner, handed) = (at(0x10), at(0x30), at(0x40), at(0x51));
        assert_eq!(back.container(inner).unwrap().map(|(o, _)| o), Some(profile), "Inner sits in the profile");
        (over(back), r, profile, handed)
    };
    let unsound = |parent: &str| Some(Reason::ParentDefaultUnsound { parent: parent.into(), fault: None });
    assert_eq!(refused(&mut built(false, true).0, "element.remove", json!({ "ids": [id(0x50)] })).1, unsound("Inner"), "Inner takes it");
    for (with, profile_stays, r_bound) in [(0x30, false, true), (0x30, false, false), (0x40, true, true), (0x40, true, false)] {
        let (mut s, r, profile, handed) = built(false, r_bound);
        s.execute("element.remove", &json!({ "ids": [id(0x50), id(with)] })).unwrap();
        let st = s.doc();
        assert!(!st.is_alive(handed), "{with:#x}, {r_bound}: Leaf's diagram went along");
        assert_eq!(st.is_alive(profile), profile_stays, "{with:#x}, {r_bound}");
        let r_default = if r_bound { DefaultDiagram::Bound(st.by_uuid(Uuid([0x11; 16])).unwrap()) } else { DefaultDiagram::NoDiagramYet };
        assert_eq!((resolve(st, r), problems(st)), (Ok(r_default), vec![]), "{with:#x}, {r_bound}");
    }
    assert_eq!(refused(&mut built(true, false).0, "element.remove", json!({ "ids": [id(0x50), id(0x20)] })).1, unsound("R"), "R takes it");
    let (mut s, r, profile, handed) = built(true, true);
    s.execute("element.remove", &json!({ "ids": [id(0x50), id(0x20)] })).unwrap();
    let st = s.doc();
    assert_eq!(st.container(profile).unwrap().map(|(o, _)| o), Some(r), "Shop handed the profile up to R");
    assert_eq!(st.get(handed, archforge_diagram::diagram::ELEMENT).unwrap(), Value::Ref(r), "past Inner and the profile, into R");
}

/// A and B hold each other, a loop the store and a project's reading refuse, made by a recorded move
/// of A into B applied again after B moved into A. Removing C, with a class diagram, and P, holding
/// C, ends in the guard's time: B, first to stay, would take the diagram; with none yet it is refused,
/// named, nothing changed, with A going too as well, the walk going round once more. With B too no
/// package takes it: a loose template parameter, named after C, is what is refused (AR-673).
#[test]
fn a_handoff_up_a_loop_of_packages_ends_and_is_judged_by_the_first_that_stays() {
    use archforge_engine::Document;
    use archforge_uml::package::PACKAGED_ELEMENT;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut st, model) = store();
        let a = add(&mut st, model, PACKAGE, 0x10);
        let b = add(&mut st, model, PACKAGE, 0x20);
        st.set(b, archforge_uml::named_element::NAME, Value::str("B")).unwrap();
        st.begin();
        st.remove_value(model, archforge_core::model::MEMBER, &Value::Ref(a)).unwrap();
        st.push(b, PACKAGED_ELEMENT, Value::Ref(a)).unwrap();
        let a_into_b = st.end().unwrap();
        st.apply(&Store::invert(&a_into_b)).unwrap();
        st.remove_value(model, archforge_core::model::MEMBER, &Value::Ref(b)).unwrap();
        st.push(a, PACKAGED_ELEMENT, Value::Ref(b)).unwrap();
        st.apply(&a_into_b).unwrap();
        let up = |o| st.container(o).unwrap().map(|(c, _)| c);
        assert_eq!((up(a), up(b)), (Some(b), Some(a)), "A and B hold each other");
        let p = nested(&mut st, b, 0x40, "P");
        let c = nested(&mut st, p, 0x30, "C");
        let handed = diagram(&mut st, model, 0x31, STRUCTURE_DIAGRAM, c);
        bind(&mut st, model, 0x32, c, handed);
        st.create_with_uuid(archforge_uml::CLASSIFIER_TEMPLATE_PARAMETER, Uuid([0x50; 16])).unwrap();
        let mut s = over(st);
        let ids = [vec![id(0x30), id(0x40)], vec![id(0x30), id(0x50), id(0x40), id(0x10)], vec![id(0x30), id(0x50), id(0x40), id(0x20), id(0x10)]];
        let _ = tx.send(ids.map(|ids| refused(&mut s, "element.remove", json!({ "ids": ids })).1));
    });
    let got = rx.recv_timeout(std::time::Duration::from_secs(30));
    assert!(!matches!(got, Err(std::sync::mpsc::RecvTimeoutError::Timeout)), "the walk up the loop never ended");
    let [below, past_a, all] = got.expect("element.remove was refused with nothing changed");
    let unsound = Some(Reason::ParentDefaultUnsound { parent: "B".into(), fault: None });
    assert_eq!((below, past_a), (unsound.clone(), unsound), "B takes it, past A too");
    assert!(matches!(all, Some(Reason::ElementNotRemovable { .. })), "with A and B going no package takes it: {all:?}");
}

/// Mending a package with three bindings, one the end of a trace link, takes that link along with
/// its binding instead of being refused: the first binding by id stays and names the chosen
/// diagram, the other two and the link go, each named in the answer, and one undo step brings every
/// row back (AR-733).
#[test]
fn choosing_a_default_removes_a_duplicate_binding_with_its_trace_link() {
    let (mut st, model, sales, overview, detail) = unbound_sales();
    let first = bind(&mut st, model, 0x1a, sales, overview);
    let linked = bind(&mut st, model, 0x1b, sales, detail);
    let other = bind(&mut st, model, 0x1c, sales, detail);
    let trace = archforge_core::link(&mut st, model, archforge_core::LinkKind::Trace, linked, sales, None).unwrap();
    let trace_id = st.uuid(trace).unwrap().to_string();
    let mut s = over(st);
    let (before, alive) = (rows_of(s.doc()), s.doc().alive().count());
    let made = set(&mut s, 0x10, 0x12);
    let removed = json!([id(0x1b), id(0x1c)]);
    assert_eq!(made, json!({ "package": id(0x10), "binding": id(0x1a), "diagram": id(0x12), "removed": removed, "links_removed": [trace_id] }));
    let st = s.doc();
    assert_eq!(alive - st.alive().count(), 3, "two bindings and the link went, nothing else");
    assert!(st.is_alive(first) && !st.is_alive(linked) && !st.is_alive(other) && !st.is_alive(trace));
    assert_eq!((resolve(st, sales), problems(st)), (Ok(DefaultDiagram::Bound(detail)), vec![]));
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(rows_of(s.doc()) == before, "one undo step brings the bindings and the link back");
}
