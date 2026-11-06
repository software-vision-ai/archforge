//! The MOF core over the toy metamodel: the universe's resolution, every op and its inverse, each
//! refusal naming its key and changing nothing, containment, opposites, subsets, derived unions,
//! destroy with referrers, indexes exact against a rebuild, random sequences with undo back to the
//! identical store, and the store as the engine's document.

use archforge_engine::{CommandError, Ctx, Document, EngineError, Facts, Options, Params, Registry, Session, View, always, cmd};
use archforge_mof::toy::{self, *};
use archforge_mof::{Delta, Invalid, NameKind, Obj, Op, Problem, Store, TypeRef, Value};
use serde_json::{Value as Json, json};

fn s(v: &str) -> Value {
    Value::str(v)
}

fn store() -> Store {
    toy::store().unwrap()
}

/// A package with a class that has a typed attribute (with its two multiplicity literals), an
/// operation with a parameter, a generalization to a second class, an association between two
/// attributes, and a comment.
struct Sample {
    pkg: Obj,
    class: Obj,
    other: Obj,
    attr: Obj,
    other_attr: Obj,
    lower: Obj,
    op: Obj,
    param: Obj,
    generalization: Obj,
    assoc: Obj,
    comment: Obj,
}

fn sample(st: &mut Store) -> Sample {
    let pkg = st.create(PACKAGE).unwrap();
    st.set(pkg, NAMED_ELEMENT_NAME, s("model")).unwrap();
    let class = st.create(CLASS).unwrap();
    st.set(class, NAMED_ELEMENT_NAME, s("Customer")).unwrap();
    st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(class)).unwrap();
    let other = st.create(CLASS).unwrap();
    st.set(other, NAMED_ELEMENT_NAME, s("Party")).unwrap();
    st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(other)).unwrap();
    let attr = st.create(PROPERTY).unwrap();
    st.set(attr, NAMED_ELEMENT_NAME, s("id")).unwrap();
    st.set(attr, TYPED_ELEMENT_TYPE, Value::Ref(other)).unwrap();
    st.push(class, CLASS_OWNED_ATTRIBUTE, Value::Ref(attr)).unwrap();
    let lower = st.create(LITERAL_INTEGER).unwrap();
    st.set(lower, LITERAL_INTEGER_VALUE, Value::Int(1)).unwrap();
    st.set(attr, MULTIPLICITY_LOWER_VALUE, Value::Ref(lower)).unwrap();
    let upper = st.create(LITERAL_UNLIMITED_NATURAL).unwrap();
    st.set(attr, MULTIPLICITY_UPPER_VALUE, Value::Ref(upper)).unwrap();
    let other_attr = st.create(PROPERTY).unwrap();
    st.set(other_attr, NAMED_ELEMENT_NAME, s("customers")).unwrap();
    st.push(other, CLASS_OWNED_ATTRIBUTE, Value::Ref(other_attr)).unwrap();
    let op = st.create(OPERATION).unwrap();
    st.set(op, NAMED_ELEMENT_NAME, s("rename")).unwrap();
    st.push(class, CLASS_OWNED_OPERATION, Value::Ref(op)).unwrap();
    let param = st.create(PARAMETER).unwrap();
    st.set(param, NAMED_ELEMENT_NAME, s("to")).unwrap();
    st.push(op, OPERATION_OWNED_PARAMETER, Value::Ref(param)).unwrap();
    let generalization = st.create(GENERALIZATION).unwrap();
    st.set(generalization, GENERALIZATION_GENERAL, Value::Ref(other)).unwrap();
    st.push(class, CLASS_GENERALIZATION, Value::Ref(generalization)).unwrap();
    let assoc = st.create(ASSOCIATION).unwrap();
    st.set(assoc, NAMED_ELEMENT_NAME, s("places")).unwrap();
    st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(assoc)).unwrap();
    st.push(assoc, ASSOCIATION_MEMBER_END, Value::Ref(attr)).unwrap();
    st.push(assoc, ASSOCIATION_MEMBER_END, Value::Ref(other_attr)).unwrap();
    let comment = st.create(COMMENT).unwrap();
    st.set(comment, COMMENT_BODY, s("the buyer")).unwrap();
    st.push(class, ELEMENT_OWNED_COMMENT, Value::Ref(comment)).unwrap();
    st.push(comment, COMMENT_ANNOTATED_ELEMENT, Value::Ref(class)).unwrap();
    Sample { pkg, class, other, attr, other_attr, lower, op, param, generalization, assoc, comment }
}

/// Everything observable about a store, for exact comparisons.
#[derive(Debug, Clone, PartialEq)]
struct Picture {
    objects: Vec<(Obj, archforge_mof::ClassId, Option<(Obj, archforge_mof::PropId)>, Vec<(archforge_mof::PropId, Vec<Value>)>)>,
    referrers: Vec<(Obj, Vec<(Obj, archforge_mof::PropId)>)>,
    names: Vec<(Obj, Vec<Obj>)>,
}

fn picture(st: &Store) -> Picture {
    let u = st.universe();
    let mut objects = Vec::new();
    let mut names = Vec::new();
    for o in st.alive() {
        let class = st.class_of(o).unwrap();
        let info = u.info(class).unwrap();
        let mut values = Vec::new();
        for p in &info.stored {
            let prop = u.prop(*p).unwrap();
            let v = if prop.is_many() { st.list(o, *p).unwrap() } else { vec![st.get(o, *p).unwrap()] };
            values.push((*p, v));
        }
        objects.push((o, class, st.container(o).unwrap(), values));
        if let Some(np) = info.name_prop
            && let Value::Str(n) = st.get(o, np).unwrap()
        {
            names.push((o, st.by_name(np, &n)));
        }
    }
    let referrers = st.alive().map(|o| (o, st.referrers(o))).collect();
    Picture { objects, referrers, names }
}

#[test]
fn the_universe_resolves_inheritance_subsets_unions_and_names() {
    let u = toy::universe().unwrap();
    let class = u.info(CLASS).unwrap();
    assert!(class.all_supers.contains(&ELEMENT) && class.all_supers.contains(&NAMESPACE) && class.all_supers.contains(&TYPE));
    assert!(u.conforms(CLASS, TypeRef::Class(PACKAGEABLE_ELEMENT)) && !u.conforms(PACKAGE, TypeRef::Class(CLASS)));
    assert_eq!(class.name_prop, Some(NAMED_ELEMENT_NAME));
    assert!(
        class.all_props.starts_with(&[ELEMENT_OWNED_ELEMENT, ELEMENT_OWNED_COMMENT, NAMED_ELEMENT_NAME, NAMESPACE_OWNED_MEMBER]),
        "{:?}",
        class.all_props
    );
    assert_eq!(class.unions[&NAMESPACE_OWNED_MEMBER], vec![CLASS_OWNED_ATTRIBUTE, CLASS_OWNED_OPERATION]);
    assert_eq!(class.unions[&ELEMENT_OWNED_ELEMENT], vec![ELEMENT_OWNED_COMMENT, CLASS_OWNED_ATTRIBUTE, CLASS_OWNED_OPERATION, CLASS_GENERALIZATION]);
    assert_eq!(class.subsetted[&CLASS_OWNED_ATTRIBUTE], vec![CLASS_FEATURE], "feature is the one stored property ownedAttribute subsets");
    assert_eq!(u.prop(PROPERTY_ASSOCIATION).unwrap().opposite, Some(ASSOCIATION_MEMBER_END));
    assert_eq!(u.prop_name(CLASS_IS_ABSTRACT), "Class::isAbstract");
    // A broken metamodel is refused.
    let mut b = archforge_mof::Builder::new(2, "archforge:bad", "bad", "1", "x");
    let a = b.class("A", false, &[], "").unwrap();
    assert!(b.class("A", false, &[], "").is_err(), "twice");
    assert!(b.class("b c", false, &[], "").is_err(), "not plain");
    assert!(
        b.property(archforge_mof::PropertySpec::new("x", a, TypeRef::Class(a)).derived().default(Value::Null)).is_err(),
        "derived with a default"
    );
    let x = b.property(archforge_mof::PropertySpec::new("x", a, TypeRef::Class(a)).opposite(archforge_mof::PropId::new(2, 7))).unwrap();
    let _ = x;
    assert!(archforge_mof::Universe::new(vec![b.finish()]).is_err(), "an opposite that does not exist");
}

