//! Profiles and stereotypes through the engine (AR-134): a profile made in a package, its
//! stereotypes extending metaclasses of the hidden UML library and owning typed properties, each
//! change one undo step; unsupported metaclasses and property types refused with the reason; a
//! profile applied to a package and unapplied; unapplying, removing and moving refused while
//! stereotype applications use them, the uses counted and named; the library kept out of types and
//! reads; saved as rows and read back identical, the model still opening; the profile's diagram;
//! the sweep. Then the applications themselves (AR-135): applied and unapplied, the picker's
//! filter, values read by type and multiplicity, defaults apart from values set, the notation on
//! cards, members and lines, each diagram's choice to show values, and nothing lost unseen.

use archforge_diagram::{Notation, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
use archforge_uml::applications;
use archforge_uml::profiles::{self, UML_URI};
use archforge_uml::{ALONE, UmlNotation, registry};
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

fn run(s: &mut S, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value["id"].as_str().unwrap_or_default().to_string()
}

fn read(s: &mut S, cmd: &str, p: Json) -> Json {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}")).value
}

/// The refusal of `cmd`, which changes nothing.
fn refused(s: &mut S, cmd: &str, p: Json) -> String {
    let before = picture(s.doc());
    let e = s.execute(cmd, &p).expect_err(&format!("{cmd} {p} was not refused"));
    assert_eq!(picture(s.doc()), before, "{cmd} changed nothing");
    format!("{e}")
}

fn obj(st: &Store, id: &str) -> Obj {
    st.by_uuid(Uuid::parse(id).unwrap()).unwrap()
}

fn name(st: &Store, o: Obj) -> String {
    archforge_core::name_of(st, o)
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

/// `cmd` changes the model as one undo step.
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

/// The sample of F1 and F5: a package Commerce with the classes Order and Customer and the
/// attribute Order.id, the profile Persistence in the root with the stereotypes Entity (extends
/// Class, properties table : String and cached : Boolean), Id (extends Property) and Reference
/// (extends Association): (commerce, order, customer, id attribute, persistence, entity, id, reference).
fn shop(s: &mut S) -> [String; 8] {
    let commerce = run(s, "uml.package.add", json!({ "name": "Commerce" }));
    let order = run(s, "uml.class.add", json!({ "package": commerce, "name": "Order" }));
    let customer = run(s, "uml.class.add", json!({ "package": commerce, "name": "Customer" }));
    let id_attr = run(s, "uml.attribute.add", json!({ "class": order, "name": "id", "type": "Integer" }));
    let persistence = run(s, "uml.profile.add", json!({ "name": "Persistence" }));
    let entity = run(s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
    run(s, "uml.attribute.add", json!({ "class": entity, "name": "table", "type": "String" }));
    run(s, "uml.attribute.add", json!({ "class": entity, "name": "cached", "type": "Boolean", "default": "false" }));
    let id = run(s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Id", "extends": ["Property"] }));
    let reference = run(s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Reference", "extends": ["Association"] }));
    [commerce, order, customer, id_attr, persistence, entity, id, reference]
}

/// The session `s` with stereotype applications made by `uml.stereotype.apply`, each
/// (stereotype, element), in a session over the rows read back.
fn with_applications(s: &S, uses: &[(&str, &str)]) -> S {
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut st = store(1 << 40);
    rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
    let mut s = session_over(st);
    for (stereotype, element) in uses {
        run(&mut s, "uml.stereotype.apply", json!({ "element": element, "stereotype": stereotype }));
    }
    s
}

/// A profile and its stereotypes are made one undo step each; each extension is UML's own shape:
/// an Extension in the profile, a base_ attribute of the stereotype typed by the metaclass, an
/// ExtensionEnd typed by the stereotype, and the profile's reference to the metaclass, which the
/// hidden library holds once, however many stereotypes extend it.
#[test]
fn profiles_and_stereotypes_are_uml_elements_made_one_step_each() {
    let mut s = session();
    let commerce = run(&mut s, "uml.package.add", json!({ "name": "Commerce" }));
    let persistence = one_step(&mut s, "uml.profile.add", json!({ "name": "Persistence", "package": commerce }));
    let entity = one_step(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
    let audited = one_step(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Audited", "extends": ["Class", "Package"] }));
    let st = s.doc();
    let (p, e, a) = (obj(st, &persistence), obj(st, &entity), obj(st, &audited));
    assert_eq!(st.class_of(p).unwrap(), archforge_uml::PROFILE);
    assert_eq!(st.class_of(e).unwrap(), archforge_uml::STEREOTYPE);
    assert_eq!(profiles::package_of(st, p), Some(obj(st, &commerce)));
    assert_eq!(profiles::stereotypes(st, p), [e, a]);
    assert_eq!(profiles::extended(st, a), ["Class", "Package"]);
    let ext = profiles::extensions(st, e)[0];
    assert_eq!(st.class_of(ext.extension).unwrap(), archforge_uml::EXTENSION);
    assert_eq!(name(st, ext.base), "base_Class");
    assert_eq!(st.get(ext.end.unwrap(), archforge_uml::extension_end::TYPE).unwrap(), Value::Ref(e));
    assert_eq!(profiles::extension_ends(st, ext.extension), Some((e, ext.metaclass)));
    assert!(profiles::is_metaclass(st, ext.metaclass));
    // One Class metaclass for both, in the unnamed library known by UML's address.
    assert_eq!(profiles::extensions(st, a)[0].metaclass, ext.metaclass);
    let library = profiles::library(st).unwrap();
    assert_eq!(st.get(library, archforge_uml::package::URI).unwrap(), Value::str(UML_URI));
    assert_eq!(name(st, library), "");
    assert_eq!(profiles::metaclasses_of(st, p).iter().map(|m| name(st, *m)).collect::<Vec<_>>(), ["Class", "Package"]);
    // The base_ attributes are no properties of the stereotype's own.
    assert!(profiles::properties(st, e).is_empty());
    assert_eq!(s.doc().first_problem_by(archforge_uml::names::kind), None, "the model holds together as it would open");
}

/// A stereotype's properties are made, edited, moved and removed with the attribute commands, one
/// step each; only a primitive type, an enumeration or a data type types one, and a name starting
/// base_ is kept for the extensions.
#[test]
fn stereotype_properties_use_the_member_commands_and_their_types() {
    let mut s = session();
    let [commerce, order, _, _, _, entity, ..] = shop(&mut s);
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": commerce, "name": "Status" }));
    let lazy = one_step(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "state", "type": status }));
    one_step(&mut s, "uml.attribute.set", json!({ "attribute": lazy, "name": "phase" }));
    one_step(&mut s, "uml.attribute.move", json!({ "attribute": lazy, "at": 0 }));
    let st = s.doc();
    let texts: Vec<String> = profiles::properties(st, obj(st, &entity)).into_iter().map(|a| archforge_uml::figures::attribute_text(st, a)).collect();
    assert_eq!(texts, ["+ phase : Status", "+ table : String", "+ cached : Boolean = false"]);
    let why = refused(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "owner", "type": order }));
    assert!(why.contains("typed by a primitive type, an enumeration or a data type"), "{why}");
    let why = refused(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "Base_Class" }));
    assert!(why.contains("base_"), "{why}");
    assert!(refused(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "TABLE" })).contains("already"));
    let gone = read(&mut s, "uml.stereotype.get", json!({ "stereotype": entity }))["properties"][0]["id"].as_str().unwrap().to_string();
    one_step(&mut s, "element.remove", json!({ "ids": [gone] }));
    // A base_ attribute is no member to remove: the extension goes through what it extends.
    let base = {
        let st = s.doc();
        s.doc().uuid(profiles::extensions(st, obj(st, &entity))[0].base).unwrap().to_string()
    };
    assert!(refused(&mut s, "element.remove", json!({ "ids": [base] })).contains("extension"));
}

