//! A project's model in its Dolt database: written, saved, closed and opened
//! again, it comes back identical; the unsaved state follows the writes and the saves; a project
//! made for other tables is refused. Needs dolt 2.6 or later, as the project tests do.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_mof::rows;
use archforge_mof::{Store, Uuid, Value};
use archforge_notations::Session;
use archforge_project::{EARLIER_MODELS, Error, ModelSchema, Project, timestamp};
use mysql::prelude::Queryable;
use serde_json::{Value as Json, json};

/// A folder of its own in the temporary directory, removed when dropped, so on every path. It is
/// made with `create_dir`, which fails for a name already there: no two runs ever share one.
struct Folder(PathBuf);

impl Folder {
    /// Makes a folder whose name starts with `prefix`, under the first number still free.
    fn new(prefix: &str) -> Folder {
        for n in 0..100_000 {
            let path = std::env::temp_dir().join(format!("{prefix}-{n}"));
            match std::fs::create_dir(&path) {
                Ok(()) => return Folder(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("{} cannot be made: {e}", path.display()),
            }
        }
        panic!("no free folder name starts with {prefix}");
    }
}

impl std::ops::Deref for Folder {
    type Target = Path;

    /// The folder's path.
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    /// Removes the folder and what it holds.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Env {
    base: Folder,
    run_root: Folder,
    dolt: Dolt,
}

impl Env {
    /// Folders of its own for the test called `name`, and the dolt the tests run against.
    fn new(name: &str) -> Env {
        let base = Folder::new(&format!("af-pm-{name}"));
        let run_root = Folder::new(&format!("afm-{name}"));
        let home = Home::prepare(&base.join("home")).unwrap();
        let dolt = Dolt::find(&home).unwrap_or_else(|e| panic!("these tests need dolt 2.6 or later on the PATH or in ARCHFORGE_DOLT: {e}"));
        Env { base, run_root, dolt }
    }

