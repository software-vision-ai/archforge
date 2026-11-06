//! Profiles and stereotypes through the engine (AR-134): a profile made in a package, its
//! stereotypes extending metaclasses of the hidden UML library and owning typed properties, each
//! change one undo step; unsupported metaclasses and property types refused with the reason; a
//! profile applied to a package and unapplied; unapplying, removing and moving refused while
//! stereotype applications (built directly in the store, as AR-135 will make them) use them, the
//! uses counted and named; the library kept out of types and reads; saved as rows and read back
//! identical, the model still opening; the profile's diagram; the sweep.

use archforge_diagram::{Notation, ProjectView};
use archforge_engine::testing::sweep_with;
use archforge_engine::{Options, Session};
use archforge_mof::{CompositeSlabs, Obj, Store, Uuid, Value, rows};
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

/// The session `s` with stereotype applications built directly in its store, as AR-135 will make
/// them: an InstanceSpecification classified by the stereotype, in the element's package, and a
/// core Link from it to the element. Gives a session over the rows read back with them.
fn with_applications(s: &S, uses: &[(&str, &str)]) -> S {
    let (tables, junctions) = tables();
    let saved = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut st = store(1 << 40);
    rows::restore(&mut st, &saved, &tables, &junctions).unwrap();
    let model = archforge_uml::commands::model(&st).unwrap();
    for (stereotype, element) in uses {
        let (stereotype, element) = (obj(&st, stereotype), obj(&st, element));
        let home = profiles::home_of(&st, element).unwrap();
        let instance = st.create(archforge_uml::INSTANCE_SPECIFICATION).unwrap();
        st.push(instance, archforge_uml::instance_specification::CLASSIFIER, Value::Ref(stereotype)).unwrap();
        st.push(home, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(instance)).unwrap();
        archforge_core::link(&mut st, model, profiles::APPLICATION_LINK, instance, element, None).unwrap();
    }
    session_over(st)
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
