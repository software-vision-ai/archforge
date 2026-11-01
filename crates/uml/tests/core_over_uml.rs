//! The shared core over UML in one store: a Model with a Folder holding a UML Package, comments and
//! tags on elements of two notations, trace links between UML classes, destroy with links and undo,
//! the core's rules, and the sweep. These tests live here, with the notation, because a dependency
//! only ever points down a layer: the core (5) cannot test over UML (6).

use archforge_core::*;
use archforge_engine::{CommandError, Ctx, Document, Options, Params, Registry, Session, View, always, cmd};
use archforge_mof::{CompositeSlabs, Invalid, Obj, Store, Uuid, Value};
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

/// A store over the core and UML together.
fn store() -> Store {
    let universe = universe_with(vec![archforge_uml::metamodel().unwrap()]).unwrap();
    let slabs = CompositeSlabs::new()
        .with(INDEX, Box::new(CoreSlabs::default()))
        .unwrap()
        .with(archforge_uml::INDEX, Box::new(archforge_uml::UmlSlabs::default()))
        .unwrap();
    Store::new(universe, Box::new(slabs), ids())
}

struct Sample {
    model: Obj,
    folder: Obj,
    pkg: Obj,
    customer: Obj,
    order: Obj,
    comment: Obj,
    tag: Obj,
    link: Obj,
}

fn sample(st: &mut Store) -> Sample {
    let model = st.create(MODEL).unwrap();
    ModelObj::set_name(st, model, Some("Shop")).unwrap();
    let folder = st.create(FOLDER).unwrap();
    FolderObj::set_name(st, folder, Some("Domain")).unwrap();
    ModelObj::push_member(st, model, Value::Ref(folder)).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_uml::PackageObj::set_name(st, pkg, Some("sales")).unwrap();
    FolderObj::push_member(st, folder, Value::Ref(pkg)).unwrap();
    let customer = st.create(archforge_uml::CLASS).unwrap();
    archforge_uml::ClassObj::set_name(st, customer, Some("Customer")).unwrap();
    st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(customer)).unwrap();
    let order = st.create(archforge_uml::CLASS).unwrap();
    archforge_uml::ClassObj::set_name(st, order, Some("Order")).unwrap();
    st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(order)).unwrap();
    let comment = st.create(COMMENT).unwrap();
    CommentObj::set_body(st, comment, Some("the buyer")).unwrap();
    CommentObj::push_annotated_element(st, comment, Value::Ref(customer)).unwrap();
    ModelObj::push_comment(st, model, Value::Ref(comment)).unwrap();
    let tag = st.create(TAG).unwrap();
    TagObj::set_name(st, tag, Some("owner")).unwrap();
    TagObj::set_value(st, tag, Some("sales team")).unwrap();
    TagObj::push_element(st, tag, Value::Ref(customer)).unwrap();
    TagObj::push_element(st, tag, Value::Ref(folder)).unwrap();
    ModelObj::push_tag(st, model, Value::Ref(tag)).unwrap();
    let link = archforge_core::link(st, model, LinkKind::Realization, order, customer, Some("an order is placed by a customer")).unwrap();
    Sample { model, folder, pkg, customer, order, comment, tag, link }
}

#[test]
fn the_core_and_uml_share_one_universe_and_one_store() {
    let mut st = store();
    let sm = sample(&mut st);
    assert_eq!(st.container(sm.pkg).unwrap(), Some((sm.folder, folder::MEMBER)), "a folder contains a UML package");
    assert_eq!(st.container(sm.folder).unwrap(), Some((sm.model, model::MEMBER)));
    assert!(st.contains(sm.model, sm.customer), "containment runs across the metamodels");
    assert_eq!(
        st.list(sm.model, element::OWNED_ELEMENT).unwrap(),
        vec![Value::Ref(sm.folder), Value::Ref(sm.comment), Value::Ref(sm.tag), Value::Ref(sm.link)]
    );
    assert_eq!(st.get(sm.link, link::KIND).unwrap(), Value::Enum(LinkKind::Realization.index()));
    assert_eq!(LinkKind::from_index(0), Some(LinkKind::Trace), "the default kind");
    assert_eq!(st.referrers(sm.customer).len(), 3, "the comment, the tag and the link refer to Customer");
    assert_eq!(st.by_name(tag::NAME, "OWNER"), vec![sm.tag]);
    assert_eq!(problems(&st), vec![]);
    assert!(TABLES.iter().any(|t| t.name == "core_link") && JUNCTIONS.iter().any(|j| j.name == "core_model_member" && j.ordered));
}

