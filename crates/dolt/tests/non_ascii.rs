//! The start-up sweep against a real dolt whose run root holds characters outside ASCII, which
//! `ps` rewrites on macOS (AR-443). Needs dolt 2.6 or later, like the server tests.
//! Measured on macOS; the Linux run is still to be done.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use archforge_dolt::{Dolt, Home, Server, ServerOptions};

/// A folder in the temporary folder made under a new random name ending in `suffix`.
fn unique_dir(suffix: &str) -> PathBuf {
    loop {
        let mut bytes = [0u8; 4];
        getrandom::fill(&mut bytes).unwrap();
        let dir = std::env::temp_dir().join(format!("a{:08x}{suffix}", u32::from_le_bytes(bytes)));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => panic!("cannot make {}: {e}", dir.display()),
        }
    }
}

/// One of the test's folders, removed when dropped, panics included, with any server left under it.
struct Folder(PathBuf);

impl Drop for Folder {
    /// Kills any server whose command line names the folder, then removes it.
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/pkill").args(["-KILL", "-f"]).arg(&self.0).stderr(Stdio::null()).status();
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// How many entries of `dir` have a name `keep` accepts.
fn count(dir: &Path, keep: &dyn Fn(&str) -> bool) -> usize {
    std::fs::read_dir(dir).map(|e| e.flatten().filter(|x| keep(&x.file_name().to_string_lossy())).count()).unwrap_or(0)
}

/// A server whose run root holds "é ü", an upper-case umlaut and a sign beyond the basic plane is
/// recognised as the recorded one once its Archforge is gone, and stopped politely with its
/// record and run folder.
#[test]
fn start_up_stops_an_orphan_whose_run_root_holds_characters_outside_ascii() {
    let (base, run_root) = (Folder(unique_dir("")), Folder(unique_dir(" é ü Ä😀")));
    let (base, run_root) = (base.0.as_path(), run_root.0.as_path());
    std::fs::create_dir(base.join("data")).unwrap();
    let home = Home::prepare(&base.join("home")).unwrap();
    let dolt = Dolt::find(&home).unwrap_or_else(|e| panic!("this test needs dolt 2.6 or later on the PATH or in ARCHFORGE_DOLT: {e}"));
    let records = base.join("records");
    let opts = ServerOptions { run_root: Some(run_root.to_path_buf()), ready_budget: Duration::from_secs(20), ..ServerOptions::new(&records) };
    let server = Server::start(&dolt, &base.join("data"), &opts).unwrap();
    assert_eq!(archforge_dolt::stop_orphans(&opts).unwrap(), 0, "its Archforge, this test, still runs");
    let left = || (count(&records, &|n| n.ends_with(".json")), count(run_root, &|n| n.starts_with("af-")));
    assert_eq!(left(), (1, 1), "its record and run folder stay");

    // The record's Archforge is gone: its process id is this test's, but its start time is not.
    let file = std::fs::read_dir(&records).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "json")).unwrap();
    let mut record: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert!(record["command"][3].as_str().unwrap().contains(" é ü Ä😀/af-"), "the recorded command line names the run root");
    record["owner_started"] = "Thu Jan  1 00:00:00 1970".into();
    std::fs::write(&file, record.to_string()).unwrap();

    assert_eq!(archforge_dolt::stop_orphans(&opts).unwrap(), 1, "the sweep recognised its own server");
    assert_eq!(left(), (0, 0), "its record and run folder went with it");
    assert!(!base.join("data/.dolt/sql-server.info").exists(), "it stopped politely, removing its lock");
    // The stopped server was this test's child: stopping it again only collects it.
    server.stop().unwrap();
}
