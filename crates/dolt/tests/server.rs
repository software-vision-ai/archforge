//! The server against a real dolt: start, the password, no usage records, a second start, crash
//! recovery, leftovers, the socket limit, an odd folder name, and a lock that names another
//! program. These tests need dolt 2.6 or later; without it they fail and say how to install it.
//! Measured on macOS; the Linux and Windows runs are still to be done.
#![cfg(unix)]

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use archforge_dolt::{Dolt, Error, Home, Server, ServerOptions};
use mysql::prelude::Queryable;

/// The marker that makes this test binary act as the crash helper.
const HELPER: &str = "ARCHFORGE_DOLT_TEST_HELPER";

/// One test's folders: a dolt home, records, a short run root, a project folder.
struct Env {
    base: PathBuf,
    run_root: PathBuf,
    home: Home,
    dolt: Dolt,
    cleanup: bool,
}

impl Env {
    fn new(name: &str) -> Env {
        let tag = format!("{}{}", std::process::id(), name.len());
        let base = std::env::temp_dir().join(format!("af-it-{name}-{tag}"));
        let run_root = std::env::temp_dir().join(format!("afr-{name}-{tag}"));
        for d in [&base, &run_root] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::create_dir_all(base.join("data")).unwrap();
        Env::at(base, run_root, true)
    }

    fn at(base: PathBuf, run_root: PathBuf, cleanup: bool) -> Env {
        let home = Home::prepare(&base.join("home")).unwrap();
        let dolt = Dolt::find(&home)
            .unwrap_or_else(|e| panic!("these tests need dolt 2.6 or later within major version 2 on the PATH or in ARCHFORGE_DOLT: {e}"));
        Env { base, run_root, home, dolt, cleanup }
    }

    fn data(&self) -> PathBuf {
        self.base.join("data")
    }

    fn opts(&self) -> ServerOptions {
        ServerOptions {
            run_root: Some(self.run_root.clone()),
            ready_budget: Duration::from_secs(20),
            ..ServerOptions::new(self.base.join("records"))
        }
    }

    fn start(&self) -> Result<Server, Error> {
        Server::start(&self.dolt, &self.data(), &self.opts())
    }

    /// The records and run folders left behind.
    fn leftovers(&self) -> (usize, usize) {
        let count = |d: &Path, f: &dyn Fn(&str) -> bool| {
            std::fs::read_dir(d).map(|e| e.flatten().filter(|x| f(&x.file_name().to_string_lossy())).count()).unwrap_or(0)
        };
        (count(&self.base.join("records"), &|n| n.ends_with(".json")), count(&self.run_root, &|n| n.starts_with("af-")))
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        if self.cleanup {
            // A test that failed half way may leave a server under this run root: none survives it.
            let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.run_root).stderr(Stdio::null()).status();
            let _ = std::fs::remove_dir_all(&self.base);
            let _ = std::fs::remove_dir_all(&self.run_root);
        }
    }
}