    /// Server options that keep everything in this test's folders.
    fn opts(&self) -> ServerOptions {
        ServerOptions {
            run_root: Some(self.run_root.0.clone()),
            ready_budget: Duration::from_secs(20),
            ..ServerOptions::new(self.base.join("records"))
        }
    }
}

impl Drop for Env {
    /// Stops what still runs from the run root; the folders go once the fields are dropped.
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.run_root.0).stderr(Stdio::null()).status();
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

/// A connection to the project's working set.
fn working(project: &Project) -> mysql::Conn {
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    c
}

/// The hash of the project's last commit.
fn head_of(project: &Project) -> String {
    working(project).query_first("SELECT HASHOF('HEAD')").unwrap().unwrap()
}

/// What reading a version must leave alone: the working set's hash, the head, how many commits
/// and how many branches there are.
fn untouched(project: &Project) -> (String, String, i64, i64) {
    let mut c = working(project);
    let unsaved: String = c.query_first("SELECT DOLT_HASHOF_DB('WORKING')").unwrap().unwrap();
    let commits: i64 = c.query_first("SELECT COUNT(*) FROM dolt_log").unwrap().unwrap();
    let branches: i64 = c.query_first("SELECT COUNT(*) FROM dolt_branches").unwrap().unwrap();
    (unsaved, head_of(project), commits, branches)
}

/// Rows read back, in the order a snapshot gives them, so they compare with rows written.
fn restored(read: &rows::Rows, schema: &ModelSchema) -> rows::Rows {
    let mut back = archforge_notations::store(ids(1 << 40)).unwrap();
    rows::restore(&mut back, read, schema.tables(), &schema.junctions()).unwrap();
    rows_of(&back, schema)
}

/// Each version reads back as saved, while unsaved work, the head and the history stay as they were.
#[test]
fn a_version_reads_as_it_was_saved_and_reading_it_changes_nothing() {
    let env = Env::new("version");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Versions", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let mut s = model();
    let first = rows_of(s.doc(), &schema);
    project.write_model(&schema, &first).unwrap();
    assert!(project.commit("First").unwrap());
    let first_hash = head_of(&project);
    run(&mut s, "er.schema.add", json!({ "name": "billing" }));
    let second = rows_of(s.doc(), &schema);
    project.write_model(&schema, &second).unwrap();
    assert!(project.commit("Second").unwrap());
    let second_hash = head_of(&project);
    run(&mut s, "er.schema.add", json!({ "name": "unsaved" }));
    let current = rows_of(s.doc(), &schema);
    project.write_model(&schema, &current).unwrap();
    // Three different states, so only the one asked for can match.
    assert_ne!(first, second);
    assert_ne!(second, current);

    let before = untouched(&project);
    assert_eq!(restored(&project.read_model_at(&schema, &first_hash).unwrap(), &schema), first, "the first version");
    assert_eq!(restored(&project.read_model_at(&schema, &second_hash).unwrap(), &schema), second, "the second version");
    assert_eq!(untouched(&project), before, "reading a version leaves the working set, the head and the history alone");
    assert!(project.has_unsaved().unwrap(), "the unsaved work is still unsaved");
    assert_eq!(restored(&project.read_model(&schema).unwrap(), &schema), current, "and is still what the project holds");

    // The server itself refuses a write over a version.
    let mut c = project.server().connect().unwrap();
    c.query_drop(format!("USE `project/{first_hash}`")).unwrap();
    assert!(c.query_drop("DELETE FROM settings").is_err(), "a version takes no writes");
    drop(c);
    assert_eq!(untouched(&project), before);
    project.close().unwrap();
}

/// A version's shapes come from that commit: a working set of another shape does not refuse it.
#[test]
fn a_version_is_checked_against_its_own_table_shapes() {
    let env = Env::new("shapes");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Shapes", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let written = rows_of(model().doc(), &schema);
    project.write_model(&schema, &written).unwrap();
    project.commit("Saved").unwrap();
    let hash = head_of(&project);
    // Only the working set takes another shape: the commit keeps the one it was saved with.
    working(&project).query_drop("ALTER TABLE diagram_style DROP COLUMN `avoid`").unwrap();
    let refused = project.read_model(&schema).err();
    assert!(matches!(&refused, Some(Error::Damaged { why, .. }) if why.contains("diagram_style")), "the working set is checked: {refused:?}");
    assert_eq!(restored(&project.read_model_at(&schema, &hash).unwrap(), &schema), written, "and the version against its own shapes");
    project.close().unwrap();
}

/// Text that is no hash, a hash Dolt does not know and another branch's commit are all refused.
#[test]
fn only_a_commit_on_the_projects_own_line_is_a_version() {
    let env = Env::new("notversion");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Refusals", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    project.write_model(&schema, &rows_of(model().doc(), &schema)).unwrap();
    project.commit("Saved").unwrap();
    // A real commit, well shaped and known to Dolt, on another line.
    let mut c = working(&project);
    c.query_drop("CALL DOLT_BRANCH('side')").unwrap();
    c.query_drop("USE `project/side`").unwrap();
    c.query_drop("INSERT INTO settings (`key`, `value`) VALUES ('elsewhere', 'x')").unwrap();
    c.query_drop("CALL DOLT_COMMIT('-Am', 'Elsewhere', '--author', 'Test <test@localhost>')").unwrap();
    let side: String = c.query_first("SELECT HASHOF('HEAD')").unwrap().unwrap();
    drop(c);
    let before = untouched(&project);
    let unknown = "0123456789abcdefghijklmnopqrstuv".to_string();
    let injected = "abc`; CALL DOLT_RESET('--hard'); -- ".to_string();
    for wrong in [String::new(), "HEAD".into(), "main".into(), "side".into(), injected, unknown, side, "x".repeat(50)] {
        let Some(Error::NotAVersion { commit }) = project.read_model_at(&schema, &wrong).err() else {
            panic!("{wrong:?} is not a version");
        };
        assert_eq!(commit, wrong.chars().take(40).collect::<String>(), "the refusal names what was asked for, within bounds");
    }
    assert_eq!(untouched(&project), before, "a refusal changes nothing");
    project.close().unwrap();
}

/// Another branch's version reads on that branch's line, as saved there, while the current branch
/// stays where it was; on a line it is not on, it is refused.
#[test]
fn another_branchs_version_reads_on_its_own_line() {
    use archforge_project::SafeBranch;
    let env = Env::new("onbranch");
    let schema = schema();
    let project = with_branches(&env, &schema, &["side", "bare"]);
    let mut s = model();
    run(&mut s, "er.schema.add", json!({ "name": "billing" }));
    let written = rows_of(s.doc(), &schema);
    let mut side = on_branch(&project, "side");
    project.set_current_branch(SafeBranch::found("side").unwrap());
    project.write_model(&schema, &written).unwrap();
    project.set_current_branch(SafeBranch::found("main").unwrap());
    let hash = commit_all(&mut side, "On side");
    let (reader, before) = (project.version_reader(), untouched(&project));
    let read = reader.read_model_on(&schema, &SafeBranch::found("side").unwrap(), &hash).unwrap();
    assert_eq!(restored(&read, &schema), written, "the version as the other branch saved it");
    assert!(matches!(reader.read_model_at(&schema, &hash), Err(Error::NotAVersion { .. })), "not on the current line");
    let refused = reader.read_model_on(&schema, &SafeBranch::found("bare").unwrap(), &hash);
    assert!(matches!(refused, Err(Error::NotAVersion { .. })), "nor on a line it is not on");
    assert_eq!((untouched(&project), project.current_branch()), (before, SafeBranch::found("main").unwrap()), "reading moves nothing");
    on_branch(&project, "main").query_drop("CALL DOLT_BRANCH('-D', 'side')").unwrap();
    let gone = reader.read_model_on(&schema, &SafeBranch::found("side").unwrap(), &hash);
    assert!(matches!(gone, Err(Error::NotAVersion { .. })), "a branch deleted since: {gone:?}");
    project.close().unwrap();
}

/// Commits everything the working set of `c` holds and gives the new commit's hash.
fn commit_all(c: &mut mysql::Conn, message: &str) -> String {
    c.exec_drop("CALL DOLT_COMMIT('-Am', ?, '--author', 'Test <test@localhost>')", (message,)).unwrap();
    c.query_first("SELECT HASHOF('HEAD')").unwrap().unwrap()
}

/// A project with the test model saved in it, its tables prepared.
fn saved_project(env: &Env, schema: &ModelSchema) -> Project {
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "History", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(schema).unwrap();
    project.write_model(schema, &rows_of(model().doc(), schema)).unwrap();
    assert!(project.commit("Saved").unwrap());
    project
}

/// A version saved by an earlier Archforge reads as the real upgrade leaves those rows, column for
/// column, while the commit and the working set stay as they were.
#[test]
fn a_version_from_before_an_upgrade_reads_as_the_upgrade_leaves_it() {
    let env = Env::new("earlier");
    let schema = schema();
    let project = saved_project(&env, &schema);
    // As an earlier Archforge saved it: later columns (one optional, two that a new object fills),
    // a list table and the list of the tables in use all missing, and a row the fills must reach.
    let mut c = working(&project);
    c.query_drop("ALTER TABLE diagram_diagram DROP COLUMN `style`").unwrap();
    c.query_drop("ALTER TABLE diagram_style DROP COLUMN `avoid`").unwrap();
    c.query_drop("ALTER TABLE diagram_style DROP COLUMN `routing`").unwrap();
    c.query_drop("INSERT INTO diagram_style (id, name, fill, stroke, `text`, line) VALUES (UNHEX('00000000000000000000000000000abc'), NULL, '#FFFFFF', NULL, NULL, 2)").unwrap();
    c.query_drop("DROP TABLE core_model_skill").unwrap();
    c.query_drop("DROP TABLE model_tables").unwrap();
    c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'model_schema'", (EARLIER_MODELS[EARLIER_MODELS.len() - 1],)).unwrap();
    let saved = commit_all(&mut c, "Saved by an earlier Archforge");
    drop(c);
    // The real upgrade of those same rows, in the working set, is what the version must read as.
    project.prepare_model(&schema).unwrap();
    let upgraded = project.read_model(&schema).unwrap();
    let before = untouched(&project);
    assert_eq!(project.read_model_at(&schema, &saved).unwrap(), upgraded, "the later columns read as the upgrade fills them");
    assert_eq!(untouched(&project), before, "and reading it upgrades nothing");
    project.close().unwrap();
}

/// A reader reads a version by itself, and once its project has closed it reads nothing.
#[test]
fn a_version_reader_reads_on_its_own_and_fails_once_the_project_has_closed() {
    let env = Env::new("reader");
    let schema = schema();
    let project = saved_project(&env, &schema);
    let hash = head_of(&project);
    let reader = project.version_reader();
    assert_eq!(reader.read_model_at(&schema, &hash).unwrap(), project.read_model_at(&schema, &hash).unwrap());
    project.close().unwrap();
    assert!(matches!(reader.read_model_at(&schema, &hash), Err(Error::Server(_) | Error::Sql { .. })));
}

/// Each commit Archforge cannot read names its own reason, and which commit it was about.
#[test]
fn a_version_of_a_shape_this_archforge_cannot_read_is_refused_with_its_reason() {
    let env = Env::new("foreign");
    let schema = schema();
    let project = saved_project(&env, &schema);
    let mut c = working(&project);
    let created: String = c.query_first("SELECT commit_hash FROM dolt_log WHERE message = 'Create the project'").unwrap().unwrap();
    c.query_drop("UPDATE settings SET `value` = '0123456789abcdef' WHERE `key` = 'model_schema'").unwrap();
    let other_model = commit_all(&mut c, "Another model's tables");
    c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'model_schema'", (EARLIER_MODELS[0],)).unwrap();
    c.query_drop("CREATE TABLE library_book (id BINARY(16) NOT NULL PRIMARY KEY, title MEDIUMTEXT NULL)").unwrap();
    let unknown_table = commit_all(&mut c, "A table Archforge does not know");
    c.query_drop("DROP TABLE library_book").unwrap();
    c.query_drop("ALTER TABLE diagram_style ADD COLUMN z DOUBLE NULL").unwrap();
    let later_column = commit_all(&mut c, "A column Archforge does not write");
    drop(c);
    for (commit, reason) in
        [(&created, "no fingerprint"), (&other_model, "0123456789abcdef"), (&unknown_table, "library_book"), (&later_column, "diagram_style")]
    {
        let Some(Error::VersionShape { commit: named, why }) = project.read_model_at(&schema, commit).err() else {
            panic!("{reason}: not refused as a shape");
        };
        assert_eq!(&named, commit);
        assert!(why.contains(reason), "{reason}: {why}");
    }
    project.close().unwrap();
}

/// The fingerprint decides, not the columns: under this Archforge's own, an earlier shape is
/// damage, though the same columns under an earlier fingerprint would read.
#[test]
fn a_version_of_this_archforges_fingerprint_must_have_exactly_its_shape() {
    let env = Env::new("exact");
    let schema = schema();
    let project = saved_project(&env, &schema);
    let mut c = working(&project);
    c.query_drop("ALTER TABLE diagram_style DROP COLUMN `avoid`").unwrap();
    let saved = commit_all(&mut c, "Current fingerprint, earlier columns");
    drop(c);
    let refused = project.read_model_at(&schema, &saved).err();
    assert!(matches!(&refused, Some(Error::Damaged { why, .. }) if why.contains("diagram_style")), "{refused:?}");
    project.close().unwrap();
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

/// AR-134: profiles, their stereotypes with extensions and properties, a profile applied to a
/// package and the hidden UML library of metaclasses are saved, closed and opened again, the
/// project's tables unchanged and its check on open passing; the reopened model holds them as made.
#[test]
fn profiles_and_the_uml_library_are_saved_and_the_project_opens_again() {
    let env = Env::new("profiles");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Profiles", "test", &timestamp(1_761_670_800)).unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let mut s = model();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Persistence" }));
    let persistence = run(&mut s, "uml.profile.add", json!({ "name": "Persistence", "package": sales }));
    let entity = run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class", "Association"] }));
    run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "table", "type": "String" }));
    run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Id", "extends": ["Property"] }));
    run(&mut s, "uml.profile.apply", json!({ "package": sales, "profile": persistence }));
    run(&mut s, "uml.profile.diagram", json!({ "profile": persistence }));
    assert_eq!(schema.fingerprint(), CURRENT, "no table changed");
    let back = save_and_reopen(&env, &schema, &path, s.doc(), "Profiles").expect("a project with profiles opens");
    assert_eq!(rows_of(back.doc(), &schema), rows_of(s.doc(), &schema));
    let st = back.doc();
    let p = st.by_uuid(Uuid::parse(&persistence).unwrap()).unwrap();
    let e = st.by_uuid(Uuid::parse(&entity).unwrap()).unwrap();
    assert_eq!(archforge_uml::profiles::extended(st, e), ["Class", "Association"]);
    assert_eq!(archforge_uml::profiles::applying(st, p).len(), 1);
    assert!(archforge_uml::profiles::library(st).is_some(), "the library comes back");
}

