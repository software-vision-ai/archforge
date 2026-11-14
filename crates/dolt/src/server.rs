//! Starting, connecting to and stopping the server of one project folder.

use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use crate::identity::identity;
use crate::record::{self, Record};
use crate::recovery::{self, Action, Lock, Proc};
use crate::watchdog::Watchdog;
use crate::{Dolt, Error, Secret, config, paths, run_dir, runner};

/// How long a connection may take to open, and a query to answer, during readiness.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);
/// The same for the connections [`Server::connect`] gives.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
/// How much of the server's log an error quotes.
const LOG_TAIL: u64 = 4096;
/// Starts after a lost race for the port.
const PORT_RETRIES: usize = 2;

/// Where a server's records and run folders go, and how long it may take.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// A folder of this user's in the application's settings, for the server records.
    pub records: PathBuf,
    /// Where run folders go; by default the user's temporary folder (the records folder on Windows).
    pub run_root: Option<PathBuf>,
    pub ready_budget: Duration,
    pub stop_budget: Duration,
}

impl ServerOptions {
    pub fn new(records: impl Into<PathBuf>) -> ServerOptions {
        ServerOptions { records: records.into(), run_root: None, ready_budget: Duration::from_secs(10), stop_budget: Duration::from_secs(5) }
    }
}

/// How to reach a running server, apart from the server itself: a read on its own thread takes
/// one, so it needs no hold on whoever owns the server. Connecting fails once the server stopped.
#[derive(Debug, Clone)]
pub struct Connector {
    port: u16,
    socket: Option<PathBuf>,
    password: Secret,
}

impl Connector {
    /// A connection to the server, over its socket on unix and over loopback on Windows.
    pub fn connect(&self) -> Result<mysql::Conn, Error> {
        mysql::Conn::new(options(self.socket.as_deref(), self.port, &self.password, CONNECT_TIMEOUT, QUERY_TIMEOUT))
            .map_err(|e| Error::Sql { what: "cannot connect to the project's database server", source: e })
    }
}

/// A running server for one project folder. Dropping it stops the server too.
#[derive(Debug)]
pub struct Server {
    child: Started,
    data_dir: PathBuf,
    run_root: PathBuf,
    run_dir: PathBuf,
    records: PathBuf,
    data_text: String,
    port: u16,
    socket: Option<PathBuf>,
    password: Secret,
    uid: Option<u32>,
    /// Dropped, and so ended, after the attempt to stop the server.
    _watchdog: Watchdog,
}

impl Server {
    /// Starts the server for `data_dir`, first recovering what an earlier start left behind.
    pub fn start(dolt: &Dolt, data_dir: &Path, opts: &ServerOptions) -> Result<Server, Error> {
        let Prepared { data_dir, data_text, run_root, uid } = prepare(data_dir, opts)?;
        let me = std::process::id();
        let owner = identity(me)?.ok_or_else(|| Error::HelperFailed {
            helper: "the identity helper".into(),
            status: "no answer".into(),
            output: format!("process {me} not found"),
        })?;
        let run_dir = run_dir::create(&run_root, uid)?;
        let launched = launch(dolt, &data_dir, &data_text, &run_root, &run_dir, uid, &owner.started, opts);
        if launched.is_err() {
            run_dir::remove(&run_root, &run_dir, uid);
        }
        launched
    }

    /// A connection to the server, over its socket on unix and over loopback on Windows.
    pub fn connect(&self) -> Result<mysql::Conn, Error> {
        self.connector().connect()
    }

