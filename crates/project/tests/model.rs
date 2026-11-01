//! A project's model in its Dolt database (archforge-kjz.3): written, saved, closed and opened
//! again, it comes back identical; the unsaved state follows the writes and the saves; a project
//! made for other tables is refused. Needs dolt 2.6 or later, as the project tests do.
#![cfg(unix)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::rows;
use archforge_mof::{Store, Uuid, Value};
use archforge_notations::Session;
use archforge_project::{EARLIER_MODELS, Error, ModelSchema, Project, timestamp};
use mysql::prelude::Queryable;
use serde_json::{Value as Json, json};

struct Env {
    base: PathBuf,
    run_root: PathBuf,
    dolt: Dolt,
}

impl Env {
    fn new(name: &str) -> Env {
        let tag = std::process::id();
        let base = std::env::temp_dir().join(format!("af-pm-{name}-{tag}"));
        let run_root = std::env::temp_dir().join(format!("afm-{name}-{tag}"));
        for d in [&base, &run_root] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d).unwrap();
        }
        let home = Home::prepare(&base.join("home")).unwrap();
        let dolt = Dolt::find(&home).unwrap_or_else(|e| panic!("these tests need dolt 2.6 or later on the PATH or in ARCHFORGE_DOLT: {e}"));
        Env { base, run_root, dolt }
    }

    fn opts(&self) -> ServerOptions {
        ServerOptions {
            run_root: Some(self.run_root.clone()),
            ready_budget: Duration::from_secs(20),
            ..ServerOptions::new(self.base.join("records"))
        }
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.run_root).stderr(Stdio::null()).status();
        let _ = std::fs::remove_dir_all(&self.base);
        let _ = std::fs::remove_dir_all(&self.run_root);
    }
}

fn ids(start: u64) -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n = start;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

fn schema() -> ModelSchema {
    let universe = archforge_notations::store(ids(0)).unwrap().universe().clone();
    let (tables, junctions) = archforge_notations::storage();
    ModelSchema::new(&universe, tables, junctions).unwrap()
}

fn run(s: &mut Session, cmd: &str, p: Json) -> String {
    let out = s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"));
    out.value.get("id").and_then(Json::as_str).unwrap_or_default().to_string()
}

fn model() -> Session {
    let mut s = archforge_notations::new_project_session("Commerce platform", ids(0)).unwrap();
    let shop = run(&mut s, "er.schema.add", json!({ "name": "shop" }));
    let customer = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Kunde · 顧客" }));
    run(&mut s, "er.attribute.add", json!({ "entity": customer, "name": "id", "type": "BIGINT", "primary_key": true }));
    let order = run(&mut s, "er.entity.add", json!({ "schema": shop, "name": "Order" }));
    run(
        &mut s,
        "er.relationship.add",
        json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
    );
    run(&mut s, "er.enum.add", json!({ "schema": shop, "name": "state", "literals": ["open", "paid"] }));
    let d = run(&mut s, "diagram.add", json!({ "kind": "er", "element": shop, "name": "Shop" }));
    run(&mut s, "diagram.place", json!({ "diagram": d, "element": customer, "x": 10.5, "y": 20, "width": 240, "height": 120 }));
    run(
        &mut s,
        "skill.add",
        json!({ "subject": shop, "name": "Naming", "applies_to": "table", "triggers": ["naming", "export"], "body": "Singular." }),
    );
    s
}

fn rows_of(st: &Store, schema: &ModelSchema) -> rows::Rows {
    rows::snapshot(st, schema.tables(), &schema.junctions()).unwrap()
}

#[test]
fn a_saved_model_opens_again_identical_and_the_unsaved_state_follows() {
    let env = Env::new("roundtrip");
    let schema = schema();
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let s = model();
    let written = rows_of(s.doc(), &schema);
    project.write_model(&schema, &written).unwrap();
    assert!(project.has_unsaved().unwrap(), "written, not yet saved");
    assert!(project.commit("Saved by the test").unwrap());
    assert!(!project.has_unsaved().unwrap());
    assert!(!project.commit("Nothing to save").unwrap(), "a save with nothing new is a save");
    // Writing twice replaces, never doubles.
    project.write_model(&schema, &written).unwrap();
    assert!(!project.has_unsaved().unwrap(), "the same rows again change nothing");
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let again = Project::open(&env.dolt, &env.opts(), &path).unwrap();
    again.prepare_model(&schema).unwrap();
    let read = again.read_model(&schema).unwrap();
    let mut back = archforge_notations::store(ids(1 << 40)).unwrap();
    rows::restore(&mut back, &read, schema.tables(), &schema.junctions()).unwrap();
    assert_eq!(rows_of(&back, &schema), written, "the model comes back identical");
    // The last save is in the history with its message.
    let mut c = again.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let newest: String = c.query_first("SELECT message FROM dolt_log ORDER BY date DESC LIMIT 1").unwrap().unwrap();
    assert_eq!(newest, "Saved by the test");
    // The project holds the tables its rows went to, not every table of the model: a table it
    // lacks reads as empty.
    let held: i64 = c.query_first("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'project'").unwrap().unwrap();
    let used = written.tables.iter().filter(|t| !t.is_empty()).count() + written.junctions.iter().filter(|j| !j.is_empty()).count();
    assert_eq!(held as usize, used + 2, "the tables used, the settings and the list of the tables used");
    // The packet limit is the one the server is started with.
    let packet: u64 = c.query_first("SELECT @@max_allowed_packet").unwrap().unwrap();
    assert_eq!(packet, archforge_dolt::MAX_PACKET as u64);
    drop(c);
    again.close().unwrap();
}

