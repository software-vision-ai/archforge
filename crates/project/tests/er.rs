//! The ER model in a real project: store then load gives the same model, damaged tables are
//! refused, unsaved changes follow the owner's decision, and a format-1 project is upgraded, also
//! after an upgrade that stopped half way. These tests need dolt 2.6 or later, as the project tests
//! do. Measured on macOS.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use archforge_dolt::{Dolt, Home, Server, ServerOptions};
use archforge_er::model::{MAX_DIAGRAMS, make_id};
use archforge_er::{Attribute, Cardinality, Diagram, Entity, ErModel, Kind, Place, Relationship};
use archforge_project::{Error, FORMAT, Hooks, Project, UpgradeStep, timestamp};
use mysql::prelude::Queryable;

struct Env {
    base: PathBuf,
    run_root: PathBuf,
    dolt: Dolt,
}

impl Env {
    fn new(name: &str) -> Env {
        let tag = std::process::id();
        let base = std::env::temp_dir().join(format!("af-er-{name}-{tag}"));
        let run_root = std::env::temp_dir().join(format!("afe-{name}-{tag}"));
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

    fn create(&self, name: &str) -> Project {
        Project::create(&self.dolt, &self.opts(), &self.base.join("Projects"), name, "Archforge test", &timestamp(1_761_670_800)).unwrap()
    }

    fn open(&self, path: &Path) -> Result<Project, Error> {
        Project::open(&self.dolt, &self.opts(), path)
    }

    /// Runs `sql` on a closed project through a bare server, so nothing is upgraded on the way, and
    /// gives the rows of the last statement as text.
    fn raw(&self, path: &Path, sql: &[&str]) -> Vec<Vec<String>> {
        let server = Server::start(&self.dolt, path, &self.opts()).unwrap();
        let mut c = server.connect().unwrap();
        c.query_drop("USE project").unwrap();
        let mut last = Vec::new();
        for q in sql {
            let rows: Vec<mysql::Row> = c.query(*q).unwrap_or_else(|e| panic!("{q}: {e}"));
            last = rows
                .iter()
                .map(|r| (0..r.len()).map(|i| r.get_opt::<Option<String>, usize>(i).unwrap().unwrap().unwrap_or_default()).collect())
                .collect();
        }
        drop(c);
        server.stop().unwrap();
        last
    }

    /// A closed project of format 1, as Archforge made it before the model: no `er_` tables.
    fn format_1(&self, name: &str) -> PathBuf {
        let p = self.create(name);
        let path = p.path().to_path_buf();
        p.close().unwrap();
        self.raw(
            &path,
            &[
                "DROP TABLE er_places",
                "DROP TABLE er_diagrams",
                "DROP TABLE er_relationships",
                "DROP TABLE er_attributes",
                "DROP TABLE er_entities",
                "DELETE FROM settings WHERE `key` = 'er_next'",
                "UPDATE settings SET `value` = '1' WHERE `key` = 'format'",
                "CALL DOLT_COMMIT('-Am', 'format 1')",
            ],
        );
        path
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.run_root).stderr(Stdio::null()).status();
        let _ = std::fs::remove_dir_all(&self.base);
        let _ = std::fs::remove_dir_all(&self.run_root);
    }
}

fn sql(project: &Project, q: &str) -> Vec<Vec<String>> {
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let rows: Vec<mysql::Row> = c.query(q).unwrap_or_else(|e| panic!("{q}: {e}"));
    rows.iter().map(|r| (0..r.len()).map(|i| r.get_opt::<Option<String>, usize>(i).unwrap().unwrap().unwrap_or_default()).collect()).collect()
}

fn commit(project: &Project) {
    sql(project, "CALL DOLT_COMMIT('-Am', 'save')");
}