/// Only the metaclasses Archforge draws are offered; one it does not draw yet, and a name that is
/// none, are refused saying so and naming those offered.
#[test]
fn unsupported_metaclasses_are_refused_with_the_reason() {
    let mut s = session();
    let [.., persistence, entity, _, _] = shop(&mut s);
    let why = refused(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Actorish", "extends": ["Actor"] }));
    assert!(why.contains("Actor is a UML metaclass Archforge does not draw yet"), "{why}");
    assert!(why.contains("Class, Interface, Enumeration, DataType, Property, Operation, Association, Generalization, Package"), "{why}");
    let why = refused(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Class", "Table"] }));
    assert!(why.contains("\"Table\" is not a metaclass"), "{why}");
    assert!(refused(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Class", "Class"] })).contains("twice"));
    let listed = read(&mut s, "uml.metaclass.list", json!({}));
    assert_eq!(listed["offered"].as_array().unwrap().len(), 9);
    assert!(listed["not_drawn"].as_array().unwrap().iter().any(|n| n == "UseCase"));
}

/// Extensions are added and dropped one step each; the profile references a metaclass while one of
/// its stereotypes extends it; a rename keeps everything and the extension end follows its name.
#[test]
fn extensions_and_names_change_one_step_each() {
    let mut s = session();
    let [_, _, _, _, persistence, entity, id, _] = shop(&mut s);
    one_step(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Class", "Interface"] }));
    let st = s.doc();
    assert_eq!(profiles::extended(st, obj(st, &entity)), ["Class", "Interface"]);
    one_step(&mut s, "uml.stereotype.set", json!({ "stereotype": id, "extends": [] }));
    let st = s.doc();
    let refs: Vec<String> = profiles::metaclasses_of(st, obj(st, &persistence)).into_iter().map(|m| name(st, m)).collect();
    assert_eq!(refs, ["Class", "Interface", "Association"], "Property is used no more");
    // The library keeps the metaclass it made: it is never made twice.
    assert!(profiles::metaclass(st, "Property").is_some());
    one_step(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "name": "Table" }));
    let st = s.doc();
    let end = profiles::extensions(st, obj(st, &entity))[0].end.unwrap();
    assert_eq!(name(st, end), "extension_Table");
    assert!(refused(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "name": "id" })).contains("already"));
    one_step(&mut s, "uml.profile.rename", json!({ "profile": persistence, "name": "Storage" }));
    assert_eq!(s.doc().first_problem_by(archforge_uml::names::kind), None);
}

/// A package applies a profile and unapplies it, one step each; its elements and those of the
/// packages inside it may use the profile's stereotypes then; applying twice is refused.
#[test]
fn a_package_applies_and_unapplies_a_profile() {
    let mut s = session();
    let [commerce, order, _, id_attr, persistence, entity, ..] = shop(&mut s);
    let inner = run(&mut s, "uml.package.add", json!({ "name": "Billing", "parent": commerce }));
    let other = run(&mut s, "uml.package.add", json!({ "name": "Other" }));
    one_step(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let st = s.doc();
    let p = obj(st, &persistence);
    for e in [&commerce, &order, &id_attr, &inner] {
        assert!(profiles::in_scope(st, p, obj(st, e)), "in scope");
    }
    assert!(!profiles::in_scope(st, p, obj(st, &other)));
    assert!(profiles::available(st, obj(st, &order)).contains(&obj(st, &entity)));
    assert!(refused(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence })).contains("already"));
    let listed = read(&mut s, "uml.package.profiles", json!({ "package": commerce }));
    assert_eq!(listed["applied"][0]["name"], "Persistence");
    one_step(&mut s, "uml.profile.unapply", json!({ "package": commerce, "profile": persistence }));
    assert!(refused(&mut s, "uml.profile.unapply", json!({ "package": commerce, "profile": persistence })).contains("does not apply"));
}

/// While elements use the stereotypes, unapplying the profile, removing a stereotype or the
/// profile, dropping an extension those elements need, moving the stereotype to another profile,
/// removing a property whose values are set, and moving a used element out of the profile's scope
/// are each refused with the uses counted and named, changing nothing; covered by another package
/// that applies the profile, unapplying is fine.
#[test]
fn uses_refuse_destructive_changes_with_the_uses_counted() {
    let mut s = session();
    let [commerce, order, customer, id_attr, persistence, entity, id, _] = shop(&mut s);
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let other = run(&mut s, "uml.package.add", json!({ "name": "Other" }));
    let billing = run(&mut s, "uml.package.add", json!({ "name": "Billing", "parent": commerce }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
    let mut s = with_applications(&s, &[(&entity, &order), (&entity, &customer), (&id, &id_attr), (&entity, &invoice)]);
    let why = refused(&mut s, "uml.profile.unapply", json!({ "package": commerce, "profile": persistence }));
    assert!(why.contains("Persistence is in use in Commerce: «Entity» on 3 elements, «Id» on id. Remove those first."), "{why}");
    assert_eq!(read(&mut s, "uml.profile.uses", json!({ "profile": persistence, "package": commerce }))["total"], 4);
    assert!(refused(&mut s, "element.remove", json!({ "ids": [entity] })).contains("Entity is in use: «Entity» on 3 elements"));
    assert!(refused(&mut s, "element.remove", json!({ "ids": [persistence] })).contains("«Id» on id"));
    let why = refused(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Interface"] }));
    assert!(why.contains("«Entity» on Class is in use"), "{why}");
    // Another metaclass that still covers the elements may be dropped and added freely.
    run(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Class", "Package"] }));
    run(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Class"] }));
    let elsewhere = run(&mut s, "uml.profile.add", json!({ "name": "Elsewhere" }));
    assert!(refused(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "profile": elsewhere })).contains("is in use"));
    let why = refused(&mut s, "uml.class.move", json!({ "class": order, "package": other }));
    assert!(why.contains("«Entity» on Order"), "{why}");
    assert!(refused(&mut s, "uml.package.move", json!({ "package": billing, "to": other })).contains("apply the profile there first"));
    // Commerce applies the profile itself, so it moves with its scope.
    one_step(&mut s, "uml.package.move", json!({ "package": commerce, "to": other }));
    s.undo().unwrap();
    assert!(refused(&mut s, "element.remove", json!({ "ids": [commerce] })).contains("Commerce applies the profiles its elements use"));
    // A Slot holding a value of table: the property stays while it is set.
    let table = read(&mut s, "uml.stereotype.get", json!({ "stereotype": entity }))["properties"][0]["id"].as_str().unwrap().to_string();
    let mut s = {
        let (tables, junctions) = tables();
        let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
        let mut st = store(1 << 50);
        rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
        let e = obj(&st, &entity);
        let instance = profiles::applications_of(&st, e)[0].instance;
        let slot = st.create(archforge_uml::SLOT).unwrap();
        st.set(slot, archforge_uml::slot::DEFINING_FEATURE, Value::Ref(obj(&st, &table))).unwrap();
        st.push(instance, archforge_uml::instance_specification::SLOT, Value::Ref(slot)).unwrap();
        session_over(st)
    };
    assert!(refused(&mut s, "element.remove", json!({ "ids": [table] })).contains("the values of table are set on 1 element"));
    // Applied around Other too, Order may go there, and Commerce may unapply it once Other covers
    // all its uses... Customer and id stay in Commerce, so it stays refused.
    run(&mut s, "uml.profile.apply", json!({ "package": other, "profile": persistence }));
    one_step(&mut s, "uml.class.move", json!({ "class": order, "package": other }));
    let root = run(&mut s, "uml.model.root", json!({}));
    run(&mut s, "uml.profile.apply", json!({ "package": root, "profile": persistence }));
    one_step(&mut s, "uml.profile.unapply", json!({ "package": commerce, "profile": persistence }));
    assert_eq!(s.doc().first_problem_by(archforge_uml::names::kind), None);
    // An application goes with the element it applies to, in the same step.
    let count = |s: &S| {
        let st = s.doc();
        profiles::applications_of(st, obj(st, &entity)).len()
    };
    let before = count(&s);
    one_step(&mut s, "element.remove", json!({ "ids": [customer] }));
    assert_eq!(count(&s), before - 1);
    assert!(
        s.doc().alive().filter(|o| s.doc().class_of(*o).ok() == Some(archforge_uml::INSTANCE_SPECIFICATION)).all(|i| profiles::element_of(
            s.doc(),
            i
        )
        .is_some()),
        "no application left without its element"
    );
}

/// Removing a profile nobody uses takes its stereotypes, extensions and applications along in one
/// step; removing an unused stereotype takes its extensions and the profile's references nothing
/// else needs.
#[test]
fn unused_profiles_and_stereotypes_go_in_one_step() {
    let mut s = session();
    let [commerce, _, _, _, persistence, entity, id, _] = shop(&mut s);
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    one_step(&mut s, "element.remove", json!({ "ids": [id] }));
    let st = s.doc();
    let refs: Vec<String> = profiles::metaclasses_of(st, obj(st, &persistence)).into_iter().map(|m| name(st, m)).collect();
    assert_eq!(refs, ["Class", "Association"]);
    assert!(profiles::extensions(st, obj(st, &entity)).len() == 1);
    one_step(&mut s, "element.remove", json!({ "ids": [persistence] }));
    let st = s.doc();
    assert!(profiles::applied(st, obj(st, &commerce)).is_empty());
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_uml::EXTENSION)));
    assert_eq!(st.first_problem_by(archforge_uml::names::kind), None);
}