    /// How to reach this server from a thread that holds nothing of it.
    pub fn connector(&self) -> Connector {
        Connector { port: self.port, socket: self.socket.clone(), password: self.password.clone() }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The server's socket (unix).
    pub fn socket(&self) -> Option<&Path> {
        self.socket.as_deref()
    }

    pub fn password(&self) -> &Secret {
        &self.password
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The server's process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Stops the server and removes its record and run folder.
    pub fn stop(mut self) -> Result<(), Error> {
        let budget = self.child.stop_budget;
        self.shutdown(budget)
    }

    /// Stops the server within `budget`, once: a later call does nothing. A stopped server's
    /// record and run folder go; one that would not stop keeps them, for recovery.
    fn shutdown(&mut self, budget: Duration) -> Result<(), Error> {
        if self.child.tried {
            return Ok(());
        }
        let result = self.child.stop_within(budget);
        if result.is_ok() {
            record::remove_if(&self.records, &self.data_text, self.child.id());
            run_dir::remove(&self.run_root, &self.run_dir, self.uid);
        }
        result
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown(Duration::from_secs(1)) {
            log::warn!("the project's database server did not stop cleanly: {e}");
        }
    }
}

/// A server this process started, from the moment it runs. Dropping it stops and reaps the server
/// unless that was tried before: whatever fails between a server's start and its [`Server`], and
/// however the start returns, no server is left without its record, its watchdog and an owner to
/// stop it. A server is asked to stop once; one that would not is left to recovery.
#[derive(Debug)]
struct Started {
    child: Child,
    stop_budget: Duration,
    /// Whether a stop was tried.
    tried: bool,
}

impl Started {
    /// The server's process id.
    fn id(&self) -> u32 {
        self.child.id()
    }

    /// Stops the server: a polite signal on unix, then a kill after `budget`.
    fn stop_within(&mut self, budget: Duration) -> Result<(), Error> {
        self.tried = true;
        stop_child(&mut self.child, budget)
    }

    /// Stops the server, saying so in the log when it would not stop; returns whether it is gone.
    fn stop(&mut self) -> bool {
        match self.stop_within(self.stop_budget) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("a database server that was to stop did not: {e}");
                false
            }
        }
    }
}

impl Drop for Started {
    /// Stops the server unless a stop was tried, as it was after [`Server::stop`].
    fn drop(&mut self) {
        if !self.tried {
            self.stop();
        }
    }
}

/// A start's checked inputs, after recovery.
struct Prepared {
    data_dir: PathBuf,
    data_text: String,
    run_root: PathBuf,
    uid: Option<u32>,
}

/// Checks the folder, the records and the run root, then sweeps and recovers.
fn prepare(data_dir: &Path, opts: &ServerOptions) -> Result<Prepared, Error> {
    paths::text("project folder", data_dir)?;
    if !data_dir.is_dir() {
        return Err(Error::NotAFolder { path: data_dir.to_path_buf() });
    }
    let data_dir = data_dir.canonicalize().map_err(|e| Error::io("cannot read", data_dir, e))?;
    let data_text = paths::text("project folder", &data_dir)?.to_string();
    private_records(&opts.records)?;
    let (run_root, uid) = run_dir::checked_root(&opts.run_root.clone().unwrap_or_else(|| run_dir::default_root(&opts.records)))?;
    sweep(&opts.records, Some(&data_text), uid, None);
    recover(&data_dir, &data_text, opts, uid)?;
    Ok(Prepared { data_dir, data_text, run_root, uid })
}

/// Recovers `data_dir` without starting a server: a server a crashed Archforge left on a folder
/// that will not be opened (an unfinished project, say) is stopped all the same. Fails as
/// [`Server::start`] would, without starting.
pub fn recover_only(data_dir: &Path, opts: &ServerOptions) -> Result<(), Error> {
    prepare(data_dir, opts).map(|_| ())
}

/// The records folder drives what recovery stops: like the dolt home, a folder only this user can
/// use, created so if missing.
fn private_records(records: &Path) -> Result<(), Error> {
    paths::text("records folder", records)?;
    crate::home::private_folder(records).map_err(|why| Error::RecordsUnsafe { path: records.to_path_buf(), why })
}

/// What the process `pid` is, judged against `record`.
fn classify(pid: u32, record: Option<&Record>) -> Result<Proc, Error> {
    let Some(id) = identity(pid)? else { return Ok(Proc::Gone) };
    if let Some(r) = record
        && id.is(&r.command)
    {
        let alive = identity(r.owner_pid)?.is_some_and(|o| o.started == r.owner_started);
        return Ok(if alive { Proc::OursOwnerAlive } else { Proc::OursOwnerDead });
    }
    Ok(if id.is_dolt_server() { Proc::OtherDoltServer } else { Proc::Other })
}

