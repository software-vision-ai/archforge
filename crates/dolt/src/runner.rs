//! Every helper program runs through here: by absolute path, in a cleared environment, with its
//! output capped and its run bounded by a budget.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};
use std::time::{Duration, Instant};

use crate::Error;

/// The most of each stream a helper's output keeps; the rest is read and dropped.
const CAP: u64 = 64 * 1024;
/// The budget of a helper; PowerShell's cold start needs longer.
pub(crate) const HELPER_BUDGET: Duration = Duration::from_secs(5);
#[cfg(windows)]
pub(crate) const POWERSHELL_BUDGET: Duration = Duration::from_secs(15);

/// What a finished helper said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Output {
    pub code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// The variables a child keeps from this process's environment; everything else is cleared.
const KEPT: &[&str] = &["PATH", "TMPDIR", "TEMP", "TMP", "SystemRoot", "USERPROFILE", "LOCALAPPDATA", "APPDATA"];

/// The environment every child starts from.
pub(crate) fn base_env() -> Vec<(OsString, OsString)> {
    KEPT.iter().filter_map(|k| std::env::var_os(k).map(|v| (OsString::from(k), v))).collect()
}

/// A command for `program` with only `env` in its environment.
pub(crate) fn command(program: &Path, env: &[(OsString, OsString)]) -> Command {
    let mut c = Command::new(program);
    c.env_clear().envs(env.iter().map(|(k, v)| (k, v))).stdin(Stdio::null());
    c
}

/// Held while a child starts: shared by every start but a watchdog's, which holds it alone.
static STARTING: RwLock<()> = RwLock::new(());
/// The longest a start waits for its turn. A start takes a moment, so a longer wait means one
/// has stalled.
pub(crate) const START_WAIT: Duration = Duration::from_millis(500);

/// A start's hold on [`STARTING`], shared or alone.
type Turn = (Option<RwLockReadGuard<'static, ()>>, Option<RwLockWriteGuard<'static, ()>>);

/// Waits at most [`START_WAIT`] for a turn to start a child, `alone` or beside other starts;
/// `None` when a stalled start still holds it then. A poisoned lock is taken all the same: it
/// guards no data.
fn turn(alone: bool) -> Option<Turn> {
    let asked = Instant::now();
    loop {
        let got = if alone {
            match STARTING.try_write() {
                Ok(g) => Some((None, Some(g))),
                Err(TryLockError::Poisoned(p)) => Some((None, Some(p.into_inner()))),
                Err(TryLockError::WouldBlock) => None,
            }
        } else {
            match STARTING.try_read() {
                Ok(g) => Some((Some(g), None)),
                Err(TryLockError::Poisoned(p)) => Some((Some(p.into_inner()), None)),
                Err(TryLockError::WouldBlock) => None,
            }
        };
        if got.is_some() || asked.elapsed() >= START_WAIT {
            return got;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Starts `c`. Where a pipe's close-on-exec mark is set only after the pipe is made (macOS), a
/// child started in between inherits the pipe and holds it open. For a helper's output that ends
/// with the child. For a watchdog's input it must not happen, so a watchdog starts `alone`, with
/// no other start of this crate under way, and every other start only keeps out of a watchdog's.
/// No start waits longer than [`START_WAIT`] for that: a stalled one never holds up a stop or a
/// recovery elsewhere. A start that goes ahead without its turn is logged; it may inherit, or as
/// a watchdog's let inherit, the pipe of a watchdog starting then, which delays that watchdog as
/// [`crate::watchdog`] says.
pub(crate) fn spawn(c: &mut Command, alone: bool) -> std::io::Result<Child> {
    let turn = turn(alone);
    if turn.is_none() {
        log::warn!("another start of a database program has stalled; {:?} starts beside it", c.get_program());
    }
    c.spawn()
}

/// Runs `program` with `args` and waits at most `budget`; a helper still running then is killed.
pub(crate) fn run(program: &Path, args: &[&OsStr], env: &[(OsString, OsString)], budget: Duration) -> Result<Output, Error> {
    let name = program.display().to_string();
    let mut c = command(program, env);
    c.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    // The wait for a turn to start counts against the budget.
    let started = Instant::now();
    let mut child = spawn(&mut c, false).map_err(|e| Error::io("cannot run", program, e))?;
    let out = child.stdout.take().map(|s| std::thread::spawn(move || drain(s)));
    let err = child.stderr.take().map(|s| std::thread::spawn(move || drain(s)));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < budget => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::HelperTimedOut { helper: name, waited: budget });
            }
            Err(e) => return Err(Error::io("cannot wait for", program, e)),
        }
    };
    let join = |h: Option<std::thread::JoinHandle<String>>| h.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok(Output { code: status.code(), success: status.success(), stdout: join(out), stderr: join(err) })
}

