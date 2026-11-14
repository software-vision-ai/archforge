//! Projects against a real dolt: create and open, a name in use, an unusable folder, damaged and
//! newer settings, foreign folders, failures and crashes during a creation. These tests need dolt
//! 2.6 or later; without it they fail and say so. Measured on macOS; Linux and Windows are still to
//! be done.
#![cfg(unix)]

use std::io::BufRead;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use archforge_dolt::{Dolt, Home, ServerOptions};
use archforge_project::{Error, FORMAT, Hooks, MARKER, Project, timestamp};
use mysql::prelude::Queryable;

const HELPER: &str = "ARCHFORGE_PROJECT_TEST_HELPER";

struct Env {
    base: PathBuf,
    run_root: PathBuf,
    dolt: Dolt,
    cleanup: bool,
}

impl Env {
    fn new(name: &str) -> Env {
        let tag = std::process::id();
        let base = std::env::temp_dir().join(format!("af-pt-{name}-{tag}"));
        let run_root = std::env::temp_dir().join(format!("afp-{name}-{tag}"));
        for d in [&base, &run_root] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d).unwrap();
        }
        Env::at(base, run_root, true)
    }

    fn at(base: PathBuf, run_root: PathBuf, cleanup: bool) -> Env {
        let home = Home::prepare(&base.join("home")).unwrap();
        let dolt = Dolt::find(&home)
            .unwrap_or_else(|e| panic!("these tests need dolt 2.6 or later within major version 2 on the PATH or in ARCHFORGE_DOLT: {e}"));
        Env { base, run_root, dolt, cleanup }
    }

    fn opts(&self) -> ServerOptions {
        ServerOptions {
            run_root: Some(self.run_root.clone()),
            ready_budget: Duration::from_secs(20),
            ..ServerOptions::new(self.base.join("records"))
        }
    }

    fn folder(&self) -> PathBuf {
        self.base.join("Projects")
    }

    fn create(&self, name: &str) -> Result<Project, Error> {
        Project::create(&self.dolt, &self.opts(), &self.folder(), name, "Archforge test", &timestamp(1_761_670_800))
    }

    fn open(&self, path: &Path) -> Result<Project, Error> {
        Project::open(&self.dolt, &self.opts(), path)
    }

    fn records(&self) -> usize {
        std::fs::read_dir(self.base.join("records"))
            .map(|e| e.flatten().filter(|x| x.file_name().to_string_lossy().ends_with(".json")).count())
            .unwrap_or(0)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.run_root).stderr(Stdio::null()).status();
            let _ = std::fs::remove_dir_all(&self.base);
            let _ = std::fs::remove_dir_all(&self.run_root);
        }
    }
}