#[test]
fn creating_setting_and_containing_follow_mof() {
    let mut st = store();
    let sm = sample(&mut st);
    assert_eq!(st.container(sm.attr).unwrap(), Some((sm.class, CLASS_OWNED_ATTRIBUTE)));
    assert_eq!(st.container(sm.lower).unwrap(), Some((sm.attr, MULTIPLICITY_LOWER_VALUE)));
    assert_eq!(st.list(sm.class, CLASS_FEATURE).unwrap(), vec![Value::Ref(sm.attr), Value::Ref(sm.op)], "subsets append to the stored superset");
    assert_eq!(st.list(sm.class, NAMESPACE_OWNED_MEMBER).unwrap(), vec![Value::Ref(sm.attr), Value::Ref(sm.op)], "a derived union reads its members");
    assert_eq!(st.owned_elements(sm.class).unwrap(), vec![sm.comment, sm.attr, sm.op, sm.generalization]);
    assert_eq!(st.get(sm.attr, PROPERTY_ASSOCIATION).unwrap(), Value::Ref(sm.assoc), "the opposite end was set with the member end");
    assert_eq!(st.get(sm.class, CLASS_IS_ABSTRACT).unwrap(), Value::Bool(false), "the default");
    assert_eq!(st.get(sm.attr, MULTIPLICITY_IS_UNIQUE).unwrap(), Value::Bool(true));
    assert!(st.contains(sm.pkg, sm.lower) && !st.contains(sm.lower, sm.pkg));
    // Moving a contained element unlinks it from its old container.
    st.push(sm.other, CLASS_OWNED_ATTRIBUTE, Value::Ref(sm.attr)).unwrap();
    assert_eq!(st.container(sm.attr).unwrap(), Some((sm.other, CLASS_OWNED_ATTRIBUTE)));
    assert_eq!(st.list(sm.class, CLASS_OWNED_ATTRIBUTE).unwrap(), vec![], "gone from the old owner");
    assert_eq!(st.list(sm.class, CLASS_FEATURE).unwrap(), vec![Value::Ref(sm.op)], "and from the superset it was only in through that");
    // Setting the other end of an opposite moves it: the association loses the old end.
    let assoc2 = st.create(ASSOCIATION).unwrap();
    st.set(sm.attr, PROPERTY_ASSOCIATION, Value::Ref(assoc2)).unwrap();
    assert_eq!(st.list(sm.assoc, ASSOCIATION_MEMBER_END).unwrap(), vec![Value::Ref(sm.other_attr)]);
    assert_eq!(st.list(assoc2, ASSOCIATION_MEMBER_END).unwrap(), vec![Value::Ref(sm.attr)]);
    st.set(sm.attr, PROPERTY_ASSOCIATION, Value::Null).unwrap();
    assert_eq!(st.list(assoc2, ASSOCIATION_MEMBER_END).unwrap(), vec![]);
    // Names, by index.
    assert_eq!(st.by_name(NAMED_ELEMENT_NAME, "CUSTOMER"), vec![sm.class]);
    st.set(sm.class, NAMED_ELEMENT_NAME, s("Buyer")).unwrap();
    assert!(st.by_name(NAMED_ELEMENT_NAME, "customer").is_empty());
    assert_eq!(st.by_prefix(NAMED_ELEMENT_NAME, "b"), vec![sm.class]);
    assert_eq!(st.by_uuid(st.uuid(sm.class).unwrap()), Some(sm.class));
    assert_eq!(picture(&st), picture(&st), "the picture is stable");
}

#[test]
fn every_refusal_names_its_cause_and_changes_nothing() {
    let mut st = store();
    let sm = sample(&mut st);
    let before = picture(&st);
    let other_mm_class = archforge_mof::ClassId::new(7, 0);
    let cases: Vec<(&str, Result<(), Invalid>)> = vec![
        ("abstract", st.create(ELEMENT).map(drop)),
        ("unknown class", st.create(other_mm_class).map(drop)),
        ("wrong kind", st.set(sm.class, NAMED_ELEMENT_NAME, Value::Int(1))),
        ("not of class", st.set(sm.class, COMMENT_BODY, s("x"))),
        ("list as single", st.set(sm.class, CLASS_OWNED_ATTRIBUTE, Value::Ref(sm.attr))),
        ("single as list", st.push(sm.class, NAMED_ELEMENT_NAME, s("x")).map(drop)),
        ("derived", st.push(sm.class, NAMESPACE_OWNED_MEMBER, Value::Ref(sm.attr)).map(drop)),
        ("does not conform", st.push(sm.class, CLASS_OWNED_ATTRIBUTE, Value::Ref(sm.op)).map(drop)),
        ("duplicate", st.push(sm.class, CLASS_OWNED_ATTRIBUTE, Value::Ref(sm.attr)).map(drop)),
        ("null in a list", st.push(sm.class, CLASS_FEATURE, Value::Null).map(drop)),
        ("self", st.push(sm.pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(sm.pkg)).map(drop)),
        ("index", st.insert(sm.class, CLASS_OWNED_ATTRIBUTE, 5, Value::Ref(sm.other_attr))),
        ("remove index", st.remove_at(sm.class, CLASS_OWNED_ATTRIBUTE, 3).map(drop)),
        ("move index", st.move_item(sm.class, CLASS_OWNED_ATTRIBUTE, 0, 9)),
        ("not finite", st.set(sm.lower, LITERAL_INTEGER_VALUE, Value::Real(f64::NAN))),
        ("too long", st.set(sm.class, NAMED_ELEMENT_NAME, Value::str(&"x".repeat(70_000)))),
        ("bad literal", st.set(sm.attr, PROPERTY_AGGREGATION, Value::Enum(9))),
        ("dead target", {
            let mut st2 = store();
            let a = st2.create(CLASS).unwrap();
            st2.destroy(a).unwrap();
            st.set(sm.attr, TYPED_ELEMENT_TYPE, Value::Ref(Obj(999)))
        }),
        ("in use", st.destroy(sm.other).map(drop)),
        ("get a list", st.get(sm.class, CLASS_OWNED_ATTRIBUTE).map(drop)),
        ("list a single", st.list(sm.class, NAMED_ELEMENT_NAME).map(drop)),
    ];
    let expected = [
        "Element is abstract",
        "there is no class",
        "NamedElement::name takes a string, not an integer",
        "has no property Comment::body",
        "Class::ownedAttribute holds a list",
        "NamedElement::name holds one value",
        "Namespace::ownedMember is derived",
        "Class::ownedAttribute does not take Operation",
        "Class::ownedAttribute already holds",
        "Class::feature takes a value, not nothing",
        "cannot contain itself",
        "Class::ownedAttribute has 1 values, not an index 5",
        "has 1 values, not an index 3",
        "not an index 9",
        "LiteralInteger::value takes a finite number",
        "at most 65535 characters",
        "takes a literal of its enumeration",
        "there is no object 999",
        "is required by",
        "holds a list, not one value",
        "holds one value, not a list",
    ];
    for ((what, r), says) in cases.iter().zip(expected) {
        let e = r.as_ref().expect_err(what).to_string();
        assert!(e.contains(says), "{what}: {e}");
    }
    assert_eq!(picture(&st), before, "a refusal changed something");
}

