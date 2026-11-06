//! A project's model in its Dolt database: written, saved, closed and opened
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

/// Classifiers nested in a class (AR-132), and one moved from a package into it, are saved in the
/// project's existing tables and come back nested, in their order, with the type they give, after
/// the project is closed and opened again: no table or column is added for them.
#[test]
fn nested_classifiers_are_saved_and_opened_again_nested() {
    let env = Env::new("nested");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Shop", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let mut s = archforge_notations::new_project_session("Shop", ids(0)).unwrap();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let priority = run(&mut s, "uml.nested.add", json!({ "owner": order, "kind": "enumeration", "name": "Priority" }));
    run(&mut s, "uml.literal.add", json!({ "enumeration": priority, "name": "Express" }));
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    run(&mut s, "uml.classifier.set-owner", json!({ "classifier": line, "owner": order }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "priority", "type": priority }));
    let written = rows_of(s.doc(), &schema);
    project.write_model(&schema, &written).unwrap();
    project.commit("Nested").unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let again = Project::open(&env.dolt, &env.opts(), &path).unwrap();
    again.prepare_model(&schema).unwrap();
    let read = again.read_model(&schema).unwrap();
    assert_eq!(read, written, "the rows come back as written");
    let session = archforge_notations::session_from_rows(&read, ids(1 << 41)).unwrap();
    let st = session.doc();
    let o = st.by_uuid(Uuid::parse(&order).unwrap()).unwrap();
    let names: Vec<String> = archforge_uml::nesting::nested(st, o).into_iter().map(|c| archforge_core::name_of(st, c)).collect();
    assert_eq!(names, ["Priority", "Line"], "nested in Order, in their order");
    let p = st.by_uuid(Uuid::parse(&priority).unwrap()).unwrap();
    assert_eq!(archforge_uml::nesting::qualified_name(st, p), "Order::Priority");
    again.close().unwrap();
}

/// Saved, closed and opened again through the project, the rows come back and a session is made
/// of them, or the reason it is not.
fn save_and_reopen(env: &Env, schema: &ModelSchema, path: &std::path::Path, st: &Store, message: &str) -> Result<Session, String> {
    let project = Project::open(&env.dolt, &env.opts(), path).unwrap();
    project.prepare_model(schema).unwrap();
    let written = rows_of(st, schema);
    project.write_model(schema, &written).unwrap();
    assert!(project.commit(message).unwrap());
    project.close().unwrap();
    let project = Project::open(&env.dolt, &env.opts(), path).unwrap();
    project.prepare_model(schema).unwrap();
    let read = project.read_model(schema).unwrap();
    project.close().unwrap();
    assert_eq!(read, written, "{message}: the rows come back as written");
    archforge_notations::session_from_rows(&read, ids(1 << 44))
}