fn alive(pid: u32) -> bool {
    Command::new("/bin/kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Kills a leftover server when a test fails half way, so no run leaves an orphan.
struct KillOnDrop(u32);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if alive(self.0) {
            let _ = Command::new("/bin/kill").args(["-KILL", &self.0.to_string()]).status();
        }
    }
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

fn one(conn: &mut mysql::Conn) -> i64 {
    conn.query_first::<i64, _>("SELECT 1").unwrap().unwrap()
}

fn tcp(server: &Server, password: Option<&str>) -> Result<mysql::Conn, mysql::Error> {
    let o =
        mysql::OptsBuilder::new().user(Some("root")).pass(password).prefer_socket(false).ip_or_hostname(Some("127.0.0.1")).tcp_port(server.port());
    mysql::Conn::new(o)
}

fn socket(server: &Server, password: Option<&str>) -> Result<mysql::Conn, mysql::Error> {
    let o = mysql::OptsBuilder::new()
        .user(Some("root"))
        .pass(password)
        .prefer_socket(false)
        .socket(server.socket().map(|s| s.to_string_lossy().into_owned()));
    mysql::Conn::new(o)
}

#[test]
fn a_server_starts_answers_on_socket_and_port_and_stops_without_leftovers() {
    let env = Env::new("start");
    let server = env.start().unwrap();
    let _guard = KillOnDrop(server.pid());
    assert_eq!(one(&mut server.connect().unwrap()), 1);
    let pw = server.password().expose().to_string();
    assert_eq!(one(&mut tcp(&server, Some(&pw)).unwrap()), 1);
    assert_eq!(one(&mut socket(&server, Some(&pw)).unwrap()), 1);
    assert_eq!(env.leftovers(), (1, 1));
    let pid = server.pid();
    server.stop().unwrap();
    assert!(!alive(pid));
    assert_eq!(env.leftovers(), (0, 0));
}

#[test]
fn the_password_is_required_on_the_socket_and_the_port() {
    let env = Env::new("password");
    let server = env.start().unwrap();
    let _guard = KillOnDrop(server.pid());
    assert!(tcp(&server, None).is_err());
    assert!(socket(&server, None).is_err());
    assert!(tcp(&server, Some("wrong")).is_err());
    server.stop().unwrap();
}

#[test]
fn no_usage_events_and_no_version_check_and_the_users_own_dolt_home_is_untouched() {
    let own = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".dolt"));
    let snapshot = |d: &Option<PathBuf>| -> Vec<(String, u64)> {
        let Some(d) = d else { return Vec::new() };
        let mut v: Vec<(String, u64)> =
            walk(d).into_iter().map(|p| (p.display().to_string(), std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0))).collect();
        v.sort();
        v
    };
    let before = snapshot(&own);
    let env = Env::new("metrics");
    let server = env.start().unwrap();
    let _guard = KillOnDrop(server.pid());
    server.connect().unwrap().query_drop("CREATE DATABASE probe").unwrap();
    server.stop().unwrap();
    let dot = env.home.path().join(".dolt");
    let events: Vec<String> = std::fs::read_dir(dot.join("eventsData"))
        .map(|e| e.flatten().map(|x| x.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    assert!(events.iter().all(|n| n == "dolt.lock"), "usage events were recorded: {events:?}");
    assert!(walk(&dot).iter().all(|p| !p.ends_with("version_check.txt")), "dolt checked its version online");
    assert_eq!(snapshot(&own), before, "the user's own ~/.dolt changed");
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(d) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk(&p)) } else { out.push(p) }
        }
    }
    out
}

#[test]
fn a_second_start_on_an_open_project_is_refused_and_the_first_keeps_running() {
    let env = Env::new("second");
    let first = env.start().unwrap();
    let _guard = KillOnDrop(first.pid());
    assert!(matches!(env.start(), Err(Error::OpenElsewhere { pid }) if pid == first.pid()));
    assert_eq!(one(&mut first.connect().unwrap()), 1);
    first.stop().unwrap();
}

/// Runs this test binary again as a helper that starts a server for `env` and waits, and returns
/// it with the server's process id once the server is ready.
fn helper(env: &Env) -> (std::process::Child, u32) {
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new(exe)
        .args(["--exact", "crash_helper", "--ignored", "--nocapture", "--test-threads=1"])
        .env(HELPER, format!("{}\n{}", env.base.display(), env.run_root.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            // The test harness prints the test's name on the same line before it.
            if let Some(pid) = line.split_once("READY ").map(|(_, p)| p) {
                let _ = tx.send(pid.trim().parse::<u32>().unwrap_or(0));
            }
        }
    });
    let pid = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|_| {
        let _ = child.kill();
        panic!("the crash helper did not start its server")
    });
    (child, pid)
}

/// The crash helper: run only by `helper`, never on its own.
#[test]
#[ignore = "run by the crash tests as a separate process"]
fn crash_helper() {
    let Some(spec) = std::env::var_os(HELPER) else { return };
    let spec = spec.to_string_lossy().into_owned();
    let (base, run_root) = spec.split_once('\n').unwrap();
    let env = Env::at(PathBuf::from(base), PathBuf::from(run_root), false);
    let server = env.start().unwrap();
    println!("READY {}", server.pid());
    std::thread::sleep(Duration::from_secs(120));
    drop(server);
}