#[test]
fn a_containment_cycle_is_refused_and_changes_nothing() {
    let mut st = store();
    let outer = st.create(PACKAGE).unwrap();
    let inner = st.create(PACKAGE).unwrap();
    st.push(outer, PACKAGE_PACKAGED_ELEMENT, Value::Ref(inner)).unwrap();
    let innermost = st.create(PACKAGE).unwrap();
    st.push(inner, PACKAGE_PACKAGED_ELEMENT, Value::Ref(innermost)).unwrap();
    let before = picture(&st);
    assert_eq!(st.push(innermost, PACKAGE_PACKAGED_ELEMENT, Value::Ref(outer)), Err(Invalid::Cycle));
    assert_eq!(st.push(inner, PACKAGE_PACKAGED_ELEMENT, Value::Ref(outer)), Err(Invalid::Cycle));
    assert_eq!(picture(&st), before);
    // Moving the innermost up is fine, and it leaves its old container.
    st.push(outer, PACKAGE_PACKAGED_ELEMENT, Value::Ref(innermost)).unwrap();
    assert_eq!(st.list(inner, PACKAGE_PACKAGED_ELEMENT).unwrap(), vec![]);
    assert_eq!(st.container(innermost).unwrap(), Some((outer, PACKAGE_PACKAGED_ELEMENT)));
}

#[test]
fn destroy_takes_the_subtree_repairs_references_and_refuses_required_ones() {
    let mut st = store();
    let sm = sample(&mut st);
    // Party is the type of Customer.id (optional: dropped) and the general of a generalization
    // (required: refused).
    assert!(matches!(st.destroy(sm.other), Err(Invalid::InUse { by, .. }) if by == sm.generalization));
    st.remove_at(sm.class, CLASS_GENERALIZATION, 0).unwrap();
    st.destroy(sm.generalization).unwrap();
    let gone = st.destroy(sm.other).unwrap();
    assert_eq!(gone, 2, "Party and its attribute");
    assert_eq!(st.get(sm.attr, TYPED_ELEMENT_TYPE).unwrap(), Value::Null, "the optional reference was dropped");
    assert_eq!(st.list(sm.assoc, ASSOCIATION_MEMBER_END).unwrap(), vec![Value::Ref(sm.attr)], "the dead end left the association");
    assert_eq!(st.list(sm.pkg, PACKAGE_PACKAGED_ELEMENT).unwrap(), vec![Value::Ref(sm.class), Value::Ref(sm.assoc)]);
    assert!(!st.is_alive(sm.other) && !st.is_alive(sm.other_attr));
    assert!(st.referrers(sm.other).is_empty());
    // Destroying the class takes its attribute, literals, operation, parameter and comment.
    let gone = st.destroy(sm.class).unwrap();
    assert_eq!(gone, 7);
    assert_eq!(st.list(sm.assoc, ASSOCIATION_MEMBER_END).unwrap(), vec![]);
    assert_eq!(st.live(), 2, "the package and the association");
    assert!(st.by_name(NAMED_ELEMENT_NAME, "customer").is_empty());
    assert!(matches!(st.get(sm.param, NAMED_ELEMENT_NAME), Err(Invalid::Dead(_))));
}

/// Applies the delta's inverse and checks the picture is exactly the one before.
fn undo_and_check(st: &mut Store, before: &Picture, delta: &Delta) {
    st.apply(&Store::invert(delta)).unwrap();
    assert_eq!(&picture(st), before);
}

