//! UML's model as rows and back, where its properties come in opposite pairs: a generalization
//! (its general class's list and the specific back-reference), an association's member ends
//! (the ends' association), a dependency, and a list whose opposite is a list (a use case's
//! subjects and a classifier's use cases) come back identical, in their order; so does a list whose
//! superset's opposite is kept in step (an activity's partitions, in its groups, each in the
//! activity).

use archforge_mof::rows;
use archforge_mof::{CompositeSlabs, Store, Uuid, Value};
use archforge_uml as u;

fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

fn store(start: u64) -> Store {
    let universe = archforge_core::universe_with(vec![u::metamodel().unwrap()]).unwrap();
    let slabs = CompositeSlabs::new()
        .with(archforge_core::INDEX, Box::new(archforge_core::CoreSlabs::default()))
        .unwrap()
        .with(u::INDEX, Box::new(u::UmlSlabs::default()))
        .unwrap();
    Store::new(universe, Box::new(slabs), ids(start))
}

#[test]
fn a_model_of_opposite_pairs_comes_back_from_its_rows_identical() {
    let tables: Vec<_> = archforge_core::TABLES.iter().chain(u::TABLES).copied().collect();
    let junctions: Vec<_> = archforge_core::JUNCTIONS.iter().chain(u::JUNCTIONS).copied().collect();
    let mut st = store(0);
    let pkg = st.create(u::PACKAGE).unwrap();
    let class = |st: &mut Store, name: &str| {
        let c = st.create(u::CLASS).unwrap();
        st.set(c, u::named_element::NAME, Value::str(name)).unwrap();
        st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        c
    };
    let (payment, card, order, pricing) =
        (class(&mut st, "Payment"), class(&mut st, "CardPayment"), class(&mut st, "Order"), class(&mut st, "Pricing"));
    let g = st.create(u::GENERALIZATION).unwrap();
    st.set(g, u::generalization::GENERAL, Value::Ref(payment)).unwrap();
    st.push(card, u::classifier::GENERALIZATION, Value::Ref(g)).unwrap();
    let a = st.create(u::ASSOCIATION).unwrap();
    st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(a)).unwrap();
    for (ty, owner) in [(order, None), (payment, Some(order))] {
        let end = st.create(u::PROPERTY).unwrap();
        st.set(end, u::typed_element::TYPE, Value::Ref(ty)).unwrap();
        match owner {
            Some(c) => {
                st.push(c, u::class::OWNED_ATTRIBUTE, Value::Ref(end)).unwrap();
                st.push(a, u::association::MEMBER_END, Value::Ref(end)).unwrap();
            }
            // An owned end is a member end, with the association as its own.
            None => {
                st.push(a, u::association::OWNED_END, Value::Ref(end)).unwrap();
                assert!(st.list(a, u::association::MEMBER_END).unwrap().contains(&Value::Ref(end)));
                assert_eq!(st.get(end, u::property::ASSOCIATION).unwrap(), Value::Ref(a));
            }
        };
    }
    let dep = st.create(u::DEPENDENCY).unwrap();
    st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(dep)).unwrap();
    st.push(dep, u::dependency::CLIENT, Value::Ref(order)).unwrap();
    st.push(dep, u::dependency::SUPPLIER, Value::Ref(pricing)).unwrap();
    // Two use cases over three classes, listed in an order of their own on each side.
    let use_case = |st: &mut Store| {
        let uc = st.create(u::USE_CASE).unwrap();
        st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(uc)).unwrap();
        uc
    };
    let (u1, u2) = (use_case(&mut st), use_case(&mut st));
    for (uc, c) in [(u1, pricing), (u1, order), (u2, card), (u2, pricing)] {
        st.push(uc, u::use_case::SUBJECT, Value::Ref(c)).unwrap();
    }
    st.move_item(pricing, u::classifier::USE_CASE, 1, 0).unwrap();
    assert!(st.validate_all().is_empty(), "{:?}", st.validate_all());
    let saved = rows::snapshot(&st, &tables, &junctions).unwrap();
    let mut back = store(1 << 40);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    let again = rows::snapshot(&back, &tables, &junctions).unwrap();
    assert_eq!(again, saved, "the model comes back identical");
}

