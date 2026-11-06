//! Running a runtime: a cleared environment, private folders, its own process group, piped streams,
//! and a supervisor that stops a turn, shuts the runtime down in steps and kills its whole group
//! when dropped.

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use archforge_secrets::Secret;
use zeroize::Zeroize;

use crate::discover::is_locale_var;
use crate::output::{OutputEvent, Tail, read_lines, read_tail};
use crate::{AgentError, ConversationFolder, Runtime, RuntimeKind, sys};

/// The longest grace period a shutdown accepts for each of its steps.
pub const MAX_GRACE: Duration = Duration::from_secs(60);
/// How long the supervisor waits between looks at a runtime that has not ended yet.
const POLL: Duration = Duration::from_millis(10);
/// The longest environment variable name accepted, in bytes.
const MAX_VAR_NAME_BYTES: usize = 128;
/// How often a start is tried again while the system says the program file is busy (being written
/// by an installer, or held open by a process starting at the same moment).
const BUSY_RETRIES: u32 = 5;

/// The caps on what a runtime's input and output may hold in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The longest line kept, in bytes: a longer line of output is dropped and reported, a longer
    /// line of input refused.
    pub max_line_bytes: usize,
    /// The most of the error stream kept, its last bytes.
    pub stderr_tail_bytes: usize,
    /// The most lines waiting in each direction: output not read yet (the reader, and then the
    /// runtime, wait) and input not written yet (a further line is refused).
    pub queued_lines: usize,
}

impl Limits {
    /// The most `max_line_bytes` accepts.
    pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;
    /// The most `stderr_tail_bytes` accepts.
    pub const MAX_STDERR_TAIL_BYTES: usize = 1024 * 1024;
    /// The most `queued_lines` accepts.
    pub const MAX_QUEUED_LINES: usize = 4096;
    /// The most one direction may hold: `max_line_bytes` times `queued_lines` and the line being
    /// read or written.
    pub const MAX_BUFFERED_BYTES: usize = 256 * 1024 * 1024;

    /// Refuses a limit of zero or past its maximum, and a pair of line limits whose product passes
    /// [`Limits::MAX_BUFFERED_BYTES`].
    pub fn check(&self) -> Result<(), AgentError> {
        let check = |limit: &'static str, value: usize, max: usize| {
            if value == 0 || value > max {
                Err(AgentError::InvalidLimit { limit, value: u128::try_from(value).unwrap_or(u128::MAX), accepted: format!("1 to {max}") })
            } else {
                Ok(())
            }
        };
        check("max_line_bytes", self.max_line_bytes, Limits::MAX_LINE_BYTES)?;
        check("stderr_tail_bytes", self.stderr_tail_bytes, Limits::MAX_STDERR_TAIL_BYTES)?;
        check("queued_lines", self.queued_lines, Limits::MAX_QUEUED_LINES)?;
        let buffered = self.max_line_bytes.saturating_mul(self.queued_lines.saturating_add(1));
        if buffered > Limits::MAX_BUFFERED_BYTES {
            return Err(AgentError::InvalidLimit {
                limit: "max_line_bytes times (queued_lines + 1)",
                value: u128::try_from(buffered).unwrap_or(u128::MAX),
                accepted: format!("at most {}", Limits::MAX_BUFFERED_BYTES),
            });
        }
        Ok(())
    }
}

impl Default for Limits {
    /// 8 MiB a line (a tool result can be large), 16 KiB of error tail, 16 lines queued: at most
    /// 136 MiB held in each direction.
    fn default() -> Limits {
        Limits { max_line_bytes: 8 * 1024 * 1024, stderr_tail_bytes: 16 * 1024, queued_lines: 16 }
    }
}

/// What a runtime is started with, beyond what this crate gives every runtime.
#[derive(Debug, Default)]
pub struct SpawnRequest {
    /// The command line after the program. Never a secret: it is visible to every process.
    pub args: Vec<OsString>,
    /// Plain environment variables. Never a secret: values may appear in `Debug`.
    pub vars: Vec<(String, OsString)>,
    /// Secret environment variables, such as `ANTHROPIC_API_KEY`: the one way a key reaches a
    /// runtime. They are also kept to scrub the error stream's tail, and overwritten when the
    /// process is dropped.
    pub secret_vars: Vec<(String, Secret)>,
    /// The caps on the runtime's output.
    pub limits: Limits,
}

/// How a runtime ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    /// The exit code, when it exited.
    pub code: Option<i32>,
    /// The signal that ended it, when one did.
    pub signal: Option<i32>,
}