#[test]
fn every_change_undoes_and_redoes_to_the_identical_store() {
    let mut st = store();
    st.begin();
    let sm = sample(&mut st);
    let build = st.end().unwrap();
    let built = picture(&st);
    let steps: Vec<(&str, Box<dyn Fn(&mut Store) -> Result<(), Invalid>>)> = vec![
        ("rename", Box::new(move |st| st.set(sm.class, NAMED_ELEMENT_NAME, s("Buyer")))),
        ("retype", Box::new(move |st| st.set(sm.attr, TYPED_ELEMENT_TYPE, Value::Null))),
        ("move attribute to the other class", Box::new(move |st| st.push(sm.other, CLASS_OWNED_ATTRIBUTE, Value::Ref(sm.attr)).map(drop))),
        (
            "reorder",
            Box::new(move |st| {
                let p2 = st.create(PROPERTY)?;
                st.push(sm.class, CLASS_OWNED_ATTRIBUTE, Value::Ref(p2))?;
                let p3 = st.create(PROPERTY)?;
                st.push(sm.class, CLASS_OWNED_ATTRIBUTE, Value::Ref(p3))?;
                st.move_item(sm.class, CLASS_OWNED_ATTRIBUTE, 1, 0)
            }),
        ),
        (
            "relink the association end",
            Box::new(move |st| {
                let a2 = st.create(ASSOCIATION)?;
                st.push(a2, ASSOCIATION_MEMBER_END, Value::Ref(sm.attr)).map(drop)
            }),
        ),
        (
            "remove and destroy the generalization",
            Box::new(move |st| {
                st.remove_at(sm.class, CLASS_GENERALIZATION, 0)?;
                st.destroy(sm.generalization).map(drop)
            }),
        ),
        ("destroy the class", Box::new(move |st| st.destroy(sm.class).map(drop))),
        ("destroy the package", Box::new(move |st| st.destroy(sm.pkg).map(drop))),
    ];
    let mut deltas = Vec::new();
    for (what, step) in steps {
        let before = picture(&st);
        st.begin();
        step(&mut st).unwrap_or_else(|e| panic!("{what}: {e}"));
        let delta = st.end().unwrap_or_else(|| panic!("{what} changed nothing"));
        let after = picture(&st);
        assert_ne!(after, before, "{what}");
        undo_and_check(&mut st, &before, &delta);
        st.apply(&delta).unwrap();
        assert_eq!(picture(&st), after, "{what}: redo");
        undo_and_check(&mut st, &before, &delta);
        st.apply(&delta).unwrap();
        deltas.push(delta);
    }
    // All the way back to the sample, then to nothing, and forward again.
    for d in deltas.iter().rev() {
        st.apply(&Store::invert(d)).unwrap();
    }
    assert_eq!(picture(&st), built);
    st.apply(&Store::invert(&build)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(st.live(), 0);
    st.apply(&build).unwrap();
    assert_eq!(picture(&st), built);
    for d in &deltas {
        st.apply(d).unwrap();
    }
    assert_eq!(st.live(), 1, "only the association's replacement end is left after the package went");
}

#[test]
fn a_delta_that_does_not_fit_is_refused_whole() {
    let mut st = store();
    let a = st.create(CLASS).unwrap();
    st.begin();
    st.set(a, NAMED_ELEMENT_NAME, s("A")).unwrap();
    st.set(a, CLASS_IS_ABSTRACT, Value::Bool(true)).unwrap();
    let delta = st.end().unwrap();
    st.set(a, NAMED_ELEMENT_NAME, s("B")).unwrap();
    let before = picture(&st);
    let mut bad = Store::invert(&delta);
    bad.0.swap(0, 1);
    assert!(st.apply(&bad).is_err(), "the name is B, not A");
    assert_eq!(picture(&st), before, "the abstract flag was rolled back");
    assert!(<Store as Document>::is_empty(&Delta(vec![
        Op::Set { obj: a, prop: NAMED_ELEMENT_NAME, old: s("x"), new: s("y") },
        Op::Set { obj: a, prop: NAMED_ELEMENT_NAME, old: s("y"), new: s("x") },
    ])));
    assert!(!<Store as Document>::is_empty(&delta));
}

#[test]
fn validate_reports_missing_values_and_duplicate_names() {
    let mut st = store();
    let pkg = st.create(PACKAGE).unwrap();
    let a = st.create(CLASS).unwrap();
    st.set(a, NAMED_ELEMENT_NAME, s("Order")).unwrap();
    st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(a)).unwrap();
    let b = st.create(CLASS).unwrap();
    st.set(b, NAMED_ELEMENT_NAME, s("order")).unwrap();
    st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(b)).unwrap();
    let g = st.create(GENERALIZATION).unwrap();
    st.push(a, CLASS_GENERALIZATION, Value::Ref(g)).unwrap();
    let problems = st.validate_all();
    assert!(problems.contains(&Problem::Missing { obj: g, prop: GENERALIZATION_GENERAL, lower: 1, found: 0 }));
    assert!(
        problems.iter().any(|p| matches!(p, Problem::DuplicateName { obj, other, .. } if (*obj == a && *other == b) || (*obj == b && *other == a)))
    );
    assert!(problems.contains(&st.first_problem().expect("the first problem is one validate_all finds")));
    st.set(g, GENERALIZATION_GENERAL, Value::Ref(b)).unwrap();
    st.set(b, NAMED_ELEMENT_NAME, s("Invoice")).unwrap();
    assert_eq!(st.validate_all(), vec![]);
    assert_eq!(st.first_problem(), None);
    // Siblings named with nothing never clash, for either check.
    for _ in 0..2 {
        let c = st.create(CLASS).unwrap();
        st.set(c, NAMED_ELEMENT_NAME, s("")).unwrap();
        st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
    }
    assert_eq!(st.validate_all(), vec![]);
    assert_eq!(st.first_problem(), None);
}

#[test]
fn the_first_problem_is_found_in_time_linear_in_the_model() {
    // Many objects of one name in different containers are fine; many of one name among siblings
    // are reported at the second. Either way each object is looked at once.
    let mut st = store();
    let mut pkgs = Vec::new();
    for _ in 0..20_000 {
        let pkg = st.create(PACKAGE).unwrap();
        let c = st.create(CLASS).unwrap();
        st.set(c, NAMED_ELEMENT_NAME, s("Id")).unwrap();
        st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        pkgs.push(pkg);
    }
    let started = std::time::Instant::now();
    assert_eq!(st.first_problem(), None);
    for _ in 0..20_000 {
        let c = st.create(CLASS).unwrap();
        st.set(c, NAMED_ELEMENT_NAME, s("id")).unwrap();
        st.push(pkgs[0], PACKAGE_PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
    }
    assert!(matches!(st.first_problem(), Some(Problem::DuplicateName { .. })));
    // A class named by a rule of its own is left out of the siblings' check.
    assert_eq!(st.first_problem_by(|c| (c != CLASS).then_some(NameKind::Class(c))), None);
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
}

/// With kinds, only siblings of one kind must be named apart, and the one-name question asks the
/// same rule as the whole-model check (AR-208).
#[test]
fn siblings_of_different_kinds_may_share_a_name_and_one_name_is_asked_by_the_same_rule() {
    // Packages and classes are kinds of their own; enumerations count as classes.
    let kind = |c| match c {
        c if c == CLASS || c == ENUMERATION => Some(NameKind::Group("classifier")),
        c if c == PACKAGE => Some(NameKind::Class(c)),
        _ => None,
    };
    let mut st = store();
    let pkg = st.create(PACKAGE).unwrap();
    let named = |st: &mut Store, class, name: &str| {
        let o = st.create(class).unwrap();
        st.set(o, NAMED_ELEMENT_NAME, s(name)).unwrap();
        st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(o)).unwrap();
        o
    };
    let sub = named(&mut st, PACKAGE, "Billing");
    let class = named(&mut st, CLASS, "billing");
    // Free (none): never checked, whatever its name.
    named(&mut st, ASSOCIATION, "BILLING");
    named(&mut st, ASSOCIATION, "Billing");
    assert_eq!(st.first_problem_by(kind), None, "a package and a class may share a name");
    assert!(matches!(st.first_problem(), Some(Problem::DuplicateName { .. })), "without kinds every sibling clashes");
    assert_eq!(st.name_clash(pkg, PACKAGE, "BILLING", None, kind), Some(sub));
    assert_eq!(st.name_clash(pkg, ENUMERATION, "BILLING", None, kind), Some(class), "an enumeration is a classifier");
    assert_eq!(st.name_clash(pkg, ENUMERATION, "billing", Some(class), kind), None, "the element itself never clashes");
    assert_eq!(st.name_clash(pkg, ASSOCIATION, "billing", None, kind), None, "a free kind never clashes");
    assert_eq!(st.name_clash(pkg, CLASS, "", None, kind), None, "an empty name names nothing");
    assert_eq!(st.name_clash(sub, CLASS, "billing", None, kind), None, "another owner's names are its own");
    let twin = named(&mut st, ENUMERATION, "BILLING");
    assert!(
        matches!(st.first_problem_by(kind), Some(Problem::DuplicateName { obj, other, .. }) if obj == twin && other == class),
        "two classifiers clash, whatever their metaclasses"
    );
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn pick(&mut self, v: &[Obj]) -> Option<Obj> {
        if v.is_empty() { None } else { Some(v[self.below(v.len())]) }
    }
}