/// The library and its metaclasses are UML's own: no type list, read or command offers or changes
/// them, and they are never a type.
#[test]
fn the_library_stays_out_of_types_reads_and_edits() {
    let mut s = session();
    let [_, order, ..] = shop(&mut s);
    let class = {
        let st = s.doc();
        st.uuid(profiles::metaclass(st, "Class").unwrap()).unwrap().to_string()
    };
    let types = read(&mut s, "uml.type.list", json!({}));
    let names: Vec<&str> = types["types"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for hidden in ["Class", "Property", "Association", "Entity", "Id"] {
        assert!(!names.contains(&hidden), "{hidden} is no type: {names:?}");
    }
    assert!(refused(&mut s, "uml.class.rename", json!({ "class": class, "name": "Klass" })).contains("UML metaclass Class"));
    assert!(refused(&mut s, "element.remove", json!({ "ids": [class] })).contains("UML metaclass Class"));
    assert!(refused(&mut s, "uml.attribute.add", json!({ "class": order, "name": "kind", "type": class })).contains("UML metaclass"));
    let packages = read(&mut s, "uml.package.list", json!({}));
    assert!(packages["packages"].as_array().unwrap().iter().all(|p| p["name"] != ""), "the library is no package");
}

/// A profile is a package's kind of name (AR-208): a profile and a package of one package never
/// share a name, whatever the case, while a classifier may take one.
#[test]
fn a_profile_shares_its_package_names_with_packages() {
    let mut s = session();
    let [commerce, ..] = shop(&mut s);
    run(&mut s, "uml.profile.add", json!({ "name": "Audit", "package": commerce }));
    assert!(refused(&mut s, "uml.package.add", json!({ "name": "AUDIT", "parent": commerce })).contains("already"));
    let sub = run(&mut s, "uml.package.add", json!({ "name": "Billing", "parent": commerce }));
    assert!(refused(&mut s, "uml.profile.add", json!({ "name": "billing", "package": commerce })).contains("already"));
    assert!(refused(&mut s, "uml.package.rename", json!({ "package": sub, "name": "audit" })).contains("already"));
    run(&mut s, "uml.class.add", json!({ "package": commerce, "name": "Audit" }));
    assert_eq!(s.doc().first_problem_by(archforge_uml::names::kind), None);
}

/// Profiles, stereotypes, extensions, metaclasses and applications are saved as rows and read back
/// identical, in their order, and the model opens again.
#[test]
fn profiles_are_saved_and_read_back_identical() {
    let mut s = session();
    let [commerce, _, _, _, persistence, ..] = shop(&mut s);
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 30);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let p = obj(&back, &persistence);
    let names: Vec<String> = profiles::stereotypes(&back, p).into_iter().map(|st| name(&back, st)).collect();
    assert_eq!(names, ["Entity", "Id", "Reference"]);
    assert_eq!(profiles::applied(&back, obj(&back, &commerce)).len(), 1);
    assert!(profiles::library(&back).is_some());
    assert_eq!(back.first_problem_by(archforge_uml::names::kind), None, "the model opens again");
}

/// A profile's diagram shows its stereotypes and the metaclasses they extend, joined by extension
/// lines with a filled triangle at the metaclass; a package's diagram shows neither.
#[test]
fn the_profile_diagram_shows_stereotypes_metaclasses_and_extensions() {
    let mut s = session();
    let [commerce, _, _, _, persistence, entity, ..] = shop(&mut s);
    let d = one_step(&mut s, "uml.profile.diagram", json!({ "profile": persistence }));
    assert_eq!(read(&mut s, "uml.profile.diagram", json!({ "profile": persistence }))["made"], false);
    let package_diagram = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "name": "C", "element": commerce }));
    let st = s.doc();
    let (dd, pd, e) = (obj(st, &d), obj(st, &package_diagram), obj(st, &entity));
    let ext = profiles::extensions(st, e)[0];
    assert!(UmlNotation.may_show(st, dd, e) && UmlNotation.may_show(st, dd, ext.metaclass));
    assert!(!UmlNotation.may_show(st, pd, e) && !UmlNotation.may_show(st, pd, ext.metaclass));
    let card = UmlNotation.card(st, e, dd).unwrap();
    assert_eq!(card.compartments[0].iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), ["+ table : String", "+ cached : Boolean = false"]);
    assert_eq!(UmlNotation.card(st, ext.metaclass, dd).unwrap().name, "Class");
    assert_eq!(UmlNotation.ends(st, ext.extension), Some((e, ext.metaclass)));
    assert_eq!(UmlNotation.line(st, ext.extension).unwrap().ends[1].mark, archforge_diagram::figure::EndMark::FilledTriangle);
    let m = st.uuid(ext.metaclass).unwrap().to_string();
    let x = st.uuid(ext.extension).unwrap().to_string();
    // Made, the diagram shows the cards and the line already.
    {
        let st = s.doc();
        assert_eq!(archforge_diagram::views_of(st, obj(st, &x)).1.len(), 1, "the extension's line is drawn");
    }
    // Dropping the extension takes its line and the metaclass's card off the profile's diagram.
    one_step(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": [] }));
    let st = s.doc();
    assert!(archforge_diagram::views_of(st, obj(st, &m)).2.is_empty());
}