// Why a sweep at start-up besides the tie from a server to its Archforge (AR-422, AR-424): the tie is
// a watchdog process, which exists on unix only, can be killed with its Archforge, and may fail to
// start. Linux's parent-death signal would not do instead: it follows the thread that started the
// server, and servers start on worker threads that end once a project is open. Windows' job objects
// kill without a polite stop and need unsafe calls. So this sweep stays the backstop everywhere.
// What neither covers: an Archforge killed between a server's start and its record. The record
// holds the server's process id, so it cannot come first; a start that fails there stops its
// server (`Started`), but a kill runs no code. That moment is the record's write to disk, measured
// at 5 to 10 ms on macOS. A server left so has no record for a sweep to find, and its run folder
// stays. Recovery treats it as any server it did not record: once it holds the project's lock, and
// if its program's file is named `dolt`, the project is refused as open elsewhere, naming the
// server's process id, until that process is stopped.
/// Stops the servers of every project whose Archforge is gone (crashed, killed or force quit), and
/// forgets the records of servers that are gone: for a start-up, so a project never opened again
/// does not keep its server running. A server whose Archforge still runs, this one or another, is
/// left alone, and a process is signalled only while its command line, checked before each signal,
/// is exactly the recorded one, whose configuration lies in a run folder with a random name made
/// for that start: a process id another program took gets no signal. Returns how many servers
/// were stopped.
pub fn stop_orphans(opts: &ServerOptions) -> Result<usize, Error> {
    private_records(&opts.records)?;
    let (_, uid) = run_dir::checked_root(&opts.run_root.clone().unwrap_or_else(|| run_dir::default_root(&opts.records)))?;
    Ok(sweep(&opts.records, None, uid, Some(opts.stop_budget)))
}

/// Goes through the records of every project but `except`: forgets those whose server is gone and,
/// given a stop budget, stops the servers whose Archforge is gone, as recovery does, then forgets
/// them. Returns how many it stopped.
fn sweep(records: &Path, except: Option<&str>, uid: Option<u32>, stop: Option<Duration>) -> usize {
    let all = match record::all(records) {
        Ok(a) => a,
        Err(e) => {
            log::warn!("the server records could not be swept: {e}");
            return 0;
        }
    };
    let mut stopped = 0;
    for (file, r) in all.into_iter().filter(|(_, r)| except != Some(r.data_dir.as_str())) {
        match (classify(r.server_pid, Some(&r)), stop) {
            (Ok(Proc::Gone | Proc::Other | Proc::OtherDoltServer), _) => forget(&file, &r, uid),
            (Ok(Proc::OursOwnerDead), Some(budget)) => {
                log::info!("stopping the database server {} that a closed Archforge left running for {}", r.server_pid, r.data_dir);
                match stop_foreign(r.server_pid, &r.command, budget) {
                    Ok(()) => {
                        forget(&file, &r, uid);
                        stopped += 1;
                    }
                    Err(e) => log::warn!("the database server {} left running for {} stays: {e}", r.server_pid, r.data_dir),
                }
            }
            (Ok(_), _) => {}
            (Err(e), _) => log::warn!("the server record {} stays: {e}", file.display()),
        }
    }
    stopped
}

/// Removes a record and the run folder it names, under the run root it records; the record only
/// while it is still the one read, since another start may have written its own since.
fn forget(file: &Path, r: &Record, uid: Option<u32>) {
    let root = Path::new(&r.run_root);
    if root.is_absolute() {
        run_dir::remove(root, Path::new(&r.run_dir), uid);
    }
    record::remove_while(file, |found| found == r);
}