impl Exit {
    /// Whether it exited with code 0.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

impl fmt::Display for Exit {
    /// Writes "exit code N" or "signal N".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.code, self.signal) {
            (Some(code), _) => write!(f, "exit code {code}"),
            (None, Some(signal)) => write!(f, "signal {signal}"),
            (None, None) => f.write_str("an unknown status"),
        }
    }
}

/// Starts `runtime` for the conversation whose folder is `folder`.
///
/// The environment is cleared and holds only: `PATH` (the runtime's [`Runtime::child_path`]),
/// the host's `LANG` and `LC_*`, `HOME` and `TMPDIR` in the conversation's folder, the runtime's
/// settings variable (`CLAUDE_CONFIG_DIR` or `CODEX_HOME`) at its private configuration folder,
/// and the request's variables. The working folder is the conversation's work folder; standard
/// input, output and error are pipes; the runtime leads a new process group.
///
/// Refused before anything starts: a limit out of range, a malformed or duplicated variable name,
/// a name this crate sets itself, a NUL in a value, and a secret's value in the command line or in
/// a plain variable.
pub fn spawn(runtime: &Runtime, folder: &ConversationFolder, request: SpawnRequest) -> Result<RuntimeProcess, AgentError> {
    if !sys::AVAILABLE {
        return Err(AgentError::NotAvailable);
    }
    let SpawnRequest { args, vars, secret_vars, limits } = request;
    limits.check()?;
    let mut names: Vec<&str> = Vec::new();
    for name in vars.iter().map(|(n, _)| n.as_str()).chain(secret_vars.iter().map(|(n, _)| n.as_str())) {
        check_name(name)?;
        if names.contains(&name) {
            return Err(AgentError::InvalidVariable { name: Some(name.to_owned()), bytes: name.len(), reason: "it is given twice".to_owned() });
        }
        names.push(name);
    }
    for (name, value) in &vars {
        if value.as_encoded_bytes().contains(&0) {
            return Err(AgentError::InvalidVariable { name: Some(name.clone()), bytes: name.len(), reason: "its value holds a NUL byte".to_owned() });
        }
    }
    for (name, secret) in &secret_vars {
        let value = secret.expose();
        if value.contains('\0') {
            return Err(AgentError::InvalidVariable { name: Some(name.clone()), bytes: name.len(), reason: "its value holds a NUL byte".to_owned() });
        }
        let appears_in = |text: &OsString| contains_bytes(text.as_encoded_bytes(), value.as_bytes());
        if args.iter().any(appears_in) {
            return Err(AgentError::SecretExposed { variable: name.clone(), place: "command line" });
        }
        if vars.iter().any(|(_, v)| appears_in(v)) {
            return Err(AgentError::SecretExposed { variable: name.clone(), place: "plain variables" });
        }
    }
    let kind = runtime.kind();
    let path = runtime.path();
    match std::fs::metadata(path) {
        Ok(meta) if sys::is_executable(&meta) => {}
        _ => {
            return Err(AgentError::Spawn {
                runtime: kind,
                path: path.to_path_buf(),
                detail: "it is no longer an executable file".to_owned(),
                not_found: true,
            });
        }
    }

    let mut command = Command::new(path);
    command
        .args(&args)
        .env_clear()
        .env("PATH", runtime.child_path())
        .envs(runtime.locale().iter().map(|(k, v)| (k, v)))
        .env("HOME", folder.home())
        .env("TMPDIR", folder.tmp())
        .env(kind.config_var(), folder.config())
        .envs(vars.iter().map(|(k, v)| (k, v)))
        .envs(secret_vars.iter().map(|(k, s)| (k, s.expose())))
        .current_dir(folder.work())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    sys::own_group(&mut command);
    let spawned = spawn_retrying(&mut command);
    // The command holds copies of the environment, keys among them: gone before anything else.
    drop(command);
    let mut child = spawned.map_err(|e| AgentError::Spawn {
        runtime: kind,
        path: path.to_path_buf(),
        detail: e.to_string(),
        not_found: matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied),
    })?;

    let thread_failed = |child: &mut Child, e: io::Error| {
        sys::kill_group(child.id());
        let _ = child.wait();
        AgentError::Spawn {
            runtime: kind,
            path: path.to_path_buf(),
            detail: format!("no thread could be started to supervise it: {e}"),
            not_found: false,
        }
    };
    let (tx, events) = sync_channel(limits.queued_lines);
    if let Some(out) = child.stdout.take() {
        let max = limits.max_line_bytes;
        start_thread(format!("{kind}-stdout"), move || read_lines(out, max, kind, tx)).map_err(|e| thread_failed(&mut child, e))?;
    }
    let tail = Arc::new(Mutex::new(Tail::new(limits.stderr_tail_bytes)));
    if let Some(err) = child.stderr.take() {
        let tail = Arc::clone(&tail);
        start_thread(format!("{kind}-stderr"), move || read_tail(err, tail)).map_err(|e| thread_failed(&mut child, e))?;
    }
    let input_failed = Arc::new(Mutex::new(None));
    let mut input = None;
    if let Some(stdin) = child.stdin.take() {
        let (lines, queue) = sync_channel(limits.queued_lines);
        let failed = Arc::clone(&input_failed);
        start_thread(format!("{kind}-stdin"), move || write_lines(stdin, queue, failed)).map_err(|e| thread_failed(&mut child, e))?;
        input = Some(lines);
    }
    let scrub = secret_vars.into_iter().map(|(_, s)| s).collect();
    Ok(RuntimeProcess { kind, input, input_failed, max_line_bytes: limits.max_line_bytes, events, tail, exit: None, scrub, child })
}