/// The sweep over the profile commands breaks nothing.
#[test]
fn the_sweep_over_profiles_breaks_nothing() {
    let mut s = session();
    let [commerce, order, _, _, persistence, entity, id, _] = shop(&mut s);
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in ["uml.profile.add", "uml.profile.apply", "uml.profile.unapply", "uml.stereotype.add", "uml.stereotype.set", "uml.profile.list"] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let ids = [commerce.as_str(), order.as_str(), persistence.as_str(), entity.as_str(), id.as_str(), "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}

/// Every refusal for uses carries its reason typed beside the unchanged English, for an interface
/// to say in its own words: what is in use, and the uses, named while a stereotype has at most two
/// and counted past that (AR-437).
#[test]
fn every_use_that_refuses_carries_its_reason_typed() {
    use archforge_engine::{EngineError, InUse, Reason, Used};
    let mut s = session();
    let [commerce, order, customer, id_attr, persistence, entity, id, reference] = shop(&mut s);
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let invoice = run(&mut s, "uml.class.add", json!({ "package": commerce, "name": "Invoice" }));
    let tools = run(&mut s, "uml.package.add", json!({ "name": "Tools" }));
    let audit = run(&mut s, "uml.profile.add", json!({ "name": "Audit", "package": tools }));
    let tracked = run(&mut s, "uml.stereotype.add", json!({ "profile": audit, "name": "Tracked", "extends": ["Class"] }));
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": audit }));
    let elsewhere = run(&mut s, "uml.profile.add", json!({ "name": "Elsewhere" }));
    let uses = [(&entity, &order), (&entity, &customer), (&entity, &invoice), (&id, &id_attr), (&tracked, &order), (&tracked, &customer)];
    let mut s = with_applications(&s, &uses.map(|(s, e)| (s.as_str(), e.as_str())));
    let text = |n: &str| n.to_string();
    // Three elements are counted; one, or two of different names, are named.
    let counted = vec![(text("Entity"), vec![], 3)];
    let both = vec![counted[0].clone(), (text("Id"), vec![text("id")], 1)];
    let named = [(text("Tracked"), vec![text("Order"), text("Customer")], 2)];
    let in_use = |what: Used, uses: &[(String, Vec<String>, usize)]| Reason::InUse(Box::new(InUse { what, uses: uses.to_vec() }));
    let check = |s: &mut S, cmd: &str, p: Json, reason: Reason, english: &str| {
        let before = picture(s.doc());
        let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
        assert_eq!(error.reason, Some(reason), "{cmd} {p}: {}", error.message);
        assert!(error.message.contains(english), "{cmd} {p}: {}", error.message);
        assert_eq!(picture(s.doc()), before, "{cmd} {p} changed nothing");
    };
    let unapplied = Used::ProfileIn { profile: text("Persistence"), package: text("Commerce") };
    let english = "Persistence is in use in Commerce: «Entity» on 3 elements, «Id» on id. Remove those first.";
    check(&mut s, "uml.profile.unapply", json!({ "package": commerce, "profile": persistence }), in_use(unapplied, &both), english);
    check(
        &mut s,
        "element.remove",
        json!({ "ids": [entity] }),
        in_use(Used::Element(text("Entity")), &counted),
        "Entity is in use: «Entity» on 3 elements",
    );
    let moved = json!({ "stereotype": entity, "profile": elsewhere });
    check(&mut s, "uml.stereotype.set", moved, in_use(Used::Moved(text("Entity")), &counted), "«Entity» is in use: «Entity» on 3 elements");
    let dropped = Used::Extension { stereotype: text("Entity"), metaclass: text("Class") };
    let other = json!({ "stereotype": entity, "extends": ["Interface"] });
    check(&mut s, "uml.stereotype.set", other, in_use(dropped, &counted), "«Entity» on Class is in use: «Entity» on 3 elements");
    let applies = in_use(Used::AppliedProfiles { package: text("Commerce") }, &[both.as_slice(), &named].concat());
    let english = "Commerce applies the profiles its elements use: «Entity» on 3 elements, «Id» on id, «Tracked» on Order and Customer.";
    check(&mut s, "element.remove", json!({ "ids": [commerce] }), applies, english);
    // UML's own library and its metaclasses, wherever a command is given one.
    let (class, library) = {
        let st = s.doc();
        (st.uuid(profiles::metaclass(st, "Class").unwrap()).unwrap().to_string(), st.uuid(profiles::library(st).unwrap()).unwrap().to_string())
    };
    let own = |metaclass: Option<&str>| Reason::UmlOwn { metaclass: metaclass.map(text) };
    let kept = "is the UML metaclass Class, kept in the UML library, which is never changed";
    check(&mut s, "uml.class.rename", json!({ "class": class, "name": "Klass" }), own(Some("Class")), kept);
    check(&mut s, "uml.attribute.add", json!({ "class": order, "name": "kind", "type": class }), own(Some("Class")), kept);
    check(&mut s, "uml.attribute.add", json!({ "class": class, "name": "kind", "type": "Integer" }), own(Some("Class")), kept);
    check(&mut s, "uml.association.add-nary", json!({ "classes": [order, customer, class] }), own(Some("Class")), kept);
    check(&mut s, "uml.stereotype.apply", json!({ "element": class, "stereotype": entity }), own(Some("Class")), kept);
    check(&mut s, "element.remove", json!({ "ids": [library] }), own(None), "it is the UML library");
    // A description names no parameter, as before.
    for (element, reason, english) in [(&class, own(Some("Class")), kept), (&library, own(None), "it is the UML library")] {
        let described = json!({ "element": element, "text": "Ours" });
        check(&mut s, "element.describe", described.clone(), reason, english);
        let Err(EngineError::Failed { error, .. }) = s.execute("element.describe", &described) else { panic!("refused") };
        assert_eq!(error.key, None, "{}", error.message);
    }
    // A package with no parent takes the profile it holds along.
    let mut top = at_top(&s, &tools);
    let held = Used::HeldProfile { package: text("Tools"), profile: text("Audit") };
    let english = "Tools holds the profile Audit, which is in use: «Tracked» on Order and Customer. Remove those first.";
    check(&mut top, "element.remove", json!({ "ids": [tools] }), in_use(held, &named), english);
    // Two elements whose names differ only by case are counted, never named twice.
    let upper = run(&mut s, "uml.attribute.add", json!({ "class": customer, "name": "ID", "type": "Integer" }));
    run(&mut s, "uml.stereotype.apply", json!({ "element": upper, "stereotype": id }));
    let alike = in_use(Used::Element(text("Id")), &[(text("Id"), vec![], 2)]);
    check(&mut s, "element.remove", json!({ "ids": [id] }), alike, "Id is in use: «Id» on 2 elements.");
    // An application that traces to no element, as a project edited by hand may hold, is counted
    // with the others, and none is named then.
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut st = store(1 << 50);
    rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
    for stereotype in [&tracked, &reference] {
        let loose = st.create(archforge_uml::INSTANCE_SPECIFICATION).unwrap();
        st.push(loose, archforge_uml::instance_specification::CLASSIFIER, Value::Ref(obj(&st, stereotype))).unwrap();
    }
    let mut s = session_over(st);
    let with_loose = in_use(Used::Element(text("Tracked")), &[(text("Tracked"), vec![], 3)]);
    check(&mut s, "element.remove", json!({ "ids": [tracked] }), with_loose, "Tracked is in use: «Tracked» on 3 elements.");
    let only_loose = in_use(Used::Element(text("Reference")), &[(text("Reference"), vec![], 1)]);
    check(&mut s, "element.remove", json!({ "ids": [reference] }), only_loose, "Reference is in use: «Reference» on 1 element.");
}

/// A profile's and a stereotype's own rules, refused, carry their reason typed beside the unchanged
/// English: a profile applied twice or unapplied where it is not applied, a profile full, a
/// metaclass not drawn yet, unknown, named twice or one too many, an extension end removed, and a
/// property removed whose values are set (AR-445).
#[test]
fn every_profile_rule_refused_carries_its_reason_typed() {
    use archforge_engine::{EngineError, Reason};
    let mut s = session();
    let [commerce, order, customer, _, persistence, entity, _, reference] = applied_shop(&mut s);
    let tools = run(&mut s, "uml.package.add", json!({ "name": "Tools" }));
    let check = |s: &mut S, cmd: &str, p: Json, reason: Reason, english: &str| {
        let before = picture(s.doc());
        let Err(EngineError::Failed { error, .. }) = s.execute(cmd, &p) else { panic!("{cmd} {p} is refused") };
        assert_eq!(error.reason, Some(reason), "{cmd} {p}: {}", error.message);
        assert!(error.message.contains(english), "{cmd} {p}: {}", error.message);
        assert_eq!(picture(s.doc()), before, "{cmd} {p} changed nothing");
    };
    let applied = Reason::ProfileApplied { package: "Commerce".into(), profile: "Persistence".into() };
    check(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }), applied, "Commerce applies Persistence already");
    let not_applied = Reason::ProfileNotApplied { package: "Tools".into(), profile: "Persistence".into() };
    check(&mut s, "uml.profile.unapply", json!({ "package": tools, "profile": persistence }), not_applied, "Tools does not apply Persistence");
    // The metaclasses a stereotype is given, when it is made and when it is changed.
    let made = |extends: Json| json!({ "profile": persistence, "name": "Fresh", "extends": extends });
    let changed = |extends: Json| json!({ "stereotype": entity, "extends": extends });
    let not_drawn = || Reason::MetaclassNotDrawn { name: "Actor".into() };
    check(&mut s, "uml.stereotype.add", made(json!(["Actor"])), not_drawn(), "Actor is a UML metaclass Archforge does not draw yet");
    check(&mut s, "uml.stereotype.set", changed(json!(["Class", "Actor"])), not_drawn(), "Actor is a UML metaclass Archforge does not draw yet");
    let none = Reason::NotAMetaclass { name: "Table".into() };
    check(&mut s, "uml.stereotype.add", made(json!(["Table"])), none, "\"Table\" is not a metaclass Archforge draws");
    // A long name is cut to its first 40 characters, in the reason as in the sentence.
    let cut = Reason::NotAMetaclass { name: "T".repeat(40) };
    check(&mut s, "uml.stereotype.add", made(json!(["T".repeat(41)])), cut, &format!("{:?} is not a metaclass", "T".repeat(40)));
    let twice = Reason::NamedTwice { kind: "metaclass", name: "Class".into() };
    check(&mut s, "uml.stereotype.set", changed(json!(["Interface", "Class", "Class"])), twice, "Class is named twice");
    let most = Reason::TooMany { kind: "metaclass", max: 9 };
    check(&mut s, "uml.stereotype.add", made(json!(vec!["Class"; 10])), most, "at most 9 metaclasses");
    // An extension's end goes only with its extension, and a property stays while values are set.
    let base = {
        let st = s.doc();
        st.uuid(profiles::extensions(st, obj(st, &entity))[0].base).unwrap().to_string()
    };
    let end = Reason::ExtensionEnd { name: "base_Class".into() };
    check(&mut s, "element.remove", json!({ "ids": [base] }), end, "base_Class is a stereotype's extension end");
    let table = property(&mut s, &entity, "table");
    for (element, elements, english) in [(&order, 1, "the values of table are set on 1 element;"), (&customer, 2, "set on 2 elements;")] {
        run(&mut s, "uml.stereotype.apply", json!({ "element": element, "stereotype": entity }));
        run(&mut s, "uml.stereotype.set-value", json!({ "element": element, "stereotype": entity, "property": "table", "values": ["t"] }));
        check(&mut s, "element.remove", json!({ "ids": [table] }), Reason::ValuesSet { name: "table".into(), elements }, english);
    }
    // A profile that holds the most takes no stereotype more, made in it or moved to it.
    let full = run(&mut s, "uml.profile.add", json!({ "name": "Full" }));
    for n in 0..profiles::MAX_STEREOTYPES {
        run(&mut s, "uml.stereotype.add", json!({ "profile": full, "name": format!("S{n}") }));
    }
    let most = || Reason::TooMany { kind: "stereotype", max: 256 };
    check(&mut s, "uml.stereotype.add", json!({ "profile": full, "name": "Over" }), most(), "a profile holds at most 256 stereotypes");
    check(&mut s, "uml.stereotype.set", json!({ "stereotype": reference, "profile": full }), most(), "a profile holds at most 256 stereotypes");
}