/// One random edit against the store as it is; refusals are fine, panics are not.
fn random_edit(rng: &mut Rng, st: &mut Store) -> Result<(), Invalid> {
    let of = |st: &Store, class| -> Vec<Obj> { st.alive().filter(|o| st.class_of(*o).ok() == Some(class)).collect() };
    let classes = of(st, CLASS);
    let packages = of(st, PACKAGE);
    let props = of(st, PROPERTY);
    let assocs = of(st, ASSOCIATION);
    let n = rng.next();
    match rng.below(17) {
        0 => {
            let p = st.create(PACKAGE)?;
            st.set(p, NAMED_ELEMENT_NAME, s(&format!("p{n}")))?;
            if let Some(parent) = rng.pick(&packages) {
                st.push(parent, PACKAGE_PACKAGED_ELEMENT, Value::Ref(p))?;
            }
        }
        1 | 2 => {
            let c = st.create(CLASS)?;
            st.set(c, NAMED_ELEMENT_NAME, s(&format!("C{n}")))?;
            if let Some(pkg) = rng.pick(&packages) {
                st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(c))?;
            }
        }
        3 | 4 => {
            if let Some(c) = rng.pick(&classes) {
                let p = st.create(PROPERTY)?;
                st.set(p, NAMED_ELEMENT_NAME, s(&format!("a{}", n % 7)))?;
                if let Some(t) = rng.pick(&classes) {
                    st.set(p, TYPED_ELEMENT_TYPE, Value::Ref(t))?;
                }
                let at = rng.below(st.list(c, CLASS_OWNED_ATTRIBUTE)?.len() + 1) as u32;
                st.insert(c, CLASS_OWNED_ATTRIBUTE, at, Value::Ref(p))?;
            }
        }
        5 => {
            if let (Some(c), Some(g)) = (rng.pick(&classes), rng.pick(&classes)) {
                let generalization = st.create(GENERALIZATION)?;
                st.set(generalization, GENERALIZATION_GENERAL, Value::Ref(g))?;
                st.push(c, CLASS_GENERALIZATION, Value::Ref(generalization))?;
            }
        }
        6 => {
            if let Some(pkg) = rng.pick(&packages) {
                let a = st.create(ASSOCIATION)?;
                st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(a))?;
                for _ in 0..2 {
                    if let Some(p) = rng.pick(&props) {
                        let _ = st.push(a, ASSOCIATION_MEMBER_END, Value::Ref(p));
                    }
                }
            }
        }
        7 => {
            if let (Some(p), Some(a)) = (rng.pick(&props), rng.pick(&assocs)) {
                st.set(p, PROPERTY_ASSOCIATION, if n.is_multiple_of(3) { Value::Null } else { Value::Ref(a) })?;
            }
        }
        8 => {
            if let Some(c) = rng.pick(&classes) {
                st.set(c, NAMED_ELEMENT_NAME, s(&format!("R{n}")))?;
            }
        }
        9 => {
            if let (Some(c), Some(pkg)) = (rng.pick(&classes), rng.pick(&packages)) {
                st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(c))?;
            }
        }
        10 => {
            if let Some(c) = rng.pick(&classes) {
                let l = st.list(c, CLASS_OWNED_ATTRIBUTE)?.len();
                if l > 1 {
                    st.move_item(c, CLASS_OWNED_ATTRIBUTE, rng.below(l) as u32, rng.below(l) as u32)?;
                }
            }
        }
        11 => {
            if let Some(c) = rng.pick(&classes) {
                let l = st.list(c, CLASS_OWNED_ATTRIBUTE)?.len();
                if l > 0 {
                    st.remove_at(c, CLASS_OWNED_ATTRIBUTE, rng.below(l) as u32)?;
                }
            }
        }
        12 => {
            if let Some(c) = rng.pick(&classes) {
                st.destroy(c)?;
            }
        }
        // An end owned by an association, which makes it a member end with the association as
        // its own; then an owned or member end let go.
        14 => {
            if let (Some(p), Some(a)) = (rng.pick(&props), rng.pick(&assocs)) {
                st.push(a, ASSOCIATION_OWNED_END, Value::Ref(p))?;
            }
        }
        15 | 16 => {
            let list = if rng.below(2) == 0 { ASSOCIATION_OWNED_END } else { ASSOCIATION_MEMBER_END };
            if let Some(a) = rng.pick(&assocs) {
                let l = st.list(a, list)?.len();
                if l > 0 {
                    st.remove_at(a, list, rng.below(l) as u32)?;
                }
            }
        }
        _ => {
            if let Some(p) = rng.pick(&props) {
                st.destroy(p)?;
            }
        }
    }
    Ok(())
}

/// The indexes rebuilt from scratch, to compare with the ones kept.
fn rebuilt_indexes(st: &Store) -> (Vec<(Obj, Obj, archforge_mof::PropId)>, Vec<(archforge_mof::PropId, String, Obj)>) {
    let u = st.universe();
    let mut refs = Vec::new();
    let mut names = Vec::new();
    for o in st.alive() {
        let class = st.class_of(o).unwrap();
        for p in &u.info(class).unwrap().stored {
            let prop = u.prop(*p).unwrap();
            let values = if prop.is_many() { st.list(o, *p).unwrap() } else { vec![st.get(o, *p).unwrap()] };
            for v in values {
                match v {
                    Value::Ref(t) if !prop.is_composite => refs.push((t, o, *p)),
                    Value::Str(n) if prop.is_name => names.push((*p, n.to_lowercase(), o)),
                    _ => {}
                }
            }
        }
    }
    refs.sort();
    names.sort();
    (refs, names)
}

fn kept_indexes(st: &Store) -> (Vec<(Obj, Obj, archforge_mof::PropId)>, Vec<(archforge_mof::PropId, String, Obj)>) {
    let mut refs: Vec<_> = st.alive().flat_map(|t| st.referrers(t).into_iter().map(move |(s, p)| (t, s, p))).collect();
    refs.sort();
    let u = st.universe();
    let mut names = Vec::new();
    for o in st.alive() {
        let class = st.class_of(o).unwrap();
        if let Some(np) = u.info(class).unwrap().name_prop
            && let Value::Str(n) = st.get(o, np).unwrap()
        {
            assert!(st.by_name(np, &n).contains(&o), "{o} is not under its name");
            names.push((np, n.to_lowercase(), o));
        }
    }
    names.sort();
    (refs, names)
}