/// Starts a named thread running `work`; the system's refusal is an error, never a panic.
pub(crate) fn start_thread(name: String, work: impl FnOnce() + Send + 'static) -> io::Result<()> {
    std::thread::Builder::new().name(name).spawn(work).map(|_| ())
}

/// Writes each queued line to the runtime's input, overwriting it once written. The first failure is
/// kept in `failed` and ends the thread; the lines still queued are overwritten and dropped. When
/// the queue closes, the input closes.
fn write_lines(mut stdin: ChildStdin, queue: Receiver<Vec<u8>>, failed: Arc<Mutex<Option<String>>>) {
    while let Ok(mut line) = queue.recv() {
        let written = stdin.write_all(&line).and_then(|()| stdin.flush());
        line.zeroize();
        if let Err(e) = written {
            *failed.lock().unwrap_or_else(PoisonError::into_inner) = Some(e.to_string());
            drop(stdin);
            for mut left in queue.try_iter() {
                left.zeroize();
            }
            return;
        }
    }
}

/// Refuses an environment variable name that is malformed, a locale variable, or one this crate
/// sets itself.
fn check_name(name: &str) -> Result<(), AgentError> {
    let malformed = |reason: &str| AgentError::InvalidVariable { name: None, bytes: name.len(), reason: reason.to_owned() };
    if name.is_empty() || name.len() > MAX_VAR_NAME_BYTES {
        return Err(malformed(&format!("a name is 1 to {MAX_VAR_NAME_BYTES} bytes")));
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(malformed("a name is ASCII letters, digits and underscores, not starting with a digit"));
    }
    let reserved = ["PATH", "HOME", "TMPDIR", RuntimeKind::Claude.config_var(), RuntimeKind::Codex.config_var()];
    if reserved.contains(&name) || is_locale_var(name) {
        return Err(AgentError::InvalidVariable {
            name: Some(name.to_owned()),
            bytes: name.len(),
            reason: "Archforge sets it itself (PATH, HOME, TMPDIR, the settings folder and the locale come from the conversation and the host)"
                .to_owned(),
        });
    }
    Ok(())
}

/// Whether `needle` occurs in `haystack`.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Starts `command`, trying again a few times while the system says the program file is busy.
pub(crate) fn spawn_retrying(command: &mut Command) -> io::Result<Child> {
    let mut tries = 0;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == io::ErrorKind::ExecutableFileBusy && tries < BUSY_RETRIES => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// A running runtime and what supervises it. Dropping it kills the runtime's whole process group
/// and collects the runtime, so nothing it started outlives it.
pub struct RuntimeProcess {
    kind: RuntimeKind,
    child: Child,
    input: Option<SyncSender<Vec<u8>>>,
    input_failed: Arc<Mutex<Option<String>>>,
    max_line_bytes: usize,
    events: Receiver<OutputEvent>,
    tail: Arc<Mutex<Tail>>,
    exit: Option<Exit>,
    scrub: Vec<Secret>,
}

impl fmt::Debug for RuntimeProcess {
    /// Writes the runtime, its process id and state; never the environment or a secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeProcess")
            .field("kind", &self.kind)
            .field("pid", &self.child.id())
            .field("input_open", &self.input.is_some())
            .field("exit", &self.exit)
            .field("secrets", &self.scrub.len())
            .finish()
    }
}

impl RuntimeProcess {
    /// Which runtime this is.
    pub fn kind(&self) -> RuntimeKind {
        self.kind
    }