#[test]
fn the_rules_refuse_duplicate_tags_self_links_and_a_second_root() {
    let mut st = store();
    let sm = sample(&mut st);
    assert!(!tag_name_free(&st, sm.customer, "Owner", None));
    assert!(tag_name_free(&st, sm.customer, "Owner", Some(sm.tag)), "except the tag itself");
    assert!(tag_name_free(&st, sm.order, "owner", None));
    let before = st.live();
    assert!(matches!(archforge_core::link(&mut st, sm.model, LinkKind::Trace, sm.order, sm.order, None), Err(Invalid::Refused(_))));
    assert!(
        matches!(archforge_core::link(&mut st, sm.model, LinkKind::Trace, sm.order, sm.link, None), Err(Invalid::Refused(_))),
        "a link as an end"
    );
    assert!(
        matches!(archforge_core::link(&mut st, sm.folder, LinkKind::Trace, sm.order, sm.customer, None), Err(Invalid::NotOfClass { .. })),
        "a folder owns no links"
    );
    assert_eq!(st.live(), before, "every refused link left nothing behind, even the one refused after its object was made");
    // The rules over the whole store find what was built around them.
    let twin = st.create(TAG).unwrap();
    TagObj::set_name(&mut st, twin, Some("OWNER")).unwrap();
    TagObj::push_element(&mut st, twin, Value::Ref(sm.customer)).unwrap();
    ModelObj::push_tag(&mut st, sm.model, Value::Ref(twin)).unwrap();
    let second = st.create(MODEL).unwrap();
    let found = problems(&st);
    assert!(found.iter().any(|p| matches!(p, CoreProblem::DuplicateTag { element, .. } if *element == sm.customer)), "{found:?}");
    assert!(found.iter().any(|p| matches!(p, CoreProblem::Roots(r) if r.contains(&second) && r.contains(&sm.model))), "{found:?}");
    FolderObj::push_member(&mut st, sm.folder, Value::Ref(second)).unwrap();
    assert!(problems(&st).contains(&CoreProblem::NestedModel(second)));
    // A link set through the store alone may point at a link; it goes with the link it points at.
    let l2 = st.create(LINK).unwrap();
    st.set(l2, link::SOURCE, Value::Ref(sm.link)).unwrap();
    st.set(l2, link::TARGET, Value::Ref(sm.folder)).unwrap();
    ModelObj::push_link(&mut st, sm.model, Value::Ref(l2)).unwrap();
    assert_eq!(links_of(&st, sm.customer).unwrap(), vec![l2, sm.link], "the link to the link goes first");
    assert_eq!(destroy_with_links(&mut st, sm.customer).unwrap(), 3);
}