/// A class with `n` attributes, listed in the order they were made, or the other way round, as
/// rows.
fn class_rows(n: usize, reversed: bool) -> (rows::Rows, Vec<archforge_mof::schema::Table>, Vec<archforge_mof::schema::Junction>) {
    let tables: Vec<_> = archforge_core::TABLES.iter().chain(u::TABLES).copied().collect();
    let junctions: Vec<_> = archforge_core::JUNCTIONS.iter().chain(u::JUNCTIONS).copied().collect();
    let mut st = store(0);
    let c = st.create(u::CLASS).unwrap();
    for i in 0..n {
        let a = st.create(u::PROPERTY).unwrap();
        st.set(a, u::named_element::NAME, Value::str(&format!("a{i}"))).unwrap();
        if reversed {
            st.insert(c, u::class::OWNED_ATTRIBUTE, 0, Value::Ref(a)).unwrap();
        } else {
            st.push(c, u::class::OWNED_ATTRIBUTE, Value::Ref(a)).unwrap();
        }
    }
    (rows::snapshot(&st, &tables, &junctions).unwrap(), tables, junctions)
}

#[test]
fn a_long_list_with_an_opposite_is_read_back_in_its_order_in_linear_time() {
    let (saved, tables, junctions) = class_rows(20_000, true);
    let started = std::time::Instant::now();
    let mut back = store(1 << 40);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "reading took {:?}", started.elapsed());
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap(), saved);
}

#[test]
fn values_a_list_has_through_its_opposite_and_values_it_adds_take_the_saved_order() {
    let (mut saved, tables, junctions) = class_rows(3, false);
    // As a hand-edited database might hold it: the first attribute's back-reference to its class
    // is gone, so the list gets two values from their columns and the first from its own rows.
    let table = tables.iter().position(|t| t.name == "uml_property").unwrap();
    let column = tables[table].columns.iter().position(|c| c.prop == Some(u::property::CLASS)).unwrap();
    let first = saved.junctions[junctions.iter().position(|j| j.prop == u::class::OWNED_ATTRIBUTE && j.class == u::CLASS).unwrap()][0].2.clone();
    let row = saved.tables[table].iter().position(|r| r[0] == first).unwrap();
    saved.tables[table][row][column] = rows::Cell::Null;
    let mut back = store(1 << 40);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    let again = rows::snapshot(&back, &tables, &junctions).unwrap();
    let list = junctions.iter().position(|j| j.prop == u::class::OWNED_ATTRIBUTE && j.class == u::CLASS).unwrap();
    assert_eq!(again.junctions[list], saved.junctions[list], "the list in its saved order");
}

#[test]
fn an_activitys_partitions_are_in_its_groups_point_back_and_come_back_from_their_rows_identical() {
    let tables: Vec<_> = archforge_core::TABLES.iter().chain(u::TABLES).copied().collect();
    let junctions: Vec<_> = archforge_core::JUNCTIONS.iter().chain(u::JUNCTIONS).copied().collect();
    let mut st = store(0);
    let pkg = st.create(u::PACKAGE).unwrap();
    let activity = st.create(u::ACTIVITY).unwrap();
    st.push(pkg, u::package::PACKAGED_ELEMENT, Value::Ref(activity)).unwrap();
    let mut partitions = Vec::new();
    for name in ["Customer", "Warehouse"] {
        let p = st.create(u::ACTIVITY_PARTITION).unwrap();
        st.set(p, u::named_element::NAME, Value::str(name)).unwrap();
        st.push(activity, u::activity::PARTITION, Value::Ref(p)).unwrap();
        // Pushed into the subset, it is in the superset and the superset's opposite points back.
        assert!(st.list(activity, u::activity::GROUP).unwrap().contains(&Value::Ref(p)));
        assert_eq!(st.get(p, u::activity_group::IN_ACTIVITY).unwrap(), Value::Ref(activity));
        partitions.push(p);
    }
    // Out of the partitions, it leaves the groups and the activity.
    st.remove_value(activity, u::activity::PARTITION, &Value::Ref(partitions[1])).unwrap();
    assert!(!st.list(activity, u::activity::GROUP).unwrap().contains(&Value::Ref(partitions[1])));
    assert_eq!(st.get(partitions[1], u::activity_group::IN_ACTIVITY).unwrap(), Value::Null);
    st.push(activity, u::activity::PARTITION, Value::Ref(partitions[1])).unwrap();
    let saved = rows::snapshot(&st, &tables, &junctions).unwrap();
    let mut back = store(1 << 40);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    let again = rows::snapshot(&back, &tables, &junctions).unwrap();
    assert_eq!(again, saved, "the activity comes back identical");
}