    /// The runtime's process id, which is also its process group's id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Queues `line` and a line break for the runtime's input, which a thread of its own writes, so
    /// this never waits: reading output while the runtime is busy reading input cannot deadlock.
    /// Refused: a line holding a line break or longer than `max_line_bytes`, a full queue
    /// ([`AgentError::InputBacklog`], the runtime is not reading), a closed input, and any line after
    /// a write failed. A queued line is overwritten once written.
    pub fn send_line(&mut self, line: &str) -> Result<(), AgentError> {
        let kind = self.kind;
        if line.contains('\n') {
            return Err(AgentError::InvalidLine { runtime: kind, bytes: line.len(), reason: "it holds a line break; one message is one line" });
        }
        if line.len() >= self.max_line_bytes {
            return Err(AgentError::InvalidLine { runtime: kind, bytes: line.len(), reason: "it is longer than the line limit" });
        }
        if let Some(detail) = self.input_failed.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            return Err(AgentError::Io { runtime: kind, action: "write a line to", detail });
        }
        let queue = self.input.as_ref().ok_or(AgentError::InputClosed { runtime: kind })?;
        let mut bytes = Vec::with_capacity(line.len().saturating_add(1));
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
        match queue.try_send(bytes) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(mut bytes)) => {
                bytes.zeroize();
                Err(AgentError::InputBacklog { runtime: kind })
            }
            Err(TrySendError::Disconnected(mut bytes)) => {
                bytes.zeroize();
                match self.input_failed.lock().unwrap_or_else(PoisonError::into_inner).clone() {
                    Some(detail) => Err(AgentError::Io { runtime: kind, action: "write a line to", detail }),
                    None => Err(AgentError::InputClosed { runtime: kind }),
                }
            }
        }
    }

    /// Closes the runtime's input once the lines already queued are written, which asks both
    /// runtimes to finish and exit.
    pub fn close_input(&mut self) {
        self.input = None;
    }

    /// The next output event, waiting at most `timeout`; `None` when nothing came in time. After the
    /// output closed, every call answers [`OutputEvent::End`].
    pub fn next_event(&self, timeout: Duration) -> Option<OutputEvent> {
        match self.events.recv_timeout(timeout) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => Some(OutputEvent::End),
        }
    }

    /// The next output event if one is waiting.
    pub fn try_next_event(&self) -> Option<OutputEvent> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(OutputEvent::End),
        }
    }

    /// Stops the current turn: SIGINT to the runtime alone, never its group, so the tool server it
    /// started keeps running; the runtime stays up for the next turn.
    pub fn interrupt(&mut self) -> Result<(), AgentError> {
        if self.status()?.is_some() {
            return Err(AgentError::NotRunning { runtime: self.kind });
        }
        sys::signal_process(self.child.id(), sys::Sig::Interrupt).map_err(|e| self.io("interrupt", e))
    }

    /// How the runtime ended, or `None` while it runs. When it has ended, whatever it left running
    /// in its group is killed before it is collected.
    pub fn status(&mut self) -> Result<Option<Exit>, AgentError> {
        if self.exit.is_some() {
            return Ok(self.exit);
        }
        if sys::has_ended(&self.child).map_err(|e| self.io("check on", e))? { self.collect().map(Some) } else { Ok(None) }
    }

    /// Ends the runtime in steps, each given at most `grace` (refused past [`MAX_GRACE`]): its input
    /// is closed so it can finish on its own; then SIGTERM goes to its whole group; then SIGKILL.
    /// Answers how it ended. It may wait twice `grace`, so call it away from the interface thread.
    pub fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        if grace > MAX_GRACE {
            return Err(AgentError::InvalidLimit {
                limit: "grace period (ms)",
                value: grace.as_millis(),
                accepted: format!("0 to {}", MAX_GRACE.as_millis()),
            });
        }
        self.close_input();
        if let Some(exit) = self.exit {
            return Ok(exit);
        }
        if !self.wait_for_end(grace)? {
            // A failure here (the group just ended) leaves the SIGKILL of `collect` to finish.
            if let Err(e) = sys::signal_group(self.child.id(), sys::Sig::Terminate) {
                log::warn!("{} could not be sent SIGTERM: {e}", self.kind);
            }
            self.wait_for_end(grace)?;
        }
        self.collect()
    }

    /// The last bytes of the runtime's error stream, with every secret it was given replaced.
    pub fn stderr_tail(&self) -> String {
        self.tail.lock().unwrap_or_else(PoisonError::into_inner).render(&self.scrub, usize::MAX)
    }

    /// The error for a runtime that ended as `exit`: [`AgentError::Exited`] or
    /// [`AgentError::Crashed`], with at most the last [`crate::MAX_DETAIL_BYTES`] of the error
    /// stream, starting at a line, secrets replaced.
    pub fn exit_error(&self, exit: Exit) -> AgentError {
        let stderr_tail = self.tail.lock().unwrap_or_else(PoisonError::into_inner).render(&self.scrub, crate::MAX_DETAIL_BYTES);
        match (exit.code, exit.signal) {
            (None, Some(signal)) => AgentError::Crashed { runtime: self.kind, signal, stderr_tail },
            (code, _) => AgentError::Exited { runtime: self.kind, code, stderr_tail },
        }
    }

    /// Waits at most `grace` for the runtime to end, without collecting it; says whether it did.
    fn wait_for_end(&self, grace: Duration) -> Result<bool, AgentError> {
        let started = Instant::now();
        loop {
            if sys::has_ended(&self.child).map_err(|e| self.io("check on", e))? {
                return Ok(true);
            }
            if started.elapsed() >= grace {
                return Ok(false);
            }
            std::thread::sleep(POLL);
        }
    }

    /// Kills whatever is left in the runtime's group (and the runtime, when the grace ran out), then
    /// collects the runtime and keeps its exit.
    /// While it is not collected, its process id cannot be reused, so the group signal reaches only
    /// what it started.
    fn collect(&mut self) -> Result<Exit, AgentError> {
        // The runtime itself too, in case it left its group: otherwise the wait could hang.
        sys::kill_group(self.child.id());
        let status = self.child.wait().map_err(|e| self.io("collect", e))?;
        let exit = sys::exit_of(status);
        self.exit = Some(exit);
        Ok(exit)
    }

    /// An I/O failure while doing `action` to this runtime.
    fn io(&self, action: &'static str, e: io::Error) -> AgentError {
        AgentError::Io { runtime: self.kind, action, detail: e.to_string() }
    }
}

