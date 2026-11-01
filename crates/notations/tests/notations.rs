//! The project composed from its notations: every notation's metamodel, slabs and tables in one
//! store, and a model of two notations rebuilt from its rows.

use archforge_mof::rows;
use archforge_mof::{Uuid, Value};
use archforge_notations::{NOTATIONS, new_project_session, session_from_rows, storage, store, universe};

fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

#[test]
fn every_notation_joins_the_universe_and_the_tables_once() {
    let u = universe().unwrap();
    for n in NOTATIONS {
        let m = (n.metamodel)().unwrap();
        assert!(u.metamodels().any(|held| held.uri == m.uri), "{} is in the universe", n.name);
    }
    let (tables, junctions) = storage();
    assert_eq!(tables.len(), NOTATIONS.iter().map(|n| n.tables.len()).sum::<usize>());
    assert_eq!(junctions.len(), NOTATIONS.iter().map(|n| n.junctions.len()).sum::<usize>());
    let mut names = std::collections::BTreeSet::new();
    assert!(tables.iter().map(|t| t.name).chain(junctions.iter().map(|j| j.name)).all(|n| names.insert(n)), "every table once");
}

#[test]
fn a_model_of_er_and_uml_comes_back_from_its_rows() {
    let mut s = new_project_session("Shop", ids(0)).unwrap();
    let shop = s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap();
    assert!(shop.value["id"].is_string());
    let (tables, junctions) = storage();
    let mut written = rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    // A UML class in a package, a member of the same Model, beside the ER schema.
    let mut st = store(ids(1 << 40)).unwrap();
    rows::restore(&mut st, &written, &tables, &junctions).unwrap();
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_uml::PackageObj::set_name(&mut st, pkg, Some("sales")).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(pkg)).unwrap();
    let class = st.create(archforge_uml::CLASS).unwrap();
    archforge_uml::ClassObj::set_name(&mut st, class, Some("Customer")).unwrap();
    st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(class)).unwrap();
    written = rows::snapshot(&st, &tables, &junctions).unwrap();
    let back = session_from_rows(&written, ids(1 << 41)).unwrap();
    assert_eq!(rows::snapshot(back.doc(), &tables, &junctions).unwrap(), written);
    assert_eq!(
        archforge_core::name_of(back.doc(), back.doc().alive().find(|o| back.doc().class_of(*o).ok() == Some(archforge_uml::CLASS)).unwrap()),
        "Customer"
    );
}