/// An element that takes no stereotype, and a stereotype that may not be applied to one, refused,
/// carry their reason typed beside the unchanged English, under the parameter each is about; a
/// template parameter's classifier is refused with its sentence alone (AR-446).
#[test]
fn every_application_refused_carries_its_reason_typed() {
    use archforge_engine::{EngineError, Reason, Unextended};
    let mut s = session();
    let [commerce, order, customer, _, persistence, entity, id, _] = applied_shop(&mut s);
    let check = |s: &mut S, element: &str, stereotype: &str, key: &str, reason: Option<Reason>, english: &str| {
        let p = json!({ "element": element, "stereotype": stereotype });
        let before = picture(s.doc());
        let Err(EngineError::Failed { error, .. }) = s.execute("uml.stereotype.apply", &p) else { panic!("{p} is refused") };
        assert_eq!((error.reason, error.key.as_deref()), (reason, Some(key)), "{p}: {}", error.message);
        assert!(error.message.contains(english), "{p}: {}", error.message);
        assert_eq!(picture(s.doc()), before, "{p} changed nothing");
    };
    // What takes none: an application, the model itself, a profile's own, a literal, a metaclass, a parameter's classifier.
    let instance = run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    check(&mut s, &instance, &entity, "element", Some(Reason::IsApplication), "is a stereotype application; stereotypes apply");
    let model = {
        let st = s.doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap()).unwrap().to_string()
    };
    check(&mut s, &model, &entity, "element", Some(Reason::NotInModel), "is no element of the model");
    let own = Reason::InProfile { name: "Entity".into() };
    check(&mut s, &entity, &entity, "element", Some(own), "Entity belongs to a profile, which defines stereotypes and takes none");
    let status = run(&mut s, "uml.enumeration.add", json!({ "package": commerce, "name": "Status" }));
    let open = run(&mut s, "uml.literal.add", json!({ "enumeration": status, "name": "open" }));
    let none = Reason::TakesNoStereotype { name: "open".into() };
    check(&mut s, &open, &entity, "element", Some(none), "open takes no stereotype: stereotypes extend Class, Interface,");
    let (class, t) = {
        run(&mut s, "uml.template-parameter.add", json!({ "classifier": customer, "name": "T" }));
        let st = s.doc();
        let parameter = archforge_uml::templates::parameters(st, obj(st, &customer))[0];
        let t = archforge_uml::templates::element_of(st, parameter).unwrap();
        (st.uuid(profiles::metaclass(st, "Class").unwrap()).unwrap().to_string(), st.uuid(t).unwrap().to_string())
    };
    check(&mut s, &class, &entity, "element", Some(Reason::UmlOwn { metaclass: Some("Class".into()) }), "UML metaclass Class");
    check(&mut s, &t, &entity, "element", None, "is the template parameter T of Customer");
    // What may not be applied: applied already, extending another metaclass or none, out of scope.
    let applied = Reason::StereotypeApplied { stereotype: "Entity".into(), element: "Order".into() };
    check(&mut s, &order, &entity, "stereotype", Some(applied), "«Entity» is applied to Order already");
    let unextended = |stereotype: &str, extends: &[&str]| {
        let extends = extends.iter().map(|m| m.to_string()).collect();
        Some(Reason::Unextended(Box::new(Unextended { stereotype: stereotype.into(), metaclass: "Class", extends })))
    };
    check(&mut s, &order, &id, "stereotype", unextended("Id", &["Property"]), "«Id» extends Property, and Order is a class");
    // An association class is a Class to a stereotype, whatever else it is.
    let line = run(&mut s, "uml.association-class.add", json!({ "from": order, "to": customer, "name": "Line" }));
    check(&mut s, &line, &id, "stereotype", unextended("Id", &["Property"]), "«Id» extends Property, and Line is an association class");
    let bare = run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Bare" }));
    check(&mut s, &order, &bare, "stereotype", unextended("Bare", &[]), "«Bare» extends no metaclass yet");
    let logging = run(&mut s, "uml.profile.add", json!({ "name": "Logging" }));
    let tracked = run(&mut s, "uml.stereotype.add", json!({ "profile": logging, "name": "Tracked", "extends": ["Class"] }));
    let out = Reason::ProfileOutOfScope { profile: "Logging".into(), package: "Commerce".into() };
    check(&mut s, &order, &tracked, "stereotype", Some(out), "which no package around Order applies; apply it to Commerce");
    // An element that carries the most takes no stereotype more.
    for n in 1..applications::MAX_APPLIED {
        let more = run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": format!("S{n}"), "extends": ["Class"] }));
        run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": more }));
    }
    let over = run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Over", "extends": ["Class"] }));
    let most = Reason::TooMany { kind: "applied stereotype", max: 32 };
    check(&mut s, &order, &over, "stereotype", Some(most), "Order carries 32 stereotypes, the most one element carries");
}

/// The session `s` with the package `package` moved to the model's top level, as a project made
/// before the root package keeps one: removed, it takes what it holds along.
fn at_top(s: &S, package: &str) -> S {
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut st = store(1 << 45);
    rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
    let p = obj(&st, package);
    let (parent, prop) = st.container(p).unwrap().unwrap();
    st.remove_value(parent, prop, &Value::Ref(p)).unwrap();
    let model = archforge_uml::commands::model(&st).unwrap();
    st.push(model, archforge_core::model::MEMBER, Value::Ref(p)).unwrap();
    session_over(st)
}

/// A package with no parent takes its profiles along when it is removed: one whose stereotypes are
/// used elsewhere is refused with the uses, changing nothing; an unused one goes in the same step
/// with every package's application of it, leaving no application, link or reference behind.
#[test]
fn a_package_taking_a_profile_along_is_checked_for_its_uses() {
    let mut s = session();
    let [commerce, order, ..] = shop(&mut s);
    let tools = run(&mut s, "uml.package.add", json!({ "name": "Tools" }));
    let audit = run(&mut s, "uml.profile.add", json!({ "name": "Audit", "package": tools }));
    let tracked = run(&mut s, "uml.stereotype.add", json!({ "profile": audit, "name": "Tracked", "extends": ["Class"] }));
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": audit }));
    // Used on Order in Commerce: removing Tools would take Audit along, so it is refused.
    let mut used = at_top(&with_applications(&s, &[(&tracked, &order)]), &tools);
    let why = refused(&mut used, "element.remove", json!({ "ids": [tools] }));
    assert!(why.contains("Tools holds the profile Audit, which is in use: «Tracked» on Order. Remove those first."), "{why}");
    // Unused, it goes with Tools in one step, and Commerce no longer applies it.
    let mut unused = at_top(&s, &tools);
    one_step(&mut unused, "element.remove", json!({ "ids": [tools] }));
    let st = unused.doc();
    assert!(profiles::applied(st, obj(st, &commerce)).is_empty(), "no application of a gone profile");
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_uml::PROFILE_APPLICATION)));
    assert!(st.alive().all(|o| st.class_of(o).ok() != Some(archforge_core::LINK)), "no link left");
    assert_eq!(st.first_problem_by(archforge_uml::names::kind), None);
}

/// `uml.profile.diagram` fills the profile's diagram as the window's Open profile diagram does:
/// every metaclass in a row, every stereotype under them and every extension line, as one step;
/// asked again it adds only what is missing, keeping what the diagram shows where it is.
#[test]
fn the_profile_diagram_command_fills_the_diagram_keeping_what_it_shows() {
    let mut s = session();
    let [_, _, _, _, persistence, entity, ..] = shop(&mut s);
    let d = one_step(&mut s, "uml.profile.diagram", json!({ "profile": persistence }));
    let count = |s: &S| {
        let st = s.doc();
        let dd = obj(st, &d);
        (st.list(dd, archforge_diagram::diagram::NODE).unwrap().len(), st.list(dd, archforge_diagram::diagram::EDGE).unwrap().len())
    };
    assert_eq!(count(&s), (6, 3), "three metaclasses, three stereotypes, three extensions");
    assert_eq!(read(&mut s, "uml.profile.diagram", json!({ "profile": persistence }))["added"], 0);
    // A card moved by hand stays; a new stereotype and its new metaclass are added under the rest.
    let node = {
        let st = s.doc();
        st.uuid(archforge_diagram::views_of(st, obj(st, &entity)).2[0]).unwrap().to_string()
    };
    run(&mut s, "diagram.move", json!({ "nodes": [node], "dx": 40, "dy": 0 }));
    let before = {
        let st = s.doc();
        st.get(obj(st, &node), archforge_diagram::node::X).unwrap()
    };
    run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Audited", "extends": ["Package"] }));
    assert_eq!(read(&mut s, "uml.profile.diagram", json!({ "profile": persistence }))["added"], 3);
    assert_eq!(count(&s), (8, 4));
    let st = s.doc();
    assert_eq!(st.get(obj(st, &node), archforge_diagram::node::X).unwrap(), before, "a card placed stays where it is");
}