impl Drop for RuntimeProcess {
    /// Kills the runtime's whole group, and the runtime itself in case it left the group, then
    /// collects it. Only while the runtime is still this process's child (running, or ended and not
    /// collected), so its id cannot have passed to another process.
    fn drop(&mut self) {
        self.input = None;
        if self.exit.is_none() {
            let pid = self.child.id();
            match sys::has_ended(&self.child) {
                Ok(_) => {
                    sys::kill_group(pid);
                    let _ = self.child.wait();
                }
                Err(e) => log::warn!("{} (process {pid}) is not signalled on drop: it cannot be checked ({e})", self.kind),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Limits of zero or past their maximum are refused, not clamped.
    #[test]
    fn limits_out_of_range_are_refused() {
        assert!(Limits::default().check().is_ok());
        // The largest accepted values keep the total within the budget.
        assert!(Limits { max_line_bytes: Limits::MAX_LINE_BYTES, queued_lines: 3, stderr_tail_bytes: Limits::MAX_STDERR_TAIL_BYTES }.check().is_ok());
        assert!(Limits { max_line_bytes: 1, queued_lines: Limits::MAX_QUEUED_LINES, ..Limits::default() }.check().is_ok());
        for bad in [
            Limits { max_line_bytes: 0, ..Limits::default() },
            Limits { max_line_bytes: Limits::MAX_LINE_BYTES + 1, ..Limits::default() },
            Limits { stderr_tail_bytes: 0, ..Limits::default() },
            Limits { queued_lines: Limits::MAX_QUEUED_LINES + 1, ..Limits::default() },
            Limits { max_line_bytes: Limits::MAX_LINE_BYTES, queued_lines: 4, ..Limits::default() },
            Limits { max_line_bytes: 1024 * 1024, queued_lines: Limits::MAX_QUEUED_LINES, ..Limits::default() },
        ] {
            assert!(matches!(bad.check(), Err(AgentError::InvalidLimit { .. })), "{bad:?}");
        }
    }

    /// Variable names are checked, and the ones this crate sets are refused.
    #[test]
    fn variable_names_are_checked_and_reserved_ones_refused() {
        assert!(check_name("ANTHROPIC_API_KEY").is_ok() && check_name("_X1").is_ok());
        for bad in ["", "1X", "A-B", "A=B", "A B", "É"] {
            assert!(matches!(check_name(bad), Err(AgentError::InvalidVariable { name: None, .. })), "{bad}");
        }
        for reserved in ["PATH", "HOME", "TMPDIR", "CLAUDE_CONFIG_DIR", "CODEX_HOME", "LANG", "LC_ALL"] {
            assert!(matches!(check_name(reserved), Err(AgentError::InvalidVariable { name: Some(_), .. })), "{reserved}");
        }
    }
}