/// Recovery for this project: gathers the facts, decides, acts.
fn recover(data_dir: &Path, data_text: &str, opts: &ServerOptions, uid: Option<u32>) -> Result<(), Error> {
    let record = record::read(&opts.records, data_text)?;
    let lock = recovery::read_lock(data_dir)?;
    let holder = match lock {
        Lock::Pid(pid) => classify(pid, record.as_ref())?,
        _ => Proc::Gone,
    };
    let recorded = match &record {
        Some(r) => Some((r.server_pid, classify(r.server_pid, Some(r))?)),
        None => None,
    };
    let file = record::path(&opts.records, data_text);
    match recovery::decide(&lock, holder, recorded) {
        Action::Proceed => Ok(()),
        Action::Clean => {
            if let Some(r) = &record {
                forget(&file, r, uid);
            }
            Ok(())
        }
        Action::Stop(pid) => {
            if let Some(r) = &record {
                log::info!("stopping the database server {pid} that a closed Archforge left running for {data_text}");
                stop_foreign(pid, &r.command, opts.stop_budget)?;
                forget(&file, r, uid);
            }
            Ok(())
        }
        Action::OpenElsewhere(pid) => Err(Error::OpenElsewhere { pid }),
        Action::DamagedLock => Err(Error::DamagedLock { path: data_dir.join(".dolt").join("sql-server.info") }),
    }
}