/// AR-135: stereotype applications with their values set and left to their defaults, and a class
/// diagram showing the values, are saved, closed and opened again through a real dolt, the
/// project's tables unchanged and its check on open passing; the reopened model holds them as made.
#[test]
fn stereotype_applications_and_their_values_are_saved_and_opened_again() {
    let env = Env::new("applications");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Applications", "test", &timestamp(1_761_670_800)).unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let mut s = model();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "Sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let id = run(&mut s, "uml.attribute.add", json!({ "class": order, "name": "id", "type": "Integer" }));
    let fetch = run(&mut s, "uml.enumeration.add", json!({ "package": sales, "name": "Fetch" }));
    run(&mut s, "uml.literal.add", json!({ "enumeration": fetch, "name": "lazy" }));
    let persistence = run(&mut s, "uml.profile.add", json!({ "name": "Persistence", "package": sales }));
    let entity = run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
    run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "table", "type": "String" }));
    run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "cached", "type": "Boolean", "default": "false" }));
    run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "fetch", "type": fetch }));
    run(&mut s, "uml.attribute.add", json!({ "class": entity, "name": "tags", "type": "String", "multiplicity": "*" }));
    let key = run(&mut s, "uml.stereotype.add", json!({ "profile": persistence, "name": "Id", "extends": ["Property"] }));
    run(&mut s, "uml.profile.apply", json!({ "package": sales, "profile": persistence }));
    run(&mut s, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    run(&mut s, "uml.stereotype.apply", json!({ "element": id, "stereotype": key }));
    for (property, values) in [("table", json!(["orders"])), ("fetch", json!(["lazy"])), ("tags", json!(["a", "b"]))] {
        run(&mut s, "uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": property, "values": values }));
    }
    let d = run(&mut s, "diagram.add", json!({ "kind": "uml.class", "name": "Sales", "element": sales }));
    run(&mut s, "uml.stereotype.show-values", json!({ "diagram": d, "show": true }));
    assert_eq!(schema.fingerprint(), CURRENT, "no table changed");
    let back = save_and_reopen(&env, &schema, &path, s.doc(), "Applications").expect("a project with stereotype applications opens");
    assert_eq!(rows_of(back.doc(), &schema), rows_of(s.doc(), &schema));
    let st = back.doc();
    let o = st.by_uuid(Uuid::parse(&order).unwrap()).unwrap();
    let e = st.by_uuid(Uuid::parse(&entity).unwrap()).unwrap();
    let i = archforge_uml::applications::application(st, o, e).expect("Order is still an «Entity»");
    let shown: Vec<Option<String>> =
        archforge_uml::profiles::properties(st, e).into_iter().map(|p| archforge_uml::applications::shown(st, i, p)).collect();
    assert_eq!(shown, [Some("\"orders\"".into()), None, Some("lazy".into()), Some("\"a\", \"b\"".into())], "set values back, the default unset");
    assert_eq!(archforge_uml::applications::keywords(st, st.by_uuid(Uuid::parse(&id).unwrap()).unwrap()).as_deref(), Some("«Id»"));
    assert!(archforge_uml::applications::values_shown(st, st.by_uuid(Uuid::parse(&d).unwrap()).unwrap()), "the diagram's choice comes back");
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