#[test]
fn a_project_made_for_other_tables_is_refused() {
    let env = Env::new("other");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Library", "test", &timestamp(1_761_670_800)).unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    c.query_drop("INSERT INTO settings (`key`, `value`) VALUES ('model_schema', '0123456789abcdef')").unwrap();
    // A fingerprint no earlier Archforge made: refused as it is, nothing added.
    assert!(matches!(project.prepare_model(&schema), Err(Error::OtherModel { .. })));
    let tables: i64 = c.query_first("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'project'").unwrap().unwrap();
    assert_eq!(tables, 1, "only the settings");
    // An earlier Archforge's fingerprint over tables that are not an earlier shape: refused.
    c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'model_schema'", (EARLIER_MODELS[0],)).unwrap();
    // A table Archforge does not know: another model, or a later Archforge's.
    c.query_drop("CREATE TABLE library_book (id BINARY(16) NOT NULL PRIMARY KEY, title MEDIUMTEXT NULL)").unwrap();
    assert!(matches!(project.prepare_model(&schema), Err(Error::OtherModel { .. })));
    // A table Archforge knows, with columns it does not write.
    c.query_drop("DROP TABLE library_book").unwrap();
    c.query_drop("CREATE TABLE diagram_point (id BINARY(16) NOT NULL PRIMARY KEY, x MEDIUMTEXT NULL)").unwrap();
    assert!(matches!(project.prepare_model(&schema), Err(Error::OtherModel { .. })));
    // Columns past the ones Archforge writes: a later Archforge's.
    c.query_drop("DROP TABLE diagram_point").unwrap();
    c.query_drop("CREATE TABLE diagram_point (id BINARY(16) NOT NULL PRIMARY KEY, x DOUBLE NOT NULL, y DOUBLE NOT NULL, z DOUBLE NULL)").unwrap();
    assert!(matches!(project.prepare_model(&schema), Err(Error::OtherModel { .. })));
    // A column that may be empty where Archforge's may not.
    c.query_drop("DROP TABLE diagram_point").unwrap();
    c.query_drop("CREATE TABLE diagram_point (id BINARY(16) NOT NULL PRIMARY KEY, x DOUBLE NULL)").unwrap();
    assert!(matches!(project.prepare_model(&schema), Err(Error::OtherModel { .. })));
    // A list table of another shape.
    c.query_drop("DROP TABLE diagram_point").unwrap();
    c.query_drop("CREATE TABLE diagram_diagram_node (owner BINARY(16) NOT NULL, position INT NOT NULL, value MEDIUMTEXT NOT NULL, PRIMARY KEY (owner, position))").unwrap();
    assert!(matches!(project.prepare_model(&schema), Err(Error::OtherModel { .. })));
    drop(c);
    project.close().unwrap();
}

