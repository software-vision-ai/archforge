//! Starting, connecting to and stopping the server of one project folder.

use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::identity::{self, identity};
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
        let left = Arc::new(AtomicBool::new(false));
        let launched = launch(dolt, &data_dir, &data_text, &run_root, &run_dir, uid, &owner.started, opts, &left);
        // A server that would not stop may still run on its run folder, which recovery removes
        // with the server's record once it is stopped or gone.
        if launched.is_err() && !left.load(Ordering::Relaxed) {
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
    /// Set when a stop failed: the server may still run. Shared with the start that made it.
    left: Arc<AtomicBool>,
}

impl Started {
    /// The server's process id.
    fn id(&self) -> u32 {
        self.child.id()
    }

    /// Stops the server: a polite signal on unix, then a kill after `budget`. A stop that fails
    /// is noted in `left`, however it was asked for, a drop included.
    fn stop_within(&mut self, budget: Duration) -> Result<(), Error> {
        self.tried = true;
        #[cfg(all(test, unix))]
        tests::at_stop();
        #[cfg(all(test, unix))]
        let stopped = if tests::STOP_FAILS.take() { Err(Error::StillRunning { pid: self.id(), waited: budget }) } else { Ok(()) };
        #[cfg(not(all(test, unix)))]
        let stopped = Ok(());
        let stopped = stopped.and_then(|()| stop_child(&mut self.child, budget));
        // Set, never cleared: another server of the same start that did stop changes nothing.
        if stopped.is_err() {
            self.left.store(true, Ordering::Relaxed);
        }
        stopped
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
// An Archforge killed after its server started and before its record, written before the start,
// names the server's process id (the record's completion: a write to disk) leaves a server with no
// watchdog and a record with no process id: this sweep settles that record as the opening of its
// project does (`settle_pending`), finding the server by the command line recorded.
/// Stops the servers of every project whose record it lists and whose Archforge is gone (crashed,
/// killed or force quit), and forgets the records of servers that are gone: for a start-up, so that
/// such a project does not keep its server running until it is opened again. A server whose
/// Archforge still runs, this one or another, is left alone, and a process is signalled only while
/// its command line, checked before each signal, is exactly the recorded one, whose configuration
/// lies in a run folder with a random name made for that start: a process id another program took
/// gets no signal. A record without a process id, of a start that will not go on, is settled as the
/// opening of its project settles it; one whose lock is held (a start under way) is left to the
/// next start-up or opening, without a wait, and a copy under a name not its project's only goes
/// itself. Then the temporary files of record writes no longer under way go (a writer killed in the
/// middle, or one whose removal failed), each only while its record's lock can be had without a
/// wait. For a start-up: with no start of this process under way. A records folder that cannot be
/// listed in full is swept as far as it was listed, and the log says so: the records not listed,
/// and their servers, are left to a later sweep that lists them or to the opening of their project,
/// and to the opening alone where the failure comes back each time before them. Returns how many
/// servers were stopped.
pub fn stop_orphans(opts: &ServerOptions) -> Result<usize, Error> {
    private_records(&opts.records)?;
    let (_, uid) = run_dir::checked_root(&opts.run_root.clone().unwrap_or_else(|| run_dir::default_root(&opts.records)))?;
    let stopped = sweep(&opts.records, None, uid, Some(opts.stop_budget));
    record::clear_temporaries(&opts.records);
    Ok(stopped)
}

/// Goes through the records [`record::all`] lists, but that of `except`: forgets those whose server
/// is gone and, given a stop budget, stops the servers whose Archforge is gone, as recovery does,
/// then forgets them, and settles the records that name no process, which it leaves alone without
/// a budget. A folder that cannot be listed at all is logged and swept no further. Returns how many
/// it stopped.
fn sweep(records: &Path, except: Option<&str>, uid: Option<u32>, stop: Option<Duration>) -> usize {
    let all = match record::all(records) {
        Ok(a) => a,
        Err(e) => {
            log::warn!("the server records could not be swept: {e}");
            return 0;
        }
    };
    let mut stopped: usize = 0;
    for (file, r) in all.into_iter().filter(|(_, r)| except != Some(r.data_dir.as_str())) {
        let Some(server) = r.server_pid else {
            if let Some(budget) = stop {
                stopped = stopped.saturating_add(sweep_pending(records, &file, &r, budget, uid));
            }
            continue;
        };
        match (classify(server, Some(&r)), stop) {
            (Ok(Proc::Gone | Proc::Other | Proc::OtherDoltServer), _) => forget(&file, &r, uid),
            (Ok(Proc::OursOwnerDead), Some(budget)) => {
                log::info!("stopping the database server {server} that a closed Archforge left running for {}", r.data_dir);
                match stop_foreign(server, &r.command, budget) {
                    Ok(()) => {
                        forget(&file, &r, uid);
                        stopped = stopped.saturating_add(1);
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

/// Settles, for the start-up sweep, the record `r` in `file`, which names no process, as the
/// opening of its project would, but taking its lock without a wait: the application holds its
/// job lock for the whole sweep, so a lock held is a start under way elsewhere (in another
/// Archforge, or the child of a start between its fork and its exec) or another change of that
/// record, which the next start-up or opening settles. That record, one whose server's Archforge
/// still runs and one completed meanwhile are left alone; a failure is logged and leaves the
/// record. A record under a name not its project's is a stray copy: settled, it would be checked
/// under another lock than its start's, and a live start's run folder could go with it. It goes
/// alone, and nothing is signalled. Returns how many servers it stopped.
fn sweep_pending(records: &Path, file: &Path, r: &Record, budget: Duration, uid: Option<u32>) -> usize {
    if *file != record::path(records, &r.data_dir) {
        log::warn!("the server record {} is filed under a name not its project's; only it goes", file.display());
        record::remove_while(file, |found| found == r);
        return 0;
    }
    #[cfg(test)]
    tests::before_settle(file);
    let mut stopped = 0;
    match settle_pending(file, &r.data_dir, r, Duration::ZERO, budget, uid, &mut stopped) {
        Ok(_) | Err(Error::OpenElsewhere { .. }) => {}
        Err(e) => log::warn!("the server record {} stays: {e}", file.display()),
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

/// Recovery for this project: settles a record without a process id, gathers the facts, decides,
/// acts.
fn recover(data_dir: &Path, data_text: &str, opts: &ServerOptions, uid: Option<u32>) -> Result<(), Error> {
    let file = record::path(&opts.records, data_text);
    let mut record = record::read(&opts.records, data_text)?;
    if let Some(seen) = record.clone().filter(|r| r.server_pid.is_none()) {
        record = settle_pending(&file, data_text, &seen, record::LOCK_WAIT, opts.stop_budget, uid, &mut 0)?;
    }
    let lock = recovery::read_lock(data_dir)?;
    let holder = match lock {
        Lock::Pid(pid) => classify(pid, record.as_ref())?,
        _ => Proc::Gone,
    };
    let recorded = match record.as_ref().and_then(|r| r.server_pid.map(|pid| (r, pid))) {
        Some((r, pid)) => Some((pid, classify(pid, Some(r))?)),
        None => None,
    };
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

/// Settles the record `seen`, which names no process: the record of a start under way, or of one
/// that ended before it completed its record. A start holds the record's lock until its record is
/// complete, so while the lock cannot be had the start is under way and the project is refused,
/// naming its Archforge; once it can, that start will never go on. Its server is then looked for
/// by the exact command line recorded, but only where that line names the configuration in the
/// recorded run folder, a folder of this user's made for that start under a random name, which
/// makes the line that start's alone; each process listed is proven again before anything is done
/// about it. One whose Archforge still runs refuses the project, naming the server, and nothing
/// goes; one whose Archforge is gone is stopped, its configuration removed first (a removal that
/// fails fails the settling, and nothing more goes): where a child
/// cut between its fork and its exec does not keep the lock (a lock a file system emulates per
/// process), it then finds no configuration once it runs dolt, and a dolt started without its
/// configuration exits at once (measured with dolt 2.7.3). Only then do the run folder and the
/// record go. Returns the record there now, if it is a complete one, and counts in `stopped` each
/// server it stopped, a later step failing or not. It waits at most `wait` to take the lock (its
/// removals afterwards wait as every change does), and `budget` for a server to stop after each
/// signal.
fn settle_pending(
    file: &Path,
    data_text: &str,
    seen: &Record,
    wait: Duration,
    budget: Duration,
    uid: Option<u32>,
    stopped: &mut usize,
) -> Result<Option<Record>, Error> {
    let held = match record::hold(file, wait) {
        Ok(held) => held,
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::TimedOut => return Err(Error::OpenElsewhere { pid: seen.owner_pid }),
        Err(e) => return Err(e),
    };
    let now = record::look(&held, data_text)?;
    drop(held);
    let Some(r) = now.clone().filter(|r| r.server_pid.is_none()) else { return Ok(now) };
    let owner = owner_alive(&r)?;
    let (root, run) = (Path::new(&r.run_root), Path::new(&r.run_dir));
    let yaml = run.join("server.yaml");
    let tied = run_dir::is_own(root, run, uid) && r.command.iter().rev().take(2).eq([yaml.to_str(), Some("--config")].into_iter().flatten());
    if !tied {
        // A line that is not that start's alone could be any server's, and the folder anyone's:
        // nothing is signalled and no file but the record goes.
        log::warn!("the server record {} names no run folder of its start's; only the record goes", file.display());
        record::remove_while(file, |found| *found == r);
        return Ok(None);
    }
    // A link under that name goes itself, never what it points to. A configuration that stays
    // could still start a server nothing lists yet: the settling fails, and the record stays.
    if !owner
        && let Err(e) = std::fs::remove_file(&yaml)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(Error::io("cannot remove", &yaml, e));
    }
    #[cfg(test)]
    tests::before_list(run);
    #[cfg(test)]
    let listed = tests::listed(identity::running(&r.command)?);
    #[cfg(not(test))]
    let listed = identity::running(&r.command)?;
    for pid in listed {
        if !identity(pid)?.is_some_and(|i| i.is(&r.command)) {
            continue;
        }
        if owner {
            return Err(Error::OpenElsewhere { pid });
        }
        log::info!("stopping the database server {pid} that a start a closed Archforge did not finish left running for {data_text}");
        stop_foreign(pid, &r.command, budget)?;
        *stopped = stopped.saturating_add(1);
    }
    forget(file, &r, uid);
    Ok(None)
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

/// Records the start, writes the configuration, starts the server, completes the record with the
/// server's process id, and waits until the server is ready; a lost race for the port starts it
/// again. The record is written, without a process id, before a port is probed for the server,
/// and its lock is held from before it is written until it is complete: a record that names no
/// process while its lock can be had is that of a start that will never go on. Every step that
/// can fail without the server comes before its start, and a failure there removes the record;
/// what can fail after it (completing the record, the wait) returns through [`Started`], whose
/// drop stops the server, and a failure to complete the record stops the server and removes the
/// record, all still under the lock; never the record of a server that would not stop. `left` says
/// afterwards whether a server was started that would not stop: the caller then leaves the run
/// folder to it.
fn launch(
    dolt: &Dolt,
    data_dir: &Path,
    data_text: &str,
    run_root: &Path,
    run: &Path,
    uid: Option<u32>,
    owner_started: &str,
    opts: &ServerOptions,
    left: &Arc<AtomicBool>,
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
        // It is held until the record names the server. Nothing between the server's start and
        // `drop(held)` may return but through the stop below.
        let held = record::hold(&record::path(&opts.records, data_text), record::LOCK_WAIT)?;
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
        // one gives way before it probes a port or writes the configuration. That record is read
        // without the lock, which setting a damaged one aside would wait for.
        if !record::write_new(&held, &pending)? {
            drop(held);
            let other = record::read(&opts.records, data_text).ok().flatten();
            return Err(Error::OpenElsewhere { pid: other.map_or(0, |r| r.server_pid.unwrap_or(r.owner_pid)) });
        }
        let spawned = free_port().and_then(|port| {
            let settings = config::Settings { port, socket: socket_text, data_dir: data_text, cfg_dir: cfg_text, no_files: no_files_text };
            std::fs::write(&yaml_path, config::server_yaml(&settings)).map_err(|e| Error::io("cannot write", &yaml_path, e))?;
            let log = std::fs::File::create(&log_path).map_err(|e| Error::io("cannot create", &log_path, e))?;
            let log_err = log.try_clone().map_err(|e| Error::io("cannot open", &log_path, e))?;
            let mut env = dolt.env();
            env.push(("DOLT_ROOT_PASSWORD".into(), password.expose().into()));
            env.push(("DOLT_ROOT_HOST".into(), "localhost".into()));
            let mut c = runner::command(dolt.path(), &env);
            c.args(command.iter().skip(1)).current_dir(run).stdout(Stdio::from(log)).stderr(Stdio::from(log_err));
            #[cfg(all(test, unix))]
            tests::at_spawn(&held, data_text);
            let child = runner::spawn(&mut c, false).map_err(|e| Error::io("cannot start", dolt.path(), e))?;
            Ok((port, child))
        });
        let (port, child) = match spawned {
            Ok(spawned) => spawned,
            Err(e) => {
                // No server runs: the record goes, under the lock still held.
                record::remove_held(&held, |r| *r == pending);
                return Err(e);
            }
        };
        // From here to the `Server`, no return leaves the server running: see `Started`.
        let mut child = Started { child, stop_budget: opts.stop_budget, tried: false, left: left.clone() };
        #[cfg(all(test, unix))]
        let completed = tests::after_spawn(child.id()).and_then(|()| record::complete(&held, &pending, child.id()));
        #[cfg(not(all(test, unix)))]
        let completed = record::complete(&held, &pending, child.id());
        if let Err(e) = completed {
            // Still under the lock: no opening takes the record for that of a start that ended
            // while its server is being stopped. The record of a server that would not stop is not
            // removed: recovery finds the server by it.
            if child.stop() {
                record::remove_held(&held, |r| *r == pending);
            }
            drop(held);
            return Err(e);
        }
        drop(held);
        #[cfg(all(test, unix))]
        tests::WATCHED_AT_RECORD.set(Some(tests::watched(child.id())));
        // The complete record comes first: the watchdog may wait for its turn to start, and a
        // server of an Archforge killed meanwhile is one the start-up sweep finds by its record.
        // From here on a killed Archforge takes this server with it. The watchdog ends with this
        // turn of the loop or with the `Server`, in both after the attempt to stop the server.
        let watchdog = Watchdog::tie(child.id(), &command);
        #[cfg(all(test, unix))]
        let before = tests::before_ready();
        #[cfg(not(all(test, unix)))]
        let before = Ok(());
        match before.and_then(|()| wait_ready(&mut child.child, data_dir, socket.as_deref(), port, &password, &log_path, opts.ready_budget)) {
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
        /// Whether the run folder still held its configuration when a pending record's server was
        /// last looked for on this thread.
        static CONFIG_AT_LIST: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }

    thread_local! {
        /// A process a test adds to the next list of processes, as one a list may propose that is
        /// not, or no longer, the server looked for.
        static DECOY: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
        /// What a test does when the start-up sweep next comes to settle a record, before it
        /// takes the record's lock: once, with the record's file.
        static BEFORE_SETTLE: std::cell::RefCell<Option<Box<dyn FnOnce(&Path)>>> = const { std::cell::RefCell::new(None) };
    }

    /// Runs the hook a test set for the sweep's next settling, once, with the record's `file`.
    pub(super) fn before_settle(file: &Path) {
        if let Some(hook) = BEFORE_SETTLE.take() {
            hook(file);
        }
    }

    /// The list `pids`, with the process a test added to it.
    pub(super) fn listed(mut pids: Vec<u32>) -> Vec<u32> {
        pids.extend(DECOY.take());
        pids
    }

    /// Notes whether the run folder `run` still holds its configuration.
    pub(super) fn before_list(run: &Path) {
        CONFIG_AT_LIST.set(Some(run.join("server.yaml").exists()));
    }

    #[cfg(unix)]
    thread_local! {
        /// Whether this thread's next start fails right after its server has started, and the
        /// process id of the server this thread last started.
        static FAIL_AFTER_SPAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        /// Whether this thread's next start fails once its server is recorded, in the place of
        /// the wait until it is ready.
        static FAIL_BEFORE_READY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static SPAWNED: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
        /// Whether this thread's next stop of a server it started fails without trying: a server
        /// that would not stop, which a polite signal, a kill and a wait all but rule out on unix.
        pub(super) static STOP_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        /// The record of the server this thread last started, as it was just before the start.
        static RECORD_AT_SPAWN: std::cell::RefCell<Option<Record>> = const { std::cell::RefCell::new(None) };
        /// A file and what this thread's next start writes into it just before its server starts,
        /// without the record's lock, as a build without that lock would.
        static WRITE_AT_SPAWN: std::cell::RefCell<Option<(PathBuf, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
        /// What a test does when this thread next tries to stop a server it started: once.
        static AT_STOP: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
        /// Whether this thread's next start pauses (see [`pause`]) just before its server starts.
        static PAUSE_AT_SPAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        /// Whether it pauses once its server runs, before its record names the server.
        static PAUSE_AFTER_SPAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// Notes the record under the lock `held` just before the server of the project at
    /// `data_text` starts, then writes what a test asked for, and pauses there once when asked.
    #[cfg(unix)]
    pub(super) fn at_spawn(held: &record::Held, data_text: &str) {
        RECORD_AT_SPAWN.set(record::look(held, data_text).ok().flatten());
        if let Some((file, bytes)) = WRITE_AT_SPAWN.take() {
            let _ = std::fs::write(file, bytes);
        }
        if PAUSE_AT_SPAWN.take() {
            pause("SPAWNING");
        }
    }

    /// Notes the server just started, pauses there once when a test asked for it, and, once when
    /// a test asked for it, fails in the place of completing its record.
    #[cfg(unix)]
    pub(super) fn after_spawn(server: u32) -> Result<(), Error> {
        SPAWNED.set(Some(server));
        if PAUSE_AFTER_SPAWN.take() {
            pause(&format!("SPAWNED {server}"));
        }
        if FAIL_AFTER_SPAWN.take() { Err(Error::BadPath { what: "test", path: String::new() }) } else { Ok(()) }
    }

    /// Ends a line with `what` at once (the test harness may have begun it with the test's name),
    /// then sleeps two minutes, for the test that runs this process to kill it there. A process
    /// nobody killed then goes on with its start.
    #[cfg(unix)]
    fn pause(what: &str) {
        use std::io::Write;
        let mut out = std::io::stdout();
        let _ = writeln!(out, "{what}").and_then(|()| out.flush());
        std::thread::sleep(Duration::from_secs(120));
    }

    /// Runs the hook a test set for the next stop of a server this thread started, once.
    #[cfg(unix)]
    pub(super) fn at_stop() {
        if let Some(hook) = AT_STOP.take() {
            hook();
        }
    }

    /// Fails, once when a test asked for it, in the place of a server that does not get ready.
    #[cfg(unix)]
    pub(super) fn before_ready() -> Result<(), Error> {
        if FAIL_BEFORE_READY.take() { Err(Error::BadPath { what: "test", path: String::new() }) } else { Ok(()) }
    }

    /// Whether the watchdog of the server `server` runs.
    #[cfg(unix)]
    pub(super) fn watched(server: u32) -> bool {
        let watchdog = format!("archforge-server-watchdog {server}$");
        std::process::Command::new("/usr/bin/pgrep").args(["-f", &watchdog]).stdout(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    /// An Archforge killed between the two leaves a server the start-up sweep finds by its record.
    /// Before the server started its record was there, whole but for the process id, which the
    /// record names once the server runs. Needs dolt, as the tests in `tests/server.rs` do.
    #[cfg(unix)]
    #[test]
    fn a_server_is_recorded_before_its_watchdog_starts() {
        let dir = crate::home::tests::Scratch::new("order");
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts = ServerOptions { ready_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
        let server = Server::start(&dolt, &dir.0.join("data"), &opts).unwrap();
        let (pid, complete) = (server.pid(), record::all(&opts.records).unwrap().pop().map(|(_, r)| r));
        assert!(watched(pid), "the watchdog runs once the server has started");
        server.stop().unwrap();
        assert_eq!(WATCHED_AT_RECORD.get(), Some(false));
        let pending = RECORD_AT_SPAWN.take().unwrap();
        assert_eq!((pending.server_pid, complete), (None, Some(Record { server_pid: Some(pid), ..pending })));
    }

    /// The failure is put there by the test, in the place of completing the record, since a test
    /// cannot make that write fail from outside. The server is stopped and reaped before the start
    /// returns, with the record's lock still held and its record still there, which then goes;
    /// and the folder opens again. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_that_fails_once_its_server_runs_stops_the_server() {
        let (dir, root) = (crate::home::tests::Scratch::new("failed-start"), crate::home::tests::Scratch::new("f"));
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts =
            ServerOptions { run_root: Some(root.0.clone()), ready_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
        let project = dir.0.join("data").canonicalize().unwrap().display().to_string();
        let (records, at_stop) = (opts.records.clone(), std::rc::Rc::new(std::cell::Cell::new(None)));
        let seen = at_stop.clone();
        AT_STOP.set(Some(Box::new(move || {
            let held = record::hold(&record::path(&records, &project), Duration::ZERO).is_err();
            seen.set(Some((held, record::read(&records, &project).ok().flatten().map(|r| r.server_pid))));
        })));
        FAIL_AFTER_SPAWN.set(true);
        let failed = Server::start(&dolt, &dir.0.join("data"), &opts);
        assert!(matches!(failed, Err(Error::BadPath { what: "test", .. })), "{failed:?}");
        let (pid, line) = (SPAWNED.get().unwrap(), RECORD_AT_SPAWN.take().map(|r| r.command).unwrap_or_default());
        let left = identity(pid).ok().flatten().is_some_and(|i| i.is(&line));
        if left {
            let _ = std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
        }
        assert!(!left, "the server {pid} ran on after its start had failed");
        assert!(!watched(pid), "and no watchdog is left for it");
        assert_eq!(at_stop.get(), Some((true, Some(None))), "stopped under the lock, its record there and not complete");
        assert_eq!(record::all(&opts.records).unwrap(), Vec::new(), "and its record went");
        Server::start(&dolt, &dir.0.join("data"), &opts).unwrap().stop().unwrap();
    }

    /// Another record was put under the name while a start held its lock, by something that does
    /// not take it (a build without the lock): the start does not complete it, stops its server,
    /// and leaves that record as it is. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_whose_record_was_replaced_stops_its_server_and_leaves_that_record() {
        let (dir, root) = (crate::home::tests::Scratch::new("replaced"), crate::home::tests::Scratch::new("r"));
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts =
            ServerOptions { run_root: Some(root.0.clone()), ready_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
        let other = record::tests::rec(&dir.0.join("data").canonicalize().unwrap().display().to_string());
        WRITE_AT_SPAWN.set(Some((record::path(&opts.records, &other.data_dir), serde_json::to_vec(&other).unwrap())));
        let failed = Server::start(&dolt, &dir.0.join("data"), &opts);
        let (pid, line) = (SPAWNED.get().unwrap(), RECORD_AT_SPAWN.take().map(|r| r.command).unwrap_or_default());
        let left = identity(pid).ok().flatten().is_some_and(|i| i.is(&line));
        if left {
            let _ = std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
        }
        assert!(matches!(&failed, Err(Error::Io { what, .. }) if what == "cannot complete"), "{failed:?}");
        assert!(!left, "the server {pid} ran on after its start had failed");
        assert_eq!(record::read(&opts.records, &other.data_dir).unwrap(), Some(other));
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
        assert_eq!(RECORD_AT_SPAWN.take().map(|r| r.server_pid), Some(None), "its record was written first");
        assert_eq!(record::all(&opts.records).unwrap(), Vec::new(), "and went, as no server started");
    }

    /// Another start recorded itself, completely or not yet, after this one's recovery looked:
    /// this one gives way before it probes a port, writes the configuration or starts a server
    /// (here a link to dolt that is gone by then), and the other's record stays. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_that_finds_a_record_gives_way_before_it_starts_a_server() {
        let (dir, root) = (crate::home::tests::Scratch::new("give-way"), crate::home::tests::Scratch::new("g"));
        let home = crate::Home::prepare(&dir.0.join("home")).unwrap();
        std::os::unix::fs::symlink(Dolt::find(&home).unwrap().path(), dir.0.join("dolt")).unwrap();
        let dolt = Dolt::at(&dir.0.join("dolt"), &home).unwrap();
        std::fs::remove_file(dir.0.join("dolt")).unwrap();
        let opts = ServerOptions::new(dir.0.join("records"));
        private_records(&opts.records).unwrap();
        let (root, uid) = run_dir::checked_root(&root.0).unwrap();
        for (other, named) in [(record::tests::rec("/p/a"), 7), (Record { server_pid: None, ..record::tests::rec("/p/a") }, 8)] {
            assert!(record::tests::write_new(&opts.records, &other).unwrap());
            let run = run_dir::create(&root, uid).unwrap();
            let refused = launch(&dolt, &dir.0, "/p/a", &root, &run, uid, "x", &opts, &Arc::new(AtomicBool::new(false)));
            assert!(matches!(refused, Err(Error::OpenElsewhere { pid }) if pid == named), "{refused:?}");
            assert!(!run.join("server.yaml").exists(), "nothing was written for a server");
            assert_eq!(record::read(&opts.records, "/p/a").unwrap(), Some(other));
            record::remove(&record::path(&opts.records, "/p/a"));
        }
    }

    /// A project whose record would be larger than a record may be, once it names the server, is
    /// refused before anything is written for it or started. Its path, as a start is given it, is
    /// one Linux could hold (macOS cannot make a folder that deep). Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_whose_record_would_be_too_large_starts_nothing() {
        let (dir, root) = (crate::home::tests::Scratch::new("too-large"), crate::home::tests::Scratch::new("t"));
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        let opts = ServerOptions::new(dir.0.join("records"));
        private_records(&opts.records).unwrap();
        let (root, uid) = run_dir::checked_root(&root.0).unwrap();
        let (run, project) = (run_dir::create(&root, uid).unwrap(), format!("/p{}", "/deep".repeat(800)));
        let refused = launch(&dolt, &dir.0, &project, &root, &run, uid, "x", &opts, &Arc::new(AtomicBool::new(false)));
        assert!(matches!(&refused, Err(Error::RecordTooLarge { project: named, .. }) if *named == project), "{refused:?}");
        assert_eq!(SPAWNED.get(), None, "no server was started");
        let names: Vec<_> =
            std::fs::read_dir(&opts.records).unwrap().map(Result::unwrap).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| n.ends_with(".lock")), "no record and no temporary file: {names:?}");
        assert!(!run.join("server.yaml").exists());
        assert!(record::hold(&record::path(&opts.records, &project), Duration::ZERO).is_ok(), "the lock went with the start");
    }

    /// A start fails once its server runs, and the server would not stop: its run folder stays
    /// with its record, since the server may still run on them, and the project is refused as
    /// open elsewhere, naming it, also where the record was not complete yet, by the command line
    /// it recorded. Once it is gone the next start clears both, and a failed start whose server
    /// did stop leaves neither. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_failed_start_keeps_the_run_folder_of_a_server_that_would_not_stop() {
        let (dir, root) = (crate::home::tests::Scratch::new("kept"), crate::home::tests::Scratch::new("k"));
        let dolt = Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
        std::fs::create_dir(dir.0.join("data")).unwrap();
        let opts =
            ServerOptions { run_root: Some(root.0.clone()), ready_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
        // Every start here fails once its server runs and is recorded.
        let start = || {
            FAIL_BEFORE_READY.set(true);
            Server::start(&dolt, &dir.0.join("data"), &opts)
        };
        let root = root.0.canonicalize().unwrap();
        let run_folders = || std::fs::read_dir(&root).map(|d| d.map(Result::unwrap).map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default();
        let records = || record::all(&opts.records).unwrap_or_default();
        // A server this test leaves running on purpose is ended once it holds the project's lock
        // (so that no half-written lock is left), and is gone before anything is asserted.
        let end = |pid: u32| {
            let until = Instant::now() + Duration::from_secs(20);
            while recovery::read_lock(&dir.0.join("data")).ok() != Some(Lock::Pid(pid)) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
            while identity(pid).ok().flatten().is_some() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        // Before its record is complete: only the command line it recorded finds the server, once
        // the server shows it.
        FAIL_AFTER_SPAWN.set(true);
        STOP_FAILS.set(true);
        let unfinished = Server::start(&dolt, &dir.0.join("data"), &opts);
        let (pid, pending, unfinished_kept) = (SPAWNED.get().unwrap(), records(), run_folders());
        let until = Instant::now() + Duration::from_secs(5);
        while !pending.iter().all(|(_, r)| identity(pid).ok().flatten().is_some_and(|i| i.is(&r.command))) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        let (refused, still) = (start(), records());
        end(pid);
        assert!(matches!(unfinished, Err(Error::BadPath { what: "test", .. })), "{unfinished:?}");
        let pending_runs: Vec<_> = pending.iter().map(|(_, r)| (r.server_pid, PathBuf::from(&r.run_dir))).collect();
        assert_eq!((unfinished_kept.len(), pending_runs), (1, vec![(None, unfinished_kept[0].clone())]), "the folder stays with its record");
        // Refused by that record, which stays, not by the project's lock the server holds.
        assert!(matches!(refused, Err(Error::OpenElsewhere { pid: open }) if open == pid), "{refused:?}");
        assert_eq!(still, pending, "nothing went");
        STOP_FAILS.set(true);
        let failed = start();
        let pid = SPAWNED.get().unwrap();
        let (recorded, kept, again) = (records(), run_folders(), start());
        end(pid);
        assert!(matches!(failed, Err(Error::BadPath { what: "test", .. })), "{failed:?}");
        assert!(kept.len() == 1 && kept[0].join("server.yaml").is_file(), "its run folder stays as it was: {kept:?}");
        assert_eq!(recorded.iter().map(|(_, r)| (r.server_pid, PathBuf::from(&r.run_dir))).collect::<Vec<_>>(), [(Some(pid), kept[0].clone())]);
        assert!(matches!(again, Err(Error::OpenElsewhere { pid: open }) if open == pid), "{again:?}");
        assert!(matches!(start(), Err(Error::BadPath { what: "test", .. })));
        assert_eq!((run_folders(), records()), (Vec::new(), Vec::new()), "no folder and no record are left");
    }

    /// A project folder, its records, a run folder with a configuration under `dir`, and the
    /// record a start leaves before its server runs, for a server run as `program` followed by
    /// `--config` and that configuration: of this process as the Archforge (`alive`), or of one
    /// that is gone.
    fn left(dir: &Path, program: &[&str], alive: bool) -> Result<(ServerOptions, PathBuf, Record), Error> {
        let data = dir.join("data");
        std::fs::create_dir_all(&data).map_err(|e| Error::io("cannot create", &data, e))?;
        let data = data.canonicalize().map_err(|e| Error::io("cannot read", &data, e))?;
        let opts = ServerOptions { run_root: Some(dir.to_path_buf()), ..ServerOptions::new(dir.join("records")) };
        private_records(&opts.records)?;
        let (root, uid) = run_dir::checked_root(dir)?;
        let run = run_dir::create(&root, uid)?;
        std::fs::write(run.join("server.yaml"), "").map_err(|e| Error::io("cannot write", &run, e))?;
        let me = identity(std::process::id())?.map(|i| i.started).unwrap_or_default();
        let config = [String::from("--config"), run.join("server.yaml").display().to_string()];
        let r = Record {
            data_dir: data.display().to_string(),
            server_pid: None,
            run_root: root.display().to_string(),
            run_dir: run.display().to_string(),
            command: program.iter().map(|a| a.to_string()).chain(config).collect(),
            owner_pid: std::process::id(),
            owner_started: if alive { me } else { "long ago".into() },
        };
        record::tests::write_new(&opts.records, &r)?;
        Ok((opts, data, r))
    }

    /// A stand-in for a server, waiting on its input, run as `line`: started, and seen by its own
    /// command line once it has one.
    #[cfg(unix)]
    fn stand_in(line: &[String]) -> std::process::Child {
        let child = std::process::Command::new(&line[0]).args(&line[1..]).stdin(Stdio::piped()).spawn().unwrap();
        // Between fork and exec the child still shows this test's command line.
        let until = Instant::now() + Duration::from_secs(5);
        while !identity(child.id()).unwrap().is_some_and(|i| i.is(line)) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }

    /// While its start is under way, which holds the record's lock, a record without a process id
    /// refuses the project, naming that Archforge. The start-up sweep leaves it alone, without a
    /// wait for the lock: a holder that let go once waited for would have it settled.
    #[test]
    fn a_start_under_way_refuses_its_project() {
        let dir = crate::home::tests::Scratch::new("under-way");
        let (opts, data, r) = left(&dir.0, &["dolt"], true).unwrap();
        let held = record::hold(&record::path(&opts.records, &r.data_dir), record::LOCK_WAIT).unwrap();
        let refused = recover_only(&data, &opts);
        assert!(matches!(refused, Err(Error::OpenElsewhere { pid }) if pid == std::process::id()), "{refused:?}");
        record::tests::WAITING.with(|h| *h.borrow_mut() = Some(Box::new(move || drop(held))));
        assert_eq!(stop_orphans(&opts).unwrap(), 0);
        assert!(record::tests::WAITING.with(|h| h.borrow_mut().take()).is_some(), "the sweep did not wait for the lock");
        assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), Some(r.clone()));
        assert!(Path::new(&r.run_dir).join("server.yaml").exists());
    }

    /// The start-up sweep removes the temporary file a record's writer killed in the middle of a
    /// write left, and leaves one whose record's lock is held.
    #[test]
    fn the_start_up_sweep_removes_a_killed_writers_temporary_file() {
        let dir = crate::home::tests::Scratch::new("temporaries");
        let opts = ServerOptions { run_root: Some(dir.0.clone()), ..ServerOptions::new(dir.0.join("records")) };
        private_records(&opts.records).unwrap();
        let (left, busy) = (record::tests::temporary(&opts.records, "/p/a", "abcdefgh"), record::tests::temporary(&opts.records, "/p/b", "abcdefgh"));
        for file in [&left, &busy] {
            std::fs::write(file, "{}").unwrap();
        }
        let held = record::hold(&record::path(&opts.records, "/p/b"), record::LOCK_WAIT).unwrap();
        assert_eq!(stop_orphans(&opts).unwrap(), 0);
        drop(held);
        assert_eq!((left.exists(), busy.exists()), (false, true));
    }

    /// A start that ended before it completed its record: its server, found by the exact command
    /// line recorded and proven, is stopped politely once its Archforge is gone, after its
    /// configuration was removed; while its Archforge runs it refuses the project, naming the
    /// server, and is left alone. A process whose command line differs in one character gets no
    /// signal. Then the record and the run folder go.
    #[cfg(unix)]
    #[test]
    fn the_server_of_an_unfinished_start_is_found_by_its_command_line() {
        use std::os::unix::process::ExitStatusExt;
        let unique = format!("é ü {}", crate::secret::random_name(12).unwrap());
        // A process a list proposed that is not the server: proven not to be, it is passed over.
        let mut decoy = stand_in(&["/bin/sh", "-c", "read x", "decoy"].map(String::from));
        for (alive, other, signal) in [(false, false, Some(15)), (false, true, None), (true, false, None)] {
            let dir = crate::home::tests::Scratch::new("unfinished");
            let (opts, data, r) = left(&dir.0, &["/bin/sh", "-c", "read x", &unique], alive).unwrap();
            let runs: Vec<String> = r.command.iter().map(|a| if other && *a == unique { a.replace('ü', "u") } else { a.clone() }).collect();
            let mut server = stand_in(&runs);
            CONFIG_AT_LIST.set(None);
            DECOY.set(Some(decoy.id()));
            let opened = recover_only(&data, &opts);
            let (record_then, folder_then) = (record::read(&opts.records, &r.data_dir).unwrap(), Path::new(&r.run_dir).exists());
            // Its input closes here, so it ends by itself unless a signal ended it before.
            drop(server.stdin.take());
            assert_eq!(server.wait().unwrap().signal(), signal, "{runs:?}");
            if alive {
                assert!(matches!(opened, Err(Error::OpenElsewhere { pid }) if pid == server.id()), "{opened:?}");
                assert_eq!((record_then, folder_then, CONFIG_AT_LIST.get()), (Some(r.clone()), true, Some(true)), "nothing went");
                DECOY.set(Some(decoy.id()));
                recover_only(&data, &opts).unwrap();
            } else {
                opened.unwrap();
                assert_eq!(CONFIG_AT_LIST.get(), Some(false), "the configuration went before the server was looked for");
            }
            assert_eq!((record::read(&opts.records, &r.data_dir).unwrap(), Path::new(&r.run_dir).exists()), (None, false));
        }
        drop(decoy.stdin.take());
        assert_eq!(decoy.wait().unwrap().signal(), None, "the decoy got no signal");
    }

    /// The configuration of a start whose Archforge is gone cannot be removed (its run folder may
    /// not be written): the opening fails before anything is looked for, and the record, the
    /// folder and its configuration stay, with the server running on unsignalled. A configuration
    /// gone already fails nothing.
    #[cfg(unix)]
    #[test]
    fn a_configuration_that_cannot_be_removed_fails_the_opening() {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::process::ExitStatusExt;
        let dir = crate::home::tests::Scratch::new("kept-config");
        let (opts, data, r) = left(&dir.0, &["/bin/sh", "-c", "read x", &format!("é ü {}", crate::secret::random_name(12).unwrap())], false).unwrap();
        let mut server = stand_in(&r.command);
        let run = Path::new(&r.run_dir);
        std::fs::set_permissions(run, std::fs::Permissions::from_mode(0o500)).unwrap();
        // A user who may write anyway (root) removes it: the case cannot be made then.
        let writable = std::fs::File::create(run.join("probe")).is_ok();
        CONFIG_AT_LIST.set(None);
        let opened = if writable { Ok(()) } else { recover_only(&data, &opts) };
        std::fs::set_permissions(run, std::fs::Permissions::from_mode(0o700)).unwrap();
        drop(server.stdin.take());
        assert_eq!(server.wait().unwrap().signal(), None, "nothing was signalled");
        if !writable {
            assert!(matches!(&opened, Err(Error::Io { what, .. }) if what == "cannot remove"), "{opened:?}");
            assert_eq!(CONFIG_AT_LIST.get(), None, "nothing was looked for");
            assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), Some(r.clone()));
            assert!(run.join("server.yaml").is_file());
        }
        // One that is gone already is no failure: the next opening settles the record.
        std::fs::remove_file(run.join("server.yaml")).unwrap();
        recover_only(&data, &opts).unwrap();
        assert_eq!((record::read(&opts.records, &r.data_dir).unwrap(), run.exists()), (None, false));
    }

    /// A start that completed its record while the opening waited for its lock: the opening goes
    /// by the record under the lock, not by the one it read before.
    #[cfg(unix)]
    #[test]
    fn the_record_settled_is_the_one_under_the_lock() {
        let dir = crate::home::tests::Scratch::new("settled");
        let line = ["/bin/sh", "-c", "read x", &format!("é ü {}", crate::secret::random_name(12).unwrap())].map(String::from).to_vec();
        let mut server = stand_in(&line);
        let (opts, data, r) = left(&dir.0, &["nothing runs this"], true).unwrap();
        let file = record::path(&opts.records, &r.data_dir);
        let held = record::hold(&file, record::LOCK_WAIT).unwrap();
        let done = serde_json::to_vec(&Record { server_pid: Some(server.id()), command: line, ..r }).unwrap();
        record::tests::WAITING.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || {
                let _ = std::fs::write(&file, done);
                drop(held);
            }))
        });
        let opened = recover_only(&data, &opts);
        drop(server.stdin.take());
        let _ = server.wait();
        assert!(matches!(opened, Err(Error::OpenElsewhere { pid }) if pid == server.id()), "{opened:?}");
    }

    /// A record whose server line is not tied to a run folder of this user's, made for that start:
    /// nothing is signalled and no file goes but the record. Here the run
    /// folder is a folder this crate did not make, a link under a run folder's name, or the line
    /// names another configuration.
    #[cfg(unix)]
    #[test]
    fn a_record_not_tied_to_its_run_folder_signals_nothing() {
        use std::os::unix::process::ExitStatusExt;
        let dir = crate::home::tests::Scratch::new("untied");
        let (opts, data, r) = left(&dir.0, &["/bin/sh", "-c", "read x", &format!("é ü {}", crate::secret::random_name(12).unwrap())], false).unwrap();
        let mine = dir.0.join("mine");
        std::fs::create_dir(&mine).unwrap();
        std::fs::write(mine.join("server.yaml"), "").unwrap();
        let link = Path::new(&r.run_root).join("af-linkedto");
        std::os::unix::fs::symlink(&mine, &link).unwrap();
        let config = |run: &Path| [String::from("--config"), run.join("server.yaml").display().to_string()];
        let head = r.command[..4].to_vec();
        let untied = [
            Record { run_dir: mine.display().to_string(), command: head.iter().cloned().chain(config(&mine)).collect(), ..r.clone() },
            Record { run_dir: link.display().to_string(), command: head.iter().cloned().chain(config(&link)).collect(), ..r.clone() },
            Record { command: head.iter().cloned().chain(config(&mine)).collect(), ..r.clone() },
        ];
        for named in untied {
            let mut server = stand_in(&named.command);
            std::fs::write(record::path(&opts.records, &r.data_dir), serde_json::to_vec(&named).unwrap()).unwrap();
            let opened = recover_only(&data, &opts);
            drop(server.stdin.take());
            assert_eq!(server.wait().unwrap().signal(), None, "{:?}", named.command);
            opened.unwrap();
            assert!(mine.join("server.yaml").exists() && Path::new(&r.run_dir).join("server.yaml").exists());
            assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), None);
        }
    }

    /// The start-up sweep settles a record without a process id as an opening does, and the opening
    /// of another project leaves it alone. With its Archforge gone, a server found by the exact
    /// command line recorded is stopped politely, after its configuration was removed, and
    /// counted; a process whose line differs in one character, or that the list proposed and is
    /// not the server, gets no signal; then the record and the run folder go. With its Archforge
    /// running, a server found leaves everything as it was.
    #[cfg(unix)]
    #[test]
    fn the_start_up_sweep_settles_a_record_without_a_process_id() {
        use std::os::unix::process::ExitStatusExt;
        let unique = format!("é ü {}", crate::secret::random_name(12).unwrap());
        let mut decoy = stand_in(&["/bin/sh", "-c", "read x", "decoy"].map(String::from));
        // Its Archforge runs; a server runs, on a line one character off; its signal; the count.
        for (alive, runs, other, signal, count) in
            [(false, true, false, Some(15), 1), (false, true, true, None, 0), (true, true, false, None, 0), (false, false, false, None, 0)]
        {
            let dir = crate::home::tests::Scratch::new("sweep-pending");
            let (opts, _, r) = left(&dir.0, &["/bin/sh", "-c", "read x", &unique], alive).unwrap();
            let kept = || (record::read(&opts.records, &r.data_dir).unwrap(), Path::new(&r.run_dir).join("server.yaml").exists());
            std::fs::create_dir(dir.0.join("other")).unwrap();
            recover_only(&dir.0.join("other"), &opts).unwrap();
            assert_eq!(kept(), (Some(r.clone()), true), "the opening of another project leaves it alone");
            let line: Vec<String> = r.command.iter().map(|a| if other && *a == unique { a.replace('ü', "u") } else { a.clone() }).collect();
            let mut server = runs.then(|| stand_in(&line));
            CONFIG_AT_LIST.set(None);
            DECOY.set(Some(decoy.id()));
            let stopped = stop_orphans(&opts);
            // Its input closes here, so it ends by itself unless a signal ended it before.
            if let Some(s) = server.as_mut() {
                drop(s.stdin.take());
                assert_eq!(s.wait().unwrap().signal(), signal, "{line:?}");
            }
            assert_eq!(stopped.unwrap(), count, "{line:?}");
            let settled = if alive { ((Some(r.clone()), true), true, Some(true)) } else { ((None, false), false, Some(false)) };
            assert_eq!((kept(), Path::new(&r.run_dir).exists(), CONFIG_AT_LIST.get()), settled, "{line:?}");
        }
        drop(decoy.stdin.take());
        assert_eq!(decoy.wait().unwrap().signal(), None, "the decoy got no signal");
    }

    /// A copy of a record without a process id under a name not its project's goes, and nothing
    /// else does: its project's own record, whose lock a start holds (the test here, as a child
    /// between its fork and its exec does), stays with its run folder and configuration, and the
    /// server the copy names gets no signal.
    #[cfg(unix)]
    #[test]
    fn the_sweep_removes_a_stray_copy_of_a_record_alone() {
        use std::os::unix::process::ExitStatusExt;
        let dir = crate::home::tests::Scratch::new("stray");
        let (opts, _, r) = left(&dir.0, &["/bin/sh", "-c", "read x", &format!("é ü {}", crate::secret::random_name(12).unwrap())], false).unwrap();
        let mut server = stand_in(&r.command);
        let (own, copy) = (record::path(&opts.records, &r.data_dir), opts.records.join("0000000000000000.json"));
        std::fs::copy(&own, &copy).unwrap();
        let held = record::hold(&own, record::LOCK_WAIT).unwrap();
        let stopped = stop_orphans(&opts);
        drop(held);
        drop(server.stdin.take());
        assert_eq!(server.wait().unwrap().signal(), None, "nothing was signalled");
        assert_eq!((stopped.unwrap(), copy.exists()), (0, false));
        assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), Some(r.clone()));
        assert!(Path::new(&r.run_dir).join("server.yaml").exists());
    }

    /// A record the sweep cannot settle, its configuration not removable (its run folder may not be
    /// written), stays with that configuration, and the sweep goes on with the next record, which
    /// it settles.
    #[cfg(unix)]
    #[test]
    fn the_sweep_goes_on_past_a_record_it_cannot_settle() {
        use std::os::unix::fs::PermissionsExt;
        let dir = crate::home::tests::Scratch::new("sweep-on");
        let (opts, _, a) = left(&dir.0.join("a"), &["nothing runs this"], false).unwrap();
        let (b_opts, _, b) = left(&dir.0.join("b"), &["nothing runs this"], false).unwrap();
        std::fs::rename(record::path(&b_opts.records, &b.data_dir), record::path(&opts.records, &b.data_dir)).unwrap();
        // The one the sweep comes to first, in the order the folder lists them, is stuck.
        let first = record::all(&opts.records).unwrap().first().map(|(_, r)| r.clone());
        let (stuck, next) = if first.as_ref() == Some(&a) { (a, b) } else { (b, a) };
        let run = Path::new(&stuck.run_dir);
        std::fs::set_permissions(run, std::fs::Permissions::from_mode(0o500)).unwrap();
        // A user who may write anyway (root) removes it: the case cannot be made then.
        let writable = std::fs::File::create(run.join("probe")).is_ok();
        let settled_first = std::rc::Rc::new(std::cell::Cell::new(None));
        let seen = settled_first.clone();
        BEFORE_SETTLE.set(Some(Box::new(move |file: &Path| seen.set(Some(file.to_path_buf())))));
        let stopped = stop_orphans(&opts);
        std::fs::set_permissions(run, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(settled_first.take(), Some(record::path(&opts.records, &stuck.data_dir)), "the stuck record was settled first");
        assert_eq!(stopped.unwrap(), 0);
        if !writable {
            assert_eq!(record::read(&opts.records, &stuck.data_dir).unwrap(), Some(stuck.clone()));
            assert!(run.join("server.yaml").is_file());
        }
        assert_eq!((record::read(&opts.records, &next.data_dir).unwrap(), Path::new(&next.run_dir).exists()), (None, false));
    }

    /// A record completed between the sweep's read and its settling (its start went on meanwhile)
    /// is left as it is, and its server gets no signal.
    #[cfg(unix)]
    #[test]
    fn the_sweep_leaves_a_record_completed_meanwhile() {
        use std::os::unix::process::ExitStatusExt;
        let dir = crate::home::tests::Scratch::new("completed");
        let (opts, _, r) = left(&dir.0, &["/bin/sh", "-c", "read x", &format!("é ü {}", crate::secret::random_name(12).unwrap())], false).unwrap();
        let mut server = stand_in(&r.command);
        let done = Record { server_pid: Some(server.id()), ..r.clone() };
        let (file, bytes) = (record::path(&opts.records, &r.data_dir), serde_json::to_vec(&done).unwrap());
        BEFORE_SETTLE.set(Some(Box::new(move |_: &Path| {
            let _ = std::fs::write(file, bytes);
        })));
        let stopped = stop_orphans(&opts);
        drop(server.stdin.take());
        assert_eq!(server.wait().unwrap().signal(), None, "nothing was signalled");
        assert_eq!(stopped.unwrap(), 0);
        assert_eq!(record::read(&opts.records, &r.data_dir).unwrap(), Some(done));
        assert!(Path::new(&r.run_dir).join("server.yaml").exists());
    }

    /// The variable that makes this test binary the Archforge of [`owner_helper`]: where it pauses
    /// (`at` or `after` its server's spawn), its scratch folder and its run root, one to a line.
    #[cfg(unix)]
    const OWNER: &str = "ARCHFORGE_DOLT_TEST_OWNER";

    /// An Archforge whose start of the project in a scratch folder pauses as [`OWNER`] says, for
    /// a test to kill it there: run as a process of its own by that test. Without that variable it
    /// does nothing.
    #[cfg(unix)]
    #[test]
    #[ignore = "run as a separate process by the test of a killed owner"]
    fn owner_helper() {
        let Some(spec) = std::env::var_os(OWNER) else { return };
        let spec = spec.to_string_lossy().into_owned();
        let [at, base, run_root] = spec.splitn(3, '\n').collect::<Vec<_>>()[..] else { panic!("{spec:?}") };
        let pause = match at {
            "at" => &PAUSE_AT_SPAWN,
            "after" => &PAUSE_AFTER_SPAWN,
            _ => panic!("{spec:?}"),
        };
        let dolt = Dolt::find(&crate::Home::prepare(&Path::new(base).join("home")).unwrap()).unwrap();
        let opts = ServerOptions { run_root: Some(run_root.into()), ..ServerOptions::new(Path::new(base).join("records")) };
        pause.set(true);
        let _ = Server::start(&dolt, &Path::new(base).join("data"), &opts);
    }

    /// The Archforge of [`owner_helper`], killed and reaped when this is dropped; then a server
    /// that runs the command line its start recorded is killed too, each proven by that line just
    /// before its signal.
    #[cfg(unix)]
    struct Owner {
        child: std::process::Child,
        records: PathBuf,
        data_text: String,
        /// The server and its command line, once known.
        server: Option<u32>,
        line: Option<Vec<String>>,
    }

    #[cfg(unix)]
    impl Drop for Owner {
        /// Kills the owner, then the server its start left, if one runs that start's line: the line
        /// known, or else the one its record holds. Waits up to five seconds for each server killed
        /// to be gone, and says on the error output when one is not, or when the list failed.
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let Some(line) = self.line.take().or_else(|| record::read(&self.records, &self.data_text).ok().flatten().map(|r| r.command)) else {
                return;
            };
            let listed = identity::running(&line).unwrap_or_else(|e| {
                eprintln!("the servers running {line:?} could not be listed: {e}");
                Vec::new()
            });
            for pid in listed.into_iter().chain(self.server) {
                let runs = || identity(pid).ok().flatten().is_some_and(|i| i.is(&line));
                if runs() {
                    let _ = std::process::Command::new("/bin/kill").args(["-KILL", &pid.to_string()]).status();
                    let until = Instant::now() + Duration::from_secs(5);
                    while runs() && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    if runs() {
                        eprintln!("the server {pid} still runs after its kill");
                    }
                }
            }
        }
    }

    /// An Archforge killed after its server started and before its record named the server: no
    /// watchdog of its own stops it, and the next start-up finds it by the command line recorded,
    /// removes its configuration before it looks, stops it politely and counts it; its record goes
    /// only once the server is gone, and its run folder goes too. Killed before its server
    /// started, it leaves a record and run folder the start-up removes, with nothing to stop. Its
    /// Archforge is this test binary run again as a process of its own. Needs dolt.
    #[cfg(unix)]
    #[test]
    fn a_start_up_stops_the_server_of_an_archforge_killed_before_its_record_was_complete() {
        use std::io::BufRead;
        for (at, count) in [("after", 1), ("at", 0)] {
            let (dir, root) = (crate::home::tests::Scratch::new("killed-owner"), crate::home::tests::Scratch::new("o"));
            Dolt::find(&crate::Home::prepare(&dir.0.join("home")).unwrap()).unwrap();
            std::fs::create_dir(dir.0.join("data")).unwrap();
            let data = dir.0.join("data").canonicalize().unwrap();
            let opts =
                ServerOptions { run_root: Some(root.0.clone()), stop_budget: Duration::from_secs(20), ..ServerOptions::new(dir.0.join("records")) };
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "server::tests::owner_helper", "--ignored", "--nocapture", "--test-threads=1"]);
            command.env(OWNER, format!("{at}\n{}\n{}", dir.0.display(), root.0.display()));
            // Its error output is this test's: a helper that fails says why there.
            command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::inherit());
            // In its turn, so that it inherits no watchdog's pipe another test starts meanwhile.
            let child = runner::spawn(&mut command, false).unwrap();
            let data_text = data.display().to_string();
            let mut owner = Owner { child, records: opts.records.clone(), data_text: data_text.clone(), server: None, line: None };
            let out = owner.child.stdout.take().unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            // The test harness prints the test's name on the same line before it.
            std::thread::spawn(move || {
                for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(from) = line.find("SPAWN") {
                        let _ = tx.send(line[from..].to_string());
                    }
                }
            });
            let said = rx.recv_timeout(Duration::from_secs(60)).expect("the owner paused in its start");
            owner.server = said.strip_prefix("SPAWNED ").map(|pid| pid.trim().parse().unwrap());
            assert_eq!((said == "SPAWNING", owner.server.is_some()), (at == "at", at == "after"), "{said}");
            let pending = record::read(&opts.records, &data_text).unwrap().unwrap();
            assert_eq!(pending.server_pid, None, "{at}: the record names no process");
            let line = pending.command.clone();
            owner.line = Some(line.clone());
            let runs = |pid: u32| identity(pid).unwrap().is_some_and(|i| i.is(&line));
            let until = Instant::now() + Duration::from_secs(30);
            if let Some(pid) = owner.server {
                let up = || identity(pid).ok().flatten().is_some_and(|i| i.is(&line)) && recovery::read_lock(&data).ok() == Some(Lock::Pid(pid));
                while !up() && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(50));
                }
                assert!(runs(pid), "the server runs the line recorded");
                assert_eq!(recovery::read_lock(&data).unwrap(), Lock::Pid(pid), "and holds its project's lock");
                // Its start would tie one only once its record is complete, which it never comes to.
                // Checked here, before the test ties one of its own below.
                assert!(!watched(pid), "no watchdog runs");
            }
            // Should this test process die from here on, a watchdog of its own stops the server: no
            // later start-up reads this test's records. Dropped, it ends without a signal.
            let _tied = owner.server.map(|pid| Watchdog::tie(pid, &line));
            // SIGKILL: no exit path of the owner runs.
            owner.child.kill().unwrap();
            owner.child.wait().unwrap();
            assert_eq!(record::read(&opts.records, &data_text).unwrap(), Some(pending.clone()), "{at}: the record still names no process");
            assert!(Path::new(&pending.run_dir).join("server.yaml").is_file(), "{at}: its configuration is there");
            if let Some(pid) = owner.server {
                assert!(runs(pid), "the server outlives its owner");
            } else {
                assert_eq!(identity::running(&line).unwrap(), Vec::<u32>::new(), "no server was started");
            }
            CONFIG_AT_LIST.set(None);
            // Whether the server still ran when its record went, in the middle of that change.
            let ran_at_forget = std::rc::Rc::new(std::cell::Cell::new(None));
            let (seen, server, recorded) = (ran_at_forget.clone(), owner.server, line.clone());
            let hook = move |_: &Path| seen.set(Some(server.is_some_and(|pid| identity(pid).unwrap().is_some_and(|i| i.is(&recorded)))));
            record::tests::IN_CHANGE.set(Some(Box::new(hook)));
            let stopped = stop_orphans(&opts);
            assert!(matches!(stopped, Ok(n) if n == count), "{at}: {stopped:?}");
            assert!(owner.server.is_none_or(|pid| !runs(pid)), "the server is gone");
            assert_eq!(CONFIG_AT_LIST.get(), Some(false), "{at}: its configuration went before the server was looked for");
            assert_eq!(ran_at_forget.get(), Some(false), "{at}: its record went once the server was gone");
            let left = (record::read(&opts.records, &data_text).unwrap(), Path::new(&pending.run_dir).exists());
            assert_eq!(left, (None, false), "{at}: the record and the run folder went");
            assert!(!data.join(".dolt").join("sql-server.info").exists(), "{at}: no lock is left, the server stopped politely");
        }
    }

    /// A record another start wrote after this one was read stays; the one read goes.
    #[test]
    fn forgetting_keeps_a_record_written_since_it_was_read() {
        let dir = crate::home::tests::Scratch::new("forget");
        let read = record::tests::rec("/p/a");
        let newer = Record { server_pid: Some(9), ..read.clone() };
        assert!(record::tests::write_new(&dir.0, &newer).unwrap());
        let file = record::path(&dir.0, "/p/a");
        forget(&file, &read, None);
        assert_eq!(record::read(&dir.0, "/p/a").unwrap(), Some(newer.clone()), "the newer record stays");
        forget(&file, &newer, None);
        assert_eq!(record::read(&dir.0, "/p/a").unwrap(), None);
    }
}