fn alive(pid: u32) -> bool {
    Command::new("/bin/kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn wait_gone(pid: u32) -> bool {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

/// Changes a closed project through its own server, then closes it again.
fn tamper(env: &Env, path: &Path, sql: &[&str]) {
    let project = env.open(path).unwrap();
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    for q in sql {
        c.query_drop(*q).unwrap();
    }
    c.query_drop("CALL DOLT_COMMIT('-Am', 'tamper')").unwrap();
    drop(c);
    project.close().unwrap();
}

#[test]
fn a_created_project_opens_again_with_its_settings_and_its_commit() {
    let env = Env::new("roundtrip");
    let project = env.create("Commerce platform").unwrap();
    let path = project.path().to_path_buf();
    assert_eq!(path.file_name().unwrap(), "Commerce platform.archforge");
    assert_eq!(project.settings().format, FORMAT);
    assert_eq!(project.settings().name, "Commerce platform");
    assert_eq!(project.settings().created_at, "2025-10-28T17:00:00Z");
    let mut c = project.server().connect().unwrap();
    c.query_drop("USE project").unwrap();
    let newest: (String, String) = c.query_first("SELECT message, committer FROM dolt_log ORDER BY date DESC LIMIT 1").unwrap().unwrap();
    assert_eq!(newest.0, "Create the project");
    assert_eq!(newest.1, "Archforge");
    drop(c);
    project.close().unwrap();
    assert_eq!(mode(&path), 0o700, "the project folder is private");
    assert!(!path.join(MARKER).exists());
    let again = env.open(&path).unwrap();
    assert_eq!(again.settings().name, "Commerce platform");
    assert_eq!(again.settings().created_by, "Archforge test");
    again.close().unwrap();
    assert_eq!(env.records(), 0);
}

#[test]
fn a_name_in_use_is_refused_and_what_is_there_stays() {
    let env = Env::new("taken");
    std::fs::create_dir_all(env.folder()).unwrap();
    std::fs::write(env.folder().join("Taken.archforge"), "mine").unwrap();
    assert!(matches!(env.create("Taken"), Err(Error::NameTaken { .. })));
    assert_eq!(std::fs::read_to_string(env.folder().join("Taken.archforge")).unwrap(), "mine");
    let first = env.create("Twice").unwrap();
    first.close().unwrap();
    assert!(matches!(env.create("Twice"), Err(Error::NameTaken { .. })));
    assert!(env.open(&env.folder().join("Twice.archforge")).is_ok_and(|p| p.close().is_ok()));
}

#[test]
fn an_unwritable_folder_is_refused_and_nothing_is_left_in_it() {
    let env = Env::new("unwritable");
    assert_ne!(archforge_dolt::user_id().unwrap(), 0, "run these tests as a normal user: root writes anywhere");
    std::fs::create_dir_all(env.folder()).unwrap();
    std::fs::set_permissions(env.folder(), std::fs::Permissions::from_mode(0o500)).unwrap();
    let r = env.create("Nowhere");
    std::fs::set_permissions(env.folder(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(r, Err(Error::FolderUnusable { .. })), "{r:?}");
    assert_eq!(std::fs::read_dir(env.folder()).unwrap().count(), 0);
}

#[test]
fn damaged_settings_are_refused() {
    let env = Env::new("damaged");
    let cases: &[(&str, &[&str])] = &[
        ("dropped", &["DROP TABLE settings"]),
        ("format", &["UPDATE settings SET `value` = 'x' WHERE `key` = 'format'"]),
        ("noname", &["DELETE FROM settings WHERE `key` = 'name'"]),
        ("control", &["UPDATE settings SET `value` = CONCAT('a', CHAR(9), 'b') WHERE `key` = 'name'"]),
        ("long", &["ALTER TABLE settings MODIFY `value` TEXT NOT NULL", "INSERT INTO settings VALUES ('notes', REPEAT('x', 2000))"]),
        ("view", &["RENAME TABLE settings TO kept", "CREATE VIEW settings AS SELECT * FROM kept"]),
    ];
    for (name, sql) in cases {
        let p = env.create(name).unwrap();
        let path = p.path().to_path_buf();
        p.close().unwrap();
        tamper(&env, &path, sql);
        let r = env.open(&path);
        assert!(matches!(r, Err(Error::Damaged { .. })), "{name}: {r:?}");
    }
    assert_eq!(env.records(), 0, "every refused project's server was stopped");
}

#[test]
fn a_newer_format_is_refused() {
    let env = Env::new("newer");
    let p = env.create("Future").unwrap();
    let path = p.path().to_path_buf();
    p.close().unwrap();
    tamper(&env, &path, &["UPDATE settings SET `value` = '99' WHERE `key` = 'format'"]);
    assert!(matches!(env.open(&path), Err(Error::NewerFormat { found: 99, supported: FORMAT })));
}

#[test]
fn foreign_folders_are_refused_without_being_changed() {
    let env = Env::new("foreign");
    let plain = env.base.join("plain");
    std::fs::create_dir_all(plain.join("stuff")).unwrap();
    assert!(matches!(env.open(&plain), Err(Error::NotAProject { .. })));
    let empty = env.base.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(matches!(env.open(&empty), Err(Error::NotAProject { .. })), "an empty folder not named as a project is no project");
    let p = env.create("Shared").unwrap();
    let path = p.path().to_path_buf();
    p.close().unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o770)).unwrap();
    assert!(matches!(env.open(&path), Err(Error::NotPrivate { .. })));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir_all(path.join(".dolt")).unwrap();
    std::fs::write(path.join(".dolt/config.json"), r#"{"sqlserver.global.dolt_replicate_to_remote":"origin"}"#).unwrap();
    assert!(matches!(env.open(&path), Err(Error::Foreign { entry, .. }) if entry.contains("config.json")));
    assert!(path.join(".dolt/config.json").exists(), "a refused folder is not changed");
}

#[test]
fn a_failing_step_undoes_the_creation_and_never_removes_a_folder_put_in_its_place() {
    let env = Env::new("undo");
    let pid = Arc::new(std::sync::Mutex::new(0u32));
    let seen = pid.clone();
    let hooks = Hooks {
        after_start: Some(Box::new(move |p, _| {
            *seen.lock().unwrap() = p;
            Err("a step failed".into())
        })),
        ..Hooks::default()
    };
    let r = Project::create_with(&env.dolt, &env.opts(), &env.folder(), "Failing", "t", "now", &hooks);
    assert!(matches!(r, Err(Error::Damaged { .. })), "{r:?}");
    assert!(!env.folder().join("Failing.archforge").exists(), "the new folder is gone");
    assert!(!alive(*pid.lock().unwrap()), "its server is gone");
    assert_eq!(env.records(), 0);
    // A folder renamed away and replaced while the creation ran is left alone.
    let hooks = Hooks {
        after_start: Some(Box::new(|_, target: &Path| {
            std::fs::rename(target, target.with_extension("moved")).unwrap();
            std::fs::create_dir(target).unwrap();
            std::fs::write(target.join("keep.txt"), "someone else's").unwrap();
            Err("a step failed".into())
        })),
        ..Hooks::default()
    };
    let r = Project::create_with(&env.dolt, &env.opts(), &env.folder(), "Swapped", "t", "now", &hooks);
    assert!(matches!(r, Err(Error::Kept { .. })), "{r:?}");
    assert_eq!(std::fs::read_to_string(env.folder().join("Swapped.archforge/keep.txt")).unwrap(), "someone else's");
    // A cancelled creation leaves nothing.
    let hooks = Hooks { after_start: None, cancel: Some(Arc::new(AtomicBool::new(true))) };
    let r = Project::create_with(&env.dolt, &env.opts(), &env.folder(), "Cancelled", "t", "now", &hooks);
    assert!(matches!(r, Err(Error::Cancelled)), "{r:?}");
    assert!(!env.folder().join("Cancelled.archforge").exists());
}

/// Runs this test binary as a helper that creates `name` in `env`, waiting either inside the
/// creation (`mid`) or after it; gives it with the server's process id once it is there.
fn helper(env: &Env, name: &str, mid: bool) -> (std::process::Child, u32) {
    let exe = std::env::current_exe().unwrap();
    let spec = format!("{}\n{}\n{name}\n{}", env.base.display(), env.run_root.display(), if mid { "mid" } else { "after" });
    let mut child = Command::new(exe)
        .args(["--exact", "creation_helper", "--ignored", "--nocapture", "--test-threads=1"])
        .env(HELPER, spec)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some((_, pid)) = line.split_once("READY ") {
                let _ = tx.send(pid.trim().parse::<u32>().unwrap_or(0));
            }
        }
    });
    let pid = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|_| {
        let _ = child.kill();
        panic!("the creation helper did not get its server running")
    });
    (child, pid)
}