#[test]
fn a_project_of_an_earlier_archforge_gets_the_columns_and_tables_added_since() {
    let env = Env::new("upgrade");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Earlier", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let s = model();
    let written = rows_of(s.doc(), &schema);
    project.write_model(&schema, &written).unwrap();
    project.commit("Saved").unwrap();
    // As an earlier Archforge left it: the diagram's style and a style's routing and avoid (the
    // last columns: one optional, two that a new object fills), and a list table, missing.
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    c.query_drop("ALTER TABLE diagram_diagram DROP COLUMN `style`").unwrap();
    c.query_drop("ALTER TABLE diagram_style DROP COLUMN `avoid`").unwrap();
    c.query_drop("ALTER TABLE diagram_style DROP COLUMN `routing`").unwrap();
    c.query_drop("INSERT INTO diagram_style (id, name, fill, stroke, `text`, line) VALUES (UNHEX('00000000000000000000000000000abc'), NULL, '#FFFFFF', NULL, NULL, 2)").unwrap();
    c.query_drop("DROP TABLE core_model_skill").unwrap();
    c.query_drop("DROP TABLE model_tables").unwrap();
    c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'model_schema'", (EARLIER_MODELS[EARLIER_MODELS.len() - 1],)).unwrap();
    drop(c);
    project.prepare_model(&schema).unwrap();
    let read = project.read_model(&schema).unwrap();
    let mut back = archforge_notations::store(ids(1 << 40)).unwrap();
    rows::restore(&mut back, &read, schema.tables(), &schema.junctions()).unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    // The style an earlier Archforge wrote takes what a new style holds: routing as the diagram, no avoiding.
    let filled: (i64, i64) =
        c.query_first("SELECT routing, avoid FROM diagram_style WHERE id = UNHEX('00000000000000000000000000000abc')").unwrap().unwrap();
    assert_eq!(filled, (0, 0));
    let held: String = c.query_first("SELECT `value` FROM settings WHERE `key` = 'model_schema'").unwrap().unwrap();
    assert_eq!(held, schema.fingerprint(), "the fingerprint is the current one");
    // The tables it held are the ones in use; the one it lacked is not.
    let in_use: i64 = c.query_first("SELECT COUNT(*) FROM model_tables WHERE name IN ('diagram_style', 'core_model_skill')").unwrap().unwrap();
    assert_eq!(in_use, 1);
    // Opened again, nothing more to add.
    project.prepare_model(&schema).unwrap();
    drop(c);
    project.close().unwrap();
}

/// The fingerprint of the tables of the Archforge that held ER alone (before UML joined).
const ER_ALONE: &str = "8c7ee9a29f10f219";

#[test]
fn er_and_uml_live_in_one_project_and_a_project_of_er_alone_takes_uml() {
    assert!(EARLIER_MODELS.contains(&ER_ALONE));
    let env = Env::new("notations");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Shop", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    // As the Archforge of ER alone left it: an ER model, and that Archforge's fingerprint.
    let s = model();
    project.write_model(&schema, &rows_of(s.doc(), &schema)).unwrap();
    project.commit("Saved").unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'model_schema'", (ER_ALONE,)).unwrap();
    c.query_drop("DROP TABLE model_tables").unwrap();
    let path = project.path().to_path_buf();
    drop(c);
    project.close().unwrap();
    // Opened by this Archforge: brought up to date, and its ER model reads back as it was.
    let project = Project::open(&env.dolt, &env.opts(), &path).unwrap();
    project.prepare_model(&schema).unwrap();
    let mut st = archforge_notations::store(ids(1 << 40)).unwrap();
    rows::restore(&mut st, &project.read_model(&schema).unwrap(), schema.tables(), &schema.junctions()).unwrap();
    assert_eq!(rows_of(&st, &schema), rows_of(s.doc(), &schema));
    // A UML package with a class beside the ER schema, in a folder of the same Model, with a
    // trace link from the class to an ER table.
    let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
    let table = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY)).unwrap();
    let folder = st.create(archforge_core::FOLDER).unwrap();
    archforge_core::FolderObj::set_name(&mut st, folder, Some("Domain")).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(folder)).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_uml::PackageObj::set_name(&mut st, pkg, Some("sales")).unwrap();
    archforge_core::FolderObj::push_member(&mut st, folder, Value::Ref(pkg)).unwrap();
    let class = st.create(archforge_uml::CLASS).unwrap();
    archforge_uml::ClassObj::set_name(&mut st, class, Some("Customer")).unwrap();
    st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(class)).unwrap();
    archforge_core::link(&mut st, model, archforge_core::LinkKind::Realization, table, class, None).unwrap();
    assert!(st.validate_all().is_empty(), "{:?}", st.validate_all());
    let both = rows_of(&st, &schema);
    project.write_model(&schema, &both).unwrap();
    project.commit("UML joins").unwrap();
    project.close().unwrap();
    let project = Project::open(&env.dolt, &env.opts(), &path).unwrap();
    project.prepare_model(&schema).unwrap();
    let read = project.read_model(&schema).unwrap();
    assert_eq!(read, both, "ER and UML come back side by side");
    let session = archforge_notations::session_from_rows(&read, ids(1 << 41)).unwrap();
    assert!(session.doc().alive().any(|o| session.doc().class_of(o).ok() == Some(archforge_uml::CLASS)));
    project.close().unwrap();
}

#[test]
fn a_long_text_in_any_script_is_written_and_read_back_whole() {
    let env = Env::new("long");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Long", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    // The longest text the store takes, in a script of three bytes a character: more than a TEXT
    // column holds. (Commands cap their parameters lower, so the rows carry it directly.)
    let mut written = rows_of(model().doc(), &schema);
    let entities = schema.tables().iter().position(|t| t.name == "er_entity").unwrap();
    let name = schema.tables()[entities].columns.iter().position(|c| c.name == "name").unwrap();
    written.tables[entities][0][name] = rows::Cell::Text("顧".repeat(archforge_mof::MAX_STRING).into());
    project.write_model(&schema, &written).unwrap();
    assert_eq!(project.read_model(&schema).unwrap(), written);
    project.close().unwrap();
}