/// Reads a stream to its end, keeping its first [`CAP`] bytes, so a talkative helper never blocks
/// on a full pipe.
fn drain(stream: impl Read) -> String {
    let mut kept = Vec::new();
    let mut limited = stream.take(CAP);
    let _ = limited.read_to_end(&mut kept);
    let dropped = std::io::copy(&mut limited.into_inner(), &mut std::io::sink()).unwrap_or(0);
    if dropped > 0 {
        log::warn!("a helper wrote {dropped} bytes past the {CAP} kept of its output");
    }
    String::from_utf8_lossy(&kept).into_owned()
}

/// A failed helper as an error, with its output for the message.
pub(crate) fn failed(program: &Path, out: &Output) -> Error {
    let status = out.code.map(|c| format!("exit {c}")).unwrap_or_else(|| "ended by a signal".to_string());
    let text = format!("{} {}", out.stdout.trim(), out.stderr.trim()).trim().chars().take(1000).collect();
    Error::HelperFailed { helper: program.display().to_string(), status, output: text }
}

/// A program of Windows' own, by absolute path under `SystemRoot`.
#[cfg(windows)]
pub(crate) fn system32(relative: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| {
        log::warn!("SystemRoot is not set to an absolute path; using C:\\Windows");
        PathBuf::from(r"C:\Windows")
    });
    root.join("System32").join(relative)
}

/// The first `name` in an absolute entry of `path_var`; relative entries are skipped.
pub(crate) fn search(path_var: &OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var).filter(|d| d.is_absolute()).map(|d| d.join(name)).find(|p| p.is_file())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_helper_is_bounded_and_its_output_capped() {
        let sh = Path::new("/bin/sh");
        let out = run(sh, &[OsStr::new("-c"), OsStr::new("echo hi; echo oops >&2; exit 3")], &base_env(), HELPER_BUDGET).unwrap();
        assert_eq!((out.code, out.success, out.stdout.as_str(), out.stderr.trim()), (Some(3), false, "hi\n", "oops"));
        let big = run(sh, &[OsStr::new("-c"), OsStr::new("head -c 200000 /dev/zero")], &base_env(), HELPER_BUDGET).unwrap();
        assert_eq!(big.stdout.len(), 64 * 1024);
        let slow = run(sh, &[OsStr::new("-c"), OsStr::new("sleep 5")], &base_env(), Duration::from_millis(200));
        assert!(matches!(slow, Err(Error::HelperTimedOut { .. })));
        // The environment is cleared but for the kept variables.
        let env = run(Path::new("/usr/bin/env"), &[], &[(OsString::from("ONLY"), OsString::from("this"))], HELPER_BUDGET).unwrap();
        assert_eq!(env.stdout, "ONLY=this\n");
    }

    /// A watchdog's start that has stalled holds its turn alone: a stop's helper elsewhere waits
    /// no longer than [`START_WAIT`] and runs; and starts that share never keep each other out.
    #[test]
    fn a_stalled_start_never_holds_up_another_projects_helper() {
        let (tx, rx) = std::sync::mpsc::channel();
        let stalled = STARTING.write().unwrap_or_else(std::sync::PoisonError::into_inner);
        std::thread::spawn(move || tx.send(run(Path::new("/bin/sh"), &[OsStr::new("-c"), OsStr::new("echo ran")], &base_env(), HELPER_BUDGET)));
        let out = rx.recv_timeout(START_WAIT + HELPER_BUDGET).expect("the helper ran though a start is stalled").unwrap();
        assert_eq!(out.stdout, "ran\n");
        drop(stalled);
        let ordinary = STARTING.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(matches!(turn(false), Some((Some(_), None))), "a stalled ordinary start shares its turn with other starts");
        assert!(turn(true).is_none(), "and a watchdog waits its limit for it, no longer");
        drop(ordinary);
        // Other tests' starts share the turn meanwhile, each for a moment.
        let alone = std::iter::repeat_with(|| turn(true)).take(20).flatten().next();
        assert!(matches!(alone, Some((None, Some(_)))) && STARTING.try_read().is_err(), "a watchdog starts alone");
    }

    #[test]
    fn the_path_search_skips_relative_entries() {
        let path = std::env::join_paths(["bin", "/bin"]).unwrap();
        assert_eq!(search(&path, "sh"), Some(PathBuf::from("/bin/sh")));
        assert_eq!(search(&std::env::join_paths(["bin"]).unwrap(), "sh"), None);
    }
}