#[test]
fn random_sequences_keep_the_store_consistent_and_undo_exactly() {
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut st = store();
        let mut history: Vec<(Picture, Delta)> = Vec::new();
        for i in 0..150 {
            let before = picture(&st);
            st.begin();
            let r = random_edit(&mut rng, &mut st);
            let delta = st.end();
            match (r, delta) {
                (Ok(()), Some(d)) => history.push((before, d)),
                (Ok(()), None) => {}
                (Err(_), Some(d)) => {
                    // A refused edit left ops behind: the engine rolls them back; here we do.
                    st.apply(&Store::invert(&d)).unwrap();
                    assert_eq!(picture(&st), before, "seed {seed} step {i}: rollback");
                }
                (Err(_), None) => assert_eq!(picture(&st), before, "seed {seed} step {i}: a refusal changed something"),
            }
            assert_eq!(kept_indexes(&st), rebuilt_indexes(&st), "seed {seed} step {i}: indexes");
            ends_agree(&st);
            // Every live contained object is in its container's list, and the other way round.
            for o in st.alive() {
                if let Some((c, p)) = st.container(o).unwrap() {
                    assert!(st.is_alive(c), "seed {seed} step {i}: {o} is in a dead container");
                    let held = st.universe().prop(p).unwrap().is_many() && st.list(c, p).unwrap().contains(&Value::Ref(o))
                        || st.get(c, p).ok() == Some(Value::Ref(o));
                    assert!(held, "seed {seed} step {i}: {o} is not in its container's {p:?}");
                }
                for child in st.owned_elements(o).unwrap() {
                    assert_eq!(st.container(child).unwrap().map(|(c, _)| c), Some(o), "seed {seed} step {i}: a child does not know its owner");
                }
                for (src, p) in st.referrers(o) {
                    assert!(st.is_alive(src), "seed {seed} step {i}: a dead object still refers to {o} through {p:?}");
                }
            }
        }
        for (before, delta) in history.iter().rev() {
            st.apply(&Store::invert(delta)).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            assert_eq!(&picture(&st), before, "seed {seed}: undo");
        }
        assert_eq!(st.live(), 0, "seed {seed}");
        assert_eq!(kept_indexes(&st), rebuilt_indexes(&st));
    }
}

// The store as the engine's document, through a few toy commands.

#[derive(Debug, Clone, PartialEq, Default)]
struct Selection(Vec<Obj>);

impl View<Store> for Selection {
    fn retain_valid(&mut self, doc: &Store) {
        self.0.retain(|o| doc.is_alive(*o));
    }
}

type C<'a> = Ctx<'a, Store, Selection>;

fn obj(ctx: &C<'_>, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let u = p.str(key).and_then(archforge_mof::Uuid::parse).ok_or_else(|| CommandError::about(key, "not an id"))?;
    ctx.doc.by_uuid(u).ok_or_else(|| CommandError::about(key, "there is no such element"))
}

fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn add_class(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let c = ctx.doc.create(CLASS).map_err(fail)?;
    ctx.doc.set(c, NAMED_ELEMENT_NAME, s(p.str("name").unwrap_or("Class"))).map_err(fail)?;
    if let Some(pkg) = p.str("package") {
        let pkg = archforge_mof::Uuid::parse(pkg)
            .and_then(|u| ctx.doc.by_uuid(u))
            .ok_or_else(|| CommandError::about("package", "there is no such package"))?;
        ctx.doc.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    }
    ctx.view.0 = vec![c];
    Ok(json!({ "id": ctx.doc.uuid(c).map_err(fail)?.to_string() }))
}

fn add_package(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let c = ctx.doc.create(PACKAGE).map_err(fail)?;
    ctx.doc.set(c, NAMED_ELEMENT_NAME, s(p.str("name").unwrap_or("Package"))).map_err(fail)?;
    Ok(json!({ "id": ctx.doc.uuid(c).map_err(fail)?.to_string() }))
}

fn rename(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let o = obj(ctx, p, "element")?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, s(p.str("name").unwrap_or_default())).map_err(fail)?;
    Ok(Json::Null)
}

fn delete(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let o = obj(ctx, p, "element")?;
    let n = ctx.doc.destroy(o).map_err(fail)?;
    Ok(json!({ "removed": n }))
}

/// Renames, then panics: the rename must be rolled back.
#[allow(clippy::panic)]
fn explode(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let o = obj(ctx, p, "element")?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, s("half done")).map_err(fail)?;
    panic!("boom")
}

/// Renames, then fails: likewise.
fn half(ctx: &mut C<'_>, p: &Params) -> Result<Json, CommandError> {
    let o = obj(ctx, p, "element")?;
    ctx.doc.set(o, NAMED_ELEMENT_NAME, s("half done")).map_err(fail)?;
    Err(CommandError::new("changed my mind"))
}

fn count(ctx: &mut C<'_>, _: &Params) -> Result<Json, CommandError> {
    Ok(json!(ctx.doc.live()))
}

fn session() -> Session<Store, Selection> {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend([
        cmd!(
            "class.add",
            "Add class",
            &["Class"],
            None,
            "name: string = \"Class\"; its name\npackage: string = \"\"; the package's id, or nothing",
            always,
            add_class
        ),
        cmd!("package.add", "Add package", &["Package"], None, "name: string = \"Package\"; its name", always, add_package),
        cmd!("element.rename", "Rename", &["Edit"], None, "element: string; its id\nname: string; the new name", always, rename),
        cmd!("element.delete", "Delete", &["Edit"], None, "element: string; its id", always, delete),
        cmd!("element.explode", "Explode", &["Edit"], None, "element: string; its id", always, explode),
        cmd!("element.half", "Half", &["Edit"], None, "element: string; its id", always, half),
        cmd!(query "element.count", "Count", &["Edit"], None, "", always, count),
    ]);
    Session::new(Registry::new(specs).unwrap(), store(), 1, Options::default())
}

fn id_of(out: &Json) -> String {
    out["id"].as_str().unwrap().to_string()
}

#[test]
fn the_store_is_the_engines_document() {
    let mut s = session();
    let pkg = id_of(&s.execute("package.add", &json!({ "name": "model" })).unwrap().value);
    let c = id_of(&s.execute("class.add", &json!({ "name": "Customer", "package": pkg })).unwrap().value);
    let c_obj = s.doc().by_uuid(archforge_mof::Uuid::parse(&c).unwrap()).unwrap();
    assert_eq!(s.view().0, vec![c_obj]);
    let before = picture(s.doc());
    // A panic and a failure after a change: both rolled back exactly.
    assert!(matches!(s.execute("element.explode", &json!({ "element": c })), Err(EngineError::Internal { message, .. }) if message == "boom"));
    assert_eq!(picture(s.doc()), before);
    assert!(matches!(s.execute("element.half", &json!({ "element": c })), Err(EngineError::Failed { .. })));
    assert_eq!(picture(s.doc()), before);
    assert_eq!(s.doc().by_name(NAMED_ELEMENT_NAME, "customer"), vec![c_obj]);
    // Undo and redo through the engine.
    s.execute("element.rename", &json!({ "element": c, "name": "Buyer" })).unwrap();
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before);
    s.redo().unwrap();
    assert_eq!(s.doc().by_name(NAMED_ELEMENT_NAME, "buyer"), vec![c_obj]);
    // Deleting drops the selection; undoing brings the element back under the same uuid.
    s.execute("element.delete", &json!({ "element": c })).unwrap();
    assert!(s.view().0.is_empty());
    s.undo().unwrap();
    assert_eq!(s.doc().by_uuid(archforge_mof::Uuid::parse(&c).unwrap()), Some(c_obj));
    assert_eq!(s.doc().by_name(NAMED_ELEMENT_NAME, "buyer"), vec![c_obj]);
    // A batch that fails half way changes nothing.
    let before = picture(s.doc());
    let r = s.batch("two", |b| {
        b.execute("class.add", &json!({ "name": "Order", "package": pkg }))?;
        b.execute("element.half", &json!({ "element": c }))?;
        Ok(())
    });
    assert!(r.is_err());
    assert_eq!(picture(s.doc()), before);
    // A gesture: preview over preview, then commit as one step.
    s.begin_interaction("element.rename").unwrap();
    s.preview(&json!({ "element": c, "name": "B" })).unwrap();
    s.preview(&json!({ "element": c, "name": "Bu" })).unwrap();
    s.preview(&json!({ "element": c, "name": "Buy" })).unwrap();
    s.commit().unwrap();
    assert_eq!(s.doc().by_name(NAMED_ELEMENT_NAME, "buy"), vec![c_obj]);
    assert!(s.doc().by_name(NAMED_ELEMENT_NAME, "bu").is_empty());
    s.undo().unwrap();
    assert_eq!(picture(s.doc()), before, "one step for the whole gesture");
    // The adversarial sweep, with ids of every kind.
    let calls = archforge_engine::testing::sweep_with(&mut s, &[&pkg, &c, "not-an-id"], picture).unwrap();
    assert!(calls > 200, "{calls}");
    let infos = s.registry().infos(&Facts { doc: s.doc(), view: s.view(), can_undo: s.can_undo(), can_redo: s.can_redo() });
    assert!(infos.iter().any(|i| i.id == "class.add"));
}