/// Stops a server that is not this process's child, checking before every signal that it is still
/// the recorded one.
fn stop_foreign(pid: u32, command: &[String], budget: Duration) -> Result<(), Error> {
    let ours = || -> Result<bool, Error> { Ok(identity(pid)?.is_some_and(|i| i.is(command))) };
    let pid_text = pid.to_string();
    #[cfg(unix)]
    let signals: &[&[&str]] = &[&["-TERM"], &["-KILL"]];
    #[cfg(windows)]
    let signals: &[&[&str]] = &[&["/F"]];
    for signal in signals {
        if !ours()? {
            return Ok(());
        }
        #[cfg(unix)]
        let (program, args) = (PathBuf::from("/bin/kill"), [signal[0], pid_text.as_str()].map(OsStr::new).to_vec());
        #[cfg(windows)]
        let (program, args) = (runner::system32("taskkill.exe"), ["/PID", pid_text.as_str(), signal[0]].map(OsStr::new).to_vec());
        // A failing signal may only mean the process is already gone: the poll below decides.
        let _ = runner::run(&program, &args, &runner::base_env(), runner::HELPER_BUDGET)?;
        let sent = Instant::now();
        while sent.elapsed() < budget {
            if !ours()? {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    if ours()? { Err(Error::StillRunning { pid, waited: budget }) } else { Ok(()) }
}

/// Stops this process's own child: a polite signal on unix, then a kill after `budget`.
#[cfg_attr(not(unix), allow(unused_variables))]
fn stop_child(child: &mut Child, budget: Duration) -> Result<(), Error> {
    if let Ok(Some(_)) = child.try_wait() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        let pid = child.id().to_string();
        let _ = runner::run(Path::new("/bin/kill"), &[OsStr::new("-TERM"), OsStr::new(&pid)], &runner::base_env(), budget.min(runner::HELPER_BUDGET));
        let sent = Instant::now();
        while sent.elapsed() < budget {
            if let Ok(Some(_)) = child.try_wait() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    child.kill().map_err(|e| Error::io("cannot stop the database server", format!("process {}", child.id()), e))?;
    child.wait().map(|_| ()).map_err(|e| Error::io("cannot wait for the database server", format!("process {}", child.id()), e))
}

/// Writes the configuration, starts the server, records it, and waits until it is ready; a lost
/// race for the port starts it again. Every step that can fail without the server comes before
/// its start; what can fail after it (the record, the wait) returns through [`Started`], whose
/// drop stops the server.
fn launch(
    dolt: &Dolt,
    data_dir: &Path,
    data_text: &str,
    run_root: &Path,
    run: &Path,
    uid: Option<u32>,
    owner_started: &str,
    opts: &ServerOptions,
) -> Result<Server, Error> {
    let socket = if cfg!(unix) {
        let s = run.join("db.sock");
        paths::socket_fits(&s)?;
        Some(s)
    } else {
        None
    };
    let socket_text = socket.as_deref().map(|s| paths::text("socket", s)).transpose()?;
    let cfg_dir = run.join("doltcfg");
    let no_files = run.join("nofiles");
    for d in [&cfg_dir, &no_files] {
        std::fs::create_dir(d).map_err(|e| Error::io("cannot create", d, e))?;
    }
    let yaml_path = run.join("server.yaml");
    let log_path = run.join("server.log");
    let password = Secret::generate()?;
    let command: Vec<String> = vec![
        paths::text("dolt binary", dolt.path())?.to_string(),
        "sql-server".to_string(),
        "--config".to_string(),
        paths::text("configuration", &yaml_path)?.to_string(),
    ];
    let run_root_text = paths::text("run folder location", run_root)?.to_string();
    let run_text = paths::text("run folder", run)?.to_string();
    let (cfg_text, no_files_text) = (paths::text("run folder", &cfg_dir)?, paths::text("run folder", &no_files)?);
    let mut attempt = 0;
    loop {
        // The record's lock comes first: a wait for it leaves no server behind and no port to lose.
        // Nothing added between the server's start and `drop(held)` may return: the lock would be
        // held while the server is stopped (the tests' hook does that, in tests only).
        let held = record::hold(&record::path(&opts.records, data_text), record::LOCK_WAIT)?;
        let port = free_port()?;
        let yaml =
            config::server_yaml(&config::Settings { port, socket: socket_text, data_dir: data_text, cfg_dir: cfg_text, no_files: no_files_text });
        std::fs::write(&yaml_path, yaml).map_err(|e| Error::io("cannot write", &yaml_path, e))?;
        let log = std::fs::File::create(&log_path).map_err(|e| Error::io("cannot create", &log_path, e))?;
        let log_err = log.try_clone().map_err(|e| Error::io("cannot open", &log_path, e))?;
        let mut env = dolt.env();
        env.push(("DOLT_ROOT_PASSWORD".into(), password.expose().into()));
        env.push(("DOLT_ROOT_HOST".into(), "localhost".into()));
        let mut c = runner::command(dolt.path(), &env);
        c.args(command.iter().skip(1)).current_dir(run).stdout(Stdio::from(log)).stderr(Stdio::from(log_err));
        let child = runner::spawn(&mut c, false).map_err(|e| Error::io("cannot start", dolt.path(), e))?;
        // From here to the `Server`, no return leaves the server running: see `Started`.
        let mut child = Started { child, stop_budget: opts.stop_budget, tried: false };
        #[cfg(all(test, unix))]
        tests::after_spawn(child.id())?;
        let rec = Record {
            data_dir: data_text.to_string(),
            server_pid: child.id(),
            run_root: run_root_text.clone(),
            run_dir: run_text.clone(),
            command: command.clone(),
            owner_pid: std::process::id(),
            owner_started: owner_started.to_string(),
        };
        // Another start may have recorded a server since recovery looked: its record stays, and this
        // one gives way.
        let written = record::write_new(&held, &rec);
        drop(held);
        match written {
            #[cfg(all(test, unix))]
            Ok(true) => tests::WATCHED_AT_RECORD.set(Some(tests::watched(child.id()))),
            #[cfg(not(all(test, unix)))]
            Ok(true) => {}
            Ok(false) => {
                child.stop();
                let pid = record::read(&opts.records, data_text).ok().flatten().map_or(0, |r| r.server_pid);
                return Err(Error::OpenElsewhere { pid });
            }
            // Dropping `child` stops the server.
            Err(e) => return Err(e),
        }
        // The record comes first: the watchdog may wait for its turn to start, and a server of an
        // Archforge killed meanwhile is one the start-up sweep finds by its record. From here on a
        // killed Archforge takes this server with it. The watchdog ends with this turn of the loop
        // or with the `Server`, in both after the attempt to stop the server.
        let watchdog = Watchdog::tie(child.id(), &command);
        match wait_ready(&mut child.child, data_dir, socket.as_deref(), port, &password, &log_path, opts.ready_budget) {
            Ok(()) => {
                return Ok(Server {
                    child,
                    data_dir: data_dir.to_path_buf(),
                    run_root: run_root.to_path_buf(),
                    run_dir: run.to_path_buf(),
                    records: opts.records.clone(),
                    data_text: data_text.to_string(),
                    port,
                    socket,
                    password,
                    uid,
                    _watchdog: watchdog,
                });
            }
            Err(e) => {
                let pid = child.id();
                // A server that would not stop keeps its record, so recovery can still find it.
                let cleared = child.stop() && record::remove_if(&opts.records, data_text, pid);
                let lost_port = matches!(e, Error::StartFailed { .. }) && TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err();
                // With a record still under the name another try would only be refused by it.
                if lost_port && cleared && attempt < PORT_RETRIES {
                    attempt += 1;
                    continue;
                }
                return Err(e);
            }
        }
    }
}

/// A port nobody listens on now; the server may still lose it to a race, which a retry covers.
fn free_port() -> Result<u16, Error> {
    let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| Error::io("cannot find a free port on", "127.0.0.1", e))?;
    l.local_addr().map(|a| a.port()).map_err(|e| Error::io("cannot find a free port on", "127.0.0.1", e))
}

/// Ready means: the child still runs, the project's lock names it, and a connection with the
/// password works over the socket (unix) or loopback (Windows); then on unix the socket must exist.
fn wait_ready(
    child: &mut Child,
    data_dir: &Path,
    socket: Option<&Path>,
    port: u16,
    password: &Secret,
    log: &Path,
    budget: Duration,
) -> Result<(), Error> {
    let started = Instant::now();
    let mut pause = Duration::from_millis(20);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Err(Error::StartFailed { status: status.to_string(), log_tail: tail(log) }),
            Ok(None) => {}
            Err(e) => return Err(Error::io("cannot watch the database server", format!("process {}", child.id()), e)),
        }
        let locked = recovery::read_lock(data_dir).is_ok_and(|l| l == Lock::Pid(child.id()));
        let socket_there = socket.is_none_or(is_socket);
        if locked && socket_there && mysql::Conn::new(options(socket, port, password, PROBE_TIMEOUT, PROBE_TIMEOUT)).is_ok() {
            return Ok(());
        }
        if started.elapsed() >= budget {
            if locked && let Some(s) = socket.filter(|s| !is_socket(s)) {
                return Err(Error::SocketMissing { path: s.to_path_buf() });
            }
            return Err(Error::NotReady { waited: budget, log_tail: tail(log) });
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(100));
    }
}

#[cfg(unix)]
fn is_socket(p: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_socket())
}