#[test]
fn destroying_an_element_takes_its_links_and_undo_brings_all_of_it_back() {
    let mut st = store();
    let sm = sample(&mut st);
    assert_eq!(links_of(&st, sm.pkg).unwrap(), vec![sm.link], "a link whose ends are inside the package");
    assert!(matches!(st.destroy(sm.customer), Err(Invalid::InUse { .. })), "the MOF core alone refuses: the link needs its end");
    st.begin();
    let gone = destroy_with_links(&mut st, sm.customer).unwrap();
    let delta = st.end().unwrap();
    assert_eq!(gone, 2, "the link and the class");
    assert!(!st.is_alive(sm.link) && !st.is_alive(sm.customer));
    assert_eq!(st.list(sm.comment, comment::ANNOTATED_ELEMENT).unwrap(), vec![], "the comment lets go");
    assert_eq!(st.list(sm.tag, tag::ELEMENT).unwrap(), vec![Value::Ref(sm.folder)], "the tag keeps its other element");
    assert_eq!(st.list(sm.model, model::LINK).unwrap(), vec![]);
    assert_eq!(problems(&st), vec![]);
    st.apply(&Store::invert(&delta)).unwrap();
    assert!(st.is_alive(sm.link) && st.is_alive(sm.customer));
    assert_eq!(st.get(sm.link, link::TARGET).unwrap(), Value::Ref(sm.customer));
    assert_eq!(st.list(sm.comment, comment::ANNOTATED_ELEMENT).unwrap(), vec![Value::Ref(sm.customer)]);
    assert_eq!(st.list(sm.tag, tag::ELEMENT).unwrap(), vec![Value::Ref(sm.customer), Value::Ref(sm.folder)]);
    // Destroying the folder takes the package, both classes and the link in one go.
    let gone = destroy_with_links(&mut st, sm.folder).unwrap();
    assert_eq!(gone, 5);
    assert_eq!(st.list(sm.model, element::OWNED_ELEMENT).unwrap(), vec![Value::Ref(sm.comment), Value::Ref(sm.tag)]);
    assert_eq!(st.list(sm.tag, tag::ELEMENT).unwrap(), vec![]);
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Selection(Vec<Obj>);

impl View<Store> for Selection {
    fn retain_valid(&mut self, doc: &Store) {
        self.0.retain(|o| doc.is_alive(*o));
    }
}

type C<'a> = Ctx<'a, Store, Selection>;

fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn obj(ctx: &C<'_>, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let u = p.str(key).and_then(Uuid::parse).ok_or_else(|| CommandError::about(key, "not an id"))?;
    ctx.doc.by_uuid(u).ok_or_else(|| CommandError::about(key, "there is no such element"))
}

fn root(ctx: &C<'_>) -> Result<Obj, CommandError> {
    ctx.doc.alive().find(|o| ctx.doc.class_of(*o).ok() == Some(MODEL)).ok_or_else(|| CommandError::new("no model"))
}

fn add_class(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let m = root(ctx)?;
    let c = ctx.doc.create(archforge_uml::CLASS).map_err(fail)?;
    archforge_uml::ClassObj::set_name(ctx.doc, c, Some(p.str("name").unwrap_or("Class"))).map_err(fail)?;
    ModelObj::push_member(ctx.doc, m, Value::Ref(c)).map_err(fail)?;
    ctx.view.0 = vec![c];
    Ok(json!({ "id": ctx.doc.uuid(c).map_err(fail)?.to_string() }))
}

fn add_link(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let m = root(ctx)?;
    let (a, b) = (obj(ctx, p, "source")?, obj(ctx, p, "target")?);
    let kind = LinkKind::from_index(p.integer("kind").unwrap_or(0).try_into().unwrap_or(99))
        .ok_or_else(|| CommandError::about("kind", "not a link kind"))?;
    let l = archforge_core::link(ctx.doc, m, kind, a, b, p.str("note")).map_err(fail)?;
    Ok(json!({ "id": ctx.doc.uuid(l).map_err(fail)?.to_string() }))
}

fn tag_element(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let m = root(ctx)?;
    let e = obj(ctx, p, "element")?;
    let name = p.str("name").unwrap_or("tag");
    if !tag_name_free(ctx.doc, e, name, None) {
        return Err(CommandError::about("name", "the element has a tag of that name"));
    }
    let t = ctx.doc.create(TAG).map_err(fail)?;
    TagObj::set_name(ctx.doc, t, Some(name)).map_err(fail)?;
    TagObj::push_element(ctx.doc, t, Value::Ref(e)).map_err(fail)?;
    ModelObj::push_tag(ctx.doc, m, Value::Ref(t)).map_err(fail)?;
    Ok(json!({ "id": ctx.doc.uuid(t).map_err(fail)?.to_string() }))
}

fn delete(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let o = obj(ctx, p, "element")?;
    Ok(json!({ "removed": destroy_with_links(ctx.doc, o).map_err(fail)? }))
}

#[test]
fn the_sweep_finds_nothing_on_the_combined_store() {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend([
        cmd!("class.add", "Add class", &["Class"], None, "name: string = \"Class\"; its name", always, add_class),
        cmd!(
            "link.add",
            "Add link",
            &["Link"],
            None,
            "source: string; an id\ntarget: string; an id\nkind: integer = 0, 0..5; the kind\nnote: string = \"\"; a note",
            always,
            add_link
        ),
        cmd!("tag.add", "Tag", &["Tag"], None, "element: string; an id\nname: string = \"tag\"; the tag's name", always, tag_element),
        cmd!("element.delete", "Delete", &["Edit"], None, "element: string; an id", always, delete),
    ]);
    let mut st = store();
    st.create(MODEL).unwrap();
    let mut session = Session::new(Registry::new(specs).unwrap(), st, 1, Options::default());
    let a = session.execute("class.add", &json!({ "name": "A" })).unwrap().value["id"].as_str().unwrap().to_string();
    let b = session.execute("class.add", &json!({ "name": "B" })).unwrap().value["id"].as_str().unwrap().to_string();
    let l = session.execute("link.add", &json!({ "source": a, "target": b, "kind": 1 })).unwrap().value["id"].as_str().unwrap().to_string();
    assert!(session.execute("link.add", &json!({ "source": a, "target": a })).is_err(), "a self-link is refused");
    assert!(session.execute("tag.add", &json!({ "element": a, "name": "x" })).is_ok());
    assert!(session.execute("tag.add", &json!({ "element": a, "name": "X" })).is_err(), "a second tag of that name is refused");
    let picture = |st: &Store| (st.live(), st.alive().map(|o| (o, st.container(o).unwrap(), st.referrers(o))).collect::<Vec<_>>(), problems(st));
    let calls = archforge_engine::testing::sweep_with(&mut session, &[&a, &b, &l, "nope"], picture).unwrap();
    assert!(calls > 200, "{calls}");
    assert_eq!(problems(session.doc()), vec![]);
    while session.can_undo() {
        session.undo().unwrap();
    }
    assert_eq!(session.doc().live(), 1, "only the model is left");
}

#[test]
fn every_element_is_named_by_its_own_name_whatever_its_notation() {
    let mut st = store();
    let sm = sample(&mut st);
    // The core's model and folder, UML's package and classes: each by its own name property.
    assert_eq!(name_of(&st, sm.model), "Shop");
    assert_eq!(name_of(&st, sm.folder), "Domain");
    assert_eq!(name_of(&st, sm.pkg), "sales");
    assert_eq!(name_of(&st, sm.customer), "Customer");
    // A UML property, an operation and an association end are named the same way.
    let attr = st.create(archforge_uml::PROPERTY).unwrap();
    archforge_uml::PropertyObj::set_name(&mut st, attr, Some("email")).unwrap();
    st.push(sm.customer, archforge_uml::class::OWNED_ATTRIBUTE, Value::Ref(attr)).unwrap();
    assert_eq!(name_of(&st, attr), "email");
    // What has no name says so: a comment, a tag's link, and an element never named.
    assert_eq!(name_of(&st, sm.comment), "");
    assert_eq!(name_of(&st, sm.link), "");
    let unnamed = st.create(archforge_uml::CLASS).unwrap();
    assert_eq!(name_of(&st, unnamed), "");
}
