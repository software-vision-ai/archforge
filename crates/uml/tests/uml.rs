//! The generated UML metamodel over the MOF core: the universe builds, the counts are OMG's, a
//! class with a typed attribute behaves as UML says through the store, and the generic machinery
//! (derived unions, opposites, destroy, undo) holds on the real metaclasses.

use archforge_engine::{CommandError, Ctx, Document, Options, Params, Registry, Session, View, always, cmd};
use archforge_mof::{Obj, Store, Uuid, Value};
use archforge_uml::*;
use serde_json::{Value as Json, json};

fn ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 0;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

fn store() -> Store {
    Store::new(universe().unwrap(), Box::new(UmlSlabs::default()), ids())
}

fn s(v: &str) -> Value {
    Value::str(v)
}

#[test]
fn the_universe_builds_with_omgs_counts() {
    let m = metamodel().unwrap();
    assert_eq!(m.classes.len(), 242);
    assert_eq!(m.classes.iter().filter(|c| c.is_abstract).count(), 49, "51 isAbstract flags in the file, two of them on associations");
    assert_eq!(m.props.len(), 622, "950 Property elements in the file, 328 of them association-owned ends");
    assert_eq!(m.props.iter().filter(|p| p.is_derived || p.is_derived_union).count(), 73);
    assert_eq!(m.props.iter().filter(|p| p.is_composite).count(), 213);
    assert_eq!(m.data_types.len(), 5 + 13, "the five primitives and thirteen enumerations");
    let u = universe().unwrap();
    let class = u.info(CLASS).unwrap();
    assert!(class.all_supers.contains(&ELEMENT) && class.all_supers.contains(&NAMESPACE) && class.all_supers.contains(&CLASSIFIER));
    assert_eq!(class.name_prop, Some(named_element::NAME));
    assert!(class.unions[&element::OWNED_ELEMENT].contains(&class::OWNED_ATTRIBUTE));
    assert!(class.unions[&namespace::OWNED_MEMBER].contains(&class::OWNED_ATTRIBUTE));
    assert_eq!(u.prop(property::CLASS).unwrap().opposite, Some(class::OWNED_ATTRIBUTE));
    assert_eq!(u.prop(class::OWNED_ATTRIBUTE).unwrap().opposite, Some(property::CLASS));
    assert_eq!(u.prop_name(classifier::IS_ABSTRACT), "Classifier::isAbstract");
    assert!(u.has(CLASS, class::IS_ABSTRACT) && !u.has(CLASS, classifier::IS_ABSTRACT), "Class redefines isAbstract");
    assert_eq!(VisibilityKind::from_index(0), Some(VisibilityKind::Public));
    assert_eq!(VisibilityKind::Public.literal(), "public");
    assert!(TABLES.iter().any(|t| t.name == "uml_class") && JUNCTIONS.iter().any(|j| j.name == "uml_class_owned_attribute" && j.ordered));
}

#[test]
fn a_class_with_a_typed_attribute_behaves_as_uml_says() {
    let mut st = store();
    let pkg = st.create(PACKAGE).unwrap();
    PackageObj::set_name(&mut st, pkg, Some("model")).unwrap();
    let string = st.create(PRIMITIVE_TYPE).unwrap();
    PrimitiveTypeObj::set_name(&mut st, string, Some("String")).unwrap();
    st.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(string)).unwrap();
    let customer = st.create(CLASS).unwrap();
    ClassObj::set_name(&mut st, customer, Some("Customer")).unwrap();
    PackageObj::push_packaged_element(&mut st, pkg, Value::Ref(customer)).unwrap();
    let name = st.create(PROPERTY).unwrap();
    PropertyObj::set_name(&mut st, name, Some("name")).unwrap();
    PropertyObj::set_type_(&mut st, name, Some(string)).unwrap();
    ClassObj::push_owned_attribute(&mut st, customer, Value::Ref(name)).unwrap();
    // Both ends of ownedAttribute and class, containment, and the derived unions.
    assert_eq!(st.get(name, property::CLASS).unwrap(), Value::Ref(customer));
    assert_eq!(st.container(name).unwrap(), Some((customer, class::OWNED_ATTRIBUTE)));
    assert_eq!(st.list(customer, element::OWNED_ELEMENT).unwrap(), vec![Value::Ref(name)]);
    assert_eq!(st.list(customer, namespace::OWNED_MEMBER).unwrap(), vec![Value::Ref(name)]);
    assert_eq!(st.list(customer, classifier::ATTRIBUTE).unwrap(), vec![Value::Ref(name)], "attribute is a derived union of ownedAttribute");
    assert_eq!(st.list(pkg, namespace::OWNED_MEMBER).unwrap(), vec![Value::Ref(string), Value::Ref(customer)]);
    assert_eq!(st.get(customer, class::IS_ABSTRACT).unwrap(), Value::Bool(false), "the default");
    assert_eq!(st.get(name, property::AGGREGATION).unwrap(), Value::Enum(AggregationKind::NoneKind.index()));
    assert_eq!(st.by_name(named_element::NAME, "customer"), vec![customer]);
    // The typed view of the object.
    let obj = st.reflect(customer).unwrap();
    assert_eq!(obj.get(named_element::NAME), Some(s("Customer")));
    // Destroying the primitive type drops the attribute's type; destroying the class takes the
    // attribute with it and clears the package's list.
    st.destroy(string).unwrap();
    assert_eq!(st.get(name, typed_element::TYPE).unwrap(), Value::Null);
    st.begin();
    assert_eq!(st.destroy(customer).unwrap(), 2);
    let delta = st.end().unwrap();
    assert_eq!(st.list(pkg, package::PACKAGED_ELEMENT).unwrap(), vec![]);
    st.apply(&Store::invert(&delta)).unwrap();
    assert_eq!(st.list(pkg, package::PACKAGED_ELEMENT).unwrap(), vec![Value::Ref(customer)]);
    assert_eq!(st.get(name, property::CLASS).unwrap(), Value::Ref(customer));
    assert_eq!(st.validate_all(), vec![]);
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Selection(Vec<Obj>);