/// A model of `n` entities with every kind of element; `odd` puts quotes, `$`, non-ASCII text and
/// awkward floats in it. Gives the model and its counter.
fn model(n: u64, odd: bool) -> (ErModel, u64) {
    let mut m = ErModel::default();
    let mut next = 1;
    let mut fresh = |kind| {
        let id = make_id(kind, next);
        next += 1;
        id
    };
    let diagram = fresh(Kind::Diagram);
    let mut places = BTreeMap::new();
    let mut ids = Vec::new();
    for i in 0..n {
        let id = fresh(Kind::Entity);
        let name = if odd && i == 0 { "Kunde \"VIP\" $1 'ü' 顧客".to_string() } else { format!("Entity{i}") };
        let attributes = (0..i % 4)
            .map(|j| Attribute {
                id: fresh(Kind::Attribute),
                name: format!("field_{j}"),
                kind: if odd { "DECIMAL(10,2) -- 'x'".into() } else { "INT".into() },
                primary_key: j == 0,
                nullable: j == 2,
                unique: j == 1,
            })
            .collect();
        m.entities.insert(id.clone(), Entity { id: id.clone(), name, attributes });
        let x = if odd && i == 0 {
            0.1 + 0.2
        } else if odd && i == 1 {
            5e-324
        } else {
            i as f64 * 250.5
        };
        places.insert(id.clone(), Place { x, y: -1e-7 * i as f64, width: 200.0 + i as f64 / 3.0, height: 120.0 });
        ids.push(id);
    }
    for (i, pair) in ids.windows(2).enumerate() {
        let id = fresh(Kind::Relationship);
        let name = if i % 2 == 0 { Some(format!("rel {i} \\ ✓")) } else { None };
        m.relationships.insert(
            id.clone(),
            Relationship { id, name, from: pair[0].clone(), to: pair[1].clone(), from_end: Cardinality::ExactlyOne, to_end: Cardinality::ZeroOrMore },
        );
    }
    m.diagrams.insert(diagram.clone(), Diagram { id: diagram, name: "Overview".into(), places });
    let empty = fresh(Kind::Diagram);
    m.diagrams.insert(empty.clone(), Diagram { id: empty, name: "Empty".into(), places: BTreeMap::new() });
    assert_eq!(m.check(next + 5), Ok(()));
    (m, next + 5)
}

#[test]
fn a_new_project_is_format_2_with_an_empty_model_and_nothing_unsaved() {
    let env = Env::new("new");
    let p = env.create("Fresh");
    assert_eq!(p.settings().format, FORMAT);
    assert_eq!(p.load_er().unwrap(), (ErModel::default(), 1));
    assert!(!p.model_uncommitted().unwrap());
    p.close().unwrap();
}

#[test]
fn store_then_load_gives_the_same_model() {
    let env = Env::new("roundtrip");
    let p = env.create("Roundtrip");
    for (n, odd) in [(2, true), (7, true), (1, false), (40, false), (1200, false)] {
        let (m, next) = model(n, odd);
        p.store_er(&m, next).unwrap();
        assert_eq!(p.load_er().unwrap(), (m.clone(), next), "{n} entities");
        // Storing the same model again changes nothing.
        commit(&p);
        p.store_er(&m, next).unwrap();
        assert!(!p.model_uncommitted().unwrap(), "{n}: rewriting identical rows left changes");
    }
    p.store_er(&ErModel::default(), 9999).unwrap();
    assert_eq!(p.load_er().unwrap(), (ErModel::default(), 9999));
    assert_eq!(sql(&p, "SELECT COUNT(*) FROM er_places"), [["0"]]);
    p.close().unwrap();
}

#[test]
fn an_invalid_or_slow_store_leaves_the_tables_as_they_were() {
    let env = Env::new("refused");
    let p = env.create("Refused");
    let (m, next) = model(3, false);
    p.store_er(&m, next).unwrap();
    let (mut bad, _) = model(5, false);
    bad.entities.values_mut().for_each(|e| e.name = "Same".into());
    assert!(matches!(p.store_er(&bad, next), Err(Error::InvalidModel { .. })));
    assert!(matches!(p.store_er(&m, 2), Err(Error::InvalidModel { .. })), "a counter below the ids is refused");
    let (big, big_next) = model(30, false);
    assert!(matches!(p.store_er_within(&big, big_next, Duration::ZERO), Err(Error::StoreTooSlow { .. })));
    assert_eq!(p.load_er().unwrap(), (m, next));
    p.close().unwrap();
}