#[test]
fn a_server_left_by_a_crashed_archforge_is_stopped_at_the_next_start() {
    let env = Env::new("crash");
    let (mut owner, orphan) = helper(&env);
    let _guard = KillOnDrop(orphan);
    owner.kill().unwrap();
    owner.wait().unwrap();
    assert!(alive(orphan), "the server outlives its owner, as measured");
    let server = env.start().unwrap();
    let _guard2 = KillOnDrop(server.pid());
    assert!(wait_gone(orphan), "the orphan was stopped");
    assert_eq!(one(&mut server.connect().unwrap()), 1);
    server.stop().unwrap();
    assert_eq!(env.leftovers(), (0, 0));
}

#[test]
fn a_dead_server_its_stale_socket_record_and_lock_do_not_stop_a_start() {
    let env = Env::new("stale");
    let (mut owner, server_pid) = helper(&env);
    let _guard = KillOnDrop(server_pid);
    owner.kill().unwrap();
    owner.wait().unwrap();
    Command::new("/bin/kill").args(["-KILL", &server_pid.to_string()]).status().unwrap();
    assert!(wait_gone(server_pid));
    assert!(env.data().join(".dolt/sql-server.info").exists(), "the killed server left its lock");
    assert_eq!(env.leftovers(), (1, 1), "and its record and run folder with the socket");
    let server = env.start().unwrap();
    let _guard2 = KillOnDrop(server.pid());
    assert_eq!(one(&mut server.connect().unwrap()), 1);
    assert_eq!(env.leftovers(), (1, 1), "only the new server's record and run folder");
    server.stop().unwrap();
    assert_eq!(env.leftovers(), (0, 0));
}

#[test]
fn a_run_root_too_long_for_a_socket_is_refused_before_anything_starts() {
    let env = Env::new("socket");
    let long = env.run_root.join("r".repeat(100));
    std::fs::create_dir_all(&long).unwrap();
    let opts = ServerOptions { run_root: Some(long.clone()), ..env.opts() };
    assert!(matches!(Server::start(&env.dolt, &env.data(), &opts), Err(Error::SocketPathTooLong { .. })));
    assert_eq!(std::fs::read_dir(&long).unwrap().count(), 0, "no run folder stays");
    assert_eq!(env.leftovers().0, 0, "no record was written");
    assert!(!env.data().join(".dolt").exists(), "no server ran");
}

#[test]
fn a_project_folder_with_dollar_quotes_and_spaces_starts() {
    let env = Env::new("names");
    let odd = env.base.join("pro$ject \"x\" ${HOME}");
    std::fs::create_dir_all(&odd).unwrap();
    let server = Server::start(&env.dolt, &odd, &env.opts()).unwrap();
    let _guard = KillOnDrop(server.pid());
    assert_eq!(one(&mut server.connect().unwrap()), 1);
    assert!(odd.join(".dolt").exists(), "the server serves that very folder");
    server.stop().unwrap();
}

#[test]
fn a_lock_naming_another_program_does_not_stop_a_start_and_that_program_lives_on() {
    let env = Env::new("otherlock");
    let mut sleeper = Command::new("/bin/sleep").arg("60").spawn().unwrap();
    std::fs::create_dir_all(env.data().join(".dolt")).unwrap();
    std::fs::write(env.data().join(".dolt/sql-server.info"), format!("{}:3306:00000000-1111-2222-3333-444444444444", sleeper.id())).unwrap();
    let server = env.start().unwrap();
    let _guard = KillOnDrop(server.pid());
    assert_eq!(one(&mut server.connect().unwrap()), 1);
    assert!(alive(sleeper.id()), "the other program was left alone");
    server.stop().unwrap();
    sleeper.kill().unwrap();
    sleeper.wait().unwrap();
}
