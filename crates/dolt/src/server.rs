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

/// Whether the Archforge that wrote `r` still runs: its process id, started when the record says.
fn owner_alive(r: &Record) -> Result<bool, Error> {
    Ok(identity(r.owner_pid)?.is_some_and(|o| o.started == r.owner_started))
}

/// What the process `pid` is, judged against `record`.
fn classify(pid: u32, record: Option<&Record>) -> Result<Proc, Error> {
    let Some(id) = identity(pid)? else { return Ok(Proc::Gone) };
    if let Some(r) = record
        && id.is(&r.command)
    {
        return Ok(if owner_alive(r)? { Proc::OursOwnerAlive } else { Proc::OursOwnerDead });
    }
    Ok(if id.is_dolt_server() { Proc::OtherDoltServer } else { Proc::Other })
}

// Why a sweep at start-up besides the tie from a server to its Archforge (AR-422, AR-424): the tie is
// a watchdog process, which exists on unix only, can be killed with its Archforge, and may fail to
// start. Linux's parent-death signal would not do instead: it follows the thread that started the
// server, and servers start on worker threads that end once a project is open. Windows' job objects
// kill without a polite stop and need unsafe calls. So this sweep stays the backstop everywhere.
// What neither covers yet: an Archforge killed after its server started and before the record
// names the server's process id. The record written before the start is there, but this sweep
// leaves a record without a process id alone; the opening of its project deals with it (`recover`).
/// Stops the servers of every project whose Archforge is gone (crashed, killed or force quit), and
/// forgets the records of servers that are gone: for a start-up, so a project never opened again
/// does not keep its server running. A server whose Archforge still runs, this one or another, is
/// left alone, and a process is signalled only while its command line, checked before each signal,
/// is exactly the recorded one, whose configuration lies in a run folder with a random name made
/// for that start: a process id another program took gets no signal. A record without a process
/// id, of a start under way or of one its Archforge was killed in, is left as it is. Returns how
/// many servers were stopped.
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
        // No process id: the record names no process to judge, so nothing is touched.
        let Some(server) = r.server_pid else { continue };
        match (classify(server, Some(&r)), stop) {
            (Ok(Proc::Gone | Proc::Other | Proc::OtherDoltServer), _) => forget(&file, &r, uid),
            (Ok(Proc::OursOwnerDead), Some(budget)) => {
                log::info!("stopping the database server {server} that a closed Archforge left running for {}", r.data_dir);
                match stop_foreign(server, &r.command, budget) {
                    Ok(()) => {
                        forget(&file, &r, uid);
                        stopped += 1;
                    }
                    Err(e) => log::warn!("the database server {server} left running for {} stays: {e}", r.data_dir),
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

/// Recovery for this project: gathers the facts, decides, acts. A record without a process id
/// counts as a server of a living owner while the Archforge that wrote it runs (a start under way,
/// refused naming that Archforge) and as a server that is gone once it does not: the record and
/// its run folder go, unless the lock names the server that start left, which is stopped first.
fn recover(data_dir: &Path, data_text: &str, opts: &ServerOptions, uid: Option<u32>) -> Result<(), Error> {
    let record = record::read(&opts.records, data_text)?;
    let lock = recovery::read_lock(data_dir)?;
    let holder = match lock {
        Lock::Pid(pid) => classify(pid, record.as_ref())?,
        _ => Proc::Gone,
    };
    let recorded = match &record {
        Some(r) => Some(match r.server_pid {
            Some(pid) => (pid, classify(pid, Some(r))?),
            None if owner_alive(r)? => (r.owner_pid, Proc::OursOwnerAlive),
            None => (r.owner_pid, Proc::Gone),
        }),
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

/// Writes the configuration, records the start, starts the server, completes the record with the
/// server's process id, and waits until it is ready; a lost race for the port starts it again.
/// Every step that can fail without the server comes before its start; what can fail after it
/// (completing the record, the wait) returns through [`Started`], whose drop stops the server. A
/// start that fails takes its record along, unless its server would not stop.
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
        let pending = Record {
            data_dir: data_text.to_string(),
            server_pid: None,
            run_root: run_root_text.clone(),
            run_dir: run_text.clone(),
            command: command.clone(),
            owner_pid: std::process::id(),
            owner_started: owner_started.to_string(),
        };
        // Another start may have recorded itself since recovery looked: its record stays, and this
        // one gives way before it has started a server.
        if !record::write_new(&opts.records, &pending)? {
            let other = record::read(&opts.records, data_text).ok().flatten();
            return Err(Error::OpenElsewhere { pid: other.map_or(0, |r| r.server_pid.unwrap_or(r.owner_pid)) });
        }
        #[cfg(all(test, unix))]
        tests::RECORD_AT_SPAWN.set(record::read(&opts.records, data_text).ok().flatten());
        let forget_pending = || record::remove_while(&record::path(&opts.records, data_text), |r| r == &pending);
        let child = runner::spawn(&mut c, false).map_err(|e| {
            forget_pending();
            Error::io("cannot start", dolt.path(), e)
        })?;
        // From here to the `Server`, no return leaves the server running: see `Started`.
        let mut child = Started { child, stop_budget: opts.stop_budget, tried: false };
        let rec = Record { server_pid: Some(child.id()), ..pending.clone() };
        #[cfg(all(test, unix))]
        let completed = tests::after_spawn(child.id()).and_then(|()| record::complete(&opts.records, &rec));
        #[cfg(not(all(test, unix)))]
        let completed = record::complete(&opts.records, &rec);
        if let Err(e) = completed {
            // A server that would not stop keeps its record, so recovery can still find it.
            if child.stop() {
                forget_pending();
            }
            return Err(e);
        }
        #[cfg(all(test, unix))]
        tests::WATCHED_AT_RECORD.set(Some(tests::watched(child.id())));
        // The complete record comes first: the watchdog may wait for its turn to start, and a server
        // of an Archforge killed meanwhile is one the start-up sweep finds by its record. From here on a
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
                if child.stop() {
                    record::remove_if(&opts.records, data_text, pid);
                }
                let lost_port = matches!(e, Error::StartFailed { .. }) && TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err();
                if lost_port && attempt < PORT_RETRIES {
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
        /// The project's record as it was just before this thread last started a server.
        pub(super) static RECORD_AT_SPAWN: std::cell::RefCell<Option<Record>> = const { std::cell::RefCell::new(None) };
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
        let server_pid = server.pid();
        assert!(watched(server_pid), "the watchdog runs once the server has started");
        let record = record::all(&opts.records).unwrap().pop().map(|(_, r)| r);
        server.stop().unwrap();
        assert_eq!(WATCHED_AT_RECORD.get(), Some(false));
        // Before the server started the record was there, whole but for the process id.
        let at_spawn = RECORD_AT_SPAWN.take().unwrap();
        assert_eq!(at_spawn.server_pid, None);
        assert_eq!(record, Some(Record { server_pid: Some(server_pid), ..at_spawn }));
    }

    /// A start whose server cannot be started takes its record along: the next start is not
    /// refused as open elsewhere. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_whose_server_cannot_start_leaves_no_record() {
        let dir = crate::home::tests::Scratch::new("no-spawn");
        let home = crate::Home::prepare(&dir.0.join("home")).unwrap();
        // A link to dolt that is gone by the time the server is to start.
        let link = dir.0.join("dolt");
        std::os::unix::fs::symlink(Dolt::find(&home).unwrap().path(), &link).unwrap();
        let dolt = Dolt::at(&link, &home).unwrap();
        std::fs::remove_file(&link).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts = ServerOptions::new(dir.0.join("records"));
        for _ in 0..2 {
            let failed = Server::start(&dolt, &dir.0.join("data"), &opts);
            assert!(matches!(failed, Err(Error::Io { .. })), "{failed:?}");
        }
        assert_eq!(record::all(&opts.records).unwrap(), Vec::new());
    }

    /// Another start recorded itself after this one's recovery looked: this one gives way before
    /// it starts a server, and the other's record stays. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_that_finds_a_record_gives_way_before_it_starts_a_server() {
        let dir = crate::home::tests::Scratch::new("give-way");
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        let opts = ServerOptions::new(dir.0.join("records"));
        private_records(&opts.records).unwrap();
        let (root, uid) = run_dir::checked_root(&std::env::temp_dir()).unwrap();
        let run = run_dir::create(&root, uid).unwrap();
        let other = record::tests::rec("/p/a");
        assert!(record::write_new(&opts.records, &other).unwrap());
        SPAWNED.set(None);
        let refused = launch(&dolt, &dir.0, "/p/a", &root, &run, uid, "x", &opts);
        assert!(run_dir::remove(&root, &run, uid));
        assert!(matches!(refused, Err(Error::OpenElsewhere { pid: 7 })), "{refused:?}");
        assert_eq!(SPAWNED.get(), None, "no server was started");
        assert_eq!(record::read(&opts.records, "/p/a").unwrap(), Some(other));
    }

    /// A project folder, its records and a run folder under `dir`, with the record a start leaves
    /// before its server runs: of this process as the Archforge (`alive`), or of one that is gone.
    fn left(dir: &Path, command: &[String], alive: bool) -> Result<(ServerOptions, PathBuf, Record), Error> {
        let data = dir.join("data");
        std::fs::create_dir_all(&data).map_err(|e| Error::io("cannot create", &data, e))?;
        let data = data.canonicalize().map_err(|e| Error::io("cannot read", &data, e))?;
        let opts = ServerOptions { run_root: Some(dir.to_path_buf()), ..ServerOptions::new(dir.join("records")) };
        private_records(&opts.records)?;
        let (root, uid) = run_dir::checked_root(dir)?;
        let me = identity(std::process::id())?.map(|i| i.started).unwrap_or_default();
        let r = Record {
            data_dir: data.display().to_string(),
            server_pid: None,
            run_root: root.display().to_string(),
            run_dir: run_dir::create(&root, uid)?.display().to_string(),
            command: command.to_vec(),
            owner_pid: std::process::id(),
            owner_started: if alive { me } else { "long ago".into() },
        };
        record::remove(&record::path(&opts.records, &r.data_dir));
        record::write_new(&opts.records, &r)?;
        Ok((opts, data, r))
    }

    /// While its Archforge runs, such a record is a start under way: the project is refused,
    /// naming that Archforge, and nothing goes. Once it is gone the start-up sweep still leaves
    /// the record and the run folder, and the project's opening removes both.
    #[test]
    fn a_record_without_a_process_id_is_left_until_its_projects_opening() {
        let dir = crate::home::tests::Scratch::new("pending");
        for alive in [true, false] {
            let (opts, data, r) = left(&dir.0, &["dolt".to_string()], alive).unwrap();
            assert_eq!(stop_orphans(&opts).unwrap(), 0);
            assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), Some(r.clone()), "the sweep leaves it");
            assert!(Path::new(&r.run_dir).is_dir());
            let opened = recover_only(&data, &opts);
            if alive {
                assert!(matches!(opened, Err(Error::OpenElsewhere { pid }) if pid == std::process::id()), "{opened:?}");
                assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), Some(r.clone()));
                assert!(Path::new(&r.run_dir).is_dir());
            } else {
                opened.unwrap();
                assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), None);
                assert!(!Path::new(&r.run_dir).exists());
            }
        }
    }

    /// The server a killed start left is known by the project's lock and its exact command line:
    /// it is stopped politely. A lock holder whose command line differs in one character gets no
    /// signal. Either way the record and the run folder go.
    #[cfg(unix)]
    #[test]
    fn the_lock_holder_a_killed_start_left_is_stopped_and_no_other_process() {
        use std::os::unix::process::ExitStatusExt;
        let line = |last: &str| ["/bin/sh", "-c", "read x", last].map(String::from).to_vec();
        for (runs, signal) in [("é ü", Some(15)), ("é u", None)] {
            let dir = crate::home::tests::Scratch::new("pending-lock");
            let mut holder = std::process::Command::new("/bin/sh").args(&line(runs)[1..]).stdin(Stdio::piped()).spawn().unwrap();
            // Between fork and exec the child still shows this test's command line.
            let until = Instant::now() + Duration::from_secs(5);
            while !identity(holder.id()).unwrap().unwrap().is(&line(runs)) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
            let (opts, data, r) = left(&dir.0, &line("é ü"), false).unwrap();
            std::fs::create_dir(data.join(".dolt")).unwrap();
            std::fs::write(data.join(".dolt/sql-server.info"), format!("{}:3306:u", holder.id())).unwrap();
            let opened = recover_only(&data, &opts);
            // Its input closes here, so it ends by itself unless a signal ended it before.
            drop(holder.stdin.take());
            assert_eq!(holder.wait().unwrap().signal(), signal, "{runs}");
            opened.unwrap();
            assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), None);
            assert!(!Path::new(&r.run_dir).exists());
        }
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

    /// A record another start wrote after this one was read stays; the one read goes.
    #[test]
    fn forgetting_keeps_a_record_written_since_it_was_read() {
        let dir = crate::home::tests::Scratch::new("forget");
        let read = record::tests::rec("/p/a");
        let newer = Record { server_pid: Some(9), ..read.clone() };
        assert!(record::write_new(&dir.0, &newer).unwrap());
        let file = record::path(&dir.0, "/p/a");
        forget(&file, &read, None);
        assert_eq!(record::read(&dir.0, "/p/a").unwrap(), Some(newer.clone()), "the newer record stays");
        forget(&file, &newer, None);
        assert_eq!(record::read(&dir.0, "/p/a").unwrap(), None);
    }
}