// Stereotype applications and their values (AR-135).

/// The shop with Persistence applied to Commerce: as [`shop`].
fn applied_shop(s: &mut S) -> [String; 8] {
    let ids = shop(s);
    run(s, "uml.profile.apply", json!({ "package": ids[0], "profile": ids[4] }));
    ids
}

/// The ids of a stereotype's property named `name`.
fn property(s: &mut S, stereotype: &str, name: &str) -> String {
    let got = read(s, "uml.stereotype.get", json!({ "stereotype": stereotype }));
    got["properties"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap()["id"].as_str().unwrap().to_string()
}

/// A stereotype is applied and unapplied one undo step each: an InstanceSpecification classified by
/// it at the project's top level, a core trace Link to the element and no other change; unapplying
/// takes its values along and keeps the stereotype's definition.
#[test]
fn stereotypes_are_applied_and_unapplied_one_step_each() {
    let mut s = session();
    let [_, order, customer, _, _, entity, ..] = applied_shop(&mut s);
    let instance = one_step(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    {
        let st = s.doc();
        let i = obj(st, &instance);
        let model = archforge_uml::commands::model(st).unwrap();
        assert_eq!(st.container(i).unwrap(), Some((model, archforge_core::model::MEMBER)), "beside the library, in no package");
        assert_eq!(profiles::element_of(st, i), Some(obj(st, &order)));
        assert_eq!(applications::stereotype_of(st, i), Some(obj(st, &entity)));
        assert_eq!(applications::keywords(st, obj(st, &order)).as_deref(), Some("«Entity»"));
        assert_eq!(profiles::stereotype_uses(st, obj(st, &entity)).total(), 1, "AR-134's refusals see it");
        assert_eq!(st.first_problem_by(archforge_uml::names::kind), None);
    }
    let listed = read(&mut s, "uml.stereotype.list-applied", json!({ "element": order }));
    assert_eq!(listed["keywords"], "«Entity»");
    assert_eq!(listed["applied"][0]["profile"], "Persistence");
    assert_eq!(listed["applied"][0]["properties"][1]["default"], "false");
    assert_eq!(listed["applied"][0]["properties"][1]["set"], false);
    run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "table", "values": ["orders"] }));
    one_step(&mut s, "uml.stereotype.unapply", json!({ "element": order, "stereotype": entity }));
    let st = s.doc();
    assert!(applications::applied_to(st, obj(st, &order)).is_empty());
    assert!(profiles::is_stereotype(st, obj(st, &entity)), "the definition stays");
    for class in [archforge_uml::INSTANCE_SPECIFICATION, archforge_uml::SLOT, archforge_uml::LITERAL_STRING, archforge_core::LINK] {
        assert!(st.alive().all(|o| st.class_of(o).ok() != Some(class)), "nothing of the application is left");
    }
    run(&mut s, "uml.stereotype.apply", json!({ "element": customer, "stereotype": entity }));
    let why = refused(&mut s, "uml.stereotype.unapply", json!({ "element": order, "stereotype": entity }));
    assert!(why.contains("«Entity» is not applied to Order"), "{why}");
}

/// The picker (F4) lists every stereotype: those that extend the element's metaclass and whose
/// profile a package around it applies are open, one applied already is marked, and the rest are
/// refused with why; applying a refused one is refused alike, changing nothing.
#[test]
fn the_picker_offers_what_extends_the_element_in_scope() {
    let mut s = session();
    let [commerce, order, _, id_attr, _, entity, id, reference] = applied_shop(&mut s);
    let logging = run(&mut s, "uml.profile.add", json!({ "name": "Logging" }));
    let tracked = run(&mut s, "uml.stereotype.add", json!({ "profile": logging, "name": "Tracked", "extends": ["Class"] }));
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let offered = read(&mut s, "uml.stereotype.applicable", json!({ "element": order }));
    let state = |name: &str| {
        let all = offered["stereotypes"].as_array().unwrap();
        let s = all.iter().find(|s| s["name"] == name).unwrap();
        (s["state"].as_str().unwrap().to_string(), s["why"].as_str().unwrap_or_default().to_string())
    };
    assert_eq!(state("Entity").0, "applied");
    assert_eq!(state("Id"), ("refused".into(), "«Id» extends Property, and Order is a class".into()));
    assert!(state("Reference").1.contains("extends Association"));
    let (tracked_state, why) = state("Tracked");
    assert_eq!(tracked_state, "refused");
    assert!(why.contains("«Tracked» is of the profile Logging, which no package around Order applies; apply it to Commerce"), "{why}");
    assert!(refused(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": tracked })).contains("no package around Order"));
    assert!(refused(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": id })).contains("«Id» extends Property"));
    assert!(refused(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity })).contains("applied to Order already"));
    // Applied around Order, Tracked opens; Id applies to an attribute, which is a Property.
    run(&mut s, "uml.profile.apply", json!({ "package": commerce, "profile": logging }));
    one_step(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": tracked }));
    one_step(&mut s, "uml.stereotype.apply", json!({ "element": id_attr, "stereotype": id }));
    assert_eq!(read(&mut s, "uml.stereotype.list-applied", json!({ "element": order }))["keywords"], "«Entity, Tracked»");
    // What takes no stereotype: a stereotype, UML's own metaclasses, an application, a template
    // parameter's classifier.
    let class = {
        let st = s.doc();
        st.uuid(profiles::metaclass(st, "Class").unwrap()).unwrap().to_string()
    };
    assert!(refused(&mut s, "uml.stereotype.apply", json!({ "element": entity, "stereotype": entity })).contains("belongs to a profile"));
    assert!(refused(&mut s, "uml.stereotype.apply", json!({ "element": class, "stereotype": entity })).contains("UML metaclass Class"));
    let instance = read(&mut s, "uml.stereotype.list-applied", json!({ "element": order }))["applied"][0]["id"].as_str().unwrap().to_string();
    assert!(refused(&mut s, "uml.stereotype.apply", json!({ "element": instance, "stereotype": entity })).contains("is a stereotype application"));
    assert_eq!(read(&mut s, "uml.stereotype.applicable", json!({ "element": instance }))["takes"], false);
    let _ = reference;
}

/// Every kind whose metaclass a stereotype extends takes it: the 0.12.0 classifiers, an
/// association class as a Class and an Association, a nested classifier, a template, an operation,
/// an association, a generalization and a package; the package around a nested classifier is its
/// owner's.
#[test]
fn every_extended_kind_takes_its_stereotypes() {
    let mut s = session();
    let [commerce, order, customer, _, persistence, ..] = applied_shop(&mut s);
    let all = run(
        &mut s,
        "uml.stereotype.add",
        json!({ "profile": persistence, "name": "Any", "extends": ["Class", "Interface", "Enumeration", "DataType", "Operation", "Association", "Generalization", "Package"] }),
    );
    let mut elements = Vec::new();
    for (cmd, name) in
        [("uml.interface.add", "Priced"), ("uml.enumeration.add", "Status"), ("uml.data-type.add", "Money"), ("uml.primitive-type.add", "Sku")]
    {
        elements.push(run(&mut s, cmd, json!({ "package": commerce, "name": name })));
    }
    elements.push(run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "enumeration", "name": "Priority" })));
    let repo = run(&mut s, "uml.class.add", json!({ "package": commerce, "name": "Repository" }));
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": repo, "name": "T" }));
    elements.push(repo);
    elements.push(run(&mut s, "uml.association-class.add", json!({ "from": order, "to": customer, "name": "Line" })));
    elements.push(run(&mut s, "uml.association.add", json!({ "from": order, "to": customer })));
    elements.push(run(&mut s, "uml.generalization.add", json!({ "specific": customer, "general": order })));
    elements.push(run(&mut s, "uml.operation.add", json!({ "class": order, "name": "total" })));
    elements.push(commerce.clone());
    for e in &elements {
        one_step(&mut s, "uml.stereotype.apply", json!({ "element": e, "stereotype": all }));
    }
    assert_eq!(profiles::stereotype_uses(s.doc(), obj(s.doc(), &all)).total(), elements.len());
    // A template parameter's classifier is its template's own.
    let param = {
        let st = s.doc();
        let r = obj(st, &elements[5]);
        let c = st.alive().find(|o| archforge_uml::templates::is_parameter_element(st, *o) && st.contains(r, *o)).unwrap();
        st.uuid(c).unwrap().to_string()
    };
    assert!(!refused(&mut s, "uml.stereotype.apply", json!({ "element": param, "stereotype": all })).is_empty());
}