/// Release only: a million objects, ten thousand edits with undo, under the spike's targets.
#[test]
#[ignore = "run with --release for the numbers"]
fn a_million_objects_edit_and_undo_inside_a_frame() {
    let mut st = store();
    let t = std::time::Instant::now();
    let mut rng = Rng(5);
    let mut classes = Vec::new();
    let pkg = st.create(PACKAGE).unwrap();
    for i in 0..100_000 {
        let c = st.create(CLASS).unwrap();
        st.set(c, NAMED_ELEMENT_NAME, s(&format!("Class{i}"))).unwrap();
        st.push(pkg, PACKAGE_PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        for j in 0..3 {
            let p = st.create(PROPERTY).unwrap();
            st.set(p, NAMED_ELEMENT_NAME, s(&format!("a{j}"))).unwrap();
            st.push(c, CLASS_OWNED_ATTRIBUTE, Value::Ref(p)).unwrap();
            let l = st.create(LITERAL_INTEGER).unwrap();
            st.set(p, MULTIPLICITY_LOWER_VALUE, Value::Ref(l)).unwrap();
            let u = st.create(LITERAL_UNLIMITED_NATURAL).unwrap();
            st.set(p, MULTIPLICITY_UPPER_VALUE, Value::Ref(u)).unwrap();
        }
        classes.push(c);
    }
    println!("built {} objects in {:?}, {} MB", st.live(), t.elapsed(), st.bytes() / 1_000_000);
    assert!(st.live() >= 1_000_000);
    let mut times = Vec::new();
    let mut deltas = Vec::new();
    for i in 0..10_000 {
        let c = classes[rng.below(classes.len())];
        if !st.is_alive(c) {
            continue;
        }
        let t = std::time::Instant::now();
        st.begin();
        match i % 4 {
            0 => st.set(c, NAMED_ELEMENT_NAME, s(&format!("R{i}"))).unwrap(),
            1 => {
                let p = st.create(PROPERTY).unwrap();
                st.push(c, CLASS_OWNED_ATTRIBUTE, Value::Ref(p)).unwrap();
            }
            2 => {
                let t = classes[rng.below(classes.len())];
                if st.is_alive(t) {
                    let attrs = st.list(c, CLASS_OWNED_ATTRIBUTE).unwrap();
                    if let Some(Value::Ref(a)) = attrs.first() {
                        st.set(*a, TYPED_ELEMENT_TYPE, Value::Ref(t)).unwrap();
                    }
                }
            }
            _ => {
                st.destroy(c).unwrap();
            }
        }
        if let Some(d) = st.end() {
            deltas.push(d);
        }
        times.push(t.elapsed());
    }
    let t = std::time::Instant::now();
    for d in deltas.iter().rev() {
        st.apply(&Store::invert(d)).unwrap();
    }
    let undo = t.elapsed();
    times.sort();
    let p99 = times[times.len() * 99 / 100];
    println!(
        "edit p50 {:?} p99 {:?} max {:?}; undo of all {:?}; {} MB",
        times[times.len() / 2],
        p99,
        times[times.len() - 1],
        undo,
        st.bytes() / 1_000_000
    );
    assert!(p99 < std::time::Duration::from_millis(16), "{p99:?}");
    assert!(st.bytes() < 250_000_000);
}

#[test]
fn a_deep_containment_chain_is_destroyed_without_recursion() {
    let mut st = store();
    let root = st.create(PACKAGE).unwrap();
    let mut parent = root;
    for _ in 0..60_000 {
        let p = st.create(PACKAGE).unwrap();
        st.push(parent, PACKAGE_PACKAGED_ELEMENT, Value::Ref(p)).unwrap();
        parent = p;
    }
    assert!(st.contains(root, parent));
    st.begin();
    assert_eq!(st.destroy(root).unwrap(), 60_001);
    let delta = st.end().unwrap();
    assert_eq!(st.live(), 0);
    st.apply(&Store::invert(&delta)).unwrap();
    assert_eq!(st.live(), 60_001);
}

#[test]
fn a_composite_with_an_opposite_keeps_both_ends_when_a_child_moves() {
    let mut st = store();
    let a = st.create(CLASS).unwrap();
    let b = st.create(CLASS).unwrap();
    let p = st.create(PROPERTY).unwrap();
    st.push(a, CLASS_OWNED_ATTRIBUTE, Value::Ref(p)).unwrap();
    assert_eq!(st.get(p, PROPERTY_CLASS).unwrap(), Value::Ref(a), "the opposite end was set");
    assert_eq!(st.container(p).unwrap(), Some((a, CLASS_OWNED_ATTRIBUTE)));
    st.push(b, CLASS_OWNED_ATTRIBUTE, Value::Ref(p)).unwrap();
    assert_eq!(st.get(p, PROPERTY_CLASS).unwrap(), Value::Ref(b));
    assert_eq!(st.list(a, CLASS_OWNED_ATTRIBUTE).unwrap(), vec![]);
    assert_eq!(st.list(a, CLASS_FEATURE).unwrap(), vec![], "and the subsetted list of the old owner");
    assert_eq!(st.list(b, CLASS_FEATURE).unwrap(), vec![Value::Ref(p)]);
    // Setting the opposite end moves the child the other way.
    st.set(p, PROPERTY_CLASS, Value::Ref(a)).unwrap();
    assert_eq!(st.container(p).unwrap(), Some((a, CLASS_OWNED_ATTRIBUTE)));
    assert_eq!(st.list(b, CLASS_OWNED_ATTRIBUTE).unwrap(), vec![]);
    assert_eq!(st.list(a, CLASS_FEATURE).unwrap(), vec![Value::Ref(p)]);
    // Unsetting it takes the child out of its container.
    st.set(p, PROPERTY_CLASS, Value::Null).unwrap();
    assert_eq!(st.container(p).unwrap(), None);
    assert_eq!(st.list(a, CLASS_OWNED_ATTRIBUTE).unwrap(), vec![]);
    assert_eq!(st.list(a, CLASS_FEATURE).unwrap(), vec![]);
    assert_eq!(kept_indexes(&st), rebuilt_indexes(&st));
}

#[test]
fn the_universe_refuses_inheritance_cycles_across_metamodels_and_non_unique_references() {
    use archforge_mof::{Builder, PropertySpec, Universe};
    // A(2) extends B(3), B extends C(3), C extends B: no class inherits from itself directly.
    let mut m3 = Builder::new(3, "archforge:three", "three", "1", "x");
    let b = m3.class("B", false, &[], "").unwrap();
    let c = m3.class("C", false, &[b], "").unwrap();
    let mut m2 = Builder::new(2, "archforge:two", "two", "1", "x");
    m2.class("A", false, &[b], "").unwrap();
    // The cycle is closed through a super that is declared later in another metamodel: the builder
    // cannot see it, the universe must.
    let mut m4 = Builder::new(4, "archforge:four", "four", "1", "x");
    let d = m4.class("D", false, &[c], "").unwrap();
    let _ = d;
    let m3 = m3.finish();
    // Build a metamodel whose class extends one of a later metamodel that extends it back.
    let mut m5 = Builder::new(5, "archforge:five", "five", "1", "x");
    let e = m5.class("E", false, &[archforge_mof::ClassId::new(6, 0)], "").unwrap();
    let mut m6 = Builder::new(6, "archforge:six", "six", "1", "x");
    m6.class("F", false, &[e], "").unwrap();
    let err = Universe::new(vec![m2.finish(), m3, m4.finish(), m5.finish(), m6.finish()]).unwrap_err();
    assert!(err.0.contains("inherits from itself"), "{err}");
    let mut m7 = Builder::new(7, "archforge:seven", "seven", "1", "x");
    let g = m7.class("G", false, &[], "").unwrap();
    let mut spec = PropertySpec::new("others", g, TypeRef::Class(g)).many();
    spec.is_unique = false;
    assert!(m7.property(spec).unwrap_err().0.contains("not unique"));
}

#[test]
fn a_refused_change_half_way_is_taken_back() {
    // An association whose memberEnd is full: setting a property's association must fail after
    // the property was already unlinked from nothing and leave everything as it was.
    let mut st = store();
    let a = st.create(CLASS).unwrap();
    let p = st.create(PROPERTY).unwrap();
    st.push(a, CLASS_OWNED_ATTRIBUTE, Value::Ref(p)).unwrap();
    let before = picture(&st);
    // A dead target refused inside insert after the pre-checks: simulate through a type that does
    // not conform to the subsetted list (feature takes NamedElement, ownedAttribute Property).
    let lit = st.create(LITERAL_INTEGER).unwrap();
    assert!(st.push(a, CLASS_OWNED_ATTRIBUTE, Value::Ref(lit)).is_err());
    st.destroy(lit).unwrap();
    assert_eq!(picture(&st), before);
    assert!(st.corrupt().is_none());
}

/// Every association's member ends point back at it, every end that points at an association is
/// among its member ends, and its owned ends are member ends.
fn ends_agree(st: &Store) {
    for o in st.alive() {
        let class = st.class_of(o).unwrap();
        if class == ASSOCIATION {
            let members = st.list(o, ASSOCIATION_MEMBER_END).unwrap();
            for v in &members {
                assert_eq!(st.get(v.as_ref().unwrap(), PROPERTY_ASSOCIATION).unwrap(), Value::Ref(o), "a member end points back");
            }
            for v in st.list(o, ASSOCIATION_OWNED_END).unwrap() {
                assert!(members.contains(&v), "an owned end is a member end");
            }
        } else if class == PROPERTY
            && let Value::Ref(a) = st.get(o, PROPERTY_ASSOCIATION).unwrap()
        {
            assert!(st.list(a, ASSOCIATION_MEMBER_END).unwrap().contains(&Value::Ref(o)), "its association lists the end");
        }
    }
}

#[test]
fn a_value_pushed_into_a_subset_list_sets_the_supersets_opposite_and_lets_go_of_it_again() {
    let mut st = store();
    let a = st.create(ASSOCIATION).unwrap();
    let b = st.create(ASSOCIATION).unwrap();
    let end = st.create(PROPERTY).unwrap();
    let before = picture(&st);
    // Pushed into ownedEnd: in memberEnd too, and the end's association is set.
    st.begin();
    st.push(a, ASSOCIATION_OWNED_END, Value::Ref(end)).unwrap();
    let delta = st.end().unwrap();
    assert_eq!(st.list(a, ASSOCIATION_MEMBER_END).unwrap(), vec![Value::Ref(end)]);
    assert_eq!(st.get(end, PROPERTY_ASSOCIATION).unwrap(), Value::Ref(a));
    ends_agree(&st);
    undo_and_check(&mut st, &before, &delta);
    st.push(a, ASSOCIATION_OWNED_END, Value::Ref(end)).unwrap();
    // Moved to another association's owned ends: the first lets go of it in both lists.
    st.push(b, ASSOCIATION_OWNED_END, Value::Ref(end)).unwrap();
    assert_eq!(st.list(a, ASSOCIATION_MEMBER_END).unwrap(), vec![]);
    assert_eq!(st.list(a, ASSOCIATION_OWNED_END).unwrap(), vec![]);
    assert_eq!(st.get(end, PROPERTY_ASSOCIATION).unwrap(), Value::Ref(b));
    ends_agree(&st);
    // Its association set to the first: it leaves the second's member ends and, with them, its
    // owned ends.
    st.set(end, PROPERTY_ASSOCIATION, Value::Ref(a)).unwrap();
    assert_eq!(st.list(b, ASSOCIATION_OWNED_END).unwrap(), vec![]);
    assert_eq!(st.list(a, ASSOCIATION_MEMBER_END).unwrap(), vec![Value::Ref(end)]);
    ends_agree(&st);
    // Owned again, then taken out of the owned ends: it leaves the member ends, and its
    // association is unset.
    st.push(a, ASSOCIATION_OWNED_END, Value::Ref(end)).unwrap();
    ends_agree(&st);
    st.remove_value(a, ASSOCIATION_OWNED_END, &Value::Ref(end)).unwrap();
    assert_eq!(st.list(a, ASSOCIATION_MEMBER_END).unwrap(), vec![]);
    assert_eq!(st.get(end, PROPERTY_ASSOCIATION).unwrap(), Value::Null);
    ends_agree(&st);
    // Owned, then taken out of the member ends: it leaves the owned ends too.
    st.push(b, ASSOCIATION_OWNED_END, Value::Ref(end)).unwrap();
    st.remove_value(b, ASSOCIATION_MEMBER_END, &Value::Ref(end)).unwrap();
    assert_eq!(st.list(b, ASSOCIATION_OWNED_END).unwrap(), vec![]);
    assert_eq!(st.get(end, PROPERTY_ASSOCIATION).unwrap(), Value::Null);
    ends_agree(&st);
    // Owned, then its association destroyed: the end goes with it, nothing dangles.
    st.push(b, ASSOCIATION_OWNED_END, Value::Ref(end)).unwrap();
    st.destroy(b).unwrap();
    assert!(!st.is_alive(end));
    ends_agree(&st);
    assert_eq!(kept_indexes(&st), rebuilt_indexes(&st));
}