#[test]
fn unsaved_changes_are_the_model_or_a_setting_other_than_the_counter() {
    let env = Env::new("unsaved");
    let p = env.create("Unsaved");
    let (m, next) = model(3, false);
    p.store_er(&m, next).unwrap();
    assert!(p.model_uncommitted().unwrap());
    commit(&p);
    assert!(!p.model_uncommitted().unwrap());
    // An add then an undo moves only the counter.
    p.store_er(&m, next + 1).unwrap();
    assert!(!p.model_uncommitted().unwrap());
    sql(&p, "UPDATE settings SET `value` = 'Renamed' WHERE `key` = 'name'");
    assert!(p.model_uncommitted().unwrap());
    p.close().unwrap();
}

#[test]
fn damaged_tables_are_refused_naming_the_rule() {
    let env = Env::new("damaged");
    let p = env.create("Damaged");
    let (m, next) = model(3, false);
    p.store_er(&m, next).unwrap();
    commit(&p);
    let many_diagrams =
        format!("INSERT INTO er_diagrams VALUES {}", (0..=MAX_DIAGRAMS).map(|i| format!("('d{}', 'D{i}')", 100 + i)).collect::<Vec<_>>().join(", "));
    let cases: Vec<(&str, Vec<String>, &str)> = vec![
        ("a missing entity", vec!["UPDATE er_relationships SET to_id = 'e999'".into()], "missing entity"),
        ("an orphan attribute", vec!["UPDATE er_attributes SET entity_id = 'e999' WHERE id = 'a4'".into()], "missing entity \"e999\""),
        ("a gap", vec!["UPDATE er_attributes SET position = 5 WHERE id = 'a4'".into()], "without a gap"),
        ("a boolean 2", vec!["UPDATE er_attributes SET is_unique = 2".into()], "not 0 or 1"),
        ("a cardinality", vec!["UPDATE er_relationships SET from_end = 'many'".into()], "not a cardinality"),
        (
            "a long name",
            vec![
                "ALTER TABLE er_entities MODIFY name VARCHAR(1000) NOT NULL".into(),
                "UPDATE er_entities SET name = REPEAT('x', 500) WHERE id = 'e2'".into(),
            ],
            "er_entities",
        ),
        ("a column of another type", vec!["ALTER TABLE er_places MODIFY x TEXT NOT NULL".into()], "er_places"),
        ("too many rows", vec![many_diagrams], "more than 500 diagrams"),
        ("no counter", vec!["DELETE FROM settings WHERE `key` = 'er_next'".into()], "no er_next"),
        ("a counter below the ids", vec!["UPDATE settings SET `value` = '2' WHERE `key` = 'er_next'".into()], "below 2"),
        ("a place in no diagram", vec!["UPDATE er_places SET diagram_id = 'd999'".into()], "missing diagram"),
    ];
    for (what, tamper, says) in &cases {
        for q in tamper {
            sql(&p, q);
        }
        match p.load_er() {
            Err(Error::Damaged { why, .. }) => assert!(why.contains(says), "{what}: {why}"),
            other => panic!("{what}: {other:?}"),
        }
        sql(&p, "CALL DOLT_RESET('--hard')");
        assert_eq!(p.load_er().unwrap(), (m.clone(), next), "{what}: the reset restores the model");
    }
    p.close().unwrap();
}