#[cfg(not(unix))]
fn is_socket(_p: &Path) -> bool {
    false
}

/// Connection options: root with the password, over the socket when there is one, else loopback,
/// never switching transports on its own.
fn options(socket: Option<&Path>, port: u16, password: &Secret, connect: Duration, query: Duration) -> mysql::Opts {
    let b = mysql::OptsBuilder::new()
        .user(Some("root"))
        .pass(Some(password.expose()))
        .prefer_socket(false)
        .tcp_connect_timeout(Some(connect))
        .read_timeout(Some(query))
        .write_timeout(Some(query))
        .max_allowed_packet(Some(crate::config::MAX_PACKET));
    let b = match socket {
        Some(s) => b.socket(Some(s.to_string_lossy())),
        None => b.ip_or_hostname(Some("127.0.0.1")).tcp_port(port),
    };
    b.into()
}

/// The last part of the server's log, for an error message.
fn tail(log: &Path) -> String {
    let Ok(mut f) = std::fs::File::open(log) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(len.saturating_sub(LOG_TAIL)));
    let mut bytes = Vec::new();
    let _ = f.take(LOG_TAIL).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        /// Whether the watchdog of the server this thread last started ran when its record was written.
        pub(super) static WATCHED_AT_RECORD: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }

    #[cfg(unix)]
    thread_local! {
        /// Whether this thread's next start fails right after its server has started, and the
        /// process id of the server this thread last started.
        static FAIL_AFTER_SPAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static SPAWNED: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
    }

    /// Notes the server just started and, once when a test asked for it, fails in the place of
    /// whatever step may fail between a server's start and its `Server`.
    #[cfg(unix)]
    pub(super) fn after_spawn(server: u32) -> Result<(), Error> {
        SPAWNED.set(Some(server));
        if FAIL_AFTER_SPAWN.take() { Err(Error::BadPath { what: "test", path: String::new() }) } else { Ok(()) }
    }

    /// Whether the watchdog of the server `server` runs.
    #[cfg(unix)]
    pub(super) fn watched(server: u32) -> bool {
        let watchdog = format!("archforge-server-watchdog {server}$");
        std::process::Command::new("/usr/bin/pgrep").args(["-f", &watchdog]).stdout(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    /// An Archforge killed between the two leaves a server the start-up sweep finds by its record.
    /// Needs dolt, as the tests in `tests/server.rs` do.
    #[cfg(unix)]
    #[test]
    fn a_server_is_recorded_before_its_watchdog_starts() {
        let dir = crate::home::tests::Scratch::new("order");
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts = ServerOptions { ready_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
        let server = Server::start(&dolt, &dir.0.join("data"), &opts).unwrap();
        assert!(watched(server.pid()), "the watchdog runs once the server has started");
        server.stop().unwrap();
        assert_eq!(WATCHED_AT_RECORD.get(), Some(false));
    }

    /// The failure is put there by the test, since a test cannot make the record's write fail
    /// from outside. The server is stopped and reaped before the start returns, and the folder
    /// opens again. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_that_fails_once_its_server_runs_stops_the_server() {
        let dir = crate::home::tests::Scratch::new("failed-start");
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts = ServerOptions { ready_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
        FAIL_AFTER_SPAWN.set(true);
        let failed = Server::start(&dolt, &dir.0.join("data"), &opts);
        assert!(matches!(failed, Err(Error::BadPath { what: "test", .. })), "{failed:?}");
        let pid = SPAWNED.get().unwrap();
        let left = identity(pid).ok().flatten().is_some_and(|i| i.is_dolt_server());
        if left {
            let _ = std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
        }
        assert!(!left, "the server {pid} ran on after its start had failed");
        assert!(!watched(pid), "and no watchdog is left for it");
        Server::start(&dolt, &dir.0.join("data"), &opts).unwrap().stop().unwrap();
    }

    /// With the record's lock held elsewhere a start fails before it comes to its server, here a
    /// link to dolt that is gone by then: no wait for the lock comes after a server's start.
    #[cfg(unix)]
    #[test]
    fn a_start_that_cannot_have_its_records_lock_starts_no_server() {
        let dir = crate::home::tests::Scratch::new("lock-first");
        let home = crate::Home::prepare(&dir.0.join("home")).unwrap();
        std::os::unix::fs::symlink(Dolt::find(&home).unwrap().path(), dir.0.join("dolt")).unwrap();
        let dolt = Dolt::at(&dir.0.join("dolt"), &home).unwrap();
        std::fs::remove_file(dir.0.join("dolt")).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts = ServerOptions::new(dir.0.join("records"));
        private_records(&opts.records).unwrap();
        let project = dir.0.join("data").canonicalize().unwrap();
        let held = record::hold(&record::path(&opts.records, &project.display().to_string()), record::LOCK_WAIT).unwrap();
        let what = |start: Result<Server, Error>| if let Err(Error::Io { what, .. }) = start { what } else { "another outcome".into() };
        assert_eq!(what(Server::start(&dolt, &project, &opts)), "cannot lock");
        drop(held);
        assert_eq!(what(Server::start(&dolt, &project, &opts)), "cannot start", "with the lock it comes to the server");
    }

    /// A record another start wrote after this one was read stays; the one read goes.
    #[test]
    fn forgetting_keeps_a_record_written_since_it_was_read() {
        let dir = crate::home::tests::Scratch::new("forget");
        let read = record::tests::rec("/p/a");
        let newer = Record { server_pid: 9, ..read.clone() };
        assert!(record::tests::write_new(&dir.0, &newer).unwrap());
        let file = record::path(&dir.0, "/p/a");
        forget(&file, &read, None);
        assert_eq!(record::read(&dir.0, "/p/a").unwrap(), Some(newer.clone()), "the newer record stays");
        forget(&file, &newer, None);
        assert_eq!(record::read(&dir.0, "/p/a").unwrap(), None);
    }
}