/// Each value is read by its property's type, as a default is: a String's any text, an Integer's a
/// whole number, a Real's a number, a Boolean's true or false, an UnlimitedNatural's a whole number
/// or *, an enumeration's a literal (kept as an InstanceValue), a data type's an expression's text;
/// a value that does not read, too many or too few for the multiplicity, and the same one twice in
/// a unique property are refused naming why, changing nothing.
#[test]
fn values_are_read_by_type_and_multiplicity() {
    let mut s = session();
    let [commerce, order, _, _, _, entity, ..] = applied_shop(&mut s);
    let fetch = run(&mut s, "uml.enumeration.add", json!({ "package": commerce, "name": "Fetch" }));
    for l in ["eager", "lazy"] {
        run(&mut s, "uml.literal.add", json!({ "enumeration": fetch, "name": l }));
    }
    let money = run(&mut s, "uml.data-type.add", json!({ "package": commerce, "name": "Money" }));
    for (name, ty, multiplicity) in [
        ("rows", "Integer", ""),
        ("ratio", "Real", ""),
        ("limit", "UnlimitedNatural", ""),
        ("fetch", fetch.as_str(), ""),
        ("budget", money.as_str(), ""),
        ("tags", "String", "0..2"),
    ] {
        run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": name, "type": ty, "multiplicity": multiplicity }));
    }
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let set = |s: &mut S, property: &str, values: Json| {
        let p = json!({ "element": order, "stereotype": entity, "property": property, "values": values });
        one_step(s, "uml.stereotype.set-value", p);
    };
    let wrong = |s: &mut S, property: &str, values: Json| {
        refused(s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": property, "values": values }))
    };
    set(&mut s, "table", json!(["orders"]));
    set(&mut s, "cached", json!(["TRUE"]));
    set(&mut s, "rows", json!([" 42 "]));
    set(&mut s, "ratio", json!(["2.5"]));
    set(&mut s, "limit", json!(["*"]));
    set(&mut s, "fetch", json!(["Lazy"]));
    set(&mut s, "budget", json!(["10 EUR"]));
    set(&mut s, "tags", json!(["a", "b"]));
    let listed = read(&mut s, "uml.stereotype.list-applied", json!({ "element": order }));
    let values: Vec<(String, Json)> = listed["applied"][0]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap().to_string(), p["values"].clone()))
        .collect();
    assert_eq!(
        values,
        [
            ("table".to_string(), json!(["orders"])),
            ("cached".into(), json!(["true"])),
            ("rows".into(), json!(["42"])),
            ("ratio".into(), json!(["2.5"])),
            ("limit".into(), json!(["*"])),
            ("fetch".into(), json!(["lazy"])),
            ("budget".into(), json!(["10 EUR"])),
            ("tags".into(), json!(["a", "b"])),
        ]
    );
    {
        let st = s.doc();
        let i = applications::application(st, obj(st, &order), obj(st, &entity)).unwrap();
        let fetch_p = profiles::properties(st, obj(st, &entity)).into_iter().find(|p| name(st, *p) == "fetch").unwrap();
        let v = applications::values(st, i, fetch_p)[0];
        assert_eq!(st.class_of(v).unwrap(), archforge_uml::INSTANCE_VALUE, "a literal is referred to, never copied as text");
    }
    assert!(wrong(&mut s, "rows", json!(["many"])).contains("\"many\" is no value of rows: an Integer is a whole number"));
    assert!(wrong(&mut s, "cached", json!(["yes"])).contains("a Boolean is true or false"));
    assert!(wrong(&mut s, "fetch", json!(["never"])).contains("one of its literals: eager, lazy"));
    assert!(wrong(&mut s, "ratio", json!(["NaN"])).contains("a Real is a number"));
    assert!(wrong(&mut s, "table", json!(["a", "b"])).contains("the multiplicity of table, [1], does not allow 2 values"));
    assert!(wrong(&mut s, "tags", json!(["a", "b", "c"])).contains("[0..2]"));
    assert!(wrong(&mut s, "tags", json!(["a", "a"])).contains("tags is unique and holds \"a\" twice"));
    assert!(wrong(&mut s, "tags", json!(["  "])).contains("a value is empty"));
    assert!(wrong(&mut s, "colour", json!(["red"])).contains("«Entity» has no property \"colour\""));
    let why = wrong(&mut s, "tags", json!("a"));
    assert!(why.contains("values"), "{why}");
}

/// A property left unset shows its default and is not set; a value set, even one equal to the
/// default, is set; clearing it shows the default again.
#[test]
fn defaults_are_kept_apart_from_values_set() {
    let mut s = session();
    let [_, order, _, _, _, entity, ..] = applied_shop(&mut s);
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let cached = |s: &mut S| read(s, "uml.stereotype.list-applied", json!({ "element": order }))["applied"][0]["properties"][1].clone();
    assert_eq!(
        (cached(&mut s)["set"].clone(), cached(&mut s)["values"].clone(), cached(&mut s)["default"].clone()),
        (json!(false), json!([]), json!("false"))
    );
    run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "cached", "values": ["false"] }));
    assert_eq!((cached(&mut s)["set"].clone(), cached(&mut s)["values"].clone()), (json!(true), json!(["false"])), "set to the default, it is set");
    one_step(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "cached", "values": [] }));
    assert_eq!(cached(&mut s)["set"], false);
    assert!(s.doc().alive().all(|o| s.doc().class_of(o).ok() != Some(archforge_uml::SLOT)), "clearing takes the slot away");
}

/// On a card the stereotypes replace the kind's word (`«Entity»` for Class), a keyword kind keeping
/// its keyword first; a member's row and a line write them before their text; a diagram lists the
/// values set in an `«Entity»` compartment only while it shows them, each diagram its own choice.
#[test]
fn cards_members_and_lines_show_their_stereotypes() {
    let mut s = session();
    let [commerce, order, customer, id_attr, persistence, entity, id, reference] = applied_shop(&mut s);
    let priced = run(&mut s, "uml.interface.add", json!({ "package": commerce, "name": "Priced" }));
    run(&mut s, "uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Class", "Interface"] }));
    let places = run(&mut s, "uml.association.add", json!({ "from": customer, "to": order, "name": "places" }));
    for (e, st) in [(&order, &entity), (&priced, &entity), (&id_attr, &id), (&places, &reference)] {
        run(&mut s, "uml.stereotype.apply", json!({ "element": e, "stereotype": st }));
    }
    run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "table", "values": ["orders"] }));
    let one = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "name": "One", "element": commerce }));
    let two = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "name": "Two", "element": commerce }));
    let rows = |s: &S, d: &str| {
        let st = s.doc();
        let card = UmlNotation.card(st, obj(st, &order), obj(st, d)).unwrap();
        card.compartments.iter().map(|c| c.iter().map(|r| r.text.clone()).collect::<Vec<_>>()).collect::<Vec<_>>()
    };
    {
        let st = s.doc();
        let (o, d) = (obj(st, &order), obj(st, &one));
        assert_eq!(UmlNotation.card(st, o, d).unwrap().eyebrow, archforge_diagram::figure::Text::Plain("«Entity»".into()));
        assert_eq!(UmlNotation.card(st, obj(st, &priced), d).unwrap().eyebrow, archforge_diagram::figure::Text::Plain("«interface, Entity»".into()));
        assert_eq!(UmlNotation.line(st, obj(st, &places)).unwrap().keyword.as_deref(), Some("«Reference»"));
        assert_eq!(UmlNotation.line(st, obj(st, &places)).unwrap().ends[0].role, None);
        assert_eq!(archforge_uml::figures::member_text(st, obj(st, &id_attr)), "«Id» + id : Integer");
    }
    assert_eq!(rows(&s, &one), [vec!["«Id» + id : Integer".to_string()], vec![]], "no values while the diagram hides them");
    one_step(&mut s, "uml.stereotype.show-values", json!({ "diagram": one, "show": true }));
    assert_eq!(rows(&s, &one)[2], ["«Entity»", "table = \"orders\""], "a value set shows, a default does not");
    assert_eq!(rows(&s, &two).len(), 2, "the other diagram keeps its own choice");
    assert_eq!(read(&mut s, "uml.stereotype.list-applied", json!({ "element": order, "diagram": one }))["values_shown"], true);
    assert_eq!(read(&mut s, "uml.stereotype.list-applied", json!({ "element": order, "diagram": two }))["values_shown"], false);
    // An outsider names its package beside its stereotypes.
    let other = run(&mut s, "uml.package.add", json!({ "name": "Other" }));
    let elsewhere = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "name": "E", "element": other }));
    {
        let st = s.doc();
        let eyebrow = UmlNotation.card(st, obj(st, &order), obj(st, &elsewhere)).unwrap().eyebrow;
        assert_eq!(
            eyebrow,
            archforge_diagram::figure::Text::Said("{keywords} · {package}", vec![("keywords", "«Entity»".into()), ("package", "Commerce".into())])
        );
    }
    // One tag lists the diagrams that show values; turning the last off takes it away.
    let tags = |s: &S| {
        let st = s.doc();
        st.alive()
            .filter(|o| st.class_of(*o).ok() == Some(archforge_core::TAG) && archforge_core::name_of(st, *o) == applications::VALUES_TAG)
            .count()
    };
    one_step(&mut s, "uml.stereotype.show-values", json!({ "diagram": two, "show": true }));
    assert_eq!(tags(&s), 1, "one tag for both diagrams");
    run(&mut s, "uml.stereotype.show-values", json!({ "diagram": two, "show": false }));
    assert_eq!(rows(&s, &one)[2], ["«Entity»", "table = \"orders\""], "the other keeps its choice");
    run(&mut s, "uml.stereotype.show-values", json!({ "diagram": one, "show": false }));
    assert_eq!(tags(&s), 0, "the last turned off takes the tag away");
    // A removed diagram leaves the tag on nothing, which the next one turned on reuses.
    run(&mut s, "uml.stereotype.show-values", json!({ "diagram": one, "show": true }));
    one_step(&mut s, "diagram.remove", json!({ "diagram": one }));
    run(&mut s, "uml.stereotype.show-values", json!({ "diagram": two, "show": true }));
    assert_eq!(tags(&s), 1);
    let st = s.doc();
    assert_eq!(st.first_problem_by(archforge_uml::names::kind), None);
    let _ = persistence;
    assert!(refused(&mut s, "uml.stereotype.show-values", json!({ "diagram": order, "show": true })).contains("is not a class diagram"));
}