#[test]
fn a_damaged_database_is_refused_with_a_reason_and_without_reading_it() {
    let env = Env::new("damaged");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Damaged", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    project.write_model(&schema, &rows_of(model().doc(), &schema)).unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let refused = |c: &mut mysql::Conn, damage: &[&str], undo: &[&str]| {
        for d in damage {
            c.query_drop(*d).unwrap_or_else(|e| panic!("{d}: {e}"));
        }
        let e = project.read_model(&schema).expect_err("a damaged model was read");
        for u in undo {
            c.query_drop(*u).unwrap_or_else(|e| panic!("{u}: {e}"));
        }
        assert!(matches!(e, Error::Damaged { .. }), "{e:?}");
        e.to_string()
    };
    // A column of a type longer than the one written: its cells could be any size.
    let e = refused(&mut c, &["ALTER TABLE er_entity MODIFY `name` LONGTEXT NULL"], &["ALTER TABLE er_entity MODIFY `name` MEDIUMTEXT NULL"]);
    assert!(e.contains("shape"), "{e}");
    // A view in place of a table.
    let e = refused(
        &mut c,
        &["RENAME TABLE er_entity TO er_entity_kept", "CREATE VIEW er_entity AS SELECT * FROM er_entity_kept"],
        &["DROP VIEW er_entity", "RENAME TABLE er_entity_kept TO er_entity"],
    );
    assert!(e.contains("not a table"), "{e}");
    // A column more, or one missing.
    let e = refused(&mut c, &["ALTER TABLE er_entity ADD COLUMN `extra` INT NULL"], &["ALTER TABLE er_entity DROP COLUMN `extra`"]);
    assert!(e.contains("shape"), "{e}");
    // A generated column of the right name and type, computed on every read.
    let e = refused(
        &mut c,
        &[
            "ALTER TABLE er_entity DROP COLUMN `description`",
            "ALTER TABLE er_entity ADD COLUMN `description` MEDIUMTEXT AS (REPEAT(`name`, 1000)) VIRTUAL AFTER `id`",
        ],
        &["ALTER TABLE er_entity DROP COLUMN `description`", "ALTER TABLE er_entity ADD COLUMN `description` MEDIUMTEXT NULL AFTER `id`"],
    );
    assert!(e.contains("shape"), "{e}");
    // A model table named in another case: the database finds it under either name, so it is
    // neither read as missing nor cleared as the model's.
    let e = refused(
        &mut c,
        &["RENAME TABLE er_entity TO entity_kept", "RENAME TABLE entity_kept TO ER_ENTITY"],
        &["RENAME TABLE ER_ENTITY TO entity_kept", "RENAME TABLE entity_kept TO er_entity"],
    );
    assert!(e.contains("another case"), "{e}");
    // A table far wider than any model table, beside the ones read: refused, not read short.
    let wide: Vec<String> = (0..700).map(|i| format!("ADD COLUMN `w{i}` INT NULL")).collect();
    let drop_wide: Vec<String> = (0..700).map(|i| format!("DROP COLUMN `w{i}`")).collect();
    let e = refused(&mut c, &[&format!("ALTER TABLE er_entity {}", wide.join(", "))], &[&format!("ALTER TABLE er_entity {}", drop_wide.join(", "))]);
    assert!(e.contains("more columns"), "{e}");
    // A table the model uses, gone: refused as lost, not read as never used.
    let e = refused(&mut c, &["RENAME TABLE er_attribute TO attribute_kept"], &["RENAME TABLE attribute_kept TO er_attribute"]);
    assert!(e.contains("missing"), "{e}");
    // The list of the tables in use, of another shape: a name written to it could be cut short.
    let e = refused(
        &mut c,
        &["ALTER TABLE model_tables MODIFY `name` VARCHAR(63) NOT NULL"],
        &["ALTER TABLE model_tables MODIFY `name` VARCHAR(64) NOT NULL"],
    );
    assert!(e.contains("shape"), "{e}");
    // After every repair the model reads again.
    assert!(project.read_model(&schema).is_ok());
    drop(c);
    project.close().unwrap();
}

#[test]
fn a_change_of_the_model_tables_keeps_the_fingerprint_it_had() {
    // When the tables change, this fingerprint goes to EARLIER_MODELS and this line takes the new
    // one: projects of this build must stay upgradable by the next.
    assert_eq!(schema().fingerprint(), CURRENT, "the model's tables changed: add {CURRENT} to EARLIER_MODELS, then update CURRENT");
    assert!(!EARLIER_MODELS.contains(&CURRENT));
}

const CURRENT: &str = "bb92f9913c8937ef";