#[test]
fn a_format_1_project_opens_upgraded_and_commits_only_its_own_tables() {
    let env = Env::new("upgrade");
    let path = env.format_1("Old");
    env.raw(&path, &["CREATE TABLE notes (id INT PRIMARY KEY)", "INSERT INTO notes VALUES (1)"]);
    let p = env.open(&path).unwrap();
    assert_eq!(p.settings().format, 2);
    assert_eq!(p.load_er().unwrap(), (ErModel::default(), 1));
    assert_eq!(sql(&p, "SELECT `value` FROM settings AS OF 'HEAD' WHERE `key` = 'format'"), [["2"]]);
    assert_eq!(sql(&p, "SELECT message FROM dolt_log LIMIT 1"), [["Upgrade the project to format 2"]]);
    assert_eq!(sql(&p, "SELECT table_name, staged FROM dolt_status"), [["notes", "0"]], "the unrelated table stays uncommitted");
    assert!(!p.model_uncommitted().unwrap());
    let commits = sql(&p, "SELECT COUNT(*) FROM dolt_log");
    p.close().unwrap();
    let again = env.open(&path).unwrap();
    assert_eq!(sql(&again, "SELECT COUNT(*) FROM dolt_log"), commits, "a second open upgrades nothing");
    again.close().unwrap();
}

#[test]
fn a_project_with_triggers_procedures_or_events_is_refused() {
    let env = Env::new("scripted");
    let cases = [
        ("Trigger", "CREATE TRIGGER t AFTER INSERT ON er_entities FOR EACH ROW UPDATE settings SET `value` = 'x' WHERE `key` = 'name'", "triggers"),
        ("Procedure", "CREATE PROCEDURE p() SELECT 1", "stored procedures"),
        ("Event", "CREATE EVENT e ON SCHEDULE EVERY 1 DAY DO SELECT 1", "events"),
    ];
    for (name, script, says) in cases {
        let p = env.create(name);
        let path = p.path().to_path_buf();
        p.close().unwrap();
        env.raw(&path, &[script, "CALL DOLT_COMMIT('-Am', 'script')"]);
        match env.open(&path) {
            Err(Error::Scripted { what, .. }) => assert_eq!(what, says),
            other => panic!("{name}: {other:?}"),
        }
    }
}

#[test]
fn something_else_staged_refuses_the_upgrade_and_changes_nothing() {
    let env = Env::new("staged");
    let path = env.format_1("Staged");
    env.raw(&path, &["CREATE TABLE notes (id INT PRIMARY KEY)", "CALL DOLT_ADD('notes')"]);
    match env.open(&path) {
        Err(Error::StagedChanges { tables, .. }) => assert_eq!(tables, ["notes"]),
        other => panic!("{other:?}"),
    }
    let left = env.raw(&path, &["SELECT (SELECT `value` FROM settings WHERE `key` = 'format'), (SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'project' AND table_name LIKE 'er\\_%')"]);
    assert_eq!(left, [["1", "0"]]);
}

#[test]
fn an_upgrade_stopped_at_any_step_finishes_on_the_next_open() {
    let env = Env::new("resume");
    for (i, step) in [UpgradeStep::TablesMade, UpgradeStep::SettingsWritten, UpgradeStep::Added].into_iter().enumerate() {
        let path = env.format_1(&format!("Stopped{i}"));
        let hooks = Hooks { upgrade: Some(Box::new(move |at| if at == step { Err("stopped here".into()) } else { Ok(()) })), ..Hooks::default() };
        let r = Project::open_with(&env.dolt, &env.opts(), &path, &hooks);
        assert!(matches!(r, Err(Error::Damaged { .. })), "{step:?}: {r:?}");
        let working = env.raw(&path, &["SELECT `value` FROM settings WHERE `key` = 'format'"]);
        assert_eq!(working, [[if step == UpgradeStep::TablesMade { "1" } else { "2" }]], "{step:?}");
        let p = env.open(&path).unwrap();
        assert_eq!(p.settings().format, 2, "{step:?}");
        assert_eq!(sql(&p, "SELECT `value` FROM settings AS OF 'HEAD' WHERE `key` = 'format'"), [["2"]], "{step:?}");
        assert_eq!(sql(&p, "SELECT COUNT(*) FROM dolt_status"), [["0"]], "{step:?}: everything is committed");
        assert_eq!(p.load_er().unwrap(), (ErModel::default(), 1));
        p.close().unwrap();
    }
}