/// Deleting an element takes its stereotype applications and their values along in one step; a
/// literal a value names, a new type for a property whose values are set, and bounds a set value
/// breaks are refused, so no value is lost unseen.
#[test]
fn values_are_never_lost_unseen() {
    let mut s = session();
    let [commerce, order, customer, id_attr, _, entity, id, _] = applied_shop(&mut s);
    let fetch = run(&mut s, "uml.enumeration.add", json!({ "package": commerce, "name": "Fetch" }));
    let lazy = run(&mut s, "uml.literal.add", json!({ "enumeration": fetch, "name": "lazy" }));
    let fetch_p = run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "fetch", "type": fetch }));
    let tags = run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "tags", "type": "String", "multiplicity": "*" }));
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    run(&mut s, "uml.stereotype.apply", json!({ "element": id_attr, "stereotype": id }));
    run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "fetch", "values": ["lazy"] }));
    run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "tags", "values": ["a", "b"] }));
    let why = refused(&mut s, "element.remove", json!({ "ids": [lazy] }));
    assert!(why.contains("the literal lazy is the value of fetch in «Entity» on Order"), "{why}");
    assert!(refused(&mut s, "element.remove", json!({ "ids": [fetch] })).contains("the literal lazy"));
    assert!(refused(&mut s, "element.remove", json!({ "ids": [fetch_p] })).contains("the values of fetch are set on 1 element"));
    let why = refused(&mut s, "uml.attribute.set", json!({ "attribute": tags, "type": "Integer" }));
    assert!(why.contains("clear them before giving it another type"), "{why}");
    let why = refused(&mut s, "uml.attribute.set", json!({ "attribute": tags, "multiplicity": "0..1" }));
    assert!(why.contains("do not fit tags [0..1]"), "{why}");
    run(&mut s, "uml.attribute.set", json!({ "attribute": tags, "multiplicity": "1..5" }));
    // Deleting the carrier: Order with its id attribute and their applications, one step.
    one_step(&mut s, "element.remove", json!({ "ids": [order] }));
    let st = s.doc();
    assert_eq!(profiles::stereotype_uses(st, obj(st, &entity)).total(), 0);
    assert_eq!(profiles::stereotype_uses(st, obj(st, &id)).total(), 0);
    for class in [archforge_uml::INSTANCE_SPECIFICATION, archforge_uml::SLOT, archforge_uml::INSTANCE_VALUE, archforge_core::LINK] {
        assert!(st.alive().all(|o| st.class_of(o).ok() != Some(class)), "nothing of the applications is left");
    }
    // Free again, the literal goes.
    one_step(&mut s, "element.remove", json!({ "ids": [lazy] }));
    let _ = customer;
}

/// Applications and their values are saved as rows and read back identical, the model opening
/// again; moving what carries them keeps them, their instance staying at the top level.
#[test]
fn applications_are_saved_read_back_and_kept_through_moves() {
    let mut s = session();
    let [commerce, order, _, _, persistence, entity, ..] = applied_shop(&mut s);
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": "table", "values": ["orders"] }));
    let other = run(&mut s, "uml.package.add", json!({ "name": "Other" }));
    run(&mut s, "uml.profile.apply", json!({ "package": other, "profile": persistence }));
    one_step(&mut s, "uml.class.move", json!({ "class": order, "package": other }));
    // Commerce, emptied of Order, may now go: its contents move up and the application stays.
    one_step(&mut s, "element.remove", json!({ "ids": [commerce] }));
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut back = store(1 << 35);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved, "the rows read back are the rows written");
    let o = obj(&back, &order);
    let e = obj(&back, &entity);
    let i = applications::application(&back, o, e).expect("still applied");
    let table = profiles::properties(&back, e)[0];
    assert_eq!(applications::value_texts(&back, i, table), ["orders"]);
    assert_eq!(back.first_problem_by(archforge_uml::names::kind), None, "the model opens again");
}

/// The sweep over the applications' commands breaks nothing.
#[test]
fn the_sweep_over_applications_breaks_nothing() {
    let mut s = session();
    let [commerce, order, _, id_attr, _, entity, id, _] = applied_shop(&mut s);
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let d = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "name": "One", "element": commerce }));
    let listed: Vec<String> = s.registry().infos(&s.facts()).into_iter().map(|i| i.id.to_string()).collect();
    for cmd in [
        "uml.stereotype.apply",
        "uml.stereotype.unapply",
        "uml.stereotype.set-value",
        "uml.stereotype.list-applied",
        "uml.stereotype.applicable",
        "uml.stereotype.show-values",
    ] {
        assert!(listed.iter().any(|l| l == cmd), "{cmd} is swept");
    }
    let table = property(&mut s, &entity, "table");
    let ids = [order.as_str(), entity.as_str(), id.as_str(), id_attr.as_str(), table.as_str(), d.as_str(), "table", "not-an-id"];
    let calls = sweep_with(&mut s, &ids, picture).unwrap();
    assert!(calls > 50, "{calls}");
}

/// An n-ary association takes the stereotypes extending Association, written above its diamond's
/// name, and each of its ends those extending Property, written at its leg's middle (AR-135 over
/// AR-128).
#[test]
fn n_ary_associations_and_their_legs_show_their_stereotypes() {
    let mut s = session();
    let [commerce, order, customer, _, _, _, id, reference] = applied_shop(&mut s);
    let product = run(&mut s, "uml.class.add", json!({ "package": commerce, "name": "Product" }));
    let a = run(&mut s, "uml.association.add-nary", json!({ "classes": [customer, order, product], "name": "recommends" }));
    one_step(&mut s, "uml.stereotype.apply", json!({ "element": a, "stereotype": reference }));
    let end = {
        let st = s.doc();
        st.uuid(archforge_uml::nary::ends(st, obj(st, &a))[0]).unwrap().to_string()
    };
    one_step(&mut s, "uml.stereotype.apply", json!({ "element": end, "stereotype": id }));
    let d = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "name": "One", "element": commerce }));
    let st = s.doc();
    let card = UmlNotation.card(st, obj(st, &a), obj(st, &d)).unwrap();
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Plain("«Reference»".into()));
    assert_eq!(UmlNotation.line(st, obj(st, &end)).unwrap().keyword.as_deref(), Some("«Id»"));
    let _ = order;
}