#[test]
fn a_long_list_with_an_opposite_whose_back_references_are_gone_is_read_back_in_linear_time() {
    // As a hand-edited database might hold it: no attribute's back-reference to its class is
    // left, so the whole list comes from its own rows, each value adding the opposite.
    // Long enough that searching the list for each value would take far longer than the limit.
    let (mut saved, tables, junctions) = class_rows(100_000, false);
    let table = tables.iter().position(|t| t.name == "uml_property").unwrap();
    let column = tables[table].columns.iter().position(|c| c.prop == Some(u::property::CLASS)).unwrap();
    for row in &mut saved.tables[table] {
        row[column] = rows::Cell::Null;
    }
    let started = std::time::Instant::now();
    let mut back = store(1 << 40);
    rows::restore(&mut back, &saved, &tables, &junctions).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "reading took {:?}", started.elapsed());
    let list = junctions.iter().position(|j| j.prop == u::class::OWNED_ATTRIBUTE && j.class == u::CLASS).unwrap();
    assert_eq!(rows::snapshot(&back, &tables, &junctions).unwrap().junctions[list], saved.junctions[list]);
}

#[test]
fn a_node_that_leaves_an_activitys_nodes_leaves_its_structured_nodes_and_groups_and_its_container() {
    let mut st = store(0);
    let (a, b) = (st.create(u::ACTIVITY).unwrap(), st.create(u::ACTIVITY).unwrap());
    let node = st.create(u::STRUCTURED_ACTIVITY_NODE).unwrap();
    // A structured node is a node and a group of the activity, contained by it.
    st.push(a, u::activity::STRUCTURED_NODE, Value::Ref(node)).unwrap();
    for list in [u::activity::NODE, u::activity::GROUP] {
        assert!(st.list(a, list).unwrap().contains(&Value::Ref(node)));
    }
    assert_eq!(st.container(node).unwrap().map(|(c, _)| c), Some(a));
    // Out of the nodes: it leaves the structured nodes and, with them, the groups, so no list
    // that contains it is left behind.
    st.remove_value(a, u::activity::NODE, &Value::Ref(node)).unwrap();
    for list in [u::activity::NODE, u::activity::STRUCTURED_NODE, u::activity::GROUP] {
        assert!(!st.list(a, list).unwrap().contains(&Value::Ref(node)), "{list:?}");
    }
    assert_eq!(st.container(node).unwrap(), None);
    // Another activity takes it whole; the first holds it nowhere.
    st.push(b, u::activity::STRUCTURED_NODE, Value::Ref(node)).unwrap();
    assert_eq!(st.container(node).unwrap().map(|(c, _)| c), Some(b));
    for list in [u::activity::NODE, u::activity::STRUCTURED_NODE, u::activity::GROUP] {
        assert!(!st.list(a, list).unwrap().contains(&Value::Ref(node)), "{list:?}");
        assert!(st.list(b, list).unwrap().contains(&Value::Ref(node)), "{list:?}");
    }
}