/// A template binding (AR-131), its substitutions and its line with its label, are saved in the
/// project's existing tables and come back as written after the project is closed and opened
/// again: no table or column is added for them, and the binding reads the same.
#[test]
fn template_bindings_are_saved_and_opened_again() {
    let env = Env::new("bindings");
    let schema = schema();
    let project = Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Shop", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let mut s = archforge_notations::new_project_session("Shop", ids(0)).unwrap();
    let sales = run(&mut s, "uml.package.add", json!({ "name": "sales" }));
    let order = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Order" }));
    let repo = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "Repository" }));
    let bound = run(&mut s, "uml.class.add", json!({ "package": sales, "name": "OrderRepository" }));
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": repo, "name": "T" }));
    run(&mut s, "uml.template-parameter.add", json!({ "classifier": repo, "name": "Key", "kind": "data-type", "default": "Integer" }));
    let b = run(&mut s, "uml.template-binding.add", json!({ "bound": bound, "template": repo, "arguments": { "T": order, "Key": "String" } }));
    let d = run(&mut s, "diagram.add", json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": sales, "name": "Sales" }));
    let from = run(&mut s, "diagram.place", json!({ "diagram": d, "element": bound, "x": 0, "y": 300 }));
    let to = run(&mut s, "diagram.place", json!({ "diagram": d, "element": repo, "x": 400, "y": 0 }));
    let e = run(&mut s, "diagram.connect", json!({ "diagram": d, "element": b, "source": from, "target": to }));
    run(&mut s, "diagram.label", json!({ "edge": e, "kind": "name", "at": "middle" }));
    let before = s.execute("uml.template-binding.list", &json!({ "classifier": bound })).unwrap().value;
    let written = rows_of(s.doc(), &schema);
    project.write_model(&schema, &written).unwrap();
    project.commit("Bindings").unwrap();
    let path = project.path().to_path_buf();
    project.close().unwrap();
    let again = Project::open(&env.dolt, &env.opts(), &path).unwrap();
    again.prepare_model(&schema).unwrap();
    let read = again.read_model(&schema).unwrap();
    assert_eq!(read, written, "the rows come back as written");
    let mut session = archforge_notations::session_from_rows(&read, ids(1 << 41)).unwrap();
    assert_eq!(rows_of(session.doc(), &schema), written, "read into a model, the same rows again");
    assert_eq!(session.execute("uml.template-binding.list", &json!({ "classifier": bound })).unwrap().value, before);
    let st = session.doc();
    let binding = st.by_uuid(Uuid::parse(&b).unwrap()).unwrap();
    assert_eq!(archforge_uml::bindings::label(st, binding), "«bind» <T -> Order, Key -> String>");
    assert_eq!(archforge_diagram::views_of(st, binding).1.len(), 1, "its line");
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

/// A failed save says whether a version exists, by the head and by whether the server answered;
/// a retry then finds nothing left and makes no second version.
#[test]
fn a_save_says_whether_it_made_a_version_and_a_retry_makes_no_second_one() {
    use archforge_project::{AfterFailure, Author, Saved};
    let env = Env::new("made");
    let schema = schema();
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    project.write_model(&schema, &rows_of(model().doc(), &schema)).unwrap();
    let lost = || mysql::Error::IoError(std::io::ErrorKind::TimedOut.into());

    // The server refuses an empty message: it answered, the head did not move, so no version.
    let refused = project.save_version("", &Author::person("")).unwrap_err();
    assert_eq!(refused.version, AfterFailure::NoVersion, "{}", refused.error);
    // A lost reply with a head that did not move: the commit may still be running.
    assert_eq!(project.save_with(|_| Err(lost())).unwrap_err().version, AfterFailure::CannotTell);
    assert!(project.has_unsaved().unwrap(), "the work is still there");

    // The commit went through and only its reply was lost: the moved head proves the version.
    let made = project.save_with(|c| c.query_drop("CALL DOLT_COMMIT('-m', 'Kept', '--author', 'T <t@t>')").and(Err(lost()))).unwrap_err();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let newest: String = c.query_first("SELECT commit_hash FROM dolt_log LIMIT 1").unwrap().unwrap();
    assert_eq!(made.version, AfterFailure::Version(newest));
    assert_eq!(project.save_version("Again", &Author::person("")).unwrap(), Saved::NothingNew, "the retry makes no second version");
    project.close().unwrap();
}

/// The versions of a line: every kind of commit in one history, read a page at a time.
#[test]
fn the_versions_are_saves_only_newest_first_and_a_page_starts_after_a_version() {
    use archforge_project::CHAT_AUTHOR;
    let env = Env::new("versions");
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let mut commit = |message: &str, author: &str| -> String {
        c.exec_drop("CALL DOLT_COMMIT('--allow-empty', '-m', ?, '--author', ?)", (message, author)).unwrap();
        c.query_first("SELECT commit_hash FROM dolt_log LIMIT 1").unwrap().unwrap()
    };
    let now = || timestamp(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    // The root, the creation and a chat commit: nothing to list yet.
    let chat = commit("Chat 1: turn 1", CHAT_AUTHOR);
    assert_eq!(project.versions(None, 50).unwrap().versions, []);

    let before = now();
    let automatic = commit("Saved 2026-11-08T10:00:00Z", "Archforge <archforge@localhost>");
    let after = now();
    // A person's name decides nothing: this one is named like the chat, the next chat like a person.
    let typed = commit("Saved the day", "Archforge chat <archforge@localhost>");
    commit("Chat 1: turn 2", "Ada <chat@localhost>");
    let agent = commit("Work of the agent", "Chat 3 <agent@localhost>");
    let newest = commit("Renamed Customer", "Ada Lovelace <archforge@localhost>");

    let first = project.versions(None, 2).unwrap();
    let hashes = |page: &archforge_project::Versions| page.versions.iter().map(|v| v.hash.clone()).collect::<Vec<_>>();
    assert_eq!(hashes(&first), [newest.clone(), agent.clone()]);
    assert!(first.more);
    assert_eq!(first.versions[0].previous.as_ref(), Some(&agent));
    assert_eq!(first.versions[1].previous.as_ref(), Some(&typed), "past the chat commit, not the Dolt parent; and off the page");
    assert_eq!(first.versions[0].author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(first.versions[0].description.as_deref(), Some("Renamed Customer"));
    assert!(!first.versions[0].by_agent);
    assert_eq!(first.versions[1].author.as_deref(), Some("Chat 3"));
    assert!(first.versions[1].by_agent);

    // A save made meanwhile shifts nothing: the next page still starts after the same version.
    commit("One more", "Ada <archforge@localhost>");
    let second = project.versions(Some(&agent), 2).unwrap();
    assert_eq!(hashes(&second), [typed.clone(), automatic.clone()]);
    assert!(!second.more, "the root and the creation are left out");
    assert_eq!(second.versions[0].previous.as_ref(), Some(&automatic));
    assert_eq!(second.versions[0].description.as_deref(), Some("Saved the day"));
    assert_eq!(second.versions[0].author.as_deref(), Some("Archforge chat"));
    let oldest = &second.versions[1];
    assert_eq!((oldest.description.as_ref(), oldest.author.as_ref(), oldest.previous.as_ref()), (None, None, None));
    assert!(before <= oldest.date && oldest.date <= after, "in UTC: {before} {} {after}", oldest.date);

    for not_one in [chat.as_str(), "nosuchversion"] {
        assert!(matches!(project.versions(Some(not_one), 2), Err(Error::NotAVersion { commit }) if commit == not_one));
    }
    assert!(matches!(project.versions(None, 201), Err(Error::TooLarge { size: 201, .. })));
    project.close().unwrap();
}

/// On a branch, the versions from where it parts from `main` on are marked shared, whatever page
/// they come on and though `main` has moved since; on `main` none is.
#[test]
fn a_branch_marks_the_versions_it_shares_with_main() {
    use archforge_project::SafeBranch;
    let env = Env::new("shared");
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    let commit = |c: &mut mysql::Conn, message: &str, author: &str| -> String {
        c.exec_drop("CALL DOLT_COMMIT('--allow-empty', '-m', ?, '--author', ?)", (message, author)).unwrap();
        c.query_first("SELECT commit_hash FROM dolt_log LIMIT 1").unwrap().unwrap()
    };
    let saved = "Ada <archforge@localhost>";
    let mut main = on_branch(&project, "main");
    let first = commit(&mut main, "First", saved);
    let second = commit(&mut main, "Second", saved);
    // The branch parts at a chat commit, which is not a version itself.
    commit(&mut main, "Chat 1: turn 1", archforge_project::CHAT_AUTHOR);
    main.query_drop("CALL DOLT_BRANCH('other')").unwrap();
    main.query_drop("CALL DOLT_BRANCH('same')").unwrap();
    commit(&mut main, "Later on main", saved);
    let mut other = on_branch(&project, "other");
    let own = commit(&mut other, "Own", saved);
    let newest = commit(&mut other, "Newest", saved);

    let marks = |page: archforge_project::Versions| page.versions.into_iter().map(|v| (v.hash, v.shared)).collect::<Vec<_>>();
    let reader = project.version_reader();
    let other_branch = SafeBranch::found("other").unwrap();
    let all = [(newest.clone(), false), (own.clone(), false), (second.clone(), true), (first.clone(), true)];
    assert_eq!(marks(reader.versions_on(&other_branch, None, 50).unwrap()), all);
    assert_eq!(marks(reader.versions_on(&other_branch, Some(&own), 1).unwrap()), [all[2].clone()], "the base went by before the page");
    assert_eq!(marks(reader.versions_on(&other_branch, None, 1).unwrap()), [all[0].clone()], "the base is beyond the page");
    let same = marks(reader.versions_on(&SafeBranch::found("same").unwrap(), None, 50).unwrap());
    assert_eq!(same, [(second.clone(), true), (first.clone(), true)], "nothing of its own yet");
    for on_main in ["main", "MAIN"] {
        let page = reader.versions_on(&SafeBranch::found(on_main).unwrap(), None, 50).unwrap();
        assert_eq!(page.versions.len(), 3);
        assert!(page.versions.iter().all(|v| !v.shared), "{on_main}");
    }
    project.close().unwrap();
}

/// Dolt takes every stored name as a branch, keeps names apart that differ only in case as typed,
/// and a `USE` of each reaches that branch.
#[test]
fn every_typed_name_is_a_branch_dolt_takes_and_a_use_reaches() {
    use archforge_project::SafeBranch;
    let env = Env::new("branches");
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let emoji = "😀".repeat(60);
    let hash = "0123456789abcdefghijklmnopqrstuv";
    for text in [
        "plain-1_x",
        "Foo",
        "foo",
        "FOO",
        "Release 1",
        "李雷 Ü",
        "a'b`c;d/e",
        "a.b",
        "a.lock",
        "..",
        "-D",
        "--amend",
        "head",
        "HEAD",
        hash,
        emoji.as_str(),
    ] {
        let name = SafeBranch::typed(text).unwrap();
        c.exec_drop("CALL DOLT_BRANCH(?)", (name.stored(),)).unwrap_or_else(|e| panic!("{text:?} as {}: {e}", name.stored()));
        c.query_drop(format!("USE `project/{}`", name.stored())).unwrap();
        let active: String = c.query_first("SELECT active_branch()").unwrap().unwrap();
        assert_eq!(active, name.stored(), "{text:?}");
        c.query_drop("USE project").unwrap();
    }
    project.close().unwrap();
}

/// A project with the branches named, each on its own connection for the test to write through.
fn on_branch(project: &Project, branch: &str) -> mysql::Conn {
    let mut c = project.server().connect().unwrap();
    c.query_drop(format!("USE `project/{branch}`")).unwrap();
    c
}

/// What the list says of each branch, in order.
fn listed(project: &Project, max: usize) -> Vec<(String, Option<bool>, bool)> {
    project.branches(max).unwrap().listed.into_iter().map(|b| (b.name, b.unsaved, b.current)).collect()
}

/// Reads, writes, saves and a reader made before the move all follow the current branch.
#[test]
fn every_connection_follows_the_current_branch() {
    use archforge_project::SafeBranch;
    let env = Env::new("follow");
    let schema = schema();
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(&schema).unwrap();
    let empty = project.read_model(&schema).unwrap();
    project.write_model(&schema, &rows_of(model().doc(), &schema)).unwrap();
    project.commit("On main").unwrap();
    let on_main = project.read_model(&schema).unwrap();
    on_branch(&project, "main").query_drop("CALL DOLT_BRANCH('other')").unwrap();
    let reader = project.version_reader();

    project.set_current_branch(SafeBranch::found("other").unwrap());
    project.write_model(&schema, &empty).unwrap();
    assert!(project.has_unsaved().unwrap());
    assert!(project.commit("On other").unwrap());
    assert_eq!(project.newest_commit_messages(1).unwrap(), ["On other"]);
    assert_eq!(reader.versions(None, 1).unwrap().versions[0].description.as_deref(), Some("On other"), "a reader made before the move");
    assert_eq!(project.read_model(&schema).unwrap(), empty, "the shape checks see this branch");

    project.set_current_branch(SafeBranch::found("MAIN").unwrap());
    assert_eq!(project.newest_commit_messages(1).unwrap(), ["On main"]);
    assert_eq!(project.read_model(&schema).unwrap(), on_main);
    assert!(!project.has_unsaved().unwrap(), "the other branch's save left main as it was");
    let current: Vec<_> = listed(&project, 10).into_iter().filter(|b| b.2).map(|b| b.0).collect();
    assert_eq!(current, ["main"], "the case it was asked by does not make a second current branch");
    project.close().unwrap();
}

/// Each branch answers for itself, chat changes are not unsaved work, and a bad name is kept.
#[test]
fn the_branch_list_reads_each_branch_and_counts_only_the_model() {
    use archforge_project::{SafeBranch, UnsafeBranch};
    let env = Env::new("branches2");
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    let mut main = on_branch(&project, "main");
    for branch in ["chatty", "clean", "edited", "a;b"] {
        main.exec_drop("CALL DOLT_BRANCH(?)", (branch,)).unwrap();
    }
    on_branch(&project, "chatty").query_drop("CREATE TABLE chat_turn (id INT PRIMARY KEY)").unwrap();
    on_branch(&project, "edited").query_drop("CREATE TABLE er_extra (id INT PRIMARY KEY)").unwrap();
    let row = |name: &str, unsaved, current| (name.to_string(), unsaved, current);
    let expected = |main_unsaved, hostile| {
        vec![
            row("a;b", hostile, false),
            row("chatty", Some(false), false),
            row("clean", Some(false), false),
            row("edited", Some(true), false),
            row("main", main_unsaved, true),
        ]
    };
    assert_eq!(listed(&project, 10), expected(Some(false), Some(false)));

    // The current branch's own status differs from each branch's, so only theirs can decide.
    main.query_drop("CREATE TABLE er_on_main (id INT PRIMARY KEY)").unwrap();
    on_branch(&project, "a;b").query_drop("CREATE TABLE er_x (id INT PRIMARY KEY)").unwrap();
    assert_eq!(listed(&project, 10), expected(Some(true), None), "chatty stays clean though main is not, and a;b cannot be read");

    project.set_current_branch(SafeBranch::found("chatty").unwrap());
    let from_chatty = listed(&project, 10);
    assert_eq!((from_chatty[3].1, from_chatty[1].2), (Some(true), true), "edited is unsaved seen from a branch that is not");

    let list = project.branches(10).unwrap();
    assert_eq!(list.listed[0].usable, Err(UnsafeBranch::Character(';')));
    assert_eq!((list.listed.len(), list.cut), (5, false));
    let at_limit = project.branches(5).unwrap();
    assert_eq!((at_limit.listed.len(), at_limit.cut), (5, false));
    let over = project.branches(4).unwrap();
    assert_eq!((over.listed.len(), over.cut), (4, true));
    project.close().unwrap();
}

/// A list over the most branches is refused before anything is read: the refusal comes even where
/// a connection could not be opened.
#[test]
fn a_list_of_more_branches_than_the_most_is_refused_before_it_is_read() {
    use archforge_project::{MAX_BRANCHES, SafeBranch};
    let env = Env::new("cap");
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    assert_eq!(project.branches(MAX_BRANCHES).unwrap().listed.len(), 1);
    let refused =
        |e: Error| matches!(e, Error::TooLarge { what: "a list", size, max, unit: "branches" } if size == MAX_BRANCHES + 1 && max == MAX_BRANCHES);
    assert!(refused(project.branches(MAX_BRANCHES + 1).unwrap_err()));
    project.set_current_branch(SafeBranch::found("nosuchbranch").unwrap());
    assert!(matches!(project.branches(MAX_BRANCHES), Err(Error::Sql { .. })), "this branch cannot be opened");
    assert!(refused(project.branches(MAX_BRANCHES + 1).unwrap_err()), "the refusal needed no connection");
    project.close().unwrap();
}

/// A commit that went through on one branch is reported with that branch's head, however the
/// current branch moved before the failure was looked at; a branch gone by then cannot be told.
#[test]
fn a_lost_reply_is_read_back_from_the_branch_the_save_started_on() {
    use archforge_project::{AfterFailure, SafeBranch};
    let env = Env::new("landed");
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    let mut main = on_branch(&project, "main");
    for branch in ["landed", "gone"] {
        main.exec_drop("CALL DOLT_BRANCH(?)", (branch,)).unwrap();
        on_branch(&project, branch).query_drop("CREATE TABLE er_extra (id INT PRIMARY KEY)").unwrap();
    }
    let lost = || mysql::Error::IoError(std::io::ErrorKind::TimedOut.into());
    let found = |name: &str| SafeBranch::found(name).unwrap();
    project.set_current_branch(found("landed"));
    let landed = project
        .save_with(|c| {
            let done = c.query_drop("CALL DOLT_COMMIT('-m', 'Kept', '--author', 'T <t@t>')");
            project.set_current_branch(found("main"));
            done.and(Err(lost()))
        })
        .unwrap_err();
    let newest: String = on_branch(&project, "landed").query_first("SELECT commit_hash FROM dolt_log LIMIT 1").unwrap().unwrap();
    assert_eq!(landed.version, AfterFailure::Version(newest));

    project.set_current_branch(found("gone"));
    let unknown = project
        .save_with(|_| {
            project.set_current_branch(found("main"));
            main.query_drop("CALL DOLT_BRANCH('-D', '-f', 'gone')").unwrap();
            Err(lost())
        })
        .unwrap_err();
    assert_eq!(unknown.version, AfterFailure::CannotTell);
    project.close().unwrap();
}

/// A project with its model saved and the branches named made from it.
fn with_branches(env: &Env, schema: &ModelSchema, branches: &[&str]) -> Project {
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    project.prepare_model(schema).unwrap();
    project.write_model(schema, &rows_of(model().doc(), schema)).unwrap();
    project.commit("Saved").unwrap();
    let mut main = on_branch(&project, "main");
    for branch in branches {
        main.exec_drop("CALL DOLT_BRANCH(?)", (branch,)).unwrap();
    }
    project
}

/// Each hazard refuses the branch that has it, by its own reason and only that branch.
#[test]
fn a_branch_is_checked_for_itself_and_the_current_one_decides_nothing() {
    use archforge_project::{FORMAT, SafeBranch};
    let env = Env::new("checks");
    let schema = schema();
    let names = ["newer", "trigger", "procedure", "event", "other", "clean", "bare"];
    let project = with_branches(&env, &schema, &names);
    let found = |name: &str| SafeBranch::found(name).unwrap();
    let newer = FORMAT + 1;
    let format = format!("UPDATE settings SET `value` = '{newer}' WHERE `key` = 'format'");
    let trigger = "CREATE TRIGGER t BEFORE UPDATE ON settings FOR EACH ROW SET NEW.`value` = NEW.`value`";
    let procedure = "CREATE PROCEDURE p() SELECT 1";
    let event = "CREATE EVENT e ON SCHEDULE EVERY 1 DAY DO SELECT 1";
    let other = "UPDATE settings SET `value` = 'not this model' WHERE `key` = 'model_schema'";
    for (name, statement) in [
        ("newer", format.as_str()),
        ("trigger", trigger),
        ("procedure", procedure),
        ("event", event),
        ("other", other),
        ("bare", "DROP TABLE settings"),
    ] {
        on_branch(&project, name).query_drop(statement).unwrap();
    }
    let on_main = project.read_model(&schema).unwrap();
    let unmoved = |p: &Project| {
        assert_eq!(p.current_branch(), found("main"));
        assert_eq!(p.read_model(&schema).unwrap(), on_main, "the current branch reads as before");
    };

    assert_eq!(project.check_branch(&found("clean"), &schema).unwrap(), on_main);
    unmoved(&project);
    assert!(
        matches!(project.check_branch(&found("newer"), &schema), Err(Error::NewerFormat { found, supported }) if found == newer && supported == FORMAT)
    );
    unmoved(&project);
    for (name, reason) in [("trigger", "triggers"), ("procedure", "stored procedures"), ("event", "events")] {
        assert!(matches!(project.check_branch(&found(name), &schema), Err(Error::Scripted { what, .. }) if what == reason), "{name}");
        unmoved(&project);
    }
    assert!(matches!(project.check_branch(&found("other"), &schema), Err(Error::OtherModel { .. })));
    unmoved(&project);
    assert!(matches!(project.check_branch(&found("bare"), &schema), Err(Error::Damaged { why, .. }) if why == "it has no settings table"));
    unmoved(&project);
    let missing = project.check_branch(&found("nosuchbranch"), &schema).unwrap_err().to_string();
    assert!(missing.contains("its project/nosuchbranch database cannot be used"), "{missing}");
    unmoved(&project);

    // The other way round: from a current branch with each hazard, a clean target passes.
    for name in ["newer", "trigger", "procedure", "event"] {
        project.set_current_branch(found(name));
        assert_eq!(project.check_branch(&found("clean"), &schema).unwrap(), on_main, "seen from {name}");
        assert_eq!(project.current_branch(), found(name));
    }
    project.close().unwrap();
}

/// A branch that has not had its model prepared gets it there and nowhere else, its own rows are
/// what comes back, and conversation tables waiting on it are no part of the check.
#[test]
fn a_branch_is_prepared_and_read_on_its_own() {
    use archforge_project::SafeBranch;
    let env = Env::new("own");
    let schema = schema();
    let project =
        Project::create(&env.dolt, &env.opts(), &env.base.join("Projects"), "Commerce platform", "test", &timestamp(1_761_670_800)).unwrap();
    let found = |name: &str| SafeBranch::found(name).unwrap();
    // Made before the model was: this branch has no fingerprint yet.
    on_branch(&project, "main").query_drop("CALL DOLT_BRANCH('fresh')").unwrap();
    project.prepare_model(&schema).unwrap();
    project.write_model(&schema, &rows_of(model().doc(), &schema)).unwrap();
    on_branch(&project, "fresh").query_drop("CREATE TABLE chat_turn (id INT PRIMARY KEY)").unwrap();
    let on_main = project.read_model(&schema).unwrap();
    let fresh = project.check_branch(&found("fresh"), &schema).unwrap();
    assert_ne!(fresh, on_main, "the branch's own rows, not the current branch's");
    assert_eq!((project.current_branch(), project.read_model(&schema).unwrap()), (found("main"), on_main));
    let mut c = on_branch(&project, "fresh");
    let kept: Option<String> = c.query_first("SELECT `value` FROM settings WHERE `key` = 'model_schema'").unwrap();
    assert!(kept.is_some(), "the fingerprint went to the branch checked");
    project.close().unwrap();
}