#[test]
#[ignore = "run by the crash tests as a separate process"]
fn creation_helper() {
    let Some(spec) = std::env::var_os(HELPER) else { return };
    let spec = spec.to_string_lossy().into_owned();
    let parts: Vec<&str> = spec.split('\n').collect();
    let env = Env::at(PathBuf::from(parts[0]), PathBuf::from(parts[1]), false);
    if parts[3] == "mid" {
        let hooks = Hooks {
            after_start: Some(Box::new(|pid, _| {
                println!("READY {pid}");
                std::thread::sleep(Duration::from_secs(120));
                Ok(())
            })),
            ..Hooks::default()
        };
        let _ = Project::create_with(&env.dolt, &env.opts(), &env.folder(), parts[2], "t", "now", &hooks);
    } else {
        let p = env.create(parts[2]).unwrap();
        println!("READY {}", p.server().pid());
        std::thread::sleep(Duration::from_secs(120));
    }
}

/// Kills the helper as a crash that takes the server's watchdog along, which leaves the server
/// running for recovery to find.
fn orphan(owner: &mut std::process::Child, server: u32) {
    let watchdog = format!("archforge-server-watchdog {server}$");
    assert!(Command::new("/usr/bin/pkill").args(["-KILL", "-f", &watchdog]).status().unwrap().success(), "the server had a watchdog");
    owner.kill().unwrap();
    owner.wait().unwrap();
    assert!(alive(server), "the server outlives its owner and its watchdog");
}

/// A crash inside a creation, its server left running, leaves a project that reads as unfinished.
#[test]
fn a_creation_killed_half_way_reads_as_unfinished_and_its_server_is_stopped() {
    let env = Env::new("midcrash");
    let (mut owner, orphan) = helper(&env, "Halfway", true);
    self::orphan(&mut owner, orphan);
    assert!(matches!(env.create("Halfway"), Err(Error::Unfinished { .. })));
    assert!(wait_gone(orphan), "recovery stopped the orphaned server");
    assert!(matches!(env.open(&env.folder().join("Halfway.archforge")), Err(Error::Unfinished { .. })));
    // An empty project folder, as a crash before the marker leaves it, is unfinished too.
    std::fs::create_dir_all(env.folder().join("Empty.archforge")).unwrap();
    assert!(matches!(env.create("Empty"), Err(Error::Unfinished { .. })));
}

/// A crash with a project open, its server left running, does not keep the project from opening.
#[test]
fn a_project_open_when_archforge_crashed_opens_again() {
    let env = Env::new("aftercrash");
    let (mut owner, orphan) = helper(&env, "Survivor", false);
    self::orphan(&mut owner, orphan);
    let p = env.open(&env.folder().join("Survivor.archforge")).unwrap();
    assert!(wait_gone(orphan), "the orphaned server was stopped");
    assert_eq!(p.settings().name, "Survivor");
    p.close().unwrap();
}

#[test]
fn every_name_rule_refuses_a_creation_before_anything_is_written() {
    let env = Env::new("names");
    let long = "x".repeat(archforge_names::MAX_BYTES + 1);
    for bad in ["", "a/b", "a:b", ".hidden", "ends.", " lead", "con", "a\tb", long.as_str()] {
        assert!(matches!(env.create(bad), Err(Error::BadName { .. })), "{bad:?}");
    }
    assert!(!env.folder().exists() || std::fs::read_dir(env.folder()).unwrap().count() == 0);
}