impl View<Store> for Selection {
    fn retain_valid(&mut self, doc: &Store) {
        self.0.retain(|o| doc.is_alive(*o));
    }
}

type C<'a> = Ctx<'a, Store, Selection>;

fn fail(e: archforge_mof::Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn obj(ctx: &C<'_>, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let u = p.str(key).and_then(Uuid::parse).ok_or_else(|| CommandError::about(key, "not an id"))?;
    ctx.doc.by_uuid(u).ok_or_else(|| CommandError::about(key, "there is no such element"))
}

fn add_class(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let c = ctx.doc.create(CLASS).map_err(fail)?;
    ClassObj::set_name(ctx.doc, c, Some(p.str("name").unwrap_or("Class"))).map_err(fail)?;
    ctx.view.0 = vec![c];
    Ok(json!({ "id": ctx.doc.uuid(c).map_err(fail)?.to_string() }))
}

fn add_attribute(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let c = obj(ctx, p, "class")?;
    let a = ctx.doc.create(PROPERTY).map_err(fail)?;
    PropertyObj::set_name(ctx.doc, a, Some(p.str("name").unwrap_or("attribute"))).map_err(fail)?;
    ClassObj::push_owned_attribute(ctx.doc, c, Value::Ref(a)).map_err(fail)?;
    Ok(json!({ "id": ctx.doc.uuid(a).map_err(fail)?.to_string() }))
}

fn delete(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let o = obj(ctx, p, "element")?;
    Ok(json!({ "removed": ctx.doc.destroy(o).map_err(fail)? }))
}

#[test]
fn the_sweep_finds_nothing_on_the_uml_metaclasses() {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend([
        cmd!("class.add", "Add class", &["Class"], None, "name: string = \"Class\"; its name", always, add_class),
        cmd!(
            "class.add-attribute",
            "Add attribute",
            &["Class"],
            None,
            "class: string; the class's id\nname: string = \"attribute\"; its name",
            always,
            add_attribute
        ),
        cmd!("element.delete", "Delete", &["Edit"], None, "element: string; its id", always, delete),
    ]);
    let mut session = Session::new(Registry::new(specs).unwrap(), store(), 1, Options::default());
    let c = session.execute("class.add", &json!({ "name": "Order" })).unwrap().value["id"].as_str().unwrap().to_string();
    let a = session.execute("class.add-attribute", &json!({ "class": c, "name": "total" })).unwrap().value["id"].as_str().unwrap().to_string();
    let picture = |st: &Store| (st.live(), st.alive().map(|o| (o, st.container(o).unwrap())).collect::<Vec<_>>());
    let calls = archforge_engine::testing::sweep_with(&mut session, &[&c, &a, "nope"], picture).unwrap();
    assert!(calls > 100, "{calls}");
    while session.can_undo() {
        session.undo().unwrap();
    }
    assert_eq!(session.doc().live(), 0, "every edit the sweep made undone");
}

/// Release only: the MOF core's million-object figures on the real metaclasses.
#[test]
#[ignore = "run with --release for the numbers"]
fn a_million_uml_objects_edit_inside_a_frame() {
    let mut st = store();
    let pkg = st.create(PACKAGE).unwrap();
    let mut classes = Vec::new();
    for i in 0..100_000 {
        let c = st.create(CLASS).unwrap();
        ClassObj::set_name(&mut st, c, Some(&format!("Class{i}"))).unwrap();
        st.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        for j in 0..3 {
            let p = st.create(PROPERTY).unwrap();
            PropertyObj::set_name(&mut st, p, Some(&format!("a{j}"))).unwrap();
            st.push(c, class::OWNED_ATTRIBUTE, Value::Ref(p)).unwrap();
            let l = st.create(LITERAL_INTEGER).unwrap();
            st.set(p, multiplicity_element::LOWER_VALUE, Value::Ref(l)).unwrap();
            let u = st.create(LITERAL_UNLIMITED_NATURAL).unwrap();
            st.set(p, multiplicity_element::UPPER_VALUE, Value::Ref(u)).unwrap();
        }
        classes.push(c);
    }
    println!("built {} objects, {} MB", st.live(), st.bytes() / 1_000_000);
    assert!(st.live() >= 1_000_000);
    let mut times = Vec::new();
    for i in 0..10_000 {
        let c = classes[i * 7 % classes.len()];
        let t = std::time::Instant::now();
        ClassObj::set_name(&mut st, c, Some(&format!("R{i}"))).unwrap();
        times.push(t.elapsed());
    }
    times.sort();
    let p99 = times[times.len() * 99 / 100];
    println!("rename p99 {p99:?}, {} MB", st.bytes() / 1_000_000);
    assert!(p99 < std::time::Duration::from_millis(16));
}