/// AR-208: a project whose names differ only in case across kinds (an attribute and an operation,
/// a package and a class in one package) saved, but its check as it opened refused it, so it never
/// opened again. As UML has it, elements of different kinds share names: such a project opens, one
/// that opened before still does, and so does one saved in that state before the fix; two
/// elements of one kind named alike still never open.
#[test]
fn names_shared_across_kinds_are_saved_and_the_project_opens_again() {
    let env = Env::new("names");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Names", "test", &timestamp(1_761_670_800)).unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    // A project that opened before opens still.
    let mut s = model();
    save_and_reopen(&env, &schema, &path, s.doc(), "As before").expect("a project that opened before opens");
    // Made by the commands, as the bug was found.
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "total" }));
    run(&mut s, "uml.operation.add", json!({ "class": order, "name": "Total" }));
    let line = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Line" }));
    run(&mut s, "uml.attribute.add", json!({ "class": line, "name": "total" }));
    run(&mut s, "uml.operation.add", json!({ "class": line, "name": "total" }));
    run(&mut s, "uml.package.add", json!({ "parent": sales, "name": "Billing" }));
    run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Billing" }));
    let back = save_and_reopen(&env, &schema, &path, s.doc(), "Names shared across kinds").expect("names of different kinds open");
    assert_eq!(rows_of(back.doc(), &schema), rows_of(s.doc(), &schema));
    // Saved before the fix, written straight into the store as no command would now: a data
    // type's attribute and operation named alike, and a package beside a classifier of its name.
    let mut st = archforge_notations::store(ids(1 << 42)).unwrap();
    rows::restore(&mut st, &rows_of(s.doc(), &schema), schema.tables(), &schema.junctions()).unwrap();
    let sales = st.by_uuid(Uuid::parse(&sales).unwrap()).unwrap();
    let named = |st: &mut Store, owner, prop, class, name: &str| {
        let o = st.create(class).unwrap();
        st.set(o, archforge_uml::named_element::NAME, Value::str(name)).unwrap();
        st.push(owner, prop, Value::Ref(o)).unwrap();
        o
    };
    let money = named(&mut st, sales, archforge_uml::package::PACKAGED_ELEMENT, archforge_uml::DATA_TYPE, "Money");
    named(&mut st, money, archforge_uml::data_type::OWNED_ATTRIBUTE, archforge_uml::PROPERTY, "amount");
    named(&mut st, money, archforge_uml::data_type::OWNED_OPERATION, archforge_uml::OPERATION, "AMOUNT");
    named(&mut st, sales, archforge_uml::package::PACKAGED_ELEMENT, archforge_uml::PACKAGE, "money");
    save_and_reopen(&env, &schema, &path, &st, "Saved in the old state").expect("a project saved before the fix opens");
    // Two classifiers of one name never open.
    named(&mut st, sales, archforge_uml::package::PACKAGED_ELEMENT, archforge_uml::ENUMERATION, "ORDER");
    let e = save_and_reopen(&env, &schema, &path, &st, "Two of one kind").err().expect("two classifiers of one name are refused");
    assert!(e.contains("DuplicateName"), "{e}");
}

/// AR-209: two ER schemas whose names differed only in case were added, or renamed into, and the
/// project saved, but its check as it opened refused it, so it never opened again. SQL names a
/// schema once in a database: the commands refuse the second, whatever the case, and a project of
/// distinct names, with a table and a column named like their schema, opens again.
#[test]
fn schemas_named_alike_are_refused_and_distinct_ones_are_saved_and_open_again() {
    let env = Env::new("schemas");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Schemas", "test", &timestamp(1_761_670_800)).unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    // As the bug was found: a second schema named like the first ("shop"), added or renamed into.
    let mut s = model();
    let added = s.execute("er.schema.add", &json!({ "name": "Shop" })).map(|_| ());
    save_and_reopen(&env, &schema, &path, s.doc(), "A second shop").expect("the project opens again after er.schema.add");
    let e = added.expect_err("a second schema named shop is refused");
    assert!(e.to_string().contains("there is a schema named shop already"), "{e}");
    let billing = run(&mut s, "er.schema.add", json!({ "name": "billing" }));
    let renamed = s.execute("er.schema.rename", &json!({ "schema": billing, "name": "SHOP" })).map(|_| ());
    save_and_reopen(&env, &schema, &path, s.doc(), "Renamed onto shop").expect("the project opens again after er.schema.rename");
    let e = renamed.expect_err("a schema renamed onto shop is refused");
    assert!(e.to_string().contains("there is a schema named shop already"), "{e}");
    // Distinct names, a schema's own name in another case, and a table and a column named like
    // their schema save and open again.
    run(&mut s, "er.schema.rename", json!({ "schema": billing, "name": "Billing" }));
    let table = run(&mut s, "er.entity.add", json!({ "schema": billing, "name": "billing" }));
    run(&mut s, "er.attribute.add", json!({ "entity": table, "name": "Billing", "type": "TEXT" }));
    let back = save_and_reopen(&env, &schema, &path, s.doc(), "Distinct schemas").expect("distinct schemas open");
    assert_eq!(rows_of(back.doc(), &schema), rows_of(s.doc(), &schema));
}

