//! A package's default diagram through its binding (AR-575): each state built by hand in a store
//! under fixed ids, as no command makes a binding yet: bound, none yet, missing, duplicate, a dead,
//! a wrong-kind and a foreign target, a profile, and every problem of a store in a stable order.

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DEFAULT_DIAGRAM_BINDING, DIAGRAM, DiagramObj};
use archforge_mof::rows::{self, Cell};
use archforge_mof::{ClassId, CompositeSlabs, Obj, Store, Uuid, Value};
use archforge_uml::defaults::{BindingProblem, DefaultDiagram, Diagnosis, problems, resolve};
use archforge_uml::{PACKAGE, PROFILE, STRUCTURE_DIAGRAM};

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
