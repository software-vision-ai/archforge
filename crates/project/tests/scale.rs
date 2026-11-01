//! A measurement, not a check: how a project of every notation's tables (ER's and all of UML's)
//! is created, written, read and saved. Run with `cargo test -p archforge-project --test scale -- --ignored --nocapture`.
#![cfg(unix)]

use std::time::{Duration, Instant};

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::{Uuid, rows};
use archforge_project::{ModelSchema, Project, timestamp};
use serde_json::json;

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
#[ignore = "a measurement"]
fn a_project_with_er_and_uml_tables() {
    let tag = std::process::id();
    let base = std::env::temp_dir().join(format!("af-scale-{tag}"));
    let run_root = std::env::temp_dir().join(format!("afs-{tag}"));
    for d in [&base, &run_root] {
        let _ = std::fs::remove_dir_all(d);
        std::fs::create_dir_all(d).unwrap();
    }
    let home = Home::prepare(&base.join("home")).unwrap();
    let dolt = Dolt::find(&home).unwrap();
    let opts = ServerOptions { run_root: Some(run_root.clone()), ready_budget: Duration::from_secs(30), ..ServerOptions::new(base.join("records")) };
    // Every notation's tables: the core's, the diagrams', ER's and all of UML's.
    let universe = archforge_notations::universe().unwrap();
    let (tables, junctions) = archforge_notations::storage();
    let schema = ModelSchema::new(&universe, tables, junctions).unwrap();
    println!("tables {} lists {}", schema.tables().len(), schema.junctions().len());
    let t = Instant::now();
    let project = Project::create(&dolt, &opts, &base.join("Projects"), "Scale", "test", &timestamp(1_761_670_800)).unwrap();
    println!("create project {:?}", t.elapsed());
    let t = Instant::now();
    project.prepare_model(&schema).unwrap();
    println!("prepare {:?}", t.elapsed());
    // An ER model of 60 tables written as the shell writes it; UML's tables stay empty.
    let mut s = archforge_notations::new_project_session("Scale", ids(0)).unwrap();
    let schema_id = s.execute("er.schema.add", &json!({ "name": "shop" })).unwrap().value["id"].as_str().unwrap().to_string();
    for i in 0..60 {
        let e =
            s.execute("er.entity.add", &json!({ "schema": schema_id, "name": format!("t{i}") })).unwrap().value["id"].as_str().unwrap().to_string();
        for c in 0..5 {
            s.execute("er.attribute.add", &json!({ "entity": e, "name": format!("c{c}"), "type": "TEXT" })).unwrap();
        }
    }
    let all = rows::snapshot(s.doc(), schema.tables(), &schema.junctions()).unwrap();
    let used = all.tables.iter().filter(|t| !t.is_empty()).count() + all.junctions.iter().filter(|j| !j.is_empty()).count();
    println!("rows {} in {used} tables", all.tables.iter().map(Vec::len).sum::<usize>());
    for round in 0..3 {
        let t = Instant::now();
        project.write_model(&schema, &all).unwrap();
        println!("write {round} {:?}", t.elapsed());
    }
    let t = Instant::now();
    project.commit("Saved").unwrap();
    println!("save {:?}", t.elapsed());
    let t = Instant::now();
    let read = project.read_model(&schema).unwrap();
    println!("read {:?}", t.elapsed());
    assert_eq!(read, all, "the model comes back as written");
    project.close().unwrap();
    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&run_root);
}