/// AR-209, found beside it: a domain given two checks whose names differed only in case saved, but
/// the check as it opened refused it. As SQL names a domain's constraints once, the commands refuse
/// them, and a domain whose checks are named apart opens again.
#[test]
fn a_domain_with_two_checks_named_alike_is_refused_and_one_named_apart_opens_again() {
    let env = Env::new("domain-checks");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Checks", "test", &timestamp(1_761_670_800)).unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let mut s = model();
    let shop = s.doc().alive().find(|o| s.doc().class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap();
    let shop = s.doc().uuid(shop).unwrap().to_string();
    let twice = json!([{ "expression": "VALUE > 0", "name": "positive" }, { "expression": "VALUE < 100", "name": "Positive" }]);
    let added = s.execute("er.domain.add", &json!({ "schema": shop, "name": "amount", "base_type": "INTEGER", "checks": twice })).map(|_| ());
    save_and_reopen(&env, &schema, &path, s.doc(), "Checks named alike").expect("the project opens again after er.domain.add");
    let e = added.expect_err("two checks named alike are refused");
    assert!(e.to_string().contains("two checks are named Positive"), "{e}");
    let apart = json!([{ "expression": "VALUE > 0", "name": "positive" }, { "expression": "VALUE < 100", "name": "small" }]);
    let amount = run(&mut s, "er.domain.add", json!({ "schema": shop, "name": "amount", "base_type": "INTEGER", "checks": apart }));
    let set = s.execute("er.domain.set", &json!({ "domain": amount, "name": "amount", "base_type": "INTEGER", "checks": twice })).map(|_| ());
    let back = save_and_reopen(&env, &schema, &path, s.doc(), "Checks named apart").expect("the project opens again after er.domain.set");
    let e = set.expect_err("checks set alike are refused");
    assert!(e.to_string().contains("two checks are named Positive"), "{e}");
    assert_eq!(rows_of(back.doc(), &schema), rows_of(s.doc(), &schema), "the checks named apart come back");
}

/// A template's parameters (AR-130), with their kinds, constraint, default and order, and a member
/// typed by one, are saved in the project's existing tables and come back as written after the
/// project is closed and opened again: no table or column is added for them, and the signature's
/// parameter list is read back once, not twice.
#[test]
fn template_parameters_are_saved_and_opened_again() {
    let env = Env::new("templates");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Shop", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let mut s = archforge_notations::new_project_session("Shop", ids(0)).unwrap();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let entity = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Entity" }));
    let repo = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Repository" }));
    let t = run(&mut s, "uml.template-parameter.add", json!({ "classifier": repo, "name": "T", "constraint": entity, "default": entity }));
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": repo, "name": "Key", "kind": "data-type", "default": "Integer", "at": 0 }));
    let element = s.execute("uml.template-parameter.list", &json!({ "classifier": repo })).unwrap().value["parameters"][1]["element"]
        .as_str()
        .unwrap()
        .to_string();
    run(&mut s, "uml.attribute.add", json!({ "class": repo, "name": "items", "type": element, "multiplicity": "*" }));
    let written = rows_of(s.doc(), &schema);
    project.write_model(&schema, &written).unwrap();
    project.commit("Templates").unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let again = Project::open(&env.dolt, &env.opts(), &path).unwrap();
    again.prepare_model(&schema).unwrap();
    let read = again.read_model(&schema).unwrap();
    assert_eq!(read, written, "the rows come back as written");
    let mut session = archforge_notations::session_from_rows(&read, ids(1 << 41)).unwrap();
    assert_eq!(rows_of(session.doc(), &schema), written, "read into a model, the same rows again");
    let st = session.doc();
    let r = st.by_uuid(Uuid::parse(&repo).unwrap()).unwrap();
    assert_eq!(archforge_uml::templates::signature_rows(st, r), ["Key : DataType = Integer", "T : Class > Entity = Entity"]);
    let listed = session.execute("uml.template-parameter.list", &json!({ "classifier": repo })).unwrap().value;
    assert_eq!(listed["parameters"][1]["id"], json!(t), "the parameter keeps its id");
    let card = session.execute("uml.class.get", &json!({ "class": repo })).unwrap().value;
    assert_eq!(card["attributes"][0]["type"]["id"], json!(element), "the member keeps its type");
    again.close().unwrap();
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
